// @exact/calendar's production path on Android (LLP 1133.018 stage 3): the person's calendars through the Calendar
// Provider, and the calendar app's insert screen.
//
// - A read asks `READ_CALENDAR` while `prompt`; a write asks it with `WRITE_CALENDAR`, in one request, since an
//   insert names a calendar row only a read finds (D11). The status is `READ_CALENDAR`'s, `full` once granted.
// - A read runs on the module's thread (D6): `Instances` for the range ("a blocking function and should not be done
//   on the UI thread"), in the order of every target, with the same cursor as the agent's path. An all-day
//   occurrence, which the provider keeps at UTC midnights, is read back as local dates in the device's zone (D2).
// - A floating timed event is written in the device's zone, which the answer names (D2): the provider requires
//   `EVENT_TIMEZONE` on every insert.
// - The calendar `saveEvent` writes without `calendar=` is the first visible, writable calendar whose `IS_PRIMARY` is
//   set, by the provider's order (D3).
// - The editor is `Intent.ACTION_INSERT` on `Events.CONTENT_URI`, which needs no permission: `saved` if the calendar
//   app returns `RESULT_OK`, else `unknown`.
// - A content observer on `Events` and `Calendars`, with descendants, is a change of the store.
package exact.calendar

import android.Manifest
import android.app.Activity
import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.content.Intent
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.database.Cursor
import android.os.Handler
import android.os.Looper
import android.provider.CalendarContract
import android.provider.CalendarContract.Calendars
import android.provider.CalendarContract.Events
import android.provider.CalendarContract.Instances
import dev.exact.canvas.ExtensionContext
import exact.calendar.AgentCalendar.Companion.text
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.ZoneId
import java.time.ZoneOffset
import java.util.concurrent.Executors

internal class EventStore(private val context: ExtensionContext, private val say: (String) -> Unit, private val changed: () -> Unit) {
    private companion object {
        const val READ = Manifest.permission.READ_CALENDAR
        const val WRITE = Manifest.permission.WRITE_CALENDAR
    }

    private val app get() = context.application
    private val resolver: ContentResolver get() = app.contentResolver
    private val main = Handler(Looper.getMainLooper())
    /** The module's thread: queries and writes. */
    private val work = Executors.newSingleThreadExecutor()
    private var observing = false
    private var editing = false
    /** The status last answered, which a return to the foreground compares with. */
    private var said: String? = null
    private val observer = object : ContentObserver(main) {
        override fun onChange(selfChange: Boolean) = changed()
    }

    private val declared: Set<String> by lazy {
        app.packageManager.getPackageInfo(app.packageName, PackageManager.GET_PERMISSIONS).requestedPermissions.orEmpty().toSet()
    }

    private fun has(permission: String) = app.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "calendarStatus", "requestFullCalendarAccess" -> reply(status())
            "calendars" -> allowed(listOf(READ), reply) { observe(); work.execute { answer(reply) { JSONObject().put("calendars", JSONArray(calendars())) } } }
            "calendarEvents" -> allowed(listOf(READ), reply) { observe(); page(args, reply) }
            "saveEvent" -> allowed(listOf(READ, WRITE), reply) { work.execute { answer(reply) { save(args) } } }
            "removeEvent" -> allowed(listOf(READ, WRITE), reply) { work.execute { answer(reply) { remove(args) } } }
            "showEventEditor" -> editor(args, reply)
            else -> reply(CalendarModule.error("NotSupportedError"))
        }
    }

    /** [work]'s answer on the main thread, a refusal of the provider's as `NotAllowedError`. */
    private fun answer(reply: (JSONObject) -> Unit, work: () -> JSONObject) {
        val out = try { work() } catch (e: SecurityException) { CalendarModule.error("NotAllowedError", e.message) } catch (e: IllegalArgumentException) {
            CalendarModule.error("TypeError", e.message)
        } catch (e: RuntimeException) { CalendarModule.error("UnknownError", e.message) }
        main.post { reply(out) }
    }

    // Access (D11)

    fun status(): JSONObject {
        val granted = has(READ)
        // Not granted: the state is the host's history of the request (LLP 1133.008 D8 item 7).
        val history = if (READ in declared) context.permission(READ) else null
        val out = JSONObject()
            .put("available", true)
            .put("state", if (granted) "granted" else history?.state ?: "prompt")
            .put("access", if (granted) "full" else "none")
            .put("editor", true)
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject()
                .put("shouldShowRationale", history?.shouldShowRationale == true)
                .put("permanentlyDenied", !granted && history?.permanentlyDenied == true))
        said = out.toString()
        return out
    }

    /** A return to the foreground: a grant changed in Settings is a change. */
    fun refresh() {
        val before = said ?: return
        if (status().toString() != before) changed()
    }

    /** [then] once the app holds [permissions]: asked in one request while any is not refused for good; refused while
     * the manifest lacks one or the person refused. */
    private fun allowed(permissions: List<String>, reply: (JSONObject) -> Unit, then: () -> Unit) {
        val missing = permissions.filter { it !in declared }
        if (missing.isNotEmpty()) return reply(CalendarModule.error("NotAllowedError", "the manifest declares no ${missing.joinToString(", ")}"))
        if (permissions.all(::has)) return then()
        if (permissions.any { context.permission(it).state == "denied" }) return reply(CalendarModule.error("NotAllowedError"))
        context.requestPermissions(permissions.toTypedArray()) {
            // A refusal with no rationale owed after it is permanent: the history `permission` reads.
            val activity = context.activity
            for (p in permissions) if (!has(p) && activity != null && !activity.shouldShowRequestPermissionRationale(p)) {
                app.getSharedPreferences("exact.permissions", android.content.Context.MODE_PRIVATE).edit().putBoolean(p, true).apply()
            }
            say("asked ${permissions.joinToString(",") { it.substringAfterLast('.') }}: ${if (permissions.all(::has)) "granted" else "not granted"}")
            context.changed("calendar.calendarStatus")
            if (permissions.all(::has)) then() else reply(CalendarModule.error("NotAllowedError"))
        }
    }

    private fun observe() {
        if (observing) return
        observing = true
        resolver.registerContentObserver(Events.CONTENT_URI, true, observer)
        resolver.registerContentObserver(Calendars.CONTENT_URI, true, observer)
    }

    fun stopObserving() {
        if (!observing) return
        observing = false
        resolver.unregisterContentObserver(observer)
    }

    // Calendars

    private class Calendar(val id: Long, val title: String, val color: Int, val writable: Boolean, val primary: Boolean, val visible: Boolean, val source: String)

    private fun rows(): List<Calendar> {
        val columns = arrayOf(Calendars._ID, Calendars.CALENDAR_DISPLAY_NAME, Calendars.CALENDAR_COLOR, Calendars.CALENDAR_ACCESS_LEVEL,
            Calendars.IS_PRIMARY, Calendars.VISIBLE, Calendars.ACCOUNT_NAME)
        val out = ArrayList<Calendar>()
        resolver.query(Calendars.CONTENT_URI, columns, null, null, null)?.use { c ->
            while (c.moveToNext()) out += Calendar(c.getLong(0), c.getString(1).orEmpty(), c.getInt(2), c.getInt(3) >= Calendars.CAL_ACCESS_CONTRIBUTOR,
                c.getInt(4) == 1, c.getInt(5) == 1, c.getString(6).orEmpty())
        }
        return out
    }

    /** No single default on Android (D3): the first visible, writable, primary calendar by the provider's order. */
    private fun fallback(rows: List<Calendar>) = rows.firstOrNull { it.visible && it.writable && it.primary }

    private fun calendars(): List<JSONObject> {
        val rows = rows()
        val fallback = fallback(rows)?.id
        return rows.map { c ->
            JSONObject().put("id", c.id.toString()).put("title", c.title).put("color", "#%06x".format(c.color and 0xffffff))
                .put("writable", c.writable).put("isDefault", c.id == fallback).put("source", c.source)
        }
    }

    // Occurrences (D2, D6)

    private val zone: ZoneId get() = ZoneId.systemDefault()

    private fun page(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (!args.has("start") || !args.has("end") || args.getDouble("end") < args.getDouble("start")) {
            return reply(CalendarModule.error("TypeError", "start and end are instants, end not before start"))
        }
        val (start, end) = args.getDouble("start") to args.getDouble("end")
        if (end - start > AgentCalendar.LONGEST) return reply(CalendarModule.error("TypeError", "calendar-range-too-long"))
        val after = args.text("after")?.let { AgentCalendar.cursor(it) ?: return reply(CalendarModule.error("TypeError", "after is not a cursor")) }
        val only = args.optJSONArray("calendars")?.let { a -> (0 until a.length()).mapNotNull { a.getString(it).toLongOrNull() } }
        val limit = args.optInt("limit", 500).coerceIn(1, 2000)
        work.execute {
            val began = System.nanoTime()
            answer(reply) {
                // From the cursor's start: what follows it in the order starts no earlier.
                val from = maxOf(start, after?.start ?: start)
                val list = occurrences(from.toLong(), end.toLong(), only).filter { (k, _) ->
                    k.start < end && (k.end > start || (k.start == k.end && k.start >= start)) && (after == null || k > after)
                }.sortedBy { it.first }
                val shown = list.take(limit)
                val ms = (System.nanoTime() - began) / 1_000_000
                main.post { say("page calendarEvents ${shown.size} of ${list.size} after ${args.text("after") ?: "start"} in $ms ms") }
                JSONObject().put("events", JSONArray(shown.map { it.second })).put("next", if (list.size > limit) shown.last().first.toString() else JSONObject.NULL)
            }
        }
    }

    private val instanceColumns = arrayOf(Instances.EVENT_ID, Instances.BEGIN, Instances.END, Instances.ALL_DAY, Instances.TITLE, Instances.DESCRIPTION,
        Instances.EVENT_LOCATION, Instances.EVENT_TIMEZONE, Instances.CALENDAR_ID, Instances.STATUS, Instances.AVAILABILITY, Instances.UID_2445,
        Instances.RRULE, Instances.ORIGINAL_ID, Instances.ORIGINAL_INSTANCE_TIME)

    private fun occurrences(from: Long, to: Long, only: List<Long>?): List<Pair<AgentCalendar.Key, JSONObject>> {
        val uri = Instances.CONTENT_URI.buildUpon().also { ContentUris.appendId(it, from); ContentUris.appendId(it, to) }.build()
        val selection = only?.let { "${Instances.CALENDAR_ID} IN (${it.joinToString(",").ifEmpty { "-1" }})" }
        val out = ArrayList<Pair<AgentCalendar.Key, JSONObject>>()
        resolver.query(uri, instanceColumns, selection, null, null)?.use { c -> while (c.moveToNext()) out += occurrence(c) }
        return out
    }

    private fun occurrence(c: Cursor): Pair<AgentCalendar.Key, JSONObject> {
        val allDay = c.getInt(3) == 1
        val named = c.string(7)?.let { try { ZoneId.of(it) } catch (e: java.time.DateTimeException) { null } }
        val shown = if (allDay) zone else named ?: zone
        // An all-day occurrence's UTC midnights, read as the local dates they name.
        val (s, t) = if (allDay) {
            val day = { ms: Long -> Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate().atStartOfDay(zone).toInstant().toEpochMilli() }
            day(c.getLong(1)) to day(c.getLong(2))
        } else c.getLong(1) to c.getLong(2)
        val series = c.string(13)?.toLongOrNull()
        val recurring = c.string(12) != null || series != null
        val original = if (series != null && !c.isNull(14)) c.getLong(14) else c.getLong(1)
        val recurrence = if (recurring) AgentCalendar.local(Instant.ofEpochMilli(original).atZone(shown).toLocalDateTime()) else null
        val id = (series ?: c.getLong(0)).toString()
        val event = JSONObject()
            .put("id", id).put("uid", c.string(11) ?: JSONObject.NULL).put("calendarId", c.getLong(8).toString())
            .put("title", c.string(4).orEmpty()).put("description", c.string(5) ?: JSONObject.NULL)
            .put("start", AgentCalendar.local(Instant.ofEpochMilli(s).atZone(shown).toLocalDateTime()))
            .put("timeZone", if (allDay) JSONObject.NULL else c.string(7) ?: JSONObject.NULL)
            .put("duration", if (allDay) "P${maxOf(1L, Math.round((t - s) / 86_400_000.0))}D" else AgentCalendar.iso(Math.round((t - s) / 1000.0)))
            .put("showWithoutTime", allDay).put("location", c.string(6) ?: JSONObject.NULL).put("url", JSONObject.NULL)
            .put("status", when (c.getInt(9)) { Events.STATUS_TENTATIVE -> "tentative"; Events.STATUS_CANCELED -> "cancelled"; else -> "confirmed" })
            .put("freeBusyStatus", if (c.getInt(10) == Events.AVAILABILITY_FREE) "free" else "busy")
            .put("recurrenceId", recurrence ?: JSONObject.NULL).put("utcStart", s.toDouble()).put("utcEnd", t.toDouble()).put("apple", JSONObject.NULL)
        return AgentCalendar.Key(s.toDouble(), t.toDouble(), id, recurrence.orEmpty()) to event
    }

    private fun Cursor.string(i: Int): String? = if (isNull(i)) null else getString(i)

    // Writes

    /** [event]'s fields as the provider takes them, or an error's answer; the zone written, null for all day. */
    private fun values(event: JSONObject): Pair<ContentValues, String?>? {
        val title = event.text("title") ?: return null
        val start = AgentCalendar.parts(event.optString("start")) ?: return null
        val (days, seconds) = AgentCalendar.duration(event.text("duration") ?: "PT0S") ?: return null
        val allDay = event.optBoolean("showWithoutTime")
        // Floating time is written in the device's zone, and the answer says so (D2).
        val written = if (allDay) null else event.text("timeZone") ?: zone.id
        val placed = try {
            if (allDay) start.toLocalDate().atStartOfDay(ZoneOffset.UTC) else start.atZone(ZoneId.of(written))
        } catch (e: java.time.DateTimeException) { return null }
        val begin = placed.toInstant().toEpochMilli()
        val finish = placed.plusDays(days.toLong()).plusSeconds(seconds.toLong()).toInstant().toEpochMilli()
        return ContentValues().apply {
            put(Events.TITLE, title)
            put(Events.DTSTART, begin)
            put(Events.DTEND, finish)
            put(Events.ALL_DAY, if (allDay) 1 else 0)
            put(Events.EVENT_TIMEZONE, written ?: "UTC")
            put(Events.DESCRIPTION, event.text("description"))
            put(Events.EVENT_LOCATION, event.text("location"))
            put(Events.AVAILABILITY, if (event.text("freeBusyStatus") == "free") Events.AVAILABILITY_FREE else Events.AVAILABILITY_BUSY)
            put(Events.STATUS, when (event.text("status")) { "tentative" -> Events.STATUS_TENTATIVE; "cancelled" -> Events.STATUS_CANCELED; else -> Events.STATUS_CONFIRMED })
            event.text("uid")?.let { put(Events.UID_2445, it) }
        } to written
    }

    /** The event row [id]: its calendar and whether it is a series or an occurrence of one; null where there is none. */
    private fun row(id: Long): Pair<Long, Boolean>? =
        resolver.query(ContentUris.withAppendedId(Events.CONTENT_URI, id), arrayOf(Events.CALENDAR_ID, Events.RRULE, Events.ORIGINAL_ID), null, null, null)?.use { c ->
            if (!c.moveToFirst()) null else c.getLong(0) to (!c.isNull(1) || !c.isNull(2))
        }

    private fun save(args: JSONObject): JSONObject {
        val event = args.optJSONObject("event") ?: return CalendarModule.malformed
        if (event.text("recurrenceId") != null) return CalendarModule.error("NotSupportedError", "an occurrence of a series")
        val (values, written) = values(event) ?: return CalendarModule.malformed
        val id = event.text("id")?.let { it.toLongOrNull() ?: return CalendarModule.error("NotFoundError") }
        val existing = id?.let { row(it) ?: return CalendarModule.error("NotFoundError") }
        if (existing?.second == true) return CalendarModule.error("NotSupportedError", "an occurrence of a series")
        val rows = rows()
        val calendar = when (val asked = args.text("calendar")) {
            null -> existing?.first?.let { c -> rows.firstOrNull { it.id == c } } ?: fallback(rows) ?: return CalendarModule.error("NotFoundError")
            else -> rows.firstOrNull { it.id.toString() == asked } ?: return CalendarModule.error("NotFoundError")
        }
        if (!calendar.writable) return CalendarModule.error("NotAllowedError", "calendar-read-only")
        values.put(Events.CALENDAR_ID, calendar.id)
        val saved = if (id != null) {
            resolver.update(ContentUris.withAppendedId(Events.CONTENT_URI, id), values, null, null)
            id
        } else {
            resolver.insert(Events.CONTENT_URI, values)?.let(ContentUris::parseId) ?: return CalendarModule.error("UnknownError")
        }
        main.post { say("saved $saved to ${calendar.id}") }
        val begin = values.getAsLong(Events.DTSTART)
        val finish = values.getAsLong(Events.DTEND)
        val out = JSONObject(event.toString())
            .put("id", saved.toString()).put("calendarId", calendar.id.toString()).put("uid", event.text("uid") ?: JSONObject.NULL)
            .put("timeZone", written ?: JSONObject.NULL).put("recurrenceId", JSONObject.NULL).put("apple", JSONObject.NULL)
            .put("status", event.text("status") ?: "confirmed").put("freeBusyStatus", event.text("freeBusyStatus") ?: "busy")
        // An all-day event's local dates, in the device's zone, as a read answers them.
        val (s, t) = if (written == null) {
            val day = { ms: Long -> Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate().atStartOfDay(zone).toInstant().toEpochMilli() }
            day(begin) to day(finish)
        } else begin to finish
        for (field in listOf("description", "location", "url")) if (!out.has(field)) out.put(field, JSONObject.NULL)
        return out.put("utcStart", s.toDouble()).put("utcEnd", t.toDouble())
    }

    private fun remove(args: JSONObject): JSONObject {
        val id = args.text("id")?.toLongOrNull() ?: return CalendarModule.error("NotFoundError")
        val existing = row(id) ?: return CalendarModule.error("NotFoundError")
        if (existing.second) return CalendarModule.error("NotSupportedError", "an occurrence of a series")
        if (resolver.delete(ContentUris.withAppendedId(Events.CONTENT_URI, id), null, null) == 0) return CalendarModule.error("NotFoundError")
        main.post { say("removed $id") }
        return JSONObject().put("id", id.toString())
    }

    // The editor (D15)

    private fun editor(args: JSONObject, reply: (JSONObject) -> Unit) {
        val event = args.optJSONObject("event") ?: return reply(CalendarModule.malformed)
        val (values, written) = values(event) ?: return reply(CalendarModule.malformed)
        if (editing) return reply(CalendarModule.error("InvalidStateError", "an editor is showing"))
        val android = args.optJSONObject("android")
        val intent = Intent(Intent.ACTION_INSERT, Events.CONTENT_URI).apply {
            putExtra(CalendarContract.EXTRA_EVENT_BEGIN_TIME, values.getAsLong(Events.DTSTART))
            putExtra(CalendarContract.EXTRA_EVENT_END_TIME, values.getAsLong(Events.DTEND))
            putExtra(CalendarContract.EXTRA_EVENT_ALL_DAY, written == null)
            putExtra(Events.TITLE, values.getAsString(Events.TITLE))
            values.getAsString(Events.DESCRIPTION)?.let { putExtra(Events.DESCRIPTION, it) }
            values.getAsString(Events.EVENT_LOCATION)?.let { putExtra(Events.EVENT_LOCATION, it) }
            putExtra(Events.EVENT_TIMEZONE, values.getAsString(Events.EVENT_TIMEZONE))
            putExtra(Events.AVAILABILITY, if (android?.has("availability") == true) android.getInt("availability") else values.getAsInteger(Events.AVAILABILITY))
            if (android?.has("accessLevel") == true) putExtra(Events.ACCESS_LEVEL, android.getInt("accessLevel"))
            android?.optJSONArray("emails")?.let { a -> putExtra(Intent.EXTRA_EMAIL, (0 until a.length()).joinToString(",") { a.getString(it) }) }
        }
        CalendarModule.insertIntentHatch?.invoke(intent)
        editing = true
        say("editor shown")
        context.startActivityForResult(intent) { code, _ ->
            editing = false
            val action = if (code == Activity.RESULT_OK) "saved" else "unknown"
            say("editor $action")
            reply(JSONObject().put("action", action))
        }
    }
}
