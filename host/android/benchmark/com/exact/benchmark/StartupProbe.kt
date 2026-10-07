package com.exact.benchmark

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import android.view.ViewTreeObserver
import org.json.JSONObject

/** Marks ready content after a complete window draw, separately from system TTID. */
class StartupProbe(
    private val activity: Activity,
    private val target: BenchmarkTarget,
    private val renderer: String,
    private val rows: Int,
    private val token: String,
    private val onCreateElapsedNs: Long
) : AutoCloseable {
    private val handler = Handler(Looper.getMainLooper())
    private var closed = false
    private var posted = false
    private val draw = ViewTreeObserver.OnDrawListener {
        // Read readiness during the draw. A posted callback can run after a
        // viewport update whose changed layout has not been drawn yet.
        if (!closed && !posted && target.startupReady) {
            posted = true
            handler.post {
                if (!closed && target.startupReady && target.retainedRows == rows && target.appliedRevision >= target.revision) {
                    val drawn = SystemClock.elapsedRealtimeNanos()
                    activity.reportFullyDrawn()
                    val reported = SystemClock.elapsedRealtimeNanos()
                    val report = JSONObject().put("token", token).put("renderer", renderer).put("rows", rows)
                        .put("process_start_elapsed_ms", Process.getStartElapsedRealtime())
                        .put("on_create_elapsed_ns", onCreateElapsedNs)
                        .put("first_ready_draw_elapsed_ns", drawn)
                        .put("report_fully_drawn_elapsed_ns", reported)
                    android.util.Log.i("ExactCompare", "Startup:" + report.toString())
                    close()
                } else posted = false
            }
        }
    }
    fun start() { target.view.viewTreeObserver.addOnDrawListener(draw) }
    override fun close() {
        if (closed) return
        closed = true
        if (target.view.viewTreeObserver.isAlive) target.view.viewTreeObserver.removeOnDrawListener(draw)
    }
}
