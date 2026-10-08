package com.exact.benchmark

import android.app.Activity
import android.os.Build
import android.os.Debug
import android.os.Handler
import android.os.Looper
import android.view.Choreographer
import android.view.FrameMetrics
import android.view.ViewTreeObserver
import android.view.Window
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.ceil

/** Same real-window measurement for synchronous Exact and asynchronous Compose. */
class ComparisonBenchmark(
    private val activity: Activity,
    private val target: BenchmarkTarget,
    private val renderer: String,
    private val samples: Int = 120,
    private val scrollSamples: Int = 0,
    private val scrollOnly: Boolean = false,
    private val heavyList: Boolean = false,
    private val scrollSpeedDpPerSecond: Float = 0f
) : AutoCloseable {
    private val workloads = arrayOf("counter", "paint-1", "paint-100", "paint-1000", "transform-1000")
    private val handler = Handler(Looper.getMainLooper())
    private val display = Choreographer.getInstance()
    private val results = JSONArray()
    private val actionToDraw = LongArray(samples)
    private val actionToFrame = LongArray(samples)
    private val mainThreadCpu = LongArray(samples)
    private val dispatch = LongArray(samples)
    private val dispatchAndUi = LongArray(samples)
    private val total = LongArray(samples)
    private val ui = LongArray(samples)
    private val animation = LongArray(samples)
    private val layout = LongArray(samples)
    private val recording = LongArray(samples)
    private val gpu = LongArray(samples)
    private val deadlines = LongArray(samples)
    private val frameTimes = LongArray(16)
    private val frameTotals = LongArray(16)
    private val frameEnds = LongArray(16)
    private val frameUi = LongArray(16)
    private val frameAnimation = LongArray(16)
    private val frameLayouts = LongArray(16)
    private val frameDraws = LongArray(16)
    private val frameGpu = LongArray(16)
    private val frameDeadlines = LongArray(16)
    private var scroll: ContinuousScroll? = null
    private var cursor = 0
    private var phase = 0
    private var action = 0
    private var preparing = true
    private var waiting = false
    private var drawPosted = false
    private var closed = false
    private var started = false
    private var metricsAttached = false
    private var desiredRevision = 0L
    private var startedNs = 0L
    private var recordedNs = 0L
    private var startedCpuNs = 0L
    private var recordedCpuNs = 0L
    private var dispatchNs = 0L
    private var drawnVsync = 0L
    private var currentVsync = 0L
    private var reportsDropped = 0
    private var beforeAllocation: Long? = null
    private var beforeCollections: Long? = null
    private val warmup = 30
    private val frame = Choreographer.FrameCallback { step() }
    private val ticker = object : Choreographer.FrameCallback {
        override fun doFrame(stamp: Long) {
            currentVsync = stamp
            if (!closed) display.postFrameCallback(this)
        }
    }
    private val timeout = Runnable { fail("scene/frame did not complete within 10 seconds") }

    private val metrics = Window.OnFrameMetricsAvailableListener { _, value, lost ->
        val index = cursor++ % frameTimes.size
        frameTimes[index] = value.getMetric(FrameMetrics.VSYNC_TIMESTAMP)
        frameTotals[index] = value.getMetric(FrameMetrics.TOTAL_DURATION)
        frameEnds[index] = value.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP) + frameTotals[index]
        frameAnimation[index] = value.getMetric(FrameMetrics.ANIMATION_DURATION)
        frameLayouts[index] = value.getMetric(FrameMetrics.LAYOUT_MEASURE_DURATION)
        frameDraws[index] = value.getMetric(FrameMetrics.DRAW_DURATION)
        frameUi[index] = value.getMetric(FrameMetrics.INPUT_HANDLING_DURATION) + frameAnimation[index] + frameLayouts[index] + frameDraws[index]
        frameGpu[index] = if (Build.VERSION.SDK_INT >= 31) value.getMetric(FrameMetrics.GPU_DURATION) else -1
        frameDeadlines[index] = if (Build.VERSION.SDK_INT >= 31) value.getMetric(FrameMetrics.DEADLINE) else -1
        if (waiting && action >= warmup) reportsDropped += lost
        completeIfReady()
    }
    private val drawn = ViewTreeObserver.OnDrawListener {
        if (waiting && !drawPosted && recordedNs == 0L) {
            val vsync = currentVsync
            drawPosted = true
            handler.post {
                drawPosted = false
                if (!closed && waiting && target.appliedRevision >= desiredRevision) {
                    recordedNs = System.nanoTime()
                    recordedCpuNs = Debug.threadCpuTimeNanos() - startedCpuNs
                    drawnVsync = vsync
                    completeIfReady()
                }
            }
        }
    }

    fun start() {
        if (started || closed) return
        started = true
        if (scrollOnly) {
            require(scrollSamples > 0)
            handler.postDelayed({ if (!closed) beginScroll() }, 500)
            return
        }
        activity.window.addOnFrameMetricsAvailableListener(metrics, handler)
        metricsAttached = true
        target.view.viewTreeObserver.addOnDrawListener(drawn)
        display.postFrameCallback(ticker)
        // Identical idle interval before either framework's first workload.
        handler.postDelayed({ if (!closed) beginPhase() }, 500)
    }
    private fun beginPhase() {
        action = 0
        reportsDropped = 0
        preparing = true
        target.prepare(workloads[phase])
        display.postFrameCallback(frame)
    }
    private fun runtimeStat(name: String) = Debug.getRuntimeStat(name)?.toLongOrNull()
    private fun step() {
        if (closed) return
        if (preparing) {
            if (target.appliedRevision < target.revision) {
                display.postFrameCallback(frame)
                return
            }
            preparing = false
            // Give preparation's layout/draw and render thread one whole frame.
            display.postFrameCallback(frame)
            return
        }
        if (action == warmup) {
            beforeAllocation = runtimeStat("art.gc.bytes-allocated")
            beforeCollections = runtimeStat("art.gc.gc-count")
        }
        // Inject the same UI event between frames. Mutating Compose state late
        // inside ANIMATION could otherwise artificially miss its frame clock.
        handler.post {
            if (!closed) {
                waiting = true
                recordedNs = 0L
                recordedCpuNs = 0L
                drawnVsync = 0L
                startedCpuNs = Debug.threadCpuTimeNanos()
                startedNs = System.nanoTime()
                target.performAction(workloads[phase])
                dispatchNs = System.nanoTime() - startedNs
                desiredRevision = target.revision
                handler.postDelayed(timeout, 10_000)
            }
        }
    }
    private fun completeIfReady() {
        if (closed || !waiting || recordedNs == 0L) return
        val index = frameTimes.indexOf(drawnVsync)
        if (index < 0 || frameTotals[index] < 0) return
        handler.removeCallbacks(timeout)
        if (action >= warmup) {
            val sample = action - warmup
            actionToDraw[sample] = recordedNs - startedNs
            actionToFrame[sample] = maxOf(recordedNs, frameEnds[index]) - startedNs
            mainThreadCpu[sample] = recordedCpuNs
            dispatch[sample] = dispatchNs
            dispatchAndUi[sample] = dispatchNs + frameUi[index]
            total[sample] = frameTotals[index]
            ui[sample] = frameUi[index]
            animation[sample] = frameAnimation[index]
            layout[sample] = frameLayouts[index]
            recording[sample] = frameDraws[index]
            gpu[sample] = frameGpu[index]
            deadlines[sample] = frameDeadlines[index]
        }
        waiting = false
        action++
        if (action < warmup + samples) display.postFrameCallback(frame) else finishPhase()
    }
    private fun durations(values: LongArray): JSONObject {
        val sorted = values.filter { it >= 0 }.sorted()
        fun percentile(q: Double): Any = if (sorted.isEmpty()) JSONObject.NULL else sorted[(ceil(q * sorted.size).toInt() - 1).coerceAtLeast(0)] / 1_000_000.0
        return JSONObject().put("samples", sorted.size).put("p50_ms", percentile(.5)).put("p95_ms", percentile(.95))
            .put("p99_ms", percentile(.99))
            .put("max_ms", sorted.lastOrNull()?.let { it / 1_000_000.0 } ?: JSONObject.NULL)
    }
    private fun finishPhase() {
        val allocated = runtimeStat("art.gc.bytes-allocated")
        val collections = runtimeStat("art.gc.gc-count")
        val result = JSONObject().put("renderer", renderer).put("workload", workloads[phase])
            .put("actions", samples).put("warmup_actions", warmup).put("retained_rows", target.retainedRows)
            .put("action_to_completed_draw", durations(actionToDraw)).put("action_to_completed_window_frame", durations(actionToFrame))
            .put("main_thread_cpu", durations(mainThreadCpu))
            .put("action_dispatch", durations(dispatch)).put("dispatch_plus_ui_phases", durations(dispatchAndUi))
            .put("window_frame", durations(total)).put("ui_phases", durations(ui)).put("animation_phase", durations(animation))
            .put("layout_measure", durations(layout)).put("display_list_recording", durations(recording)).put("gpu", durations(gpu))
            .put("frame_metric_reports_dropped", reportsDropped)
            .put("frame_deadline", durations(deadlines))
            .put("frame_deadline_misses", if (deadlines.all { it >= 0 }) total.indices.count { total[it] > deadlines[it] } else JSONObject.NULL)
            .put("deadline_scope", "Matched window frames with TOTAL_DURATION > DEADLINE; not physical presentation or SurfaceFlinger jank")
            .put("art_allocated_bytes", if (allocated != null && beforeAllocation != null) allocated - beforeAllocation!! else JSONObject.NULL)
            .put("art_gc_count", if (collections != null && beforeCollections != null) collections - beforeCollections!! else JSONObject.NULL)
        results.put(result)
        android.util.Log.i("ExactCompare", "CompareResult:" + result.toString())
        phase++
        if (phase < workloads.size) beginPhase() else if (scrollSamples > 0) {
            display.removeFrameCallback(ticker)
            detachMetrics()
            if (target.view.viewTreeObserver.isAlive) target.view.viewTreeObserver.removeOnDrawListener(drawn)
            beginScroll()
        } else finishReport()
    }
    private fun beginScroll() {
        scroll = ContinuousScroll(activity, target, renderer, scrollSamples,
            if (heavyList) "heavy-scroll-1000" else "scroll-1000", scrollSpeedDpPerSecond,
            { result -> results.put(result); finishReport() }, { message -> fail(message) })
        scroll!!.start()
    }
    private fun finishReport() {
        val report = JSONObject().put("renderer", renderer).put("samples", samples).put("workloads", results.length())
            .put("device", "${Build.MANUFACTURER} ${Build.MODEL}").put("android_api", Build.VERSION.SDK_INT)
            .put("hardware_accelerated", target.view.isHardwareAccelerated)
            .put("action_scope", "One shared UI state action posted between frames; affected phase acknowledges revision before completed draw")
            .put("phase_scope", "Dispatch wall time plus input/animation/layout/draw FrameMetrics; subtotal excludes other between-frame work and render thread")
            .put("cpu_scope", "Main-thread CPU from action through completed draw, includes native callees and between-frame work, excludes waiting, render thread, GPU and other threads")
            .put("frame_scope", "Window FrameMetrics matched by VSYNC timestamp; completion is recorded frame end, not physical pixel presentation")
            .put("allocation_scope", "Process ART allocation counter, includes shared harness, excludes native allocations")
            .put("list_mode", if (heavyList) "heavy-retained-1000" else "simple-retained")
            .put("scroll_speed_dp_per_second", if (scrollSpeedDpPerSecond > 0) scrollSpeedDpPerSecond else JSONObject.NULL)
            .put("scroll_scope", if (scrollSamples > 0) "Continuous native programmatic scrolling, triangular bounce; explicit speed uses monotonic VSYNC timeline rather than distance per callback; no touch/fling or virtualization" else JSONObject.NULL)
        close()
        android.util.Log.i("ExactCompare", "CompareComplete:" + report.toString())
    }
    private fun fail(message: String) {
        if (closed) return
        android.util.Log.e("ExactCompare", "CompareError:" + JSONObject().put("renderer", renderer).put("workload", if (scroll != null) { if (heavyList) "heavy-scroll-1000" else "scroll-1000" } else workloads.getOrNull(phase) ?: "scroll-1000").put("error", message))
        close()
    }
    private fun detachMetrics() {
        if (!metricsAttached) return
        activity.window.removeOnFrameMetricsAvailableListener(metrics)
        metricsAttached = false
    }
    override fun close() {
        if (closed) return
        closed = true
        scroll?.close()
        handler.removeCallbacks(timeout)
        display.removeFrameCallback(frame)
        display.removeFrameCallback(ticker)
        detachMetrics()
        if (target.view.viewTreeObserver.isAlive) target.view.viewTreeObserver.removeOnDrawListener(drawn)
    }
}

/** Paced retained scrolling; the next request never waits for drawing or metrics. */
private class ContinuousScroll(
    private val activity: Activity,
    private val target: BenchmarkTarget,
    private val renderer: String,
    private val samples: Int,
    private val workload: String,
    private val speedDpPerSecond: Float,
    private val completed: (JSONObject) -> Unit,
    private val failed: (String) -> Unit
) : AutoCloseable {
    private val warmup = 30
    private val count = samples + warmup
    // Synchronous messages can be held by ViewRoot traversal barriers. Use the
    // same async opportunity as Compose's UI dispatcher, for every renderer.
    private val handler = Handler.createAsync(Looper.getMainLooper())
    private val choreographer = Choreographer.getInstance()
    private val stamps = LongArray(count)
    private val requestedVsync = LongArray(count)
    private val requests = IntArray(count)
    private val observed = IntArray(count) { -1 }
    private val starts = LongArray(count)
    private val cpuStarts = LongArray(count)
    private val callbackCpuStarts = LongArray(count)
    private val drawEnds = LongArray(count) { -1L }
    private val drawCpu = LongArray(count) { -1L }
    private val intervalCpu = LongArray(count) { -1L }
    private val dispatchCpu = LongArray(count)
    private val dispatchWall = LongArray(count)
    private val totals = LongArray(count) { -1L }
    private val ui = LongArray(count) { -1L }
    private val layout = LongArray(count) { -1L }
    private val draw = LongArray(count) { -1L }
    private val gpu = LongArray(count) { -1L }
    private val deadline = LongArray(count) { -1L }
    private val firstDraw = LongArray(count) { -1L }
    private var tick = 0
    private var submitted = 0
    private var pending = -1
    private var submitQueued = false
    private var callbacksWithoutRequest = 0
    private var readyFrames = 0
    private var active = false
    private var closed = false
    private var started = false
    private var metricsAttached = false
    private var range = 0
    private var viewport = 0
    private var position = 0
    private var direction = 1
    private var turns = 0
    private var lost = 0
    private var allocation: Long? = null
    private var collections: Long? = null
    private var currentVsync = 0L
    private var unrequestedDraws = 0
    private val density = activity.resources.displayMetrics.density
    private val stepPx = (64 * density).toInt().coerceAtLeast(1)
    private var trajectoryBase = 0L
    private var previousSegment = 0L
    private var trajectoryPeriod = 0L
    private val timeout = Runnable { abort("continuous scroll preparation did not settle within 10 seconds") }
    private val frame = Choreographer.FrameCallback { stamp -> step(stamp) }
    private val ticker = object : Choreographer.FrameCallback {
        override fun doFrame(stamp: Long) {
            currentVsync = stamp
            if (!closed) choreographer.postFrameCallback(this)
        }
    }
    private val metrics = Window.OnFrameMetricsAvailableListener { _, value, dropped ->
        // Observer loss belongs to the measured callback interval and drain,
        // even when this particular Window timestamp matches no request.
        if (active && tick > warmup) lost += dropped
        val stamp = value.getMetric(FrameMetrics.VSYNC_TIMESTAMP)
        val index = stamps.indexOf(stamp)
        if (active && index >= 0 && stamp != 0L) {
            totals[index] = value.getMetric(FrameMetrics.TOTAL_DURATION)
            layout[index] = value.getMetric(FrameMetrics.LAYOUT_MEASURE_DURATION)
            draw[index] = value.getMetric(FrameMetrics.DRAW_DURATION)
            ui[index] = value.getMetric(FrameMetrics.INPUT_HANDLING_DURATION) + value.getMetric(FrameMetrics.ANIMATION_DURATION) + layout[index] + draw[index]
            gpu[index] = if (Build.VERSION.SDK_INT >= 31) value.getMetric(FrameMetrics.GPU_DURATION) else -1L
            deadline[index] = if (Build.VERSION.SDK_INT >= 31) value.getMetric(FrameMetrics.DEADLINE) else -1L
            firstDraw[index] = value.getMetric(FrameMetrics.FIRST_DRAW_FRAME)
        }
    }
    private val drawn = ViewTreeObserver.OnDrawListener {
        val index = tick - 1
        if (active && index in 0 until count && stamps[index] != currentVsync) unrequestedDraws++
        if (active && index in 0 until count && stamps[index] == currentVsync && drawEnds[index] < 0) {
            val vsync = currentVsync
            val submittedAtDraw = submitted
            handler.post {
                if (!closed && drawEnds[index] < 0) {
                    drawEnds[index] = System.nanoTime()
                    drawCpu[index] = Debug.threadCpuTimeNanos() - cpuStarts[index]
                    // Compose may place during dispatchDraw after root OnDraw.
                    // Read after the whole draw, before the next mutation, and
                    // reject a probe that slipped into a later frame callback.
                    observed[index] = if (currentVsync == vsync && submitted == submittedAtDraw) target.renderedScrollPositionPx else -1
                }
            }
        }
    }
    fun start() {
        if (started || closed) return
        started = true
        activity.window.addOnFrameMetricsAvailableListener(metrics, handler)
        metricsAttached = true
        target.view.viewTreeObserver.addOnDrawListener(drawn)
        require(speedDpPerSecond.isFinite() && speedDpPerSecond >= 0f)
        target.prepare(workload)
        handler.postDelayed(timeout, 10_000)
        choreographer.postFrameCallback(ticker)
        choreographer.postFrameCallback(frame)
    }
    private fun stat(name: String) = Debug.getRuntimeStat(name)?.toLongOrNull()
    private fun step(stamp: Long) {
        if (closed) return
        if (!active) {
            val ready = target.startupReady && target.appliedRevision >= target.revision && !target.view.isLayoutRequested &&
                target.retainedRows == 1000 && target.scrollRangePx > 0 && target.scrollViewportPx > 0
            readyFrames = if (ready) readyFrames + 1 else 0
            if (readyFrames < 2) { choreographer.postFrameCallback(frame); return }
            handler.removeCallbacks(timeout)
            range = target.scrollRangePx
            viewport = target.scrollViewportPx
            val rate = target.view.display?.refreshRate?.toDouble() ?: 60.0
            trajectoryPeriod = (1e9 / rate.coerceAtLeast(1.0)).toLong()
            if (target.scrollPositionPx != 0) { abort("scroll scene did not reset to offset zero"); return }
            active = true
        }
        val cpu = Debug.threadCpuTimeNanos()
        if (tick > 0 && intervalCpu[tick - 1] < 0 && (pending >= 0 || tick == count))
            intervalCpu[tick - 1] = cpu - callbackCpuStarts[tick - 1]
        if (tick == count) {
            // One extra callback closes the last main-thread CPU interval. Give
            // asynchronous metrics callbacks time to drain without more scrolls.
            handler.postDelayed({ finish() }, 1000)
            return
        }
        if (target.scrollRangePx != range || target.scrollViewportPx != viewport) { abort("scroll geometry changed during the paced phase"); return }
        if (pending >= 0) {
            stamps[pending] = stamp
            callbackCpuStarts[pending] = cpu
            tick = pending + 1
            pending = -1
        } else if (submitted > 0) callbacksWithoutRequest++
        if (submitted < count && !submitQueued) {
            submitQueued = true
            // Two message hops let this frame's posted completed-draw probe
            // run before the next mutation. All frameworks receive the next
            // scroll between frames, ahead of the next snapshot/frame clock.
            handler.post { handler.post { if (!closed) submit() } }
        }
        choreographer.postFrameCallback(frame)
    }
    private fun submit() {
        submitQueued = false
        val index = submitted++
        if (index == warmup) {
            allocation = stat("art.gc.bytes-allocated")
            collections = stat("art.gc.gc-count")
        }
        requestedVsync[index] = currentVsync + trajectoryPeriod
        if (speedDpPerSecond > 0f) {
            if (trajectoryBase == 0L) trajectoryBase = currentVsync
            val elapsed = (requestedVsync[index] - trajectoryBase).coerceAtLeast(0L)
            val travel = (elapsed * speedDpPerSecond.toDouble() * density / 1e9).toLong()
            val segment = travel / range
            turns += (segment - previousSegment).coerceAtLeast(0L).toInt()
            previousSegment = segment
            val remainder = travel % (range.toLong() * 2)
            position = (if (remainder <= range) remainder else range.toLong() * 2 - remainder).toInt()
        } else {
            position += direction * stepPx
            if (position >= range) { position = range; direction = -1; turns++ }
            else if (position <= 0) { position = 0; direction = 1; turns++ }
        }
        requests[index] = position
        cpuStarts[index] = Debug.threadCpuTimeNanos()
        starts[index] = System.nanoTime()
        target.scrollTo(position)
        dispatchWall[index] = System.nanoTime() - starts[index]
        dispatchCpu[index] = Debug.threadCpuTimeNanos() - cpuStarts[index]
        pending = index
    }
    private fun durations(values: LongArray): JSONObject {
        val sorted = values.drop(warmup).filter { it >= 0 }.sorted()
        fun at(q: Double): Any = if (sorted.isEmpty()) JSONObject.NULL else sorted[(ceil(q * sorted.size).toInt() - 1).coerceAtLeast(0)] / 1e6
        return JSONObject().put("samples", sorted.size).put("p50_ms", at(.5)).put("p95_ms", at(.95)).put("p99_ms", at(.99))
            .put("max_ms", sorted.lastOrNull()?.let { it / 1e6 } ?: JSONObject.NULL)
    }
    private fun finish() {
        if (closed) return
        val indices = warmup until count
        val valid = indices.filter { totals[it] >= 0 }
        val eligible = valid.filter { deadline[it] >= 0 && firstDraw[it] == 0L }
        val misses = eligible.count { totals[it] > deadline[it] }
        val gaps = LongArray(count) { if (it > 0) stamps[it] - stamps[it - 1] else -1L }
        val warmupPeriod = gaps.slice(1 until warmup).sorted().let { it[it.size / 2] }
        val refreshRate = target.view.display?.refreshRate?.toDouble() ?: 0.0
        val period = if (refreshRate > 0) (1e9 / refreshRate).toLong() else warmupPeriod
        val skipped = indices.sumOf { ((gaps[it] + period / 2) / period - 1).coerceAtLeast(0) }
        val missingMetrics = indices.count { drawEnds[it] >= 0 && totals[it] < 0 }
        val readbackMismatch = indices.count { observed[it] >= 0 && observed[it] != requests[it] }
        val invalidReadback = indices.count { drawEnds[it] >= 0 && observed[it] !in 0..range }
        val measuredDuration = stamps.last() - stamps[warmup - 1]
        val result = JSONObject().put("renderer", renderer).put("workload", workload)
            .put("elapsed_minus_monotonic_ns", android.os.SystemClock.elapsedRealtimeNanos() - System.nanoTime())
            .put("actions", samples).put("warmup_actions", warmup).put("retained_rows", target.retainedRows)
            .put("list_mode", if (workload == "heavy-scroll-1000") "heavy-retained-1000" else "simple-retained")
            .put("scroll_range_px", range).put("scroll_viewport_px", viewport)
            .put("scroll_step_px", if (speedDpPerSecond == 0f) stepPx else JSONObject.NULL)
            .put("scroll_speed_dp_per_second", if (speedDpPerSecond > 0f) speedDpPerSecond else JSONObject.NULL)
            .put("trajectory_base_vsync_ns", trajectoryBase).put("trajectory_reference_period_ns", trajectoryPeriod)
            .put("trajectory_scope", "VSYNC-based dp/s with catch-up and triangular bounce")
            .put("measured_duration_ns", measuredDuration)
            .put("paced_callbacks_per_second", samples.toDouble() * 1e9 / measuredDuration)
            .put("recorded_window_frames_per_second", valid.size.toDouble() * 1e9 / measuredDuration)
            .put("frame_rate_scope", "Matched Window reports per elapsed interval; no panel FPS")
            .put("scroll_min_offset_px", indices.minOf { requests[it] }).put("scroll_max_offset_px", indices.maxOf { requests[it] })
            .put("scroll_turnarounds", turns).put("scroll_readback_mismatches", readbackMismatch)
            .put("unrequested_draws_during_scroll", unrequestedDraws)
            .put("callbacks_without_submitted_request", callbacksWithoutRequest).put("invalid_scroll_readbacks", invalidReadback)
            .put("drawn_requests", indices.count { drawEnds[it] >= 0 }).put("undrawn_requests", indices.count { drawEnds[it] < 0 })
            .put("frame_metrics_received", valid.size).put("drawn_requests_without_frame_metrics", missingMetrics)
            .put("drawn_requests_with_frame_metrics", indices.count { drawEnds[it] >= 0 && totals[it] >= 0 })
            .put("frame_metrics_without_completed_draw", valid.count { drawEnds[it] < 0 })
            .put("requests_without_frame_metrics", samples - valid.size)
            .put("window_metrics_coverage", valid.size.toDouble() / samples)
            .put("window_metrics_missing_value", -1)
            .put("window_metrics_coverage_scope", "Observed Window samples only; -1 unavailable; drops independent")
            .put("frame_metric_reports_dropped", lost).put("frame_deadline_eligible", eligible.size).put("frame_deadline_misses", misses)
            .put("frame_deadline_miss_fraction", if (eligible.isEmpty()) JSONObject.NULL else misses.toDouble() / eligible.size)
            .put("estimated_skipped_vsync_intervals", skipped)
            .put("estimated_skipped_vsync_fraction", skipped.toDouble() / (samples + skipped))
            .put("vsync_reference_period_ns", period)
            .put("display_refresh_rate_hz", refreshRate).put("warmup_median_callback_interval_ns", warmupPeriod)
            .put("deadline_scope", "Window TOTAL_DURATION>DEADLINE; first draw excluded")
            .put("skipped_vsync_scope", "Rounded callback VSYNC gaps; compositor drops independent")
            .put("readback_scope", "Placed offset after root draw, same VSYNC before next request")
            .put("scroll_scope", "Native paced scroll; 1000 eager rows; no touch/fling")
            .put("cpu_scope", "Main-thread callback interval CPU including shared harness")
            .put("main_thread_cpu", durations(intervalCpu)).put("paced_callback_interval_cpu", durations(intervalCpu))
            .put("action_to_completed_draw_cpu", durations(drawCpu))
            .put("scroll_dispatch_cpu", durations(dispatchCpu)).put("action_dispatch", durations(dispatchWall))
            .put("window_frame", durations(totals)).put("ui_phases", durations(ui)).put("layout_measure", durations(layout))
            .put("display_list_recording", durations(draw)).put("gpu", durations(gpu)).put("frame_deadline", durations(deadline))
            .put("vsync_interval", durations(gaps))
            .put("art_allocated_bytes", stat("art.gc.bytes-allocated")?.let { now -> allocation?.let { now - it } } ?: JSONObject.NULL)
            .put("art_gc_count", stat("art.gc.gc-count")?.let { now -> collections?.let { now - it } } ?: JSONObject.NULL)
        val columns = JSONArray(listOf("measured_vsync_ns", "request_submitted_ns", "requested_px", "observed_draw_px", "paced_callback_interval_cpu_ns", "dispatch_cpu_ns", "dispatch_wall_ns", "action_to_completed_draw_cpu_ns", "window_ns", "ui_ns", "layout_ns", "display_list_ns", "gpu_ns", "deadline_ns", "first_draw", "vsync_interval_ns", "requested_vsync_ns", "completed_draw_ns"))
        // Freeze observations before exporting. liblog can drop a tight burst
        // even while adb drains Logcat; pacing belongs outside measured work.
        active = false
        close()
        // Bounded chunks stay below Logcat's single-message limit. All values
        // are integers; -1 means no observation, never a fabricated zero.
        for (base in warmup until count step 8) {
            val rows = JSONArray()
            for (i in base until minOf(base + 8, count)) rows.put(JSONArray(listOf(stamps[i], starts[i], requests[i], observed[i], intervalCpu[i], dispatchCpu[i], dispatchWall[i], drawCpu[i], totals[i], ui[i], layout[i], draw[i], gpu[i], deadline[i], firstDraw[i], gaps[i], requestedVsync[i], drawEnds[i])))
            val chunk = JSONObject().put("renderer", renderer).put("workload", workload).put("start", base - warmup).put("columns", columns).put("rows", rows)
            android.util.Log.i("ExactCompare", "CompareSamples:" + chunk.toString())
            Thread.sleep(5)
        }
        val resultPayload = result.toString()
        val payloadBytes = resultPayload.toByteArray(Charsets.UTF_8).size
        if (payloadBytes > 3600) {
            failed("continuous scroll result exceeds Logcat payload budget: $payloadBytes bytes")
            return
        }
        android.util.Log.i("ExactCompare", "CompareResult:" + resultPayload)
        // Preserve the complete raw run even when observer loss or invalid
        // readbacks make it unsuitable for comparison. Heavy-list presentation
        // outcomes are independently classified by native FrameTimeline.
        if (lost > 0 || invalidReadback > 0 || (workload != "heavy-scroll-1000" && missingMetrics > 0)) {
            failed("continuous scroll has lost metrics or invalid readbacks: lost=$lost missing=$missingMetrics offsets=$invalidReadback")
            return
        }
        completed(result)
    }
    private fun abort(message: String) { if (!closed) { close(); failed(message) } }
    override fun close() {
        if (closed) return
        closed = true
        handler.removeCallbacks(timeout)
        choreographer.removeFrameCallback(frame)
        choreographer.removeFrameCallback(ticker)
        if (metricsAttached) {
            activity.window.removeOnFrameMetricsAvailableListener(metrics)
            metricsAttached = false
        }
        if (target.view.viewTreeObserver.isAlive) target.view.viewTreeObserver.removeOnDrawListener(drawn)
    }
}
