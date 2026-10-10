#if os(iOS)
// Source: T3ActivityReporter.swift at 887b2491b182f851b11253655f6aa84fe2a26708.
// Mobile policy: upstream 365aa87982 apps/mobile/src/connection/background-activity.ts.
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

final class T3ActivityScopes {
    private var entries: [String: (environment: String, scope: [String: Any], count: Int)] = [:]
    func retain(environment: String, method: String, payload: [String: Any]) -> (() -> Void)? {
        let scope: [String: Any]
        if method == "subscribeResourceTelemetry" { scope = ["type": "diagnostics"] }
        else if method == "subscribeVcsStatus", payload["includeRemote"] as? Bool != false, let cwd = payload["cwd"] as? String { scope = ["type": "vcs-status", "cwd": cwd] }
        else { return nil }
        let parts: [Any] = [environment, scope["type"]!, scope["cwd"] ?? NSNull()]
        let key = String(data: try! JSONSerialization.data(withJSONObject: parts), encoding: .utf8)!
        var entry = entries[key] ?? (environment, scope, 0)
        entry.count += 1; entries[key] = entry
        var released = false
        return { [weak self] in
            guard !released, let self, var entry = self.entries[key] else { return }
            released = true; entry.count -= 1
            self.entries[key] = entry.count == 0 ? nil : entry
        }
    }
    func values(_ environment: String) -> [[String: Any]] {
        entries.keys.sorted().compactMap { key in entries[key].flatMap { $0.environment == environment ? $0.scope : nil } }
    }
}

final class T3ActivityReporter: @unchecked Sendable {
    typealias Sender = ([String: Any], @escaping () -> Void) -> Void
    private let queue = DispatchQueue(label: "t3.mobile.activity")
    private let scopes = T3ActivityScopes()
    private var connections: [UUID: (environment: String, send: Sender)] = [:]
    private var releases: [UUID: () -> Void] = [:]
    private var debounce: DispatchWorkItem?
    private var timer: DispatchSourceTimer?
    private var reporting = false, reportAgain = false, alive = true
    private var remainingReports = 0
    private var appState = "unknown"
    private var clientID: String?
    private let persistent: Bool
    // UIKit observers are installed and removed on main; reporting data belongs to queue.
    private var observers: [NSObjectProtocol] = []
    private var stoppedObserving = false

    init(persistent: Bool) {
        self.persistent = persistent
        DispatchQueue.main.async { [weak self] in self?.observe() }
    }

    func writePreferences(_ text: String, to url: URL) throws {
        // Identity is in Keychain, so ordinary preference saves cannot replace or leak it.
        guard (try JSONSerialization.jsonObject(with: Data(text.utf8))) is [String: Any] else { throw CocoaError(.fileReadCorruptFile) }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: url, options: .atomic)
    }
    func connect(_ id: UUID, environment: String, send: @escaping Sender) {
        queue.async { [self] in
            guard alive else { return }
            connections[id] = (environment, send)
            if timer == nil {
                let timer = DispatchSource.makeTimerSource(queue: queue)
                timer.schedule(deadline: .now() + 25, repeating: 25)
                timer.setEventHandler { [weak self] in self?.requestReport() }
                self.timer = timer; timer.resume()
            }
            requestReport()
        }
    }
    func disconnect(_ id: UUID) { queue.async { [self] in connections[id] = nil; requestReport() } }
    func changed() { queue.async { [self] in requestReport() } }
    func retain(_ id: UUID, environment: String, method: String, payload: [String: Any]) {
        queue.async { [self] in
            guard alive else { return }
            releases[id]?()
            releases[id] = scopes.retain(environment: environment, method: method, payload: payload)
            if releases[id] != nil { requestReport() }
        }
    }
    func release(_ id: UUID) {
        queue.async { [self] in if let release = releases.removeValue(forKey: id) { release(); requestReport() } }
    }
    private func requestReport() {
        guard alive else { return }
        debounce?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.report() }
        debounce = work; queue.asyncAfter(deadline: .now() + 0.25, execute: work)
    }
    private func report() {
        guard alive, !connections.isEmpty else { return }
        if reporting { reportAgain = true; return }
        if clientID == nil {
            clientID = (try? T3MobileIdentity.deviceID(persistent: persistent)).map { "mobile-\($0)" } ?? "ephemeral-mobile-client"
        }
        let active = appState == "active", now = Date()
        var sent = Set<String>()
        let targets = connections.values.filter { sent.insert($0.environment).inserted }
        reporting = true; remainingReports = targets.count
        for entry in targets {
            entry.send(["environmentId": entry.environment, "clientId": clientID!, "clientKind": "mobile",
                        "visible": active, "focused": active, "recentlyInteracted": active,
                        "appState": appState, "ttlMs": 45_000, "observedAt": ISO8601DateFormatter().string(from: now),
                        "scopes": [["type": "provider-status"]] + scopes.values(entry.environment)]) { [weak self] in
                guard let self else { return }
                self.queue.async { [self] in
                    self.remainingReports -= 1
                    if self.remainingReports == 0 {
                        self.reporting = false
                        if self.reportAgain { self.reportAgain = false; self.requestReport() }
                    }
                }
            }
        }
    }
    private func observe() {
        guard !stoppedObserving else { return }
        let states: [(Notification.Name, String)] = [
            (UIApplication.didBecomeActiveNotification, "active"),
            (UIApplication.willResignActiveNotification, "inactive"),
            (UIApplication.didEnterBackgroundNotification, "background"),
        ]
        for (name, value) in states {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                self?.stateChanged(value)
            })
        }
        switch UIApplication.shared.applicationState {
        case .active: stateChanged("active")
        case .inactive: stateChanged("inactive")
        case .background: stateChanged("background")
        @unknown default: stateChanged("unknown")
        }
    }
    private func stateChanged(_ value: String) {
        queue.async { [self] in guard alive else { return }; appState = value; requestReport() }
    }
    func destroy() {
        queue.async { [self] in
            alive = false; debounce?.cancel(); debounce = nil; timer?.cancel(); timer = nil
            connections.removeAll(); for release in releases.values { release() }; releases.removeAll()
        }
        DispatchQueue.main.async { [self] in
            stoppedObserving = true
            for observer in observers { NotificationCenter.default.removeObserver(observer) }; observers.removeAll()
        }
    }
}

#endif
