package com.exact.android

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import org.json.JSONArray
import org.json.JSONObject

/** Real Looper + ScrollView feedback proof; kept outside timed cohorts. */
internal object NativeCollectionCoverageTest {
    fun runAsync(activity: Activity, complete: (String) -> Unit) {
        val handler = Handler(Looper.getMainLooper())
        val parent = activity.findViewById<ViewGroup>(android.R.id.content)
        val host = FrameLayout(activity)
        val scroll = NativeCollectionScrollView(activity)
        val content = Presenter.Box(activity)
        val boxes = (0..7).associate { 11 + it to NativeCollections.RowBox(100.0, 400.0, 500.0 + it * 100) }.toMutableMap()
        val reports = ArrayList<ByteArray>()
        val collections = NativeCollections(host, 1f, { NativeCollections.Port(scroll, content) }, boxes::get, reports::add)
        var revision = 0uL
        var pending = false
        var seeking = false
        var edge = false
        var finished = false
        fun snapshot() {
            revision++
            val rows = JSONArray()
            for (i in 0..7) rows.put(JSONObject().put("view", 11 + i).put("root", 21 + i)
                .put("epoch", "1").put("index", if (edge && i == 0) 0 else i + 5))
            val value = JSONObject().put("view", 1).put("revision", revision.toString()).put("scrollSequence", "0")
                .put("totalExtent", 2000).put("count", 20).put("rows", rows).put("pending", pending).put("seeking", seeking)
            collections.beginBatch(); collections.snapshots(JSONArray().put(value)); collections.endBatch()
        }
        fun move(offset: Int) { scroll.scrollTo(0, offset); collections.changed(1, true) }
        fun finish(result: String) {
            if (finished) return
            finished = true; collections.close(); parent.removeView(host); complete(result)
        }
        val checks = ArrayList<() -> Unit>()
        var expected = 0
        fun sent() { check(reports.size == ++expected) { "Expected $expected reports, got ${reports.size}" } }
        fun quiet() { check(reports.size == expected) { "Scroll-only work escaped: $expected -> ${reports.size}" } }
        checks += { sent(); repeat(20) { move(811 + it) } }
        checks += { quiet(); move(901) }
        checks += { sent(); move(930); move(920) }
        checks += { quiet(); collections.intent(1) }
        checks += { sent(); boxes[11] = checkNotNull(boxes[11]).copy(main = 101.0); move(921) }
        checks += { sent(); collections.pins(11, null) }
        checks += { sent(); move(923) }
        checks += { quiet(); collections.flushDeferredScroll() }
        checks += { sent(); snapshot(); move(924) } // A publication's new revision must be measured.
        checks += { sent(); pending = true; snapshot(); move(925) }
        checks += { sent(); move(926) }
        checks += { sent(); pending = false; seeking = true; snapshot(); move(927) }
        checks += { sent(); move(928) }
        checks += { sent(); seeking = false; edge = true; snapshot(); move(929) }
        checks += { sent(); move(930) }
        checks += { sent(); edge = false; snapshot(); move(931) }
        checks += { sent(); boxes[16] = checkNotNull(boxes[16]).copy(start = 1005.0); move(932) }
        checks += { sent(); move(933) }
        checks += { sent(); boxes[16] = checkNotNull(boxes[16]).copy(start = 1000.0); scroll.layout(0, 0, 320, 200); move(934) }
        checks += { sent(); snapshot(); move(935) }
        checks += { sent();
            val row = JSONObject().put("view", 90).put("root", 91).put("epoch", "1").put("index", 5)
            boxes[90] = NativeCollections.RowBox(100.0, 400.0, 500.0)
            val second = JSONObject().put("view", 2).put("revision", "1").put("scrollSequence", "0")
                .put("totalExtent", 2000).put("count", 20).put("rows", JSONArray().put(row))
            // Keep collection one plus a second observed port in the same adapter.
            val rows = JSONArray()
            for (i in 0..7) rows.put(JSONObject().put("view", 11 + i).put("root", 21 + i).put("epoch", "1").put("index", i + 5))
            val first = JSONObject().put("view", 1).put("revision", revision.toString()).put("scrollSequence", "0")
                .put("totalExtent", 2000).put("count", 20).put("rows", rows)
            collections.snapshots(JSONArray().put(first).put(second))
        }
        checks += { expected = reports.size; move(936) }
        checks += { sent(); finish("NativeCollectionCoverageTest: PASS (same-window/reverse skips; boundary/jump/size/height/pins/publication/pending/seeking/edge/gap reports)") }
        var next = 0
        fun step() {
            if (finished) return
            try { checks[next++](); if (!finished) handler.post { step() } }
            catch (failure: Throwable) { finish("NativeCollectionCoverageTest: FAIL (${failure.stackTraceToString()})") }
        }
        try {
            parent.addView(host, ViewGroup.LayoutParams(400, 200))
            host.addView(scroll, FrameLayout.LayoutParams(400, 200))
            scroll.addView(content, FrameLayout.LayoutParams(400, 2000))
            content.contentHeight = 2000; content.contentWidth = 400
            host.measure(View.MeasureSpec.makeMeasureSpec(400, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(200, View.MeasureSpec.EXACTLY))
            host.layout(0, 0, 400, 200); scroll.scrollTo(0, 810)
            check(scroll.isShown && scroll.height == 200)
            snapshot(); handler.post { step() }
            handler.postDelayed({ finish("NativeCollectionCoverageTest: FAIL (callbacks did not settle)") }, 8000)
        } catch (failure: Throwable) { finish("NativeCollectionCoverageTest: FAIL (${failure.stackTraceToString()})") }
    }
}
