package dev.exact.heavybench

import android.app.Activity
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import com.exact.android.ExactView
import com.exact.android.InlineTextPaintTest
import com.exact.android.NativeBorderTest
import com.exact.android.PendingIntrinsicsTest
import com.exact.android.Presenter
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

internal object C9Config {
    init { System.loadLibrary("exact_app") }
    @JvmStatic external fun setLive(live: Boolean)
}

/** Original Contract/provider over the production imperative Views host. */
class C9Activity : Activity() {
    private var exact: ExactView? = null
    private var harness: HeavyBenchHarness? = null
    override fun onCreate(savedInstanceState: Bundle?) {
        val created = SystemClock.elapsedRealtimeNanos()
        super.onCreate(savedInstanceState)
        check(HeavyOptions.int(intent, "BENCH_START_INDEX", 0) == 0) {
            "Cross-renderer Heavy List starts at index zero; keyed jump is a separate test"
        }
        HeavyWindow.configure(this)
        C9Config.setLive(HeavyOptions.boolean(intent, "BENCH_LIVE"))
        if (HeavyOptions.boolean(intent, "BENCH_VERIFY")) {
            Log.i("HeavyBench", InlineTextPaintTest.run(this))
            Log.i("HeavyBench", NativeBorderTest.run())
            Log.i("HeavyBench", PendingIntrinsicsTest.run())
        }
        val view = ExactView(this)
        exact = view
        HeavyWindow.install(this, view)
        HeavyWindow.configure(this)
        val target = object : ScrollTarget {
            private var sampledAt = -1L
            private var sample: Presenter.CollectionInfo? = null
            private var fraction = 0f
            private fun inspect(): Presenter.CollectionInfo? {
                val stamp = SystemClock.uptimeMillis()
                if (stamp != sampledAt) { sampledAt = stamp; sample = view.collectionInfo("messages") }
                return sample
            }
            override fun scrollByPx(delta: Float): Float {
                val scroll = inspect()?.scroll ?: return Float.NaN
                val requested = delta + fraction
                val pixels = requested.roundToInt(); fraction = requested - pixels
                val before = scroll.scrollY
                scroll.scrollBy(0, pixels)
                sampledAt = -1
                return (scroll.scrollY - before).toFloat()
            }
            override fun visibleRows() = inspect()?.visible ?: 0
            override fun mountedRows() = inspect()?.mounted ?: 0
            override fun firstVisibleId() = inspect()?.firstVisibleTestId
            override val mountedRowKind = "runner-mounted-row-wrappers"
        }
        val probe = HeavyBenchHarness(this, "c9", created)
        harness = probe
        probe.attach(view, target, { view.startupReady && view.creationToFirstHostDrawNs != null && target.visibleRows() > 0 }, {
            JSONObject().put("retained_nodes", view.retainedNodes).put("materialized_nodes", view.materializedNodes)
                .put("bridge_stats", JSONArray(view.bridgeStats().toList()))
                .put("content", "original baked Contract and full Heavy data provider")
        })
    }
    override fun onDestroy() {
        harness?.close(); exact?.close()
        harness = null; exact = null
        super.onDestroy()
    }
}
