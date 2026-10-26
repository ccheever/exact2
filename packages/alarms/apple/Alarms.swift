// @exact/alarms on Apple (LLP 1133.030 D2, D4, D7, D8, D14): AlarmKit's
// `AlarmManager.shared` on iOS 26 and later, weak-linked; every other Apple
// target, and an older iOS, answers absence (`available` false,
// `NotSupportedError`). Under the agent the runner's substitute answers and
// no AlarmKit call is made (D15).
//
// - AlarmKit reads back an alarm's id, schedule, countdown and state, not
//   its title (D4), so a record per tag, in the app's defaults, keeps the
//   title, the snooze, the platform's id and the times this module set. An
//   alarm AlarmKit no longer has is gone, and its record goes at the read.
// - A countdown's end and a paused countdown's time left, after the person
//   snoozed or paused from the system's UI, are read from the alarm's Live
//   Activity (`AlarmPresentationState`); the record answers when there is
//   none (before first unlock).
// - The countdown's Live Activity is drawn by the host's extension
//   (host/apple/widget/Alarm.swift), which the build adds for a package that
//   declares `"apple": {"activity": "alarmkit"}` (D12 item 3). The metadata
//   type both sides name, `ExactAlarmMetadata`, is the host's
//   `AlarmAttributes.swift`, compiled into this module on iOS.
// - Tier 3 (D6): `AlarmsModule.alarmConfiguration(_:)` receives each
//   configuration before `schedule(id:configuration:)`.
import ExactPackage
import Foundation
#if os(iOS)
import ActivityKit
import AlarmKit
import SwiftUI
#endif

public final class AlarmsModule: ExactPackage {
    private let context: ExactPackageContext
    private var watching: [Task<Void, Never>] = []

    public init(context: ExactPackageContext) {
        self.context = context
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        #if os(iOS)
        if #available(iOS 26.0, *) {
            let origin = context.origin
            Task { @MainActor in reply(await self.answer(name, args, origin: origin)) }
            return
        }
        #endif
        reply((try? call(name + "Sync", args)) ?? ["error": "NotSupportedError"])
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        #if os(iOS)
        if #available(iOS 26.0, *) {
            switch name {
            case "alarmStatusSync": watch(); return status()
            case "alarmsSync": watch(); return ["exact:answer": alarms()]
            default: return ["error": "NotSupportedError"]
            }
        }
        #endif
        switch name {
        case "alarmStatusSync": return ["available": false, "state": "unknown", "countdown": false]
        case "alarmsSync": return ["exact:answer": [Any]()]
        default: return ["error": "NotSupportedError"]
        }
    }

    /// The person may change the authorization in Settings while the app is
    /// away, or stop an alarm on the Lock Screen: both read again on return.
    public func foreground(_ shown: Bool) {
        guard shown else { return }
        context.changed("alarms.alarmStatus")
        context.changed("alarms.alarms")
    }

    public func destroy(_ done: @escaping () -> Void) {
        watching.forEach { $0.cancel() }
        watching = []
        done()
    }

    #if os(iOS)
    /// The app's access hatch (D6), as `alarmConfiguration(_:)` sets it.
    private static var hatch: Any?

    /// Each configuration, after the module's words are set and before it is
    /// scheduled (D6): a named sound, a pre-alert countdown before an alarm at
    /// a time. The id and the record stay the module's.
    @available(iOS 26.0, *)
    public static func alarmConfiguration(_ hatch: @escaping (inout AlarmManager.AlarmConfiguration<ExactAlarmMetadata>) -> Void) {
        Self.hatch = hatch
    }

    @available(iOS 26.0, *)
    private func watch() {
        guard watching.isEmpty else { return }
        let changed = context.changed
        watching = [
            Task { @MainActor in for await _ in AlarmManager.shared.alarmUpdates { changed("alarms.alarms") } },
            Task { @MainActor in for await _ in AlarmManager.shared.authorizationUpdates { changed("alarms.alarmStatus") } },
        ]
    }

    @available(iOS 26.0, *)
    private func answer(_ name: String, _ args: [String: Any], origin: String) async -> [String: Any] {
        watch()
        switch name {
        case "alarmStatus": return status()
        case "alarms": return ["exact:answer": alarms()]
        case "requestAlarmPermission":
            _ = try? await AlarmManager.shared.requestAuthorization()
            return status()
        case "scheduleAlarm": return await schedule(args, origin: origin)
        case "cancelAlarm": return cancel(args["tag"] as? String ?? "")
        case "pauseAlarm": return pause(args["tag"] as? String ?? "", resume: false, origin: origin)
        case "resumeAlarm": return pause(args["tag"] as? String ?? "", resume: true, origin: origin)
        default: return ["error": "NotSupportedError"]
        }
    }

    // MARK: the status (D7)

    @available(iOS 26.0, *)
    private func status() -> [String: Any] {
        let auth = AlarmManager.shared.authorizationState
        let (state, word): (String, String) = switch auth {
        case .authorized: ("granted", "authorized")
        case .denied: ("denied", "denied")
        default: ("prompt", "notDetermined")
        }
        return ["available": true, "state": state, "countdown": true,
                "apple": ["authorizationState": word], "android": NSNull()]
    }

    // MARK: the records (D4)

    private static let recordsKey = "exact.alarms.records"
    private var records: [String: [String: Any]] {
        get { UserDefaults.standard.dictionary(forKey: Self.recordsKey) as? [String: [String: Any]] ?? [:] }
        set { UserDefaults.standard.set(newValue, forKey: Self.recordsKey) }
    }

    private func nowMs() -> Double { Date().timeIntervalSince1970 * 1000 }

    @available(iOS 26.0, *)
    private func alarms() -> [[String: Any]] {
        guard let listed = try? AlarmManager.shared.alarms else { return [] }
        let byId = Dictionary(listed.map { ($0.id.uuidString, $0) }, uniquingKeysWith: { a, _ in a })
        // What the Live Activities say of each alarm's time, after the person touched it.
        var shown: [String: AlarmPresentationState.Mode] = [:]
        for activity in Activity<AlarmAttributes<ExactAlarmMetadata>>.activities {
            shown[activity.content.state.alarmID.uuidString] = activity.content.state.mode
        }
        var kept = records, out: [[String: Any]] = []
        for (tag, record) in records.sorted(by: { $0.key < $1.key }) {
            guard let id = record["id"] as? String, let alarm = byId[id] else {
                kept[tag] = nil
                continue
            }
            let state: String = switch alarm.state {
            case .scheduled: "scheduled"
            case .countdown: "countdown"
            case .paused: "paused"
            case .alerting: "alerting"
            @unknown default: "scheduled"
            }
            // A countdown that follows a ring is the system's Repeat or Snooze, which the app did not start: it began
            // when the ring ended, which `alarmUpdates` reports at once, and runs `snooze` (D4).
            var current = record
            if state == "countdown", record["seen"] as? String == "alerting", let snooze = record["snooze"] as? Double {
                current["at"] = nowMs() + snooze
                current["remaining"] = nil
            }
            current["seen"] = state
            if !NSDictionary(dictionary: current).isEqual(to: record) { kept[tag] = current }
            var at: Any = current["at"] ?? NSNull(), remaining: Any = current["remaining"] ?? NSNull()
            // A countdown's end already past is a record the system's UI moved while the app did not run: unknown.
            if state == "countdown", let end = at as? Double, end <= nowMs() { at = NSNull() }
            // The Live Activity's time wins where its mode is the alarm's state; it lags a change the app made, and
            // then the record is the current one.
            switch shown[id] {
            case .countdown(let c)? where state == "countdown":
                at = c.fireDate.timeIntervalSince1970 * 1000
                remaining = NSNull()
            case .paused(let p)? where state == "paused":
                at = NSNull()
                remaining = (p.totalCountdownDuration - p.previouslyElapsedDuration) * 1000
            default:
                if case .fixed(let date)? = alarm.schedule, state == "scheduled" { at = date.timeIntervalSince1970 * 1000 }
            }
            out.append(["tag": tag, "title": record["title"] ?? "", "state": state, "at": at,
                        "duration": record["duration"] ?? NSNull(), "remaining": remaining,
                        "snooze": record["snooze"] ?? NSNull()])
        }
        if !NSDictionary(dictionary: kept).isEqual(to: records) { records = kept }
        return out
    }

    // MARK: the sends (D2, D7, D8)

    @available(iOS 26.0, *)
    private func schedule(_ args: [String: Any], origin: String) async -> [String: Any] {
        // Only the person sets an alarm (D7); the runner refuses first, this holds for data code too.
        guard origin == "app" else { return ["error": "NotAllowedError"] }
        guard let tag = args["tag"] as? String, let title = args["title"] as? String, !title.isEmpty else {
            return ["error": "TypeError"]
        }
        let at = args["at"] as? Double, duration = args["duration"] as? Double, snooze = args["snooze"] as? Double
        guard (at == nil) != (duration == nil), at.map({ $0 > nowMs() }) ?? true,
              duration.map({ $0 >= 1000 }) ?? true, snooze.map({ $0 >= 1000 }) ?? true else {
            return ["error": "TypeError"]
        }
        let manager = AlarmManager.shared
        if manager.authorizationState == .notDetermined {
            do { _ = try await manager.requestAuthorization() } catch { context.log("alarms: requestAuthorization failed: \(error)") }
        }
        guard manager.authorizationState == .authorized else {
            context.log("alarms: \(tag) not scheduled, authorization \(manager.authorizationState)")
            return ["error": "NotAllowedError"]
        }
        let apple = args["apple"] as? [String: Any] ?? [:]
        let tint = (apple["tintColor"] as? String).flatMap(Self.color) ?? Color.accentColor
        let text = { (s: String) in LocalizedStringResource(String.LocalizationValue(s)) }
        // The second button only with `snooze`: Snooze for an alarm at a time, Repeat for a countdown (D3).
        let second = snooze.map { _ in
            AlarmButton(text: text(duration == nil ? "Snooze" : "Repeat"), textColor: tint,
                        systemImageName: duration == nil ? "zzz" : "repeat")
        }
        let alert: AlarmPresentation.Alert
        if #available(iOS 26.1, *) {
            alert = .init(title: text(title), secondaryButton: second, secondaryButtonBehavior: second.map { _ in .countdown })
        } else {
            // Before 26.1 the alert takes a Stop button, which 26.1 makes the system's (D3).
            alert = .init(title: text(title), stopButton: AlarmButton(text: text("Stop"), textColor: .white, systemImageName: "stop.circle"),
                          secondaryButton: second, secondaryButtonBehavior: second.map { _ in .countdown })
        }
        let presentation = AlarmPresentation(
            alert: alert,
            countdown: .init(title: text(title)),
            paused: .init(title: text(title), resumeButton: AlarmButton(text: text("Resume"), textColor: tint, systemImageName: "play.fill")))
        let attributes = AlarmAttributes<ExactAlarmMetadata>(presentation: presentation, metadata: ExactAlarmMetadata(), tintColor: tint)
        var configuration: AlarmManager.AlarmConfiguration<ExactAlarmMetadata>
        if let duration {
            configuration = .init(countdownDuration: .init(preAlert: duration / 1000, postAlert: snooze.map { $0 / 1000 }), attributes: attributes)
        } else {
            configuration = .init(countdownDuration: snooze.map { .init(preAlert: nil, postAlert: $0 / 1000) },
                                  schedule: .fixed(Date(timeIntervalSince1970: (at ?? 0) / 1000)), attributes: attributes)
        }
        if let hatch = Self.hatch as? (inout AlarmManager.AlarmConfiguration<ExactAlarmMetadata>) -> Void {
            hatch(&configuration)
            context.log("alarms: \(tag)'s configuration changed by the app's hatch")
        }
        let id = UUID()
        do {
            _ = try await manager.schedule(id: id, configuration: configuration)
        } catch let error as AlarmManager.AlarmError where error == .maximumLimitReached {
            return ["error": "QuotaExceededError"]
        } catch {
            context.log("alarms: schedule failed: \(error)")
            return ["error": manager.authorizationState == .authorized ? "UnknownError" : "NotAllowedError"]
        }
        // A tag that named an alarm is replaced: the new one is set, then the old one ends (D2).
        if let old = records[tag]?["id"] as? String, let oldId = UUID(uuidString: old) { try? manager.cancel(id: oldId) }
        var record: [String: Any] = ["id": id.uuidString, "title": title]
        record["snooze"] = snooze
        record["duration"] = duration
        record["at"] = duration.map { nowMs() + $0 } ?? at
        records[tag] = record
        context.changed("alarms.alarms")
        return [:]
    }

    @available(iOS 26.0, *)
    private func alarm(_ tag: String) -> (UUID, Alarm?)? {
        guard let id = (records[tag]?["id"] as? String).flatMap(UUID.init(uuidString:)) else { return nil }
        return (id, (try? AlarmManager.shared.alarms)?.first { $0.id == id })
    }

    /// Ends the alarm whatever its state; cancelling nothing does nothing (D2).
    @available(iOS 26.0, *)
    private func cancel(_ tag: String) -> [String: Any] {
        guard let (id, alarm) = alarm(tag) else { return [:] }
        try? AlarmManager.shared.cancel(id: id)
        // Stage 1 measures whether `cancel` ends a ring; `stop` ends one that it leaves.
        if alarm?.state == .alerting, (try? AlarmManager.shared.alarms)?.contains(where: { $0.id == id }) == true {
            try? AlarmManager.shared.stop(id: id)
            context.log("alarms: cancel left \(tag) ringing; stop(id:) ended it")
        } else if alarm?.state == .alerting {
            context.log("alarms: cancel ended \(tag)'s ring")
        }
        records[tag] = nil
        context.changed("alarms.alarms")
        return [:]
    }

    @available(iOS 26.0, *)
    private func pause(_ tag: String, resume: Bool, origin: String) -> [String: Any] {
        // A resume commits the device again: the person's alone, as a schedule is (D7).
        if resume && origin != "app" { return ["error": "NotAllowedError"] }
        guard let (id, alarm) = alarm(tag), let alarm, alarm.state == (resume ? .paused : .countdown) else {
            return ["error": "InvalidStateError"]
        }
        var record = records[tag] ?? [:]
        do {
            if resume {
                try AlarmManager.shared.resume(id: id)
                let left = record["remaining"] as? Double ?? 0
                record["at"] = nowMs() + left
                record["remaining"] = nil
            } else {
                try AlarmManager.shared.pause(id: id)
                let at = record["at"] as? Double ?? nowMs()
                record["remaining"] = max(0, at - nowMs())
                record["at"] = nil
            }
        } catch {
            return ["error": "InvalidStateError"]
        }
        records[tag] = record
        context.changed("alarms.alarms")
        return [:]
    }

    /// `#rrggbb` or `#rrggbbaa`.
    private static func color(_ hex: String) -> Color? {
        let digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        guard digits.count == 6 || digits.count == 8, let n = UInt64(digits, radix: 16) else { return nil }
        let v = digits.count == 6 ? n << 8 | 0xff : n
        return Color(.sRGB, red: Double(v >> 24 & 0xff) / 255, green: Double(v >> 16 & 0xff) / 255,
                     blue: Double(v >> 8 & 0xff) / 255, opacity: Double(v & 0xff) / 255)
    }
    #else
    private func watch() {}
    #endif
}
