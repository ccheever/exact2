// backgroundActivityReporter.ts (T3 Code 1e2ecbd975). AppKit supplies window
// facts; one reporter owns the debounce, cadence and scopes for all transports.
import AppKit

final class T3ActivityScopes {
    private var entries: [String: (environment: String, scope: [String: Any], count: Int)] = [:]
    static func recentlyInteracted(_ last: Double, at now: Double) -> Bool { last <= now && now - last <= 45_000 }
    func retain(environment: String, method: String, payload: [String: Any]) -> (() -> Void)? {
        let scope: [String: Any]
        if method == "subscribeResourceTelemetry" { scope = ["type": "diagnostics"] }
        else if method == "subscribeVcsStatus", let cwd = payload["cwd"] as? String { scope = ["type": "vcs-status", "cwd": cwd] }
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
    private let queue = DispatchQueue(label: "t3.activity")
    private let scopes = T3ActivityScopes()
    private var connections: [UUID: (environment: String, send: Sender)] = [:]
    private var releases: [UUID: () -> Void] = [:]
    private var debounce: DispatchWorkItem?
    private var timer: DispatchSourceTimer?
    private var reporting = false, reportAgain = false
    private var remainingReports = 0
    private var lastInteraction = Date().timeIntervalSince1970 * 1000
    private var visible = false, focused = false, alive = true
    // AppKit objects are used only on the main queue.
    private var observers: [NSObjectProtocol] = []
    private var eventMonitor: Any?
    private let mouseWindows = NSMapTable<NSWindow, NSNumber>.weakToStrongObjects()
    private var observing = false
    private var stoppedObserving = false
    private let observeWindows: Bool
    private let identityLock = NSLock()
    private var identity: String?
    private let persistent: Bool
    private let preferencesURL: URL?
    private static let identityKey = "backgroundActivityClientId"

    init(persistent: Bool, dataDirectory: URL?, observeWindows: Bool = true) {
        self.observeWindows = observeWindows
        self.persistent = persistent
        preferencesURL = dataDirectory?.appendingPathComponent("t3-code.json")
    }

    // Called under identityLock, shared with writes so saving drafts never loses the id.
    private func clientIDLocked() -> String {
        if let identity { return identity }
        guard persistent, let url = preferencesURL else {
            let value = UUID().uuidString.lowercased(); identity = value; return value
        }
        do {
            var preferences: [String: Any] = [:]
            if FileManager.default.fileExists(atPath: url.path) {
                let data = try Data(contentsOf: url)
                guard let decoded = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw CocoaError(.fileReadCorruptFile) }
                preferences = decoded
            }
            if let existing = preferences[Self.identityKey] as? String, !existing.isEmpty, existing.count <= 128 {
                identity = existing; return existing
            }
            let value = UUID().uuidString.lowercased(); preferences[Self.identityKey] = value
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try JSONSerialization.data(withJSONObject: preferences).write(to: url, options: .atomic)
            identity = value; return value
        } catch { identity = "ephemeral-client"; return "ephemeral-client" }
    }
    func writePreferences(_ text: String, to url: URL) throws {
        identityLock.lock(); defer { identityLock.unlock() }
        guard var preferences = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { throw CocoaError(.fileReadCorruptFile) }
        preferences[Self.identityKey] = clientIDLocked()
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONSerialization.data(withJSONObject: preferences).write(to: url, options: .atomic)
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
                DispatchQueue.main.async { [weak self] in self?.observe() }
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
        guard alive else { return }
        if reporting { reportAgain = true; return }
        guard !connections.isEmpty else { return }
        let now = Date(), millis = now.timeIntervalSince1970 * 1000
        identityLock.lock(); let clientID = clientIDLocked(); identityLock.unlock()
        var sent = Set<String>()
        let targets = connections.values.filter { sent.insert($0.environment).inserted }
        reporting = true
        remainingReports = targets.count
        for entry in targets {
            entry.send(["environmentId": entry.environment, "clientId": clientID, "clientKind": "desktop-renderer",
                        "visible": visible, "focused": focused, "recentlyInteracted": T3ActivityScopes.recentlyInteracted(lastInteraction, at: millis),
                        "appState": visible ? "active" : "background", "ttlMs": 45_000,
                        "observedAt": ISO8601DateFormatter().string(from: now),
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
        guard observeWindows, !observing, !stoppedObserving else { return }; observing = true
        let names: [Notification.Name] = [NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification]
        // Explicit AppKit names also cover hide, minimize, close and occlusion.
        let windowNames = [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification,
                           NSWindow.didChangeOcclusionStateNotification, NSWindow.didMiniaturizeNotification,
                           NSWindow.didDeminiaturizeNotification, NSWindow.willCloseNotification,
                           NSApplication.didHideNotification, NSApplication.didUnhideNotification]
        for name in names + windowNames {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.windowFacts() })
        }
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .keyDown, .scrollWheel]) { [weak self] event in
            self?.interaction(); return event
        }
        windowFacts()
    }
    private func windowFacts() {
        let windows = NSApp.windows.filter { $0.canBecomeMain && $0.isVisible }
        for window in windows where mouseWindows.object(forKey: window) == nil {
            mouseWindows.setObject(NSNumber(value: window.acceptsMouseMovedEvents), forKey: window)
            window.acceptsMouseMovedEvents = true
        }
        let visible = !NSApp.isHidden && windows.contains { !$0.isMiniaturized && $0.occlusionState.contains(.visible) }
        let focused = NSApp.isActive && windows.contains { $0.isKeyWindow }
        queue.async { [self] in self.visible = visible; self.focused = focused; requestReport() }
    }
    private func interaction() {
        let now = Date().timeIntervalSince1970 * 1000
        queue.async { [self] in
            let recent = T3ActivityScopes.recentlyInteracted(lastInteraction, at: now)
            lastInteraction = now
            if !recent { requestReport() }
        }
    }
    func destroy() {
        queue.async { [self] in
            alive = false; debounce?.cancel(); debounce = nil; timer?.cancel(); timer = nil
            connections.removeAll(); for release in releases.values { release() }; releases.removeAll()
        }
        DispatchQueue.main.async { [self] in
            stoppedObserving = true
            for window in mouseWindows.keyEnumerator().allObjects.compactMap({ $0 as? NSWindow }) {
                window.acceptsMouseMovedEvents = mouseWindows.object(forKey: window)?.boolValue ?? false
            }
            mouseWindows.removeAllObjects()
            for observer in observers { NotificationCenter.default.removeObserver(observer) }; observers.removeAll()
            if let eventMonitor { NSEvent.removeMonitor(eventMonitor) }; eventMonitor = nil
        }
    }
}
