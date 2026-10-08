package dev.exact.heavybench

import android.app.Activity
import android.content.Intent
import android.graphics.Insets
import android.os.Debug
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import android.os.Trace
import android.util.Log
import android.view.Choreographer
import android.view.FrameMetrics
import android.view.View
import android.view.ViewTreeObserver
import android.view.Window
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.widget.FrameLayout
import org.json.JSONObject
import java.io.File
import kotlin.math.ceil

interface ScrollTarget {
    fun scrollByPx(delta: Float): Float
    fun visibleRows(): Int
    fun mountedRows(): Int
    fun firstVisibleId(): String?
    val mountedRowKind: String get() = "attached-row-roots"
}

object HeavyWindow {
    fun configure(activity: Activity) {
        // PhoneWindow implementations may require the decor to exist before exposing their controller.
        val decor = activity.window.decorView
        if (Build.VERSION.SDK_INT >= 30) activity.window.setDecorFitsSystemWindows(false)
        else decor.systemUiVisibility = View.SYSTEM_UI_FLAG_LAYOUT_STABLE or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
            View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
        activity.window.attributes = activity.window.attributes.apply {
            layoutInDisplayCutoutMode = if (Build.VERSION.SDK_INT >= 30) WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                else WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            val display = activity.windowManager.defaultDisplay
            val current = display.mode
            display.supportedModes.filter { it.physicalWidth == current.physicalWidth && it.physicalHeight == current.physicalHeight }
                .maxByOrNull { it.refreshRate }?.let { preferredRefreshRate = it.refreshRate; preferredDisplayModeId = it.modeId }
        }
        if (Build.VERSION.SDK_INT >= 30) activity.window.insetsController?.let {
            it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            it.hide(WindowInsets.Type.systemBars())
        }
    }
    private class CutoutRoot(activity: Activity, val content: View) : FrameLayout(activity) {
        var applied = false
        init { addView(content, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT)) }
        override fun dispatchApplyWindowInsets(insets: WindowInsets): WindowInsets {
            val cutout = insets.displayCutout
            setPadding(cutout?.safeInsetLeft ?: 0, cutout?.safeInsetTop ?: 0, cutout?.safeInsetRight ?: 0, cutout?.safeInsetBottom ?: 0)
            applied = true
            val zero = if (Build.VERSION.SDK_INT >= 30) WindowInsets.Builder(insets)
                .setInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout(), Insets.NONE)
                .setInsetsIgnoringVisibility(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout(), Insets.NONE)
                .setDisplayCutout(null).setVisible(WindowInsets.Type.systemBars(), false).build()
                else insets.consumeDisplayCutout().consumeSystemWindowInsets()
            // Dispatch explicitly even when legacy consumed insets would stop ViewGroup's normal traversal.
            content.dispatchApplyWindowInsets(zero)
            return zero
        }
    }
    fun install(activity: Activity, content: View): FrameLayout {
        configure(activity)
        val root = CutoutRoot(activity, content)
        activity.setContentView(root); configure(activity); root.requestApplyInsets()
        return root
    }
    private fun wrapper(content: View): CutoutRoot? {
        var cursor: View? = content
        while (cursor != null) { if (cursor is CutoutRoot) return cursor; cursor = cursor.parent as? View }
        return null
    }
    fun viewportMetadata(content: View): JSONObject {
        val wrapper = wrapper(content)
        return JSONObject().put("cutout_wrapper_applied", wrapper?.applied == true)
            .put("cutout_left_px", wrapper?.paddingLeft ?: JSONObject.NULL).put("cutout_top_px", wrapper?.paddingTop ?: JSONObject.NULL)
            .put("cutout_right_px", wrapper?.paddingRight ?: JSONObject.NULL).put("cutout_bottom_px", wrapper?.paddingBottom ?: JSONObject.NULL)
    }
    fun settled(content: View): Boolean = content.rootWindowInsets?.let {
        wrapper(content)?.applied == true && if (Build.VERSION.SDK_INT >= 30) !it.isVisible(WindowInsets.Type.statusBars()) && !it.isVisible(WindowInsets.Type.navigationBars())
        else it.systemWindowInsetTop == 0 && it.systemWindowInsetBottom == 0
    } ?: false
}

object HeavyOptions {
    private fun value(intent: Intent, key: String): Any? = intent.extras?.get(key)
    fun boolean(intent: Intent, key: String): Boolean = value(intent, key).let { it == true || it == 1 || it == "1" || it == "true" }
    fun int(intent: Intent, key: String, fallback: Int): Int = value(intent, key)?.toString()?.toIntOrNull() ?: fallback
    fun long(intent: Intent, key: String, fallback: Long): Long = value(intent, key)?.toString()?.toLongOrNull() ?: fallback
    fun float(intent: Intent, key: String, fallback: Float): Float = value(intent, key)?.toString()?.toFloatOrNull()?.takeIf { it.isFinite() } ?: fallback
    fun token(intent: Intent): String = value(intent, "BENCH_TOKEN")?.toString() ?: "heavy-${Process.myPid()}-${SystemClock.elapsedRealtimeNanos()}"
}

/** Shared observational probe. Window UI timings never stand in for a Surface renderer's presented FPS. */
class HeavyBenchHarness(private val activity: Activity, private val renderer: String, private val createdNs: Long) {
    private val main = Handler(Looper.getMainLooper())
    private val choreographer = Choreographer.getInstance()
    private val token = HeavyOptions.token(activity.intent)
    private val speed = HeavyOptions.float(activity.intent, "BENCH_SPEED_DP_S", 0f)
    private val durationMs = HeavyOptions.long(activity.intent, "BENCH_DURATION_MS", 12000).coerceIn(100, 60000)
    private val warmupMs = HeavyOptions.long(activity.intent, "BENCH_WARMUP_MS", 1000).coerceIn(0, 30000)
    private val automatic = activity.intent.hasExtra("BENCH_SPEED_DP_S") || activity.intent.hasExtra("BENCH_DURATION_MS")
    private lateinit var content: View
    private lateinit var target: ScrollTarget
    private var ready: () -> Boolean = { false }
    private var extra: () -> JSONObject = { JSONObject() }
    private var closed = false
    private var firstContent = 0L
    private var lastWindowDraw = 0L
    private var readyPosted = false
    @Volatile private var measuring = false
    @Volatile private var startNano = 0L
    @Volatile private var endNano = Long.MAX_VALUE
    private var startElapsed = 0L
    private var startThreadCpu = 0L
    private var startProcessCpu = 0L
    private var previousPace = 0L
    private var requestedPx = 0.0
    private var acknowledgedPx = 0.0
    private var unknownAcks = 0
    private var pacingCallbacks = 0
    private var maxVisible = -1
    private var maxMounted = -1
    private val pacingGaps = ArrayList<Double>()
    private val frameLock = Any()
    private val windowDurations = ArrayList<Double>()
    private var windowBusyNs = 0L
    private var deadlineMisses = 0
    private var listenerLoss = 0
    private val metricsThread = HandlerThread("HeavyBench-window-metrics")
    private var metricsStarted = false
    private val drawObserver = ViewTreeObserver.OnDrawListener {
        lastWindowDraw = SystemClock.elapsedRealtimeNanos()
        observeReady()
    }
    private val metricsObserver = Window.OnFrameMetricsAvailableListener { _, metrics, dropped ->
        val stamp = metrics.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP)
        if (measuring && stamp >= startNano && stamp < endNano) synchronized(frameLock) {
            val total = metrics.getMetric(FrameMetrics.TOTAL_DURATION)
            if (total >= 0 && windowDurations.size < 20000) windowDurations.add(total / 1e6)
            val deadline = if (Build.VERSION.SDK_INT >= 31) metrics.getMetric(FrameMetrics.DEADLINE) else -1L
            if (deadline > 0 && total > deadline) deadlineMisses++
            windowBusyNs += listOf(FrameMetrics.INPUT_HANDLING_DURATION, FrameMetrics.ANIMATION_DURATION,
                FrameMetrics.LAYOUT_MEASURE_DURATION, FrameMetrics.DRAW_DURATION).sumOf { metrics.getMetric(it).coerceAtLeast(0) }
            listenerLoss += dropped
        }
    }
    private val pace = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (closed) return
            observeReady()
            if (measuring) {
                if (previousPace != 0L) {
                    val gap = (frameTimeNanos - previousPace).coerceAtLeast(0)
                    pacingGaps += gap / 1e6
                    val requested = speed * activity.resources.displayMetrics.density * (gap / 1e9f)
                    requestedPx += requested
                    val acknowledged = target.scrollByPx(requested)
                    if (acknowledged.isFinite()) acknowledgedPx += acknowledged else unknownAcks++
                }
                previousPace = frameTimeNanos; pacingCallbacks++
                maxVisible = maxOf(maxVisible, target.visibleRows()); maxMounted = maxOf(maxMounted, target.mountedRows())
            }
            choreographer.postFrameCallback(this)
        }
    }
    fun attach(content: View, target: ScrollTarget, ready: () -> Boolean, extra: () -> JSONObject = { JSONObject() }) {
        this.content = content; this.target = target; this.ready = ready; this.extra = extra
        content.viewTreeObserver.addOnDrawListener(drawObserver)
        metricsThread.start(); metricsStarted = true
        activity.window.addOnFrameMetricsAvailableListener(metricsObserver, Handler(metricsThread.looper))
        choreographer.postFrameCallback(pace)
    }
    private fun observeReady() {
        if (firstContent != 0L || readyPosted || lastWindowDraw == 0L || content.width <= 0 || content.height <= 0 || !HeavyWindow.settled(content) || !ready()) return
        readyPosted = true
        main.post {
            readyPosted = false
            if (closed || firstContent != 0L || !ready()) return@post
            firstContent = SystemClock.elapsedRealtimeNanos()
            activity.reportFullyDrawn()
            val reported = SystemClock.elapsedRealtimeNanos()
            val marker = JSONObject().put("renderer", renderer).put("scene", "heavy-list").put("token", token).put("pid", Process.myPid())
                .put("process_start_elapsed_ms", Process.getStartElapsedRealtime()).put("on_create_elapsed_ns", createdNs)
                .put("first_content_elapsed_ns", firstContent).put("last_window_draw_elapsed_ns", lastWindowDraw)
                .put("report_fully_drawn_elapsed_ns", reported).put("root_width_px", content.width).put("root_height_px", content.height)
                .put("density", activity.resources.displayMetrics.density).put("font_scale", activity.resources.configuration.fontScale)
                .put("requested_refresh_hz", activity.window.attributes.preferredRefreshRate)
                .put("display_refresh_rate_api_hz", activity.windowManager.defaultDisplay.refreshRate).put("viewport", HeavyWindow.viewportMetadata(content))
                .put("live", HeavyOptions.boolean(activity.intent, "BENCH_LIVE")).put("start_index", HeavyOptions.int(activity.intent, "BENCH_START_INDEX", 0))
                .put("visible_rows", known(target.visibleRows())).put("mounted_rows", known(target.mountedRows())).put("mounted_rows_kind", target.mountedRowKind)
                .put("first_visible_id", target.firstVisibleId() ?: JSONObject.NULL).put("extra", extra())
                .put("ready_endpoint", "renderer-ready-observed-after-window-draw-not-presentation")
            Log.i("HeavyBench", "Startup:$marker")
            if (automatic) main.postDelayed({ if (!closed) beginMeasure() }, warmupMs)
        }
    }
    private fun beginMeasure() {
        startNano = System.nanoTime(); startElapsed = SystemClock.elapsedRealtimeNanos()
        startThreadCpu = Debug.threadCpuTimeNanos(); startProcessCpu = Process.getElapsedCpuTime()
        previousPace = 0; measuring = true
        Trace.beginAsyncSection("HeavyBench.Measure", token.hashCode())
        Log.i("HeavyBench", "Begin:" + JSONObject().put("renderer", renderer).put("token", token).put("start_elapsed_ns", startElapsed)
            .put("start_nano_ns", startNano).put("speed_dp_s", speed).put("duration_ms", durationMs))
        main.postDelayed({ if (!closed) finishMeasure() }, durationMs)
    }
    private fun finishMeasure() {
        endNano = System.nanoTime(); val ended = SystemClock.elapsedRealtimeNanos()
        val threadCpu = Debug.threadCpuTimeNanos() - startThreadCpu
        val processCpu = Process.getElapsedCpuTime() - startProcessCpu
        measuring = false; Trace.endAsyncSection("HeavyBench.Measure", token.hashCode())
        val elapsedMs = (ended - startElapsed) / 1e6
        // Memory reads occur after the measured interval, never in the paced frame callbacks.
        val memory = Debug.MemoryInfo(); Debug.getMemoryInfo(memory)
        val rss = File("/proc/self/status").readLines().firstOrNull { it.startsWith("VmRSS:") }?.split(Regex("\\s+"))?.getOrNull(1)?.toLongOrNull()
        val result = JSONObject().put("renderer", renderer).put("token", token).put("scene", "heavy-list")
            .put("start_elapsed_ns", startElapsed).put("end_elapsed_ns", ended).put("start_nano_ns", startNano).put("end_nano_ns", endNano)
            .put("measured_duration_ms", elapsedMs).put("speed_dp_s", speed).put("requested_scroll_px", requestedPx)
            .put("acknowledged_scroll_px", if (unknownAcks == 0) acknowledgedPx else JSONObject.NULL).put("unknown_scroll_ack_callbacks", unknownAcks)
            .put("pacing_callbacks", pacingCallbacks).put("pacing_gap_p50_ms", percentile(pacingGaps, .5)).put("pacing_gap_p95_ms", percentile(pacingGaps, .95))
            .put("main_thread_cpu_ns", threadCpu).put("main_thread_cpu_pct", threadCpu / (elapsedMs * 1e6) * 100)
            .put("process_cpu_ms", processCpu).put("process_cpu_pct", processCpu / elapsedMs * 100)
            .put("max_visible_rows", known(maxVisible)).put("max_mounted_rows", known(maxMounted)).put("mounted_rows_kind", target.mountedRowKind)
            .put("memory_pss_kib_after_measure", memory.totalPss).put("memory_rss_kib_after_measure", rss ?: JSONObject.NULL)
            .put("java_heap_bytes_after_measure", Runtime.getRuntime().totalMemory() - Runtime.getRuntime().freeMemory())
            .put("first_visible_id", target.firstVisibleId() ?: JSONObject.NULL).put("extra", extra())
            .put("pacing_scope", "Choreographer callback cadence and requested motion; NOT presented application FPS")
            .put("window_metrics_scope", "Android Window UI frames; excludes independent Surface/Vulkan presentation; headline frame data requires FrameTimeline")
        synchronized(frameLock) {
            result.put("window_ui_frames_observed", windowDurations.size).put("window_total_duration_p50_ms", percentile(windowDurations, .5))
                .put("window_total_duration_p95_ms", percentile(windowDurations, .95)).put("window_frames_over_deadline", deadlineMisses)
                .put("window_ui_measured_busy_ns", windowBusyNs).put("window_ui_measured_busy_pct", windowBusyNs / (elapsedMs * 1e6) * 100)
                .put("window_metrics_listener_lost_notifications", listenerLoss)
                .put("window_deadline_metric_available", Build.VERSION.SDK_INT >= 31)
        }
        Log.i("HeavyBench", "Measure:$result")
    }
    fun close() {
        if (closed) return
        closed = true
        if (measuring) { measuring = false; Trace.endAsyncSection("HeavyBench.Measure", token.hashCode()) }
        main.removeCallbacksAndMessages(null); choreographer.removeFrameCallback(pace)
        if (::content.isInitialized && content.viewTreeObserver.isAlive) content.viewTreeObserver.removeOnDrawListener(drawObserver)
        if (metricsStarted) { activity.window.removeOnFrameMetricsAvailableListener(metricsObserver); metricsThread.quitSafely() }
    }
    private fun known(value: Int): Any = if (value < 0) JSONObject.NULL else value
    private fun percentile(values: List<Double>, fraction: Double): Any = if (values.isEmpty()) JSONObject.NULL
        else values.sorted()[ceil(values.size * fraction).toInt().coerceAtLeast(1) - 1]
}
