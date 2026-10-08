package dev.exact.heavybench

import android.app.Activity
import android.os.Bundle
import android.os.SystemClock
import org.json.JSONObject
import com.exact.android.NativeEnvironment

/** Benchmark adapter of main's public native-window painter. */
class MainActivity : Activity() {
    private lateinit var surface: MainSurface
    private var benchmark: HeavyBenchHarness? = null
    override fun onCreate(state: Bundle?) {
        val createdNs = SystemClock.elapsedRealtimeNanos()
        super.onCreate(state)
        NativeEnvironment.configure(this)
        HeavyWindow.configure(this)
        require(HeavyOptions.int(intent, "BENCH_START_INDEX", 0) == 0) {
            "The public main Handle has no row-index scroll/readback API"
        }
        surface = MainSurface(this, HeavyOptions.boolean(intent, "BENCH_LIVE"))
        HeavyWindow.install(this, surface)
        val target = object : ScrollTarget {
            override fun scrollByPx(delta: Float): Float { surface.scrollByPixels(delta); return Float.NaN }
            override fun visibleRows() = -1
            override fun mountedRows() = -1
            override fun firstVisibleId(): String? = null
            override val mountedRowKind = "unavailable-in-public-main-handle"
        }
        benchmark = HeavyBenchHarness(this, "main", createdNs).apply {
            attach(surface, target, { surface.ready }) {
                JSONObject().put("native_submission_counter", surface.submittedFrames)
                    .put("renderer_ready", "public-Handle.first_frame-submission-ack-before-independent-GPU-presentation")
                    .put("scroll_acknowledgment", "unavailable-queued-command")
                    .put("geometry_readback", "unavailable-in-public-main-handle")
            }
        }
    }
    override fun onDestroy() {
        benchmark?.close(); benchmark = null
        if (::surface.isInitialized) surface.close()
        super.onDestroy()
    }
}
