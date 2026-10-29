// @exact/calls on Android (LLP 1133.033 D9, D11, D15, D19): Core-Telecom's `CallsManager`, each call added inside the
// host's `Call` foreground service (D2). The runner keeps the call list and hands it here (`exact:listed`, Charlie's
// 2026-10-27 ruling); this module follows it with Telecom and reports what the person did on the system's surfaces as
// the list's declared choices (`exact-choice:`). Under the agent nothing is registered (D15), and the runner's
// substitute is the call.
//
// - The app's own acts have already moved the list; each becomes Telecom's operation (`answer`, `setActive`,
//   `setInactive`, `disconnect`), and an operation Telecom answers `CallControlResult.Error` ends the call `failed`.
// - Telecom's callbacks no operation of ours caused (`onAnswer`, `onDisconnect`, `onSetActive`, `onSetInactive`: a
//   car, a watch, a Bluetooth headset) are the person's, reported as `answer`, `hangup`, `unhold` and `hold`. The
//   notification's own buttons reach [CallActionReceiver], which performs Telecom's `answer` or `disconnect` first and
//   then reports the choice (D9: the host enforces, then tells).
// - The `Call` service is claimed with `phoneCall`, and `microphone` while `RECORD_AUDIO` is granted. Its notification
//   is a `CallStyle` (API 31 and later): incoming with answer and decline while ringing, with a full-screen intent
//   where `canUseFullScreenIntent()` allows it (else Android shows it heads-up), ongoing with hang-up after (D11).
// - Core-Telecom's `CallControlScope` is a `CoroutineScope`, and the library names kotlinx-coroutines-core only at
//   runtime, so the package names it (1.8.1, the version Core-Telecom 1.0.1 asks) for the compile.
// - Tier 3 (D15): [CallsModule.attributes] edits each `CallAttributesCompat` before `addCall`, [CallsModule.control]
//   hears each `CallControlScope` for its call's life, and [CallsModule.notification] each notification builder.
package exact.calls

import android.Manifest
import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Person
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.telecom.DisconnectCause
import android.util.Log
import androidx.core.telecom.CallAttributesCompat
import androidx.core.telecom.CallControlResult
import androidx.core.telecom.CallControlScope
import androidx.core.telecom.CallsManager
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.ForegroundClaim
import dev.exact.canvas.ForegroundPermit
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.Executor

class CallsModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The live session's module, which the notification's receiver reaches. */
        @Volatile internal var live: WeakReference<CallsModule>? = null

        /** The app's access hatches (D15). */
        /** Each call's attributes before `addCall`, replaced by what it returns: the display name, address, direction
         * and type are the module's, its capabilities the app's to add. */
        @JvmStatic var attributes: ((CallAttributesCompat, JSONObject) -> CallAttributesCompat)? = null
        @JvmStatic var control: ((CallControlScope, JSONObject) -> Unit)? = null
        @JvmStatic var notification: ((Notification.Builder, JSONObject) -> Unit)? = null
    }

    /** A call Telecom holds: its entry as last handed over, its scope once added, and what our own operations asked. */
    private class Live(var entry: JSONObject) {
        var scope: CallControlScope? = null
        val ended = CompletableDeferred<Unit>()
        val asked = HashSet<String>()
        var claim: ForegroundClaim? = null
    }

    private val calls = LinkedHashMap<String, Live>()
    private val main = Handler(Looper.getMainLooper())
    private val work = CoroutineScope(SupervisorJob() + Executor { main.post(it) }.asCoroutineDispatcher())
    private val manager: CallsManager by lazy {
        CallsManager(context.application).also {
            it.registerAppWithTelecom(CallsManager.CAPABILITY_BASELINE or CallsManager.CAPABILITY_SUPPORTS_VIDEO_CALLING)
        }
    }

    init {
        if (!context.agent) live = WeakReference(this)
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "exact:listed" -> { if (!context.agent) follow(args.optJSONArray("entries") ?: JSONArray()); reply(JSONObject()) }
            "callsStatus", "callsStatusSync" -> reply(JSONObject().put("available", true).put("systemUI", !context.agent))
            // The list's names are the runner's (`declared/handed.rs`), never sent here.
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    override fun destroy(done: () -> Unit) {
        if (live?.get() === this) live = null
        done()
    }

    /** The person's act, as the list's declared choice. */
    internal fun choose(id: String, choice: String) {
        Log.i("exact", "calls: $id $choice (the system's surface)")
        context.changed("exact-choice:" + JSONObject().put("extension", "calls").put("state", "calls").put("key", id).put("choice", choice))
    }

    private fun failed(id: String, what: String, result: CallControlResult) {
        if (result is CallControlResult.Success) return
        Log.i("exact", "calls: $id $what refused by Telecom: $result")
        choose(id, "failed")
    }

    /** The runner's list now: each change of an entry since the last becomes Telecom's (module doc). */
    private fun follow(entries: JSONArray) {
        val seen = HashSet<String>()
        for (i in 0 until entries.length()) {
            val e = entries.optJSONObject(i) ?: continue
            val id = e.optString("id").takeIf { it.isNotEmpty() } ?: continue
            val state = e.optString("state")
            seen += id
            val call = calls[id]
            if (call == null) {
                if (state != "ended") add(id, e)
                continue
            }
            val before = call.entry.optString("state")
            call.entry = e
            claimChanged(call)
            if (state == before) continue
            val scope = call.scope ?: continue
            work.launch {
                when {
                    state == "ended" -> ended(id, call, e)
                    before == "ringing" && state == "connected" ->
                        if (!call.asked.remove("answer-person")) failed(id, "answer", scope.answer(callType(e)))
                    before == "held" && state == "connected" ->
                        if (!call.asked.remove("unhold-person")) failed(id, "setActive", scope.setActive())
                    state == "held" -> if (!call.asked.remove("hold-person")) failed(id, "setInactive", scope.setInactive())
                    state == "connected" -> failed(id, "setActive", scope.setActive())
                }
            }
        }
        // An entry gone from the list without our seeing it end has ended.
        for (id in calls.keys.filter { it !in seen }) calls[id]?.let { call -> work.launch { ended(id, call, JSONObject().put("endedReason", "remote")) } }
    }

    private fun callType(e: JSONObject) =
        if (e.optBoolean("video")) CallAttributesCompat.CALL_TYPE_VIDEO_CALL else CallAttributesCompat.CALL_TYPE_AUDIO_CALL

    private suspend fun ended(id: String, call: Live, e: JSONObject) {
        calls.remove(id)
        if (!call.asked.remove("end-person")) {
            val cause = when (e.optString("endedReason")) {
                "local" -> DisconnectCause.LOCAL
                "declined" -> DisconnectCause.REJECTED
                "remote" -> DisconnectCause.REMOTE
                "unanswered" -> DisconnectCause.MISSED
                "answeredElsewhere", "declinedElsewhere" -> DisconnectCause.ANSWERED_ELSEWHERE
                else -> DisconnectCause.ERROR
            }
            call.scope?.disconnect(DisconnectCause(cause))
        }
        call.ended.complete(Unit)
        call.claim?.release()
    }

    /** Add [e] to Telecom inside the `Call` service: the service is claimed first, and the call lives in its block. */
    private fun add(id: String, e: JSONObject) {
        val call = Live(e)
        calls[id] = call
        val incoming = e.optString("direction") == "incoming"
        val mic = context.application.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        val types = if (mic && context.foregroundService.declared("call", "microphone")) setOf("phoneCall", "microphone") else setOf("phoneCall")
        call.claim = context.foregroundService.claim("call", types, context.origin, permit = if (context.resumed) null else ForegroundPermit.NotificationAction,
            notification = { b -> style(b, id, call) }, lost = { why -> Log.i("exact", "calls: $id lost its service: $why"); choose(id, "failed") })
        if (call.claim == null) { calls.remove(id); choose(id, "failed"); return }
        val name = e.optString("displayName").ifEmpty { e.optString("handle") }
        val address = when (e.optString("handleType")) {
            "phoneNumber" -> Uri.fromParts("tel", e.optString("handle"), null)
            "emailAddress" -> Uri.fromParts("mailto", e.optString("handle"), null)
            else -> Uri.fromParts("sip", e.optString("handle"), null)
        }
        val attrs = CallAttributesCompat(name, address,
            if (incoming) CallAttributesCompat.DIRECTION_INCOMING else CallAttributesCompat.DIRECTION_OUTGOING,
            callType(e), CallAttributesCompat.SUPPORTS_SET_INACTIVE)
        val chosen = attributes?.invoke(attrs, e)?.let { CallAttributesCompat(attrs.displayName, attrs.address, attrs.direction, attrs.callType, it.callCapabilities) } ?: attrs
        work.launch {
            try {
                manager.addCall(chosen,
                    onAnswer = { _ -> call.asked += "answer-person"; choose(id, "answer") },
                    onDisconnect = { _ -> if (calls.containsKey(id)) { call.asked += "end-person"; choose(id, "hangup") } },
                    onSetActive = { if (call.entry.optString("state") == "held") { call.asked += "unhold-person"; choose(id, "unhold") } },
                    onSetInactive = { if (call.entry.optString("state") == "connected") { call.asked += "hold-person"; choose(id, "hold") } },
                ) {
                    call.scope = this
                    control?.invoke(this, e)
                    // The scope is a coroutine scope for the call's life; the call stays added until it ends.
                    launch {
                        if (!incoming && call.entry.optString("state") == "connected") failed(id, "setActive", setActive())
                        call.ended.await()
                    }
                }
            } catch (x: Exception) {
                Log.i("exact", "calls: $id not added: ${x.javaClass.simpleName}: ${x.message}")
                calls.remove(id)
                call.claim?.release()
                choose(id, "failed")
            }
        }
    }

    private fun claimChanged(call: Live) {
        call.claim?.update { b -> style(b, call.entry.optString("id"), call) }
    }

    /** The call's `CallStyle` (D11), after the host's parts of the service's notification. */
    private fun style(b: Notification.Builder, id: String, call: Live) {
        val e = call.entry
        val app = context.application
        b.setCategory(Notification.CATEGORY_CALL)
        if (Build.VERSION.SDK_INT >= 31) {
            val person = Person.Builder().setName(e.optString("displayName").ifEmpty { e.optString("handle") }).setImportant(true).build()
            val ringing = e.optString("state") == "ringing"
            b.setStyle(if (ringing) Notification.CallStyle.forIncomingCall(person, action(app, id, "decline"), action(app, id, "answer"))
                else Notification.CallStyle.forOngoingCall(person, action(app, id, "hangup")))
            if (ringing) {
                val manager = app.getSystemService(NotificationManager::class.java)
                val allowed = Build.VERSION.SDK_INT < 34 || manager.canUseFullScreenIntent()
                val launch = app.packageManager.getLaunchIntentForPackage(app.packageName)
                if (allowed && launch != null) b.setFullScreenIntent(PendingIntent.getActivity(app, 1, launch, PendingIntent.FLAG_IMMUTABLE), true)
            }
        }
        notification?.invoke(b, e)
    }

    private fun action(app: Context, id: String, what: String): PendingIntent =
        PendingIntent.getBroadcast(app, (id + what).hashCode(), Intent(app, CallActionReceiver::class.java).setAction("exact.calls.$what").putExtra("id", id),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

    /** A button of the call's notification (D11): Telecom's operation first, then the person's choice. */
    internal fun notificationAction(id: String, what: String) {
        val call = calls[id] ?: return
        val scope = call.scope ?: return
        work.launch {
            when (what) {
                "answer" -> {
                    val r = scope.answer(callType(call.entry))
                    if (r is CallControlResult.Success) { call.asked += "answer-person"; choose(id, "answer") } else failed(id, "answer", r)
                }
                "decline", "hangup" -> {
                    call.asked += "end-person"
                    scope.disconnect(DisconnectCause(if (what == "decline") DisconnectCause.REJECTED else DisconnectCause.LOCAL))
                    choose(id, "hangup")
                }
            }
        }
    }
}

/** The call notification's buttons (`exact.calls.answer`, `decline`, `hangup`), sent to it by explicit intent, so its manifest entry names no action. */
class CallActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val id = intent.getStringExtra("id") ?: return
        val what = intent.action?.removePrefix("exact.calls.") ?: return
        CallsModule.live?.get()?.notificationAction(id, what) ?: Log.i("exact", "calls: $what for $id: no session holds the call")
    }
}
