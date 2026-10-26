// @exact/alarms on Android (LLP 1133.030 D2, D4, D7, D9): each alarm is set with `AlarmManager.setAlarmClock`, which
// rings through Doze and may start a foreground service from the background. The build makes one module per host when
// the app binds the extension, outside the agent; under the agent the runner's substitute answers, unless the drive
// binds it (`--bind alarms`).
//
// - The records ([AlarmRecords]) live in device-protected storage, so the receiver and the ringing service, which run
//   before first unlock with no session, read them; they are the package's only state, and the app's list.
// - The status composes exact alarms with notifications (D7): a blocked alarm channel first, so `denied` wins over
//   every other row (Sol's note), then notifications' shared state, then `canScheduleExactAlarms()`.
// - What runs at fire is [AlarmReceiver] and [AlarmService] (`AlarmRing.kt`), with no session; a running process's
//   module hears each change through [AlarmsModule.live].
package exact.alarms

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

class AlarmsModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The live session's module, which the receiver and the service tell of each change. */
        @Volatile internal var live: WeakReference<AlarmsModule>? = null
        internal fun changed() { live?.get()?.let { it.context.changed("alarms.alarms"); it.context.changed("alarms.alarmStatus") } }
    }

    private val app: Context get() = context.application

    init {
        if (!context.agent) live = WeakReference(this)
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "alarmStatus", "alarmStatusSync" -> reply(status())
            "alarms", "alarmsSync" -> reply(JSONObject().put("exact:answer", list()))
            "requestAlarmPermission" -> request { reply(status()) }
            "scheduleAlarm" -> schedule(args, reply)
            "cancelAlarm" -> { AlarmRing.cancel(app, args.optString("tag")); changed(); reply(JSONObject()) }
            "pauseAlarm" -> reply(pause(args.optString("tag")))
            "resumeAlarm" -> reply(resume(args.optString("tag")))
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** The person may change either switch in Settings while the app is away: read again on return (D7). */
    override fun foreground(shown: Boolean) { if (shown) changed() }

    override fun destroy(done: () -> Unit) {
        if (live?.get() === this) live = null
        done()
    }

    // MARK: the status (D7)

    private fun notifications(): String {
        val enabled = app.getSystemService(NotificationManager::class.java).areNotificationsEnabled()
        if (Build.VERSION.SDK_INT < 33) return if (enabled) "granted" else "denied"
        // `prompt` by the shared history is never a denial, though notifications read disabled until it is asked.
        val shared = context.permission(Manifest.permission.POST_NOTIFICATIONS).state
        return if (shared != "prompt" && !enabled) "denied" else shared
    }

    private fun exact(): Boolean = Build.VERSION.SDK_INT < 31 || app.getSystemService(AlarmManager::class.java).canScheduleExactAlarms()

    private fun state(notifications: String = notifications(), exact: Boolean = exact()): String = when {
        AlarmRing.channelBlocked(app) -> "denied"
        notifications == "denied" -> "denied"
        notifications == "prompt" -> "prompt"
        !exact -> "prompt"
        else -> "granted"
    }

    private fun status(): JSONObject {
        val notifications = notifications()
        val exact = exact()
        val state = state(notifications, exact)
        // Revocation ended every alarm the system held, and a denial ends them here, as a ring that cannot be stopped
        // is worse than none; a paused countdown, which holds no system alarm, stays (D7).
        if (!exact || state == "denied") AlarmRing.dropHeld(app)
        return JSONObject().put("available", true).put("state", state).put("countdown", true).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("canScheduleExactAlarms", exact).put("notifications", notifications))
    }

    private fun list(): JSONArray {
        val out = JSONArray()
        for (r in AlarmRecords.all(app)) out.put(r.json())
        return out
    }

    // MARK: the request (D7): notifications first, then exact alarms; each skipped when decided

    private fun request(done: () -> Unit) {
        if (Build.VERSION.SDK_INT >= 33 && notifications() == "prompt") {
            context.askPermission(Manifest.permission.POST_NOTIFICATIONS) {
                if (notifications() == "granted") askExact(done) else done()
            }
        } else if (notifications() == "granted") askExact(done) else done()
    }

    private fun askExact(done: () -> Unit) {
        if (exact()) return done()
        val intent = Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:${app.packageName}"))
        context.startActivityForResult(intent) { _, _ -> done() }
    }

    // MARK: the sends (D2, D7, D9)

    private fun schedule(args: JSONObject, reply: (JSONObject) -> Unit) {
        val tag = args.optString("tag")
        val title = args.optString("title")
        val at = args.optDouble("at").takeIf { args.has("at") && !it.isNaN() }
        val duration = args.optDouble("duration").takeIf { args.has("duration") && !it.isNaN() }
        val snooze = args.optDouble("snooze").takeIf { args.has("snooze") && !it.isNaN() }
        val now = System.currentTimeMillis().toDouble()
        val malformed = tag.isEmpty() || title.isEmpty() || (at == null) == (duration == null) || (at != null && at <= now) ||
            (duration != null && duration < 1000) || (snooze != null && snooze < 1000)
        if (malformed) return reply(JSONObject().put("error", "TypeError"))
        val set = {
            when (state()) {
                "granted" -> {
                    AlarmRing.set(app, AlarmRecord(tag, title, at ?: (System.currentTimeMillis() + duration!!), duration, snooze,
                        null, if (duration != null) "countdown" else "scheduled"))
                    changed()
                    reply(JSONObject())
                }
                else -> reply(JSONObject().put("error", "NotAllowedError"))
            }
        }
        // `asks`: while `prompt` the request runs first, then the schedule (D7).
        if (state() == "prompt") request(set) else set()
    }

    private fun pause(tag: String): JSONObject {
        val r = AlarmRecords.get(app, tag)
        if (r == null || r.state != "countdown") return JSONObject().put("error", "InvalidStateError")
        AlarmRing.pause(app, r)
        changed()
        return JSONObject()
    }

    private fun resume(tag: String): JSONObject {
        val r = AlarmRecords.get(app, tag)
        if (r == null || r.state != "paused") return JSONObject().put("error", "InvalidStateError")
        if (state() != "granted") return JSONObject().put("error", "NotAllowedError")
        AlarmRing.resume(app, r)
        changed()
        return JSONObject()
    }
}

/** One alarm, by its tag (D4): its wall-clock instant, and for a countdown the anchor that measures real time (D9). */
internal class AlarmRecord(
    val tag: String, val title: String, var at: Double?, val duration: Double?, val snooze: Double?,
    var remaining: Double?, var state: String,
    var code: Int = 0, var anchorWall: Long = 0, var anchorElapsed: Long = 0, var anchorBoot: Int = -1,
) {
    fun json(): JSONObject = JSONObject().put("tag", tag).put("title", title).put("state", state)
        .put("at", at ?: JSONObject.NULL).put("duration", duration ?: JSONObject.NULL)
        .put("remaining", remaining ?: JSONObject.NULL).put("snooze", snooze ?: JSONObject.NULL)

    fun stored(): JSONObject = json().put("code", code).put("anchorWall", anchorWall).put("anchorElapsed", anchorElapsed).put("anchorBoot", anchorBoot)

    companion object {
        fun from(o: JSONObject): AlarmRecord {
            val num = { k: String -> if (o.isNull(k)) null else o.optDouble(k).takeIf { !it.isNaN() } }
            return AlarmRecord(o.getString("tag"), o.optString("title"), num("at"), num("duration"), num("snooze"), num("remaining"),
                o.optString("state"), o.optInt("code"), o.optLong("anchorWall"), o.optLong("anchorElapsed"), o.optInt("anchorBoot", -1))
        }
    }
}

/** The records, in device-protected storage: readable before first unlock, as the Clock app's alarms are (D9). */
internal object AlarmRecords {
    private fun prefs(c: Context) = c.createDeviceProtectedStorageContext().getSharedPreferences("exact.alarms", Context.MODE_PRIVATE)

    @Synchronized fun all(c: Context): List<AlarmRecord> {
        val p = prefs(c)
        return p.all.keys.filter { it.startsWith("alarm:") }.sorted().mapNotNull { k ->
            p.getString(k, null)?.let { runCatching { AlarmRecord.from(JSONObject(it)) }.getOrNull() }
        }
    }

    @Synchronized fun get(c: Context, tag: String): AlarmRecord? =
        prefs(c).getString("alarm:$tag", null)?.let { runCatching { AlarmRecord.from(JSONObject(it)) }.getOrNull() }

    @Synchronized fun put(c: Context, r: AlarmRecord) { prefs(c).edit().putString("alarm:${r.tag}", r.stored().toString()).commit() }

    @Synchronized fun remove(c: Context, tag: String) { prefs(c).edit().remove("alarm:$tag").commit() }

    /** A request code per tag, kept so a replacement's PendingIntent never collides with the one it replaces. */
    @Synchronized fun nextCode(c: Context): Int {
        val p = prefs(c)
        val n = p.getInt("next", 1)
        p.edit().putInt("next", n + 1).commit()
        return n
    }
}
