// @ref LLP 1126.001 WP9 — loaded only for a send or exactActivities demand.
import ActivityKit
import Foundation

public typealias ActivityReport = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

@MainActor private final class Activities {
    typealias Live = Activity<ExactActivityAttributes>
    static let shared = Activities()
    private let authorization = ActivityAuthorizationInfo()
    private var listeners: [UInt: (UnsafeMutableRawPointer?, ActivityReport)] = [:]
    private var next: UInt = 0
    private var live: [String: Live] = [:]
    private var tasks: [String: [Task<Void, Never>]] = [:]
    private var processTasks: [Task<Void, Never>] = []
    private var tokens: [String: String] = [:]

    func join(_ context: UnsafeMutableRawPointer?, _ report: @escaping ActivityReport) -> UInt {
        next += 1
        listeners[next] = (context, report)
        if processTasks.isEmpty {
            // Subscribe before enumerating, so an activity arriving during recovery is seen.
            processTasks = [Task { for await activity in Live.activityUpdates { observe(activity); publish() } },
                            Task { for await _ in authorization.frequentPushEnablementUpdates { publish() } }]
            for activity in Live.activities { observe(activity) }
        }
        publish()
        return next
    }
    func leave(_ id: UInt) { listeners.removeValue(forKey: id) }
    private func emit(_ value: [String: Any], to listener: (UnsafeMutableRawPointer?, ActivityReport)) {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else { return }
        data.withUnsafeBytes { raw in listener.1(listener.0, raw.bindMemory(to: UInt8.self).baseAddress, raw.count) }
    }
    private func observe(_ activity: Live) {
        guard live[activity.id] == nil else { return }
        live[activity.id] = activity
        if let token = activity.pushToken { tokens[activity.id] = hex(token) }
        tasks[activity.id] = [
            Task { for await token in activity.pushTokenUpdates { tokens[activity.id] = hex(token); publish() } },
            Task { for await _ in activity.activityStateUpdates { publish() } },
            Task { for await _ in activity.contentUpdates { publish() } },
        ]
    }
    private func hex(_ data: Data) -> String { data.map { String(format: "%02x", $0) }.joined() }
    private func state(_ activity: Live) -> String {
        if #available(iOS 26.0, *), activity.activityState == .pending { return "pending" }
        switch activity.activityState {
        case .active: return "active"
        case .stale: return "stale"
        case .ended: return "ended"
        case .dismissed: return "dismissed"
        default: return "unknown"
        }
    }
    private func publish() {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let entries: [[String: Any]] = live.values.sorted { $0.id < $1.id }.map { activity in
            let data = (try? encoder.encode(activity.content.state.state)) ?? Data("null".utf8)
            return ["id": activity.id, "kind": activity.attributes.kind, "state": String(decoding: data, as: UTF8.self),
                    "activityState": state(activity), "token": tokens[activity.id].map { $0 as Any } ?? NSNull()]
        }
        for listener in listeners.values { emit(["activities": entries, "frequentPushesEnabled": authorization.frequentPushesEnabled], to: listener) }
        // The final report includes the terminal state. ActivityKit no longer enumerates dismissed instances.
        let dismissed = live.values.filter { $0.activityState == .dismissed }
        for activity in dismissed {
            tasks.removeValue(forKey: activity.id)?.forEach { $0.cancel() }
            live.removeValue(forKey: activity.id)
            tokens.removeValue(forKey: activity.id)
        }
        if !dismissed.isEmpty { publish() }
    }
    private func failure(_ message: String) -> NSError { NSError(domain: "exact.activity", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    private func content(_ state: Any, attributes: ExactActivityAttributes, directory current: URL, stale: Any? = nil) throws -> ActivityContent<ExactActivityAttributes.ContentState> {
        let value = try JSONDecoder().decode(ActivityJSON.self, from: JSONSerialization.data(withJSONObject: state, options: [.fragmentsAllowed]))
        let content = ExactActivityAttributes.ContentState(shape: attributes.shape, state: value)
        try attributes.validate(content)
        // Decode against the state shape in the pinned generation, without running a render.
        let directory = current.deletingLastPathComponent().appendingPathComponent(attributes.generation)
        guard try Data(contentsOf: directory.appendingPathComponent("compatibility")) == Data(contentsOf: current.appendingPathComponent("compatibility")) else { throw failure("activity generation belongs to another compatibility cohort") }
        guard !attributes.kind.isEmpty, attributes.kind.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }) else { throw failure("invalid activity kind") }
        let schema = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("activities/\(attributes.kind).schema.json"))) as? [String: Any] ?? [:]
        guard matches(state, schema) else { throw failure("activity state does not match \(attributes.kind)'s declared shape") }
        let date = (stale as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue / 1000) }
        return ActivityContent(state: content, staleDate: date)
    }
    private func matches(_ value: Any, _ schema: [String: Any]) -> Bool {
        if let choices = schema["anyOf"] as? [[String: Any]] { return choices.contains { matches(value, $0) } }
        if let choices = schema["enum"] as? [String] { return (value as? String).map(choices.contains) ?? false }
        switch schema["type"] as? String {
        case "object":
            guard let record = value as? [String: Any], let fields = schema["properties"] as? [String: [String: Any]], Set(record.keys) == Set(fields.keys) else { return false }
            return fields.allSatisfy { key, field in record[key].map { matches($0, field) } ?? false }
        case "array": return (value as? [Any]).map { list in list.allSatisfy { matches($0, schema["items"] as? [String: Any] ?? [:]) } } ?? false
        case "string": return value is String
        case "boolean": return (value as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } ?? false
        case "number": return (value as? NSNumber).map { CFGetTypeID($0) != CFBooleanGetTypeID() && $0.doubleValue.isFinite } ?? false
        case "null": return value is NSNull
        default: return false
        }
    }
    func send(_ json: Data, ticket: UInt64, listener id: UInt) async {
        guard let listener = listeners[id] else { return }
        do {
            guard let request = try JSONSerialization.jsonObject(with: json) as? [String: Any], let name = request["command"] as? String,
                  let args = request["args"] as? [Any], args.count >= 2, let identity = args[0] as? String, let generationDirectory = request["generationDirectory"] as? String else { throw failure("invalid activity send") }
            let directory = URL(fileURLWithPath: generationDirectory, isDirectory: true)
            let activity: Live
            if name == "startActivity" {
                let registry = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("plans/activities.json"))) as? [[String: Any]] ?? []
                guard let entry = registry.first(where: { $0["kind"] as? String == identity }), let shape = entry["shape"] as? String else { throw failure("no activity entry \(identity)") }
                let attributes = ExactActivityAttributes(kind: identity, generation: directory.lastPathComponent, shape: shape)
                activity = try Live.request(attributes: attributes, content: content(args[1], attributes: attributes, directory: directory), pushType: entry["push"] as? Bool == true ? .token : nil)
                observe(activity)
            } else {
                guard let existing = Live.activities.first(where: { $0.id == identity }) else { throw failure("no alive activity \(identity)") }
                activity = existing
                observe(activity)
                if name == "updateActivity" {
                    let message = args.count > 2 ? args[2] as? String : nil
                    let alert = message.map { AlertConfiguration(title: LocalizedStringResource(stringLiteral: $0), body: "", sound: .default) }
                    await activity.update(try content(args[1], attributes: activity.attributes, directory: directory, stale: args.count > 3 ? args[3] : nil), alertConfiguration: alert)
                } else if name == "endActivity" {
                    guard args.count == 3 else { throw failure("endActivity needs a dismissal policy") }
                    let policy: ActivityUIDismissalPolicy
                    if args[2] as? String == "immediate" { policy = .immediate }
                    else if args[2] as? String == "default" { policy = .default }
                    else if let at = args[2] as? NSNumber { policy = .after(Date(timeIntervalSince1970: at.doubleValue / 1000)) }
                    else { throw failure("dismissal policy is immediate, default, or Unix milliseconds") }
                    await activity.end(try content(args[1], attributes: activity.attributes, directory: directory), dismissalPolicy: policy)
                } else { throw failure("unknown activity send \(name)") }
            }
            publish()
            emit(["ticket": ticket, "status": 200, "body": ["id": activity.id]], to: listener)
        } catch {
            emit(["ticket": ticket, "status": 400, "error": String(describing: error)], to: listener)
        }
    }
}

@_cdecl("exact_activity_open")
public func activityOpen(_ context: UnsafeMutableRawPointer?, _ report: ActivityReport?) -> UInt {
    guard let report else { return 0 }
    return MainActor.assumeIsolated { Activities.shared.join(context, report) }
}
@_cdecl("exact_activity_close")
public func activityClose(_ id: UInt) { MainActor.assumeIsolated { Activities.shared.leave(id) } }
@_cdecl("exact_activity_send")
public func activitySend(_ id: UInt, _ ticket: UInt64, _ bytes: UnsafePointer<UInt8>?, _ length: Int) {
    guard let bytes else { return }
    let data = Data(bytes: bytes, count: length)
    Task { @MainActor in await Activities.shared.send(data, ticket: ticket, listener: id) }
}
