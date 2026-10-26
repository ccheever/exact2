// The agent's calendars on Android (LLP 1133.018 §10), as Apple's `AgentCalendar.swift` is: what @exact/calendar
// reads and writes under the agent, where it asks the Calendar Provider for nothing. It is the package's
// `fixture/calendar.json`, or another file there the agent's `store` names, which an APK carries as the app's assets
// under `calendar-fixture/` (the fixture app's `assets/`), changed for the session by the app's writes and the
// editor's saves. A recurring series is listed one occurrence at a time, each with its `recurrenceId`, so nothing
// here expands a rule.
//
// Time follows JSCalendar (RFC 8984): an event with a `timeZone` is fixed in it; one without, and an all-day one, is
// placed in the session's zone (`EXACT_AGENT_TIME_ZONE`, UTC unless the drive names one).
package exact.calendar

import android.content.res.AssetManager
import org.json.JSONArray
import org.json.JSONObject
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

internal class AgentCalendar(private val assets: AssetManager) {
    companion object {
        /** EventKit's predicate spans at most four years, which every target keeps (D2). */
        const val LONGEST = 4 * 365.25 * 86_400_000.0
        private val LOCAL = DateTimeFormatter.ofPattern("uuuu-MM-dd'T'HH:mm:ss")

        /** Field [key] as text, null where it is missing or null (`optString` answers "null" for a null). */
        fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key)

        /** `2026-10-23T09:00:00` (or a date, or to the minute) as a local date-time. */
        fun parts(local: String): LocalDateTime? {
            val f = local.split('-', 'T', ':').map { it.toIntOrNull() ?: return null }
            if (f.size !in 3..6) return null
            return try { LocalDateTime.of(f[0], f[1], f[2], f.getOrElse(3) { 0 }, f.getOrElse(4) { 0 }, f.getOrElse(5) { 0 }) } catch (e: java.time.DateTimeException) { null }
        }

        fun local(at: LocalDateTime): String = at.format(LOCAL)

        /** An ISO 8601 duration (`P1D`, `PT1H30M`, `P1W`) as nominal days and exact seconds. */
        fun duration(text: String): Pair<Int, Int>? {
            if (!text.startsWith("P")) return null
            var days = 0
            var seconds = 0
            var number = ""
            var inTime = false
            for (ch in text.drop(1)) {
                if (ch.isDigit()) { number += ch; continue }
                if (ch == 'T') { inTime = true; continue }
                val n = number.toIntOrNull() ?: return null
                number = ""
                when {
                    ch == 'W' && !inTime -> days += 7 * n
                    ch == 'D' && !inTime -> days += n
                    ch == 'H' && inTime -> seconds += 3600 * n
                    ch == 'M' && inTime -> seconds += 60 * n
                    ch == 'S' && inTime -> seconds += n
                    else -> return null
                }
            }
            return if (number.isEmpty()) days to seconds else null
        }

        /** Exact seconds as an ISO 8601 duration. */
        fun iso(seconds: Long): String {
            if (seconds <= 0) return "PT0S"
            val (h, m, s) = Triple(seconds / 3600, seconds % 3600 / 60, seconds % 60)
            return "PT" + (if (h > 0) "${h}H" else "") + (if (m > 0) "${m}M" else "") + (if (s > 0) "${s}S" else "")
        }

        /** The instants [event] spans in [zone] where it names none, ms since the epoch; null for one whose start or
         * duration is not well formed. */
        fun span(event: JSONObject, zone: ZoneId): Pair<Double, Double>? {
            val start = parts(event.optString("start")) ?: return null
            val (days, seconds) = duration(event.optString("duration").ifEmpty { "PT0S" }) ?: return null
            val allDay = event.optBoolean("showWithoutTime")
            val named = if (allDay) null else event.text("timeZone")?.let {
                try { ZoneId.of(it) } catch (e: java.time.DateTimeException) { return null }
            }
            val from = (if (allDay) start.toLocalDate().atStartOfDay() else start).atZone(named ?: zone)
            val to = from.plusDays(days.toLong()).plusSeconds(seconds.toLong())
            return from.toInstant().toEpochMilli().toDouble() to to.toInstant().toEpochMilli().toDouble()
        }

        fun cursor(after: String): Key? {
            val f = after.split('|')
            if (f.size != 4) return null
            return Key(f[0].toDoubleOrNull() ?: return null, f[1].toDoubleOrNull() ?: return null, f[2], f[3])
        }
    }

    /** An occurrence's place in the order of every target's pages (D3), which a cursor names. */
    data class Key(val start: Double, val end: Double, val id: String, val recurrence: String) : Comparable<Key> {
        override fun compareTo(other: Key) = compareValuesBy(this, other, Key::start, Key::end, Key::id, Key::recurrence)
        override fun toString() = "${start.toLong()}|${end.toLong()}|$id|$recurrence"
    }

    var calendars: List<JSONObject> = emptyList()
        private set
    val events = ArrayList<JSONObject>()
    private var made = 0
    var access = "none"
    val zone: ZoneId = try { ZoneId.of(System.getenv("EXACT_AGENT_TIME_ZONE") ?: "UTC") } catch (e: java.time.DateTimeException) { ZoneId.of("UTC") }
    /** Said once when the fixture is not in the APK. */
    var missing: String? = null
        private set

    init { if (!load("calendar.json")) missing = "no agent calendars: the APK carries no assets/calendar-fixture/calendar.json" }

    /** Reads fixture file [name]; false where there is none. */
    fun load(name: String): Boolean {
        if (name.contains('/')) return false
        val json = try {
            JSONObject(assets.open("app/assets/calendar-fixture/$name").bufferedReader().use { it.readText() })
        } catch (e: java.io.IOException) { return false } catch (e: org.json.JSONException) { return false }
        val list = json.optJSONArray("calendars") ?: JSONArray()
        calendars = (0 until list.length()).map(list::getJSONObject)
        val all = json.optJSONArray("events") ?: JSONArray()
        events.clear()
        events += (0 until all.length()).map(all::getJSONObject)
        return true
    }

    val defaultCalendar: String? get() = calendars.firstOrNull { it.optBoolean("isDefault") }?.optString("id")

    fun calendar(id: String): JSONObject? = calendars.firstOrNull { it.optString("id") == id }

    /** Whether [id] names a recurring series: its occurrences carry a `recurrenceId`. */
    fun isSeries(id: String) = events.any { it.optString("id") == id && it.text("recurrenceId") != null }

    /** The event as answered: every field, its instants beside it. */
    fun answered(event: JSONObject): JSONObject {
        val out = JSONObject(event.toString())
        for (field in listOf("id", "uid", "calendarId", "description", "timeZone", "location", "url", "recurrenceId", "apple")) {
            if (!out.has(field)) out.put(field, JSONObject.NULL)
        }
        span(event, zone)?.let { (s, t) -> out.put("utcStart", s).put("utcEnd", t) }
        return out
    }

    /** A new or changed event, normalised as the store keeps it. */
    fun stored(event: JSONObject, id: String?, calendar: String): JSONObject {
        val out = JSONObject(event.toString())
        out.put("id", id ?: "n${++made}")
        out.put("uid", event.text("uid") ?: "${out.getString("id")}@agent.exact")
        out.put("calendarId", calendar)
        out.put("showWithoutTime", event.optBoolean("showWithoutTime"))
        out.put("status", event.text("status") ?: "confirmed")
        out.put("freeBusyStatus", event.text("freeBusyStatus") ?: "busy")
        out.put("recurrenceId", JSONObject.NULL)
        out.remove("utcStart")
        out.remove("utcEnd")
        return out
    }

    fun put(event: JSONObject) {
        events.removeAll { it.optString("id") == event.optString("id") }
        events += event
    }

    fun remove(id: String): Boolean = events.removeAll { it.optString("id") == id }
}
