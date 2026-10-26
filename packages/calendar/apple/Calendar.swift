// @exact/calendar on Apple (LLP 1133.018): the person's calendars, read and
// written under access, and an event added through the system's editor.
// Under the agent it asks EventKit for nothing and answers from the agent's
// calendars (AgentCalendar.swift, §10); the runner holds the permission,
// editor and raise tickets and answers the status. Otherwise EventKit
// answers (EventStore.swift).
//
// - `calendarEvents` answers the occurrences overlapping `[start, end)`, at
//   most four years, in one order on every target: `utcStart`, `utcEnd`,
//   `id`, `recurrenceId` (D3). Its cursor is the last occurrence's place in
//   that order, so a page asked again after a change starts after the same
//   occurrence (D6).
// - Under write-only access every read and `removeEvent` fails
//   `NotAllowedError`, `write-only`, and `saveEvent` adds to the default
//   calendar only and answers no `id` (D5, D11).
// - The editor's `save` adds the event to the store. It answers `saved` on
//   iOS, whose editor reports it, and `unknown` on macOS, whose editor is an
//   `.ics` file (D15).
// - A change is announced, coalesced (D7): while the re-asks one
//   announcement caused are in flight, further changes set a mark, and one
//   more announcement follows them landing.
// - Tier 3 (D10): the hatches below, called on the main thread, never under
//   the agent.
import EventKit
#if canImport(UIKit)
import EventKitUI
#endif
import ExactPackage
import Foundation

public final class CalendarModule: ExactPackage {
    /// The module's one `EKEventStore`, when made and before its first access
    /// request: the app may read reminders under its own usage key, and
    /// recurrence and alarms. Its change observer and `reset()` are the module's.
    public static var eventStore: ((EKEventStore) -> Void)?
    #if canImport(UIKit)
    /// After the module fills the editor's event, before it is presented: the
    /// app may set the event's alarms, recurrence and structured location.
    /// The delegate is the module's.
    public static var eventEditor: ((EKEventEditViewController) -> Void)?
    #endif
    let context: ExactPackageContext
    private let book = AgentCalendar()
    /// EventKit, outside the agent.
    private lazy var real = EventStore(log: { [weak self] in self?.say($0) }, changed: { [weak self] in self?.changedStore() })
    /// The live names read and not released, which observing follows.
    private var readers: Set<String> = []
    /// Coalescing (D7): an announcement's re-asks in flight; a change since.
    private var awaiting = false, dirty = false, rounds = 0
    /// The agent's words last seen, so each new value acts once.
    private var store = "calendar.json", notify: Int?

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("calendar: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        guard context.agent else {
            if name == "calendars" || name == "calendarEvents" { readers.insert(name) }
            return real.later(name, args, origin: context.origin) { [weak self] answer in
                reply(answer)
                if name == "calendars" || name == "calendarEvents" { self?.asked() }
            }
        }
        switch name {
        case "calendarStatus": reply(status())
        case "calendars":
            readers.insert(name)
            reply(refusal() ?? ["calendars": book.calendars])
            asked()
        case "calendarEvents":
            readers.insert(name)
            reply(page(args))
            asked()
        case "saveEvent": reply(save(args))
        case "removeEvent": reply(remove(args))
        case "showEventEditor": reply(editor(args, (preferred as? [String: Any])?["tap"] as? String))
        case "requestFullCalendarAccess": reply(status())
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard name == "calendarStatus" else { return ["error": "NotSupportedError"] }
        return context.agent ? status() : real.status()
    }

    public func release(_ name: String) {
        guard readers.remove(name) != nil, readers.isEmpty else { return }
        say("no reader: observing stops")
        if !context.agent { real.stopObserving() }
    }

    /// Under the agent: the access the runner's status gives, the store the
    /// agent names, and a burst of `notify` changes.
    public func preferred(_ values: [String: Any]) {
        let access = (values["calendarStatus"] as? [String: Any])?["access"] as? String ?? book.access
        let file = values["store"] as? String ?? "calendar.json"
        let changed = access != book.access
        book.access = access
        if file != store {
            store = file
            say(book.load(file) ? "store \(file)" : "store \(file): no such fixture")
            changedStore()
        } else if changed {
            // A raise to full is a change of what reads answer (D7: reset, then announce).
            say("access \(access)")
            changedStore()
        }
        let burst = (values["notify"] as? NSNumber)?.intValue
        if let n = burst, burst != notify {
            say("notify \(n)")
            for _ in 0..<n { changedStore() }
        }
        notify = burst
    }

    public func permissions(_ states: [String: String]) {
        if states["calendar"] == "denied" { preferred(["calendarStatus": ["access": "none"], "store": store, "notify": notify as Any]) }
    }

    public func foreground(_ shown: Bool) {
        guard shown else { return }
        // Access may have changed in Settings meanwhile (D5).
        if !context.agent { real.refresh() }
        changedStore()
    }

    // MARK: announcing (D7)

    private func changedStore() {
        guard !readers.isEmpty else { context.changed("calendar.calendarStatus"); return }
        if awaiting { dirty = true; return }
        announce()
    }

    private func announce() {
        awaiting = true
        rounds += 1
        say("announced (round \(rounds))")
        for name in ["calendarEvents", "calendars", "calendarStatus"] { context.changed("calendar.\(name)") }
        let round = rounds
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            if self?.rounds == round { self?.landed() }
        }
    }

    /// A read answered: once the round's re-asks have, a marked change is announced.
    private func asked() {
        if awaiting { DispatchQueue.main.async { [weak self] in self?.landed() } }
    }

    private func landed() {
        guard awaiting else { return }
        awaiting = false
        if dirty {
            dirty = false
            announce()
        }
    }

    // MARK: the agent's answers (§10)

    private func status() -> [String: Any] {
        ["available": true, "state": "prompt", "access": book.access, "editor": true, "apple": NSNull(), "android": NSNull()]
    }

    /// Why a read or a change of an existing event is refused at this access, or nil.
    private func refusal() -> [String: Any]? {
        switch book.access {
        case "full": return nil
        case "write-only": return ["error": "NotAllowedError", "message": "write-only"]
        default: return ["error": "NotAllowedError"]
        }
    }

    private func page(_ args: [String: Any]) -> [String: Any] {
        if let refused = refusal() { return refused }
        guard let start = (args["start"] as? NSNumber)?.doubleValue, let end = (args["end"] as? NSNumber)?.doubleValue, end >= start else {
            return ["error": "TypeError", "message": "start and end are instants, end not before start"]
        }
        guard end - start <= AgentCalendar.longest else { return ["error": "TypeError", "message": "calendar-range-too-long"] }
        let only = (args["calendars"] as? [String]).map(Set.init)
        typealias Key = (Double, Double, String, String)
        var list: [(Key, [String: Any])] = book.events.compactMap { event in
            let e = book.answered(event)
            guard let s = e["utcStart"] as? Double, let t = e["utcEnd"] as? Double,
                  only?.contains(e["calendarId"] as? String ?? "") ?? true,
                  s < end && (t > start || (s == t && s >= start)) else { return nil }
            return ((s, t, e["id"] as? String ?? "", e["recurrenceId"] as? String ?? ""), e)
        }
        list.sort { $0.0 < $1.0 }
        if let after = args["after"] as? String {
            let f = after.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
            guard f.count == 4, let s = Double(f[0]), let t = Double(f[1]) else { return ["error": "TypeError", "message": "after is not a cursor"] }
            let cursor: Key = (s, t, f[2], f[3])
            list = list.filter { $0.0 > cursor }
        }
        let limit = max(1, min(2000, (args["limit"] as? NSNumber)?.intValue ?? 500))
        let shown = Array(list.prefix(limit))
        let next: Any = list.count > limit ? shown.last.map { "\(Int64($0.0.0))|\(Int64($0.0.1))|\($0.0.2)|\($0.0.3)" } ?? NSNull() : NSNull()
        say("page calendarEvents \(shown.count) of \(list.count) after \(args["after"] as? String ?? "start")")
        return ["events": shown.map(\.1), "next": next]
    }

    private func save(_ args: [String: Any]) -> [String: Any] {
        guard let event = args["event"] as? [String: Any], event["title"] is String, book.span(event) != nil else {
            return ["error": "TypeError", "message": "the event needs a title, a start and a duration"]
        }
        if event["recurrenceId"] is String { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
        let asked = args["calendar"] as? String
        switch book.access {
        case "write-only":
            // Saved to the calendar the person chose, and never readable again (D11).
            if asked != nil { return ["error": "NotAllowedError", "message": "write-only"] }
            guard let calendar = book.defaultCalendar else { return ["error": "NotFoundError"] }
            let saved = book.stored(event, id: nil, calendar: calendar)
            book.put(saved)
            say("saved \(saved["id"]!) to \(calendar) (write-only)")
            changedStore()
            var out = book.answered(saved)
            out["id"] = NSNull(); out["calendarId"] = NSNull()
            return out
        case "full":
            let id = event["id"] as? String
            if let id {
                guard book.events.contains(where: { $0["id"] as? String == id }) else { return ["error": "NotFoundError"] }
                if book.isSeries(id) { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
            }
            let existing = id.flatMap { id in book.events.first { $0["id"] as? String == id }?["calendarId"] as? String }
            guard let calendar = asked ?? existing ?? book.defaultCalendar, let info = book.calendar(calendar) else { return ["error": "NotFoundError"] }
            guard info["writable"] as? Bool == true else { return ["error": "NotAllowedError", "message": "calendar-read-only"] }
            let saved = book.stored(event, id: id, calendar: calendar)
            book.put(saved)
            say("saved \(saved["id"]!) to \(calendar)")
            changedStore()
            return book.answered(saved)
        default: return ["error": "NotAllowedError"]
        }
    }

    private func remove(_ args: [String: Any]) -> [String: Any] {
        if let refused = refusal() { return refused }
        guard let id = args["id"] as? String else { return ["error": "TypeError"] }
        if book.isSeries(id) { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
        guard book.remove(id) else { return ["error": "NotFoundError"] }
        say("removed \(id)")
        changedStore()
        return ["id": id]
    }

    /// The editor's answer: the person's save adds the event whatever the app's access.
    private func editor(_ args: [String: Any], _ tap: String?) -> [String: Any] {
        guard let event = args["event"] as? [String: Any], event["title"] is String, book.span(event) != nil else {
            return ["error": "TypeError", "message": "the event needs a title, a start and a duration"]
        }
        guard tap == "save" else { return ["action": "canceled"] }
        guard let calendar = book.defaultCalendar else { return ["error": "NotFoundError"] }
        var fresh = event
        fresh["id"] = nil
        let saved = book.stored(fresh, id: nil, calendar: calendar)
        book.put(saved)
        say("editor saved \(saved["id"]!) to \(calendar)")
        changedStore()
        #if os(macOS)
        return ["action": "unknown"]
        #else
        return ["action": "saved"]
        #endif
    }
}
