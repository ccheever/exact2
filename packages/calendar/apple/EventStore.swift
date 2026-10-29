// @exact/calendar's production path on Apple (LLP 1133.018 stage 2): the
// person's calendars through EventKit, and the system's editor.
//
// - Access is asked when a call first needs it (D11): a read asks for full
//   access; a save asks for the narrowest the app declares, write-only where
//   its Info.plist carries `NSCalendarsWriteOnlyAccessUsageDescription`.
//   `requestFullCalendarAccess` raises write-only to full (TN3153).
// - A read runs off the main thread (D6) and answers a page in the order of
//   every target, `utcStart`, `utcEnd`, `id`, `recurrenceId`, with the same
//   cursor as the agent's path. EventKit's predicate spans at most four years.
// - Under write-only access the reads and `removeEvent` fail `NotAllowedError`,
//   `write-only`, and a save goes to the default calendar and answers no id.
// - The editor is `EKEventEditViewController` on iOS, which reports a save,
//   and an `.ics` file opened in Calendar on macOS, which reports nothing
//   (`unknown`, D15).
// - `EKEventStoreChanged` is a change of the store, which the module announces.
import EventKit
import Foundation
#if canImport(UIKit)
import EventKitUI
import UIKit
#else
import AppKit
#endif

final class EventStore: NSObject {
    private let store = EKEventStore()
    private let say: (String) -> Void
    private let changed: () -> Void
    private let reads = DispatchQueue(label: "exact.calendar.reads", qos: .userInitiated)
    private var observing = false
    #if canImport(UIKit)
    private var editing: ((String) -> Void)?
    #endif

    init(log: @escaping (String) -> Void, changed: @escaping () -> Void) {
        say = log
        self.changed = changed
        super.init()
        CalendarModule.eventStore?(store)
    }

    // MARK: access

    private static var authorization: EKAuthorizationStatus { EKEventStore.authorizationStatus(for: .event) }

    private static func declares(_ key: String) -> Bool { Bundle.main.object(forInfoDictionaryKey: key) != nil }

    func status() -> [String: Any] {
        // By raw value: `fullAccess` (3, before iOS 17 `authorized`) and `writeOnly` (4) are iOS 17's and macOS 14's names.
        let (state, access, word): (String, String, String) = switch Self.authorization.rawValue {
        case 0: ("prompt", "none", "notDetermined")
        case 1: ("denied", "none", "restricted")
        case 2: ("denied", "none", "denied")
        case 3: ("granted", "full", "fullAccess")
        case 4: ("granted", "write-only", "writeOnly")
        default: ("unknown", "unknown", "notDetermined")
        }
        return ["available": true, "state": state, "access": access, "editor": true,
                "apple": ["authorizationStatus": word], "android": NSNull()]
    }

    private var access: String { status()["access"] as? String ?? "none" }

    /// Asks for `level` (`full` or `write-only`) while undetermined, then `done` on the main thread.
    private func authorize(_ level: String, _ done: @escaping () -> Void) {
        guard Self.authorization == .notDetermined else { return done() }
        let key = level == "full" ? "NSCalendarsFullAccessUsageDescription" : "NSCalendarsWriteOnlyAccessUsageDescription"
        guard Self.declares(key) else { return done() }
        let answered: (Bool, Error?) -> Void = { [weak self] granted, _ in
            DispatchQueue.main.async {
                self?.say("asked \(level): \(granted ? "granted" : "not granted")")
                done()
                self?.changed()
            }
        }
        if #available(iOS 17, macOS 14, *) {
            if level == "full" { store.requestFullAccessToEvents(completion: answered) } else { store.requestWriteOnlyAccessToEvents(completion: answered) }
        } else {
            store.requestAccess(to: .event, completion: answered)
        }
    }

    /// Why a read or a change of an existing event is refused at this access, or nil.
    private func refusal() -> [String: Any]? {
        switch access {
        case "full": return nil
        case "write-only": return ["error": "NotAllowedError", "message": "write-only"]
        default: return ["error": "NotAllowedError"]
        }
    }

    private func observe() {
        guard !observing else { return }
        observing = true
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: .EKEventStoreChanged, object: store)
    }

    func stopObserving() {
        guard observing else { return }
        observing = false
        NotificationCenter.default.removeObserver(self, name: .EKEventStoreChanged, object: store)
    }

    @objc private func storeChanged() {
        DispatchQueue.main.async { [self] in
            store.reset()
            changed()
        }
    }

    /// After a raise or a return to the foreground: what EventKit answers may have changed.
    func refresh() { store.reset() }

    // MARK: names

    func later(_ name: String, _ args: [String: Any], origin: String, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "calendarStatus": reply(status())
        case "calendars": authorize("full") { [self] in observe(); reply(refusal() ?? ["calendars": calendarList()]) }
        case "calendarEvents": authorize("full") { [self] in observe(); page(args, reply) }
        case "saveEvent":
            let narrowest = Self.declares("NSCalendarsWriteOnlyAccessUsageDescription") ? "write-only" : "full"
            authorize(narrowest) { [self] in reply(save(args)) }
        case "removeEvent": reply(remove(args))
        case "requestFullCalendarAccess": raise(reply)
        case "showEventEditor": editor(args, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    private func calendarList() -> [[String: Any]] {
        let fallback = store.defaultCalendarForNewEvents?.calendarIdentifier
        return store.calendars(for: .event).map { c in
            ["id": c.calendarIdentifier, "title": c.title, "color": Self.hex(c.cgColor), "writable": c.allowsContentModifications,
             "isDefault": c.calendarIdentifier == fallback, "source": c.source?.title ?? ""]
        }
    }

    private static func hex(_ color: CGColor?) -> String {
        guard let color, let srgb = CGColorSpace(name: CGColorSpace.sRGB),
              let c = color.converted(to: srgb, intent: .defaultIntent, options: nil)?.components, c.count >= 3 else { return "#8e8e93" }
        let byte = { (v: CGFloat) in Int((max(0, min(1, v)) * 255).rounded()) }
        return String(format: "#%02x%02x%02x", byte(c[0]), byte(c[1]), byte(c[2]))
    }

    private typealias Key = (Double, Double, String, String)

    private func page(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        if let refused = refusal() { return reply(refused) }
        guard let start = (args["start"] as? NSNumber)?.doubleValue, let end = (args["end"] as? NSNumber)?.doubleValue, end >= start else {
            return reply(["error": "TypeError", "message": "start and end are instants, end not before start"])
        }
        guard end - start <= AgentCalendar.longest else { return reply(["error": "TypeError", "message": "calendar-range-too-long"]) }
        let wanted = (args["calendars"] as? [String]).map(Set.init)
        let calendars = wanted.map { ids in store.calendars(for: .event).filter { ids.contains($0.calendarIdentifier) } }
        let limit = max(1, min(2000, (args["limit"] as? NSNumber)?.intValue ?? 500))
        let after = args["after"] as? String
        reads.async { [self] in
            let began = Date()
            let predicate = store.predicateForEvents(withStart: Date(timeIntervalSince1970: start / 1000), end: Date(timeIntervalSince1970: end / 1000), calendars: calendars)
            var list: [(Key, EKEvent)] = store.events(matching: predicate).map { e in
                let (s, t) = Self.instants(e)
                return ((s, t, e.eventIdentifier ?? "", Self.recurrence(e) ?? ""), e)
            }
            list.sort { $0.0 < $1.0 }
            if let after {
                let f = after.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
                guard f.count == 4, let s = Double(f[0]), let t = Double(f[1]) else {
                    return DispatchQueue.main.async { reply(["error": "TypeError", "message": "after is not a cursor"]) }
                }
                let cursor: Key = (s, t, f[2], f[3])
                list = list.filter { $0.0 > cursor }
            }
            let shown = Array(list.prefix(limit))
            let events = shown.map { Self.event($0.1) }
            let next: Any = list.count > limit ? shown.last.map { "\(Int64($0.0.0))|\(Int64($0.0.1))|\($0.0.2)|\($0.0.3)" } ?? NSNull() : NSNull()
            let ms = Int(Date().timeIntervalSince(began) * 1000)
            DispatchQueue.main.async { [self] in
                say("page calendarEvents \(shown.count) of \(list.count) after \(after ?? "start") in \(ms) ms")
                reply(["events": events, "next": next])
            }
        }
    }

    // MARK: an event, both ways (D2)

    /// The instants an occurrence spans, ms: an all-day one from its first day's start to its last day's end.
    private static func instants(_ e: EKEvent) -> (Double, Double) {
        let from: Date = e.startDate ?? Date(timeIntervalSince1970: 0), to: Date = e.endDate ?? from
        guard e.isAllDay else { return (from.timeIntervalSince1970 * 1000, to.timeIntervalSince1970 * 1000) }
        let days = Calendar.current
        let end = days.date(byAdding: .day, value: 1, to: days.startOfDay(for: to)) ?? to
        return (days.startOfDay(for: from).timeIntervalSince1970 * 1000, end.timeIntervalSince1970 * 1000)
    }

    /// `date` as a local date-time in `zone`, JSCalendar's LocalDateTime.
    private static func local(_ date: Date, _ zone: TimeZone) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone
        let c = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        return String(format: "%04d-%02d-%02dT%02d:%02d:%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0, c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
    }

    /// A recurring occurrence's id within its series: its original start, local.
    private static func recurrence(_ e: EKEvent) -> String? {
        guard e.hasRecurrenceRules || e.isDetached, let date = e.occurrenceDate else { return nil }
        return local(date, e.timeZone ?? .current)
    }

    private static func iso(seconds: Int) -> String {
        guard seconds > 0 else { return "PT0S" }
        let h = seconds / 3600, m = seconds % 3600 / 60, s = seconds % 60
        return "PT" + (h > 0 ? "\(h)H" : "") + (m > 0 ? "\(m)M" : "") + (s > 0 ? "\(s)S" : "")
    }

    static func event(_ e: EKEvent) -> [String: Any] {
        let (s, t) = instants(e)
        let zone = e.isAllDay ? TimeZone.current : (e.timeZone ?? .current)
        let duration = e.isAllDay ? "P\(max(1, Int(((t - s) / 86_400_000).rounded())))D" : iso(seconds: Int(((t - s) / 1000).rounded()))
        let status = switch e.status { case .tentative: "tentative"; case .canceled: "cancelled"; default: "confirmed" }
        let availability = switch e.availability {
        case .busy: "busy"; case .free: "free"; case .tentative: "tentative"; case .unavailable: "unavailable"; default: "notSupported"
        }
        return [
            "id": e.eventIdentifier as Any? ?? NSNull(), "uid": e.calendarItemExternalIdentifier as Any? ?? NSNull(),
            "calendarId": e.calendar?.calendarIdentifier as Any? ?? NSNull(), "title": e.title ?? "",
            "description": e.notes as Any? ?? NSNull(), "start": local(Date(timeIntervalSince1970: s / 1000), zone),
            "timeZone": e.isAllDay ? NSNull() : e.timeZone?.identifier as Any? ?? NSNull(), "duration": duration,
            "showWithoutTime": e.isAllDay, "location": e.location as Any? ?? NSNull(), "url": e.url?.absoluteString as Any? ?? NSNull(),
            "status": status, "freeBusyStatus": e.availability == .free ? "free" : "busy",
            "recurrenceId": recurrence(e) as Any? ?? NSNull(), "utcStart": s, "utcEnd": t,
            "apple": ["availability": availability, "isDetached": e.isDetached,
                      "occurrenceDate": e.occurrenceDate.map { $0.timeIntervalSince1970 * 1000 } as Any? ?? NSNull()],
        ]
    }

    /// `event`'s fields written onto `e`; an error's answer where they are not well formed.
    private func fill(_ e: EKEvent, from event: [String: Any]) -> [String: Any]? {
        guard let title = event["title"] as? String, let start = event["start"] as? String, var parts = AgentCalendar.parts(start),
              let d = AgentCalendar.duration(event["duration"] as? String ?? "PT0S") else {
            return ["error": "TypeError", "message": "the event needs a title, a start and a duration"]
        }
        let allDay = event["showWithoutTime"] as? Bool == true
        let zone = allDay ? nil : (event["timeZone"] as? String).flatMap(TimeZone.init(identifier:))
        if !allDay, let name = event["timeZone"] as? String, zone == nil { return ["error": "TypeError", "message": "no time zone \(name)"] }
        if allDay { parts.hour = 0; parts.minute = 0; parts.second = 0 }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone ?? .current
        guard let from = calendar.date(from: parts), let days = calendar.date(byAdding: .day, value: d.days, to: from) else {
            return ["error": "TypeError", "message": "the start is not a date"]
        }
        e.title = title
        e.isAllDay = allDay
        // A floating event has no zone (RFC 8984 §4.7.1), as EventKit's `timeZone` nil.
        e.timeZone = zone
        e.startDate = from
        let to = days.addingTimeInterval(TimeInterval(d.seconds))
        e.endDate = allDay ? max(from, to.addingTimeInterval(-1)) : to
        e.notes = event["description"] as? String
        e.location = event["location"] as? String
        e.url = (event["url"] as? String).flatMap(URL.init(string:))
        e.availability = event["freeBusyStatus"] as? String == "free" ? .free : .busy
        return nil
    }

    // MARK: writes

    private func save(_ args: [String: Any]) -> [String: Any] {
        guard let event = args["event"] as? [String: Any] else { return ["error": "TypeError"] }
        if event["recurrenceId"] is String { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
        let asked = args["calendar"] as? String, level = access
        guard level == "full" || level == "write-only" else { return ["error": "NotAllowedError"] }
        if level == "write-only", asked != nil || event["id"] is String { return ["error": "NotAllowedError", "message": "write-only"] }
        let e: EKEvent
        if let id = event["id"] as? String {
            guard let found = store.event(withIdentifier: id) else { return ["error": "NotFoundError"] }
            if found.hasRecurrenceRules { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
            e = found
        } else {
            e = EKEvent(eventStore: store)
        }
        if let refused = fill(e, from: event) { return refused }
        if let asked {
            guard let c = store.calendar(withIdentifier: asked) else { return ["error": "NotFoundError"] }
            guard c.allowsContentModifications else { return ["error": "NotAllowedError", "message": "calendar-read-only"] }
            e.calendar = c
        } else if e.calendar == nil {
            guard let c = store.defaultCalendarForNewEvents else { return ["error": "NotFoundError"] }
            e.calendar = c
        }
        do {
            try store.save(e, span: .thisEvent, commit: true)
        } catch {
            say("save failed: \(error)")
            return ["error": "UnknownError", "message": error.localizedDescription]
        }
        say("saved \(e.eventIdentifier ?? "an event")\(level == "write-only" ? " (write-only)" : "")")
        var out = Self.event(e)
        if level == "write-only" { out["id"] = NSNull(); out["calendarId"] = NSNull(); out["uid"] = NSNull() }
        return out
    }

    private func remove(_ args: [String: Any]) -> [String: Any] {
        if let refused = refusal() { return refused }
        guard let id = args["id"] as? String else { return ["error": "TypeError"] }
        guard let e = store.event(withIdentifier: id) else { return ["error": "NotFoundError"] }
        if e.hasRecurrenceRules { return ["error": "NotSupportedError", "message": "an occurrence of a series"] }
        do {
            try store.remove(e, span: .thisEvent, commit: true)
        } catch {
            return ["error": "UnknownError", "message": error.localizedDescription]
        }
        say("removed \(id)")
        return ["id": id]
    }

    /// Write-only raised to full: iOS 17 and macOS 14 ask the person again.
    private func raise(_ reply: @escaping ([String: Any]) -> Void) {
        guard #available(iOS 17, macOS 14, *), Self.authorization.rawValue == 4, Self.declares("NSCalendarsFullAccessUsageDescription") else { return reply(status()) }
        store.requestFullAccessToEvents { [weak self] granted, _ in
            DispatchQueue.main.async {
                guard let self else { return }
                self.say("raised to full: \(granted ? "granted" : "not granted")")
                self.store.reset()
                reply(self.status())
                if granted { self.changed() }
            }
        }
    }

    // MARK: the editor

    private func editor(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let event = args["event"] as? [String: Any] else { return reply(["error": "TypeError"]) }
        let e = EKEvent(eventStore: store)
        if let refused = fill(e, from: event) { return reply(refused) }
        #if canImport(UIKit)
        guard editing == nil else { return reply(["error": "InvalidStateError", "message": "an editor is showing"]) }
        guard let top = Self.topController() else { return reply(["error": "InvalidStateError", "message": "no window to show the editor in"]) }
        let shown = EKEventEditViewController()
        shown.eventStore = store
        shown.event = e
        shown.editViewDelegate = self
        CalendarModule.eventEditor?(shown)
        if shown.editViewDelegate !== self {
            say("the eventEditor hatch changed the delegate, which the module owns (LLP 1133.018 D10)")
            shown.editViewDelegate = self
        }
        editing = { [weak self] action in
            self?.say("editor \(action)")
            reply(["action": action])
        }
        top.present(shown, animated: true)
        #else
        // The Mac has no editor to embed: Calendar opens the file and asks the person where to add it.
        guard let file = Self.ics(event, e) else { return reply(["error": "TypeError"]) }
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-calendar", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let name = (e.title ?? "event").components(separatedBy: CharacterSet(charactersIn: "/\\:*?\"<>|")).joined(separator: " ")
        let url = dir.appendingPathComponent("\(name.isEmpty ? "event" : name).ics")
        do { try file.write(to: url, atomically: true, encoding: .utf8) } catch { return reply(["error": "UnknownError", "message": error.localizedDescription]) }
        let calendarApp = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.iCal")
        let config = NSWorkspace.OpenConfiguration()
        let opened: (Any?, Error?) -> Void = { [weak self] _, error in
            DispatchQueue.main.async {
                self?.say("editor: \(url.lastPathComponent) opened\(error.map { " (failed: \($0.localizedDescription))" } ?? "")")
                reply(error == nil ? ["action": "unknown"] : ["error": "UnknownError", "message": error!.localizedDescription])
            }
        }
        if let calendarApp { NSWorkspace.shared.open([url], withApplicationAt: calendarApp, configuration: config, completionHandler: opened) } else { NSWorkspace.shared.open(url); opened(nil, nil) }
        #endif
    }

    #if os(macOS)
    /// The event as an iCalendar object (RFC 5545), as the web's editor writes it: a zoned start in UTC, a floating
    /// one as local time, an all-day one as a DATE.
    static func ics(_ event: [String: Any], _ e: EKEvent) -> String? {
        let text = { (s: String) in s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: ";", with: "\\;")
            .replacingOccurrences(of: ",", with: "\\,").replacingOccurrences(of: "\r\n", with: "\\n").replacingOccurrences(of: "\n", with: "\\n") }
        let utc = DateFormatter()
        utc.locale = Locale(identifier: "en_US_POSIX"); utc.timeZone = TimeZone(identifier: "UTC"); utc.dateFormat = "yyyyMMdd'T'HHmmss'Z'"
        let wall = DateFormatter()
        wall.locale = Locale(identifier: "en_US_POSIX"); wall.timeZone = .current
        let start: String
        if e.isAllDay { wall.dateFormat = "yyyyMMdd"; start = "DTSTART;VALUE=DATE:\(wall.string(from: e.startDate))" }
        else if e.timeZone != nil { start = "DTSTART:\(utc.string(from: e.startDate))" }
        else { wall.dateFormat = "yyyyMMdd'T'HHmmss"; start = "DTSTART:\(wall.string(from: e.startDate))" }
        var lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Exact//@exact/calendar//EN", "BEGIN:VEVENT",
                     "UID:\(event["uid"] as? String ?? "\(UUID().uuidString)@exact")", "DTSTAMP:\(utc.string(from: Date()))", start,
                     "DURATION:\(event["duration"] as? String ?? "PT1H")", "SUMMARY:\(text(e.title ?? ""))"]
        if let notes = e.notes { lines.append("DESCRIPTION:\(text(notes))") }
        if let location = e.location { lines.append("LOCATION:\(text(location))") }
        if let url = e.url { lines.append("URL:\(url.absoluteString)") }
        lines.append("TRANSP:\(e.availability == .free ? "TRANSPARENT" : "OPAQUE")")
        lines += ["END:VEVENT", "END:VCALENDAR"]
        return lines.map(fold).joined(separator: "\r\n") + "\r\n"
    }

    /// A content line folded at 75 octets (RFC 5545 §3.1), never inside a UTF-8 sequence.
    private static func fold(_ line: String) -> String {
        var out: [String] = [], current = "", width = 75
        for ch in line {
            if current.utf8.count + String(ch).utf8.count > width { out.append(current); current = ""; width = 74 }
            current.append(ch)
        }
        out.append(current)
        return out.joined(separator: "\r\n ")
    }
    #endif

    #if canImport(UIKit)
    static func topController() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        var top = scenes.flatMap(\.windows).first { $0.isKeyWindow }?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }
    #endif
}

#if canImport(UIKit)
extension EventStore: EKEventEditViewDelegate {
    func eventEditViewController(_ controller: EKEventEditViewController, didCompleteWith action: EKEventEditViewAction) {
        controller.dismiss(animated: true)
        let word = switch action { case .saved: "saved"; case .deleted: "deleted"; default: "canceled" }
        let done = editing
        editing = nil
        done?(word)
        if action == .saved { changed() }
    }
}
#endif
