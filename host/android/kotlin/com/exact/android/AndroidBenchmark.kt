package com.exact.android

import android.app.Activity
import android.os.Build
import android.os.Debug
import android.os.Handler
import android.os.Looper
import android.view.Choreographer
import android.view.FrameMetrics
import android.view.ViewGroup
import android.view.Window
import android.widget.ScrollView
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.ceil

/** Resolve once before measurement; the presenter keeps this platform control. */
internal fun benchmarkScrollView(exact: ExactView): ScrollView {
    val box = checkNotNull(exact.actionView("core-scroll")) as ViewGroup
    for (index in 0 until box.childCount) {
        val child = box.getChildAt(index)
        if (child is ScrollView) return child
    }
    error("The core scroll node has no native ScrollView")
}

/** An opt-in, bounded real-window run. Choreographer paces input; FrameMetrics measures rendering. */
internal class AndroidBenchmark(private val activity: Activity, private val exact: ExactView, private val samples: Int) : AutoCloseable {
    private data class Workload(val name: String, val prepare: String?, val click: String)
    private val workloads = listOf(
        Workload("counter", "rows-1", "increment"),
        Workload("paint-1", "rows-1", "toggle-batch"),
        Workload("paint-100", "rows-100", "toggle-batch"),
        Workload("paint-1000", "rows-1000", "toggle-batch"),
        Workload("transform-1000", "rows-1000", "toggle-move")
    )
    private val handler = Handler(Looper.getMainLooper())
    private val display = Choreographer.getInstance()
    private val results = JSONArray()
    private val native = LongArray(samples * 4 + 16)
    private val decode = LongArray(native.size)
    private val totalFrames = LongArray(native.size)
    private val drawFrames = LongArray(native.size)
    private val layoutFrames = LongArray(native.size)
    private var transactions = 0
    private var frameCount = 0
    private var droppedReports = 0
    private var phase = -1
    private var action = 0
    private var settle = 0
    private var startVsync = Long.MAX_VALUE
    private var endVsync = Long.MAX_VALUE
    private var beforeBridge = LongArray(9)
    private var beforeAllocation: Long? = null
    private var beforeCollections: Long? = null
    private var started = false
    private var closed = false
    private var collecting = false
    private var idle = JSONObject()
    private lateinit var actionView: android.view.View
    private val warmup = 30

    private val metrics = Window.OnFrameMetricsAvailableListener { _, frame, lost ->
        val stamp = frame.getMetric(FrameMetrics.VSYNC_TIMESTAMP)
        val total = frame.getMetric(FrameMetrics.TOTAL_DURATION)
        if (collecting && stamp >= startVsync && stamp <= endVsync && total >= 0 && frameCount < totalFrames.size) {
            totalFrames[frameCount] = total
            drawFrames[frameCount] = frame.getMetric(FrameMetrics.DRAW_DURATION).coerceAtLeast(0)
            layoutFrames[frameCount] = frame.getMetric(FrameMetrics.LAYOUT_MEASURE_DURATION).coerceAtLeast(0)
            frameCount++
            droppedReports += lost
        }
    }
    private val next = Choreographer.FrameCallback { stamp -> step(stamp) }
    private val observer: (Long, Long) -> Unit = { runtime, presentation ->
        if (transactions < native.size) {
            native[transactions] = runtime
            decode[transactions] = presentation
            transactions++
        }
    }

    fun start() {
        if (started || closed) return
        started = true
        activity.window.addOnFrameMetricsAvailableListener(metrics, handler)
        val beforeIdle = exact.bridgeStats()
        val start = android.os.SystemClock.uptimeMillis()
        handler.postDelayed({
            if (!closed) {
                val afterIdle = exact.bridgeStats()
                idle = JSONObject().put("observed_ms", android.os.SystemClock.uptimeMillis() - start)
                    .put("runtime_transactions", afterIdle[0] - beforeIdle[0])
                    .put("measure_callbacks", afterIdle[3] - beforeIdle[3])
                    .put("buffer_reset_java_calls", afterIdle[8] - beforeIdle[8])
                phase = 0
                beginPhase()
            }
        }, 500)
    }
    private fun beginPhase() {
        collecting = false
        exact.observeTransaction = null
        action = 0; settle = 0; transactions = 0; frameCount = 0; droppedReports = 0
        startVsync = Long.MAX_VALUE; endVsync = Long.MAX_VALUE
        workloads[phase].prepare?.let { check(exact.activate(it)) { "Missing benchmark button '$it'" } }
        actionView = checkNotNull(exact.actionView(workloads[phase].click)) { "Missing benchmark action '${workloads[phase].click}'" }
        display.postFrameCallback(next)
    }
    private fun runtimeStat(name: String) = Debug.getRuntimeStat(name)?.toLongOrNull()
    private fun step(stamp: Long) {
        if (closed) return
        if (action == warmup) {
            beforeBridge = exact.bridgeStats()
            beforeAllocation = runtimeStat("art.gc.bytes-allocated")
            beforeCollections = runtimeStat("art.gc.gc-count")
            startVsync = stamp
            collecting = true
            exact.observeTransaction = observer
        }
        if (action < warmup + samples) {
            check(actionView.performClick()) { "Benchmark action has no native click listener" }
            action++
            if (action == warmup + samples) endVsync = stamp
            display.postFrameCallback(next)
        } else if (++settle < 3) {
            // Let the render thread report the last action's real window frame.
            display.postFrameCallback(next)
        } else finishPhase()
    }
    private fun durations(values: LongArray, count: Int): JSONObject {
        val sorted = values.copyOf(count).apply { sort() }
        fun percentile(q: Double): Double? = if (count == 0) null else sorted[(ceil(q * count).toInt() - 1).coerceAtLeast(0)] / 1_000_000.0
        return JSONObject().put("samples", count).put("p50_ms", percentile(.5) ?: JSONObject.NULL)
            .put("p95_ms", percentile(.95) ?: JSONObject.NULL)
            .put("max_ms", if (count == 0) JSONObject.NULL else sorted.last() / 1_000_000.0)
    }
    private fun finishPhase() {
        collecting = false
        exact.observeTransaction = null
        val allocated = runtimeStat("art.gc.bytes-allocated")
        val collections = runtimeStat("art.gc.gc-count")
        val afterBridge = exact.bridgeStats()
        val names = arrayOf("runtime_transactions", "batch_bytes", "output_wrapper_allocations", "measure_callbacks",
            "measure_wrapper_allocations", "font_callbacks", "font_wrapper_allocations", "wake_callbacks", "buffer_reset_java_calls")
        val bridge = JSONObject()
        for (index in names.indices) bridge.put(names[index], afterBridge[index] - beforeBridge[index])
        val report = JSONObject().put("workload", workloads[phase].name).put("actions", samples).put("warmup_actions", warmup)
            .put("retained_nodes", exact.retainedNodes).put("native_commit", durations(native, transactions))
            .put("decode_and_apply", durations(decode, transactions)).put("window_frame", durations(totalFrames, frameCount))
            .put("display_list_recording", durations(drawFrames, frameCount)).put("platform_layout", durations(layoutFrames, frameCount))
            .put("frame_metric_reports_dropped", droppedReports).put("bridge", bridge)
            .put("art_allocated_bytes", if (allocated != null && beforeAllocation != null) allocated - beforeAllocation!! else JSONObject.NULL)
            .put("art_gc_count", if (collections != null && beforeCollections != null) collections - beforeCollections!! else JSONObject.NULL)
        results.put(report)
        android.util.Log.i("ExactBenchmark", "Result:" + report.toString())
        phase++
        if (phase < workloads.size) beginPhase() else {
            val metadata = JSONObject().put("device", "${Build.MANUFACTURER} ${Build.MODEL}").put("android_api", Build.VERSION.SDK_INT)
                .put("hardware_accelerated", exact.isHardwareAccelerated).put("samples_per_workload", samples)
                .put("creation_to_first_host_draw_ms", exact.creationToFirstHostDrawNs?.let { it / 1_000_000.0 } ?: JSONObject.NULL)
                .put("first_draw_scope", "ExactView creation to completion of first host dispatchDraw; excludes process startup and pixel presentation")
                .put("idle", idle).put("workloads", results.length())
                .put("allocation_scope", "Approximate process ART allocations; includes benchmark callbacks; excludes native allocations")
                .put("render_scope", "Real Window FrameMetrics; Choreographer only paces native button clicks")
            android.util.Log.i("ExactBenchmark", "Complete:" + metadata.toString())
            val result = JSONObject(metadata.toString()).put("results", results)
            activity.openFileOutput("android-benchmark.json", Activity.MODE_PRIVATE).bufferedWriter().use { it.write(result.toString(2)) }
            close()
        }
    }
    override fun close() {
        if (closed) return
        closed = true
        exact.observeTransaction = null
        display.removeFrameCallback(next)
        if (started) activity.window.removeOnFrameMetricsAvailableListener(metrics)
    }
}
