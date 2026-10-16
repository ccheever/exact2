package com.exact.android

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.View
import org.json.JSONObject
import kotlin.math.abs

/** Dedicated test adapter; the normal ExactActivity and application bake are untouched. */
class ExactActivity : Activity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var token: String
    private lateinit var exact: ExactView
    private lateinit var preferredSize: (Float, Float) -> Unit
    private lateinit var probe: View
    private lateinit var follower: View
    private var turns = 0
    private var callbacks = 0
    private var ended = false
    private var stable = 0
    private var lastTurns = -1
    private val deadline = SystemClock.elapsedRealtime() + 5000
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        token = checkNotNull(intent.getStringExtra("exact_sdk_token"))
        check(token.matches(Regex("[a-f0-9-]{36}")))
        val factory = NativeViewFactory { context, name, props, _, report ->
            check(name == "async-size-probe" && props.length() == 0 && callbacks == 0)
            preferredSize = report
            object : NativeComponent {
                override val view = View(context).apply { setBackgroundColor(Color.BLUE); probe = this }
                override fun update(props: JSONObject) = Unit
            }.also { callbacks++; report(80f, 20f) }
        }
        exact = ExactView(this, nativeFactory = factory)
        exact.observeTransaction = { _, _ -> turns++ }
        exact.onFirstDraw = { handler.post { idle() } }
        setContentView(exact)
        handler.postDelayed({ finishTest(IllegalStateException("Idle intrinsic regression timed out")) }, 5000)
    }
    // Direct JNI query copies its borrowed reply; it never enters ExactView.apply or drains the queue.
    private fun node(name: String): JSONObject {
        val id = checkNotNull(exact.geometry(name)).id
        val handle = ExactView::class.java.getDeclaredField("handle").apply { isAccessible = true }.getLong(exact)
        val reply = Native.agent(handle, "{\"op\":\"node\",\"id\":$id}".toByteArray(Charsets.UTF_8))
        val bytes = ByteArray(reply.remaining()); reply.get(bytes)
        return JSONObject(String(bytes, Charsets.UTF_8)).also { check(it.getInt("id") == id) }
    }
    private fun geometry(height: Double): Pair<JSONObject, JSONObject> {
        val p = node("probe"); val f = node("follower")
        check(abs(p.getJSONObject("frame").getDouble("h") - height) < .0001)
        check(abs(f.getJSONObject("frame").getDouble("y") - height) < .0001)
        val scale = resources.displayMetrics.density
        check(probe.isAttachedToWindow && abs(probe.height - height * scale) <= 1)
        check(follower.isAttachedToWindow && abs(follower.top - height * scale) <= 1)
        return p to f
    }
    private fun guard(body: () -> Unit) { if (!ended) try { body() } catch (error: Throwable) { finishTest(error) } }
    private fun idle(): Unit = guard {
        check(SystemClock.elapsedRealtime() < deadline)
        stable = if (exact.startupReady && exact.creationToFirstHostDrawNs != null && turns == lastTurns) stable + 1 else 0
        lastTurns = turns
        if (stable < 4) { handler.postDelayed({ idle() }, 50); return@guard }
        follower = checkNotNull(exact.actionView("follower"))
        val before = geometry(20.0); val beforeTurns = turns
        check(callbacks == 1)
        callbacks++; preferredSize(80f, 60f)
        val returned = geometry(20.0)
        check(turns == beforeTurns && returned.first.getInt("id") == before.first.getInt("id"))
        val start = SystemClock.elapsedRealtimeNanos()
        // Only this observation timer runs: no clock/event/pump/fit or explicit neutral control.
        handler.postDelayed({ guard {
            check(SystemClock.elapsedRealtimeNanos() - start >= 290_000_000)
            val after = geometry(60.0)
            check(callbacks == 2 && turns == beforeTurns + 2)
            for (key in listOf("id", "parent", "epoch", "incarnation")) {
                check(before.first.get(key) == after.first.get(key) && before.second.get(key) == after.second.get(key))
            }
            finishTest(null)
        } }, 300)
    }
    private fun finishTest(error: Throwable?) {
        if (ended) return
        ended = true; handler.removeCallbacksAndMessages(null)
        exact.close()
        if (error == null) Log.i("ExactSDKTest", "IntrinsicWakeTest: PASS $token (real idle owner, one callback, kernel/native probe20->60 and follower20->60, no external turn)")
        else Log.e("ExactSDKTest", "IntrinsicWakeTest: FAIL $token", error)
    }
    override fun onDestroy() { handler.removeCallbacksAndMessages(null); if (::exact.isInitialized) exact.close(); super.onDestroy() }
}
