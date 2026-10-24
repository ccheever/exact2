// @exact/calendar on Android (LLP 1133.018): the person's calendars, read and written through the Calendar Provider,
// and an event added through the calendar app's insert screen. Under the agent it asks the provider for nothing and
// answers from the agent's calendars (AgentCalendar.kt, §10); the runner holds the permission and editor tickets and
// answers the status. Otherwise the provider answers (EventStore.kt).
//
// - `calendarEvents` answers the occurrences overlapping `[start, end)`, at most four years, in one order on every
//   target: `utcStart`, `utcEnd`, `id`, `recurrenceId` (D3). Its cursor is the last occurrence's place in that order.
// - Android has no write-only access (D11): the write-only line derives both permissions, and `access` is `full`
//   once they are granted, so `requestFullCalendarAccess` answers the status as it is.
// - The editor's `save` adds the event to the store; Android's calendar apps report nothing, so it answers
//   `unknown`, as production does unless the calendar app returns `RESULT_OK` (D15).
// - A change is announced, coalesced (D7): while the re-asks one announcement caused are in flight, further changes
//   set a mark, and one more announcement follows them landing.
// - Tier 3 (D10): `insertIntentHatch` sees the editor's `Intent` after the module's extras, before it starts, on the
//   main thread, never under the agent.
package exact.calendar

import android.content.Intent
import android.os.Handler
import android.os.Looper
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import exact.calendar.AgentCalendar.Companion.text
import org.json.JSONArray
import org.json.JSONObject

class CalendarModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's editor hatch (D10): the insert `Intent`, which it may add extras to (`Events.RRULE`). */
        @JvmStatic var insertIntentHatch: ((Intent) -> Unit)? = null

        internal fun error(code: String, message: String? = null) = JSONObject().put("error", code).apply { if (message != null) put("message", message) }

        internal val malformed get() = error("TypeError", "the event needs a title, a start and a duration")
    }

    private val main = Handler(Looper.getMainLooper())
    private val book by lazy { AgentCalendar(context.application.assets) }
    /** The Calendar Provider, outside the agent. */
    private val real by lazy { EventStore(context, ::say, ::changedStore) }
    /** The live names read and not released, which observing follows. */
    private val readers = HashSet<String>()
    /** Coalescing (D7): an announcement's re-asks in flight; a change since. */
    private var awaiting = false
    private var dirty = false
    private var rounds = 0
    /** The agent's words last seen, so each new value acts once. */
    private var store = "calendar.json"
    private var notify: Int? = null

    init { if (context.agent) book.missing?.let { say(it) } }

    private fun say(line: String) = context.log("calendar: $line")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        val read = name == "calendars" || name == "calendarEvents"
        if (!context.agent) {
            if (read) readers += name
            return real.later(name, args) { answer ->
                reply(answer)
                if (read) asked()
            }
        }
        when (name) {
            "calendarStatus", "requestFullCalendarAccess" -> reply(status())
            "calendars" -> {
                readers += name
                reply(refusal() ?: JSONObject().put("calendars", JSONArray(book.calendars)))
                asked()
            }
            "calendarEvents" -> {
                readers += name
                reply(page(args))
                asked()
            }
            "saveEvent" -> reply(save(args))
            "removeEvent" -> reply(remove(args))
            "showEventEditor" -> reply(editor(args, (preferred as? JSONObject)?.text("tap")))
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun release(name: String) {
        if (!readers.remove(name) || readers.isNotEmpty()) return
        say("no reader: observing stops")
        if (!context.agent) real.stopObserving()
    }

    /** Under the agent: the access the runner's status gives, the store the agent names, and a burst of `notify`
     * changes. */
    override fun preferred(values: JSONObject) {
        if (!context.agent) return
        val access = values.optJSONObject("calendarStatus")?.text("access") ?: book.access
        val file = values.text("store") ?: "calendar.json"
        val changed = access != book.access
        book.access = access
        if (file != store) {
            store = file
            say(if (book.load(file)) "store $file" else "store $file: no such fixture")
            changedStore()
        } else if (changed) {
            say("access $access")
            changedStore()
        }
        val burst = if (values.has("notify")) values.optInt("notify") else null
        if (burst != null && burst != notify) {
            say("notify $burst")
            repeat(burst) { changedStore() }
        }
        notify = burst
    }

    override fun permissions(states: JSONObject) {
        if (context.agent && states.optString("calendar") == "denied") {
            preferred(JSONObject().put("calendarStatus", JSONObject().put("access", "none")).put("store", store).apply { notify?.let { put("notify", it) } })
        }
    }

    override fun foreground(shown: Boolean) {
        if (!shown) return
        // Access may have changed in Settings meanwhile (D5).
        if (!context.agent) real.refresh()
        changedStore()
    }

    // Announcing (D7)

    private fun changedStore() {
        if (readers.isEmpty()) return
        if (awaiting) { dirty = true; return }
        announce()
    }

    private fun announce() {
        awaiting = true
        rounds += 1
        say("announced (round $rounds)")
        for (name in listOf("calendarEvents", "calendars", "calendarStatus")) context.changed("calendar.$name")
        val round = rounds
        main.postDelayed({ if (rounds == round) landed() }, 2000)
    }

    /** A read answered: once the round's re-asks have, a marked change is announced. */
    private fun asked() { if (awaiting) main.post { landed() } }

    private fun landed() {
        if (!awaiting) return
        awaiting = false
        if (dirty) {
            dirty = false
            announce()
        }
    }

    // The agent's answers (§10)

    private fun status() = JSONObject().put("available", true).put("state", "prompt").put("access", book.access).put("editor", true)
        .put("apple", JSONObject.NULL).put("android", JSONObject.NULL)

    /** Why a read or a change of an existing event is refused at this access, or null. */
    private fun refusal(): JSONObject? = when (book.access) {
        "full" -> null
        "write-only" -> error("NotAllowedError", "write-only")
        else -> error("NotAllowedError")
    }

    private fun page(args: JSONObject): JSONObject {
        refusal()?.let { return it }
        if (!args.has("start") || !args.has("end")) return error("TypeError", "start and end are instants, end not before start")
        val (start, end) = args.getDouble("start") to args.getDouble("end")
        if (end < start) return error("TypeError", "start and end are instants, end not before start")
        if (end - start > AgentCalendar.LONGEST) return error("TypeError", "calendar-range-too-long")
        val only = args.optJSONArray("calendars")?.let { a -> (0 until a.length()).map(a::getString).toSet() }
        var list = book.events.mapNotNull { event ->
            val e = book.answered(event)
            if (!e.has("utcStart")) return@mapNotNull null
            val (s, t) = e.getDouble("utcStart") to e.getDouble("utcEnd")
            if (only != null && e.text("calendarId") !in only) return@mapNotNull null
            if (!(s < end && (t > start || (s == t && s >= start)))) return@mapNotNull null
            AgentCalendar.Key(s, t, e.text("id").orEmpty(), e.text("recurrenceId").orEmpty()) to e
        }.sortedBy { it.first }
        args.text("after")?.let { after ->
            val cursor = AgentCalendar.cursor(after) ?: return error("TypeError", "after is not a cursor")
            list = list.filter { it.first > cursor }
        }
        val limit = args.optInt("limit", 500).coerceIn(1, 2000)
        val shown = list.take(limit)
        val next: Any = if (list.size > limit) shown.last().first.toString() else JSONObject.NULL
        say("page calendarEvents ${shown.size} of ${list.size} after ${args.text("after") ?: "start"}")
        return JSONObject().put("events", JSONArray(shown.map { it.second })).put("next", next)
    }

    private fun save(args: JSONObject): JSONObject {
        val event = args.optJSONObject("event") ?: return malformed
        if (event.text("title") == null || AgentCalendar.span(event, book.zone) == null) return malformed
        if (event.text("recurrenceId") != null) return error("NotSupportedError", "an occurrence of a series")
        val asked = args.text("calendar")
        return when (book.access) {
            "write-only" -> {
                // Saved to the calendar the person chose, and never readable again (D11).
                if (asked != null) return error("NotAllowedError", "write-only")
                val calendar = book.defaultCalendar ?: return error("NotFoundError")
                val saved = book.stored(event, null, calendar)
                book.put(saved)
                say("saved ${saved.getString("id")} to $calendar (write-only)")
                changedStore()
                book.answered(saved).put("id", JSONObject.NULL).put("calendarId", JSONObject.NULL)
            }
            "full" -> {
                val id = event.text("id")
                if (id != null) {
                    if (book.events.none { it.optString("id") == id }) return error("NotFoundError")
                    if (book.isSeries(id)) return error("NotSupportedError", "an occurrence of a series")
                }
                val existing = id?.let { i -> book.events.firstOrNull { it.optString("id") == i }?.text("calendarId") }
                val calendar = asked ?: existing ?: book.defaultCalendar ?: return error("NotFoundError")
                val info = book.calendar(calendar) ?: return error("NotFoundError")
                if (!info.optBoolean("writable")) return error("NotAllowedError", "calendar-read-only")
                val saved = book.stored(event, id, calendar)
                book.put(saved)
                say("saved ${saved.getString("id")} to $calendar")
                changedStore()
                book.answered(saved)
            }
            else -> error("NotAllowedError")
        }
    }

    private fun remove(args: JSONObject): JSONObject {
        refusal()?.let { return it }
        val id = args.text("id") ?: return error("TypeError")
        if (book.isSeries(id)) return error("NotSupportedError", "an occurrence of a series")
        if (!book.remove(id)) return error("NotFoundError")
        say("removed $id")
        changedStore()
        return JSONObject().put("id", id)
    }

    /** The editor's answer: the person's save adds the event whatever the app's access. */
    private fun editor(args: JSONObject, tap: String?): JSONObject {
        val event = args.optJSONObject("event") ?: return malformed
        if (event.text("title") == null || AgentCalendar.span(event, book.zone) == null) return malformed
        if (tap != "save") return JSONObject().put("action", "canceled")
        val calendar = book.defaultCalendar ?: return error("NotFoundError")
        val fresh = JSONObject(event.toString()).apply { remove("id") }
        val saved = book.stored(fresh, null, calendar)
        book.put(saved)
        say("editor saved ${saved.getString("id")} to $calendar")
        changedStore()
        return JSONObject().put("action", "unknown")
    }
}
