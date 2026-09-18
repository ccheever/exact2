// The GPU module's optional input, message and agent seams (LLP 1041.002 §3).
// The runner never learns what a surface's world is; the web host is the oracle.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
import AVFAudio
#endif

extension NodeView {
    var inputCanvas: NodeView? {
        #if os(macOS)
        var ancestor: NSView? = self
        #else
        var ancestor: UIView? = self
        #endif
        while let view = ancestor {
            if let node = view as? NodeView, node.canvasInput != nil { return node }
            ancestor = view.superview
        }
        return nil
    }
    func forwardsCanvasKey(_ code: String, command: Bool = false) -> Bool {
        guard !disabled, !inert, field == nil, textArea == nil, !command, code != "Tab" else { return false }
        return !(["Space", "Enter", "NumpadEnter"].contains(code)
            && (kind == "button" || ["button", "link"].contains(props["accessibilityRole"] ?? "")))
    }
}

// The file carrier has a product budget before allocation; engine limits remain
// the second, structural boundary. The same limit applies before base64 capture.
struct WorldCarrier {
    static let limit = 256 * 1024 * 1024
    static let refusal = "world carrier exceeds 256 MiB limit"
    static func check(_ count: Int) throws {
        if count > limit { throw NSError(domain: "ExactWorld", code: 1, userInfo: [NSLocalizedDescriptionKey: refusal]) }
    }
    static func read(_ path: String?) -> (bytes: Data?, error: String?) {
        guard let path else { return (nil, nil) }
        do {
            let file = try FileHandle(forReadingFrom: URL(fileURLWithPath: path))
            defer { try? file.close() }
            let length = try file.seekToEnd()
            guard length <= UInt64(limit) else { return (nil, refusal) }
            try file.seek(toOffset: 0)
            let bytes = try file.read(upToCount: limit + 1) ?? Data()
            try check(bytes.count)
            return (bytes, nil)
        } catch { return (nil, "world carrier: \(error.localizedDescription)") }
    }
}

extension Canvases {
    func recoveredDevice(_ ok: Bool, error: String?) {
        failed = error
        if let error { fputs("exact gpu: \(error)\n", stderr); restoreJournal.append(["lines": [error]]) }
        for entry in entries.values {
            entry.presentable = ok; entry.wants = ok
            entry.uploaded = false; entry.readAt = -1
            entry.view.needsCapture = true
            if ok { messages(entry) }
        }
        session?.frames.requestCanvas()
    }
    func rendered(_ entry: Entry, _ result: UInt32) {
        if result == 3 { module?.recoverDevice() }
        else { entry.rendered(result) }
    }

    func bindSurface(_ m: GpuModule, _ e: Entry) -> UInt32 {
        guard let data = try? JSONSerialization.data(withJSONObject: e.values) else { return 1 }
        let now = session?.now() ?? 0
        return data.withUnsafeBytes { bytes in
            if let bindAt = m.bindAt { return bindAt(e.id, bytes.bindMemory(to: UInt8.self).baseAddress, data.count, now) }
            return m.bind(e.id, bytes.bindMemory(to: UInt8.self).baseAddress, data.count)
        }
    }
    func live(_ id: UInt32) -> Entry? {
        guard let e = entries[id], e.id != 0, e.view.window != nil,
              session?.presenter.views[id] === e.view else { return nil }
        return e
    }

    func restoreWorld(_ m: GpuModule, _ e: Entry) {
        guard !e.restoreAttempted, let bytes = worldInput.bytes else { return }
        e.restoreAttempted = true
        guard m.restore != nil else { return }
        if m.carry?(e.id) == UInt32.max {
            let request = Array("{\"op\":\"state\"}".utf8)
            let length = request.withUnsafeBufferPointer { m.agent?(e.id, $0.baseAddress, $0.count) ?? UInt32.max }
            guard let data = m.output(length), let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any], reply["world"] != nil else { return }
        }
        let ok = bytes.withUnsafeBytes { m.restore?(e.id, $0.bindMemory(to: UInt8.self).baseAddress, bytes.count, 0) ?? false }
        e.restorePending = true
        finishRestore(m, e, refusal: ok ? nil : m.error())
    }

    func finishRestore(_ m: GpuModule, _ e: Entry, refusal: String? = nil) {
        guard e.restorePending else { return }
        if let refusal {
            e.restorePending = false
            e.restoreError = "surface \(e.name): restore refused: \(refusal)"
            restoreJournal.append(["canvas": e.view.id, "lines": [e.restoreError!]])
            return
        }
        let request = Array("{\"op\":\"state\"}".utf8)
        let length = request.withUnsafeBufferPointer { m.agent?(e.id, $0.baseAddress, $0.count) ?? UInt32.max }
        guard let data = m.output(length),
              let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let world = reply["world"] as? [String: Any], world["restored"] as? Bool == true else { return }
        e.restorePending = false
        worldInput.bytes = nil
    }

    func restoreReply(_ reply: [String: Any]) -> [String: Any] {
        guard !terminalRestoreReported else { return reply }
        let errors = entries.values.compactMap(\.restoreError)
        let terminal = worldInput.bytes != nil && !errors.isEmpty && entries.values.allSatisfy { $0.id != 0 && $0.restoreAttempted }
        guard worldInput.error != nil || terminal else { return reply }
        terminalRestoreReported = true
        var reply = reply
        reply["error"] = worldInput.error ?? errors.joined(separator: "; ")
        return reply
    }

    func save(_ e: Entry) -> [String: Any] {
        guard let m = module, let carry = m.carry else { return ["error": "world save unavailable on this host yet"] }
        let length = carry(e.id)
        guard length != UInt32.max else {
            let state = agent(e.view.id, ["op": "state"])?["world"] as? [String: Any] ?? [:]
            return ["error": "save refused: \(state["assets"] ?? [])", "assets": state["assets"] ?? []]
        }
        guard length <= WorldCarrier.limit else { return ["error": WorldCarrier.refusal] }
        guard let bytes = length == 0 ? Data() : m.output(length) else { return ["error": "surface returned no save bytes"] }
        let state = agent(e.view.id, ["op": "state"])?["world"] as? [String: Any] ?? [:]
        return ["data": bytes.base64EncodedString(), "bytes": bytes.count, "hash": state["hash"] ?? NSNull(), "tick": state["tick"] ?? NSNull()]
    }

    func wantsInput(_ id: UInt32) -> Bool { live(id)?.wantsInput == true }

    /// Creation waits for first pixel. An agent read must wait for that same work.
    func waitUntilReady() -> Bool {
        let deadline = Date(timeIntervalSinceNow: 20)
        while !entries.isEmpty && !loadRequested {
            loadIfNeeded()
            if loadRequested { break }
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        }
        return true
    }

    func claimPublisher(_ e: Entry) {
        if publishers[e.name] == nil { publishers[e.name] = e }
        else { fputs("exact gpu: surface \(e.name): duplicate live publisher ignored\n", stderr) }
    }

    func releasePublisher(_ e: Entry) {
        guard publishers[e.name] === e else { return }
        publishers.removeValue(forKey: e.name)
        surfaceRecord(e.name, nil)
    }

    func surfaceRecord(_ name: String, _ json: String?) {
        guard let s = session else { return }
        s.surfaceRecord(name, json)
    }

    func messages(_ e: Entry) {
        if live(e.view.id) === e, let m = module, let take = m.assets, let deliver = m.asset {
            var delivered = false
            for _ in 0..<16 {
                guard let data = m.output(take(e.id)), let names = try? JSONSerialization.jsonObject(with: data) as? [String], !names.isEmpty else { break }
                delivered = true
                for name in names {
                    let delivery = Result { try session?.app.resolver.delivery("assets/" + name) }
                    let chars = Array(name.utf8)
                    let ok = chars.withUnsafeBufferPointer { chars in
                        if case .failure(let error) = delivery {
                            let reason = Array(error.localizedDescription.utf8)
                            return reason.withUnsafeBufferPointer { m.assetFailed?(e.id, chars.baseAddress, chars.count, $0.baseAddress, $0.count) ?? false }
                        }
                        if case .success(let bytes?) = delivery {
                            return bytes.withUnsafeBytes { raw in
                                // Non-null with zero length distinguishes an empty file from missing.
                                var empty: UInt8 = 0
                                return withUnsafePointer(to: &empty) { deliver(e.id, chars.baseAddress, chars.count, raw.bindMemory(to: UInt8.self).baseAddress ?? $0, bytes.count) }
                            }
                        }
                        return deliver(e.id, chars.baseAddress, chars.count, nil, 0)
                    }
                    let error = ok ? nil : m.error()
                    if let error { fputs("exact gpu: \(error)\n", stderr) }
                    finishRestore(m, e, refusal: error)
                }
            }
            // Establish the ready world's epoch at delivery, even when occlusion
            // prevents its first presentation. Recovery never moves that clock.
            if delivered, let ask = m.agent,
               let data = try? JSONSerialization.data(withJSONObject: ["op": "clock", "now": session?.now() ?? 0]) {
                data.withUnsafeBytes { _ = ask(e.id, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
            }
        }
        if let m = module { finishRestore(m, e) }
        if live(e.view.id) === e, publishers[e.name] === e, let m = module, let take = m.published {
            let length = take(e.id)
            if length != UInt32.max, let data = length == 0 ? Data() : m.output(length) {
                surfaceRecord(e.name, String(decoding: data, as: UTF8.self))
            }
        }
        guard live(e.view.id) === e, let m = module, let take = m.messages else { return }
        let length = take(e.id)
        guard length != UInt32.max, let data = m.output(length) else { return }
        guard let texts = try? JSONSerialization.jsonObject(with: data) as? [String] else {
            fputs("exact gpu: view \(e.view.id): messages must be an array of strings\n", stderr)
            return
        }
        for text in texts {
            guard live(e.view.id) === e else { break }
            if text == "exact:audio" { lifecycle.requestAudio(userInitiated: false); continue }
            if e.view.handlers.contains("message") { session?.presenter.message(e.view.id, text) }
        }
    }

    /// Real events and recognized driver events enter here, in the same clock domain.
    @discardableResult
    func input(_ view: NodeView, _ event: [String: Any], timestamp: Double? = nil) -> Bool {
        guard let e = live(view.id), e.view === view, e.wantsInput,
              event["t"] as? String == "blur" || (!view.disabled && !view.inert),
              let m = module else { return false }
        return input(e, m, event, timestamp: timestamp)
    }

    /// Already-owned surface/device identity, including a held key's final release.
    @discardableResult
    func input(_ e: Entry, _ m: GpuModule, _ event: [String: Any], timestamp: Double? = nil) -> Bool {
        guard let s = session, let send = m.input else { return false }
        if (event["t"] as? String == "key" && event["down"] as? Bool == true)
            || (event["t"] as? String == "pointer" && event["phase"] as? String == "down") {
            lifecycle.gesture()
        }
        var value = event
        value["at"] = s.clock ?? timestamp.map { ($0 - ExactEnv.t0) * 1000 } ?? s.now()
        guard let data = try? JSONSerialization.data(withJSONObject: value) else { return false }
        let result = data.withUnsafeBytes { send(e.id, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
        if result != 0 { fputs("exact gpu: \(m.error())\n", stderr) }
        messages(e)
        s.frames.requestCanvas()
        return result == 0
    }

    func agent(_ view: UInt32, _ request: [String: Any]) -> [String: Any]? {
        guard let e = live(view), let s = session, let m = module, let ask = m.agent else { return nil }
        var request = request
        let size = s.agentInstance.box(e.view)
        request["width"] = max(1, size.width)
        request["height"] = max(1, size.height)
        request["scale"] = e.view.canvasScale
        if let now = s.clock { request["now"] = now }
        guard let data = try? JSONSerialization.data(withJSONObject: request) else { return nil }
        let length = data.withUnsafeBytes { ask(e.id, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
        let answer = m.output(length)
        messages(e)
        s.frames.requestCanvas()
        guard let answer else { return nil }
        guard var value = try? JSONSerialization.jsonObject(with: answer) as? [String: Any] else {
            fputs("exact gpu: view \(view): world reply must be an object\n", stderr)
            return nil
        }
        if request["op"] as? String == "state", let error = e.restoreError, var world = value["world"] as? [String: Any] {
            world["restoreError"] = error; value["world"] = world
        }
        return value
    }

    func worlds(_ request: [String: Any]) -> [[String: Any]] {
        entries.keys.sorted().compactMap { view in
            guard let answer = agent(view, request),
                  var world = answer["world"] as? [String: Any] ?? (request["op"] as? String == "clock" ? answer : nil) else { return nil }
            world["canvas"] = view
            return world
        }
    }

    func decorate(_ request: [String: Any], _ reply: [String: Any]) -> [String: Any] {
        guard reply["error"] == nil, !Agent.worldRequest(request) else { return reply }
        var reply = reply
        switch request["op"] as? String {
        case "tree":
            if var nodes = reply["nodes"] as? [[String: Any]] {
                for i in nodes.indices {
                    if let id = nodes[i]["id"] as? UInt32, let summary = agent(id, ["op": "tree", "summary": true]), let world = summary["world"] {
                        nodes[i]["world"] = world
                    }
                }
                reply["nodes"] = nodes
            }
        case "state":
            let world = worlds(["op": "state"])
            if !world.isEmpty { reply["world"] = world }
        case "logs":
            var world: [[String: Any]] = []
            for id in entries.keys.sorted() {
                guard let e = live(id), let journal = agent(id, ["op": "logs", "since": e.logCursor]), journal["error"] == nil,
                      let from = journal["from"] as? Int, let next = journal["next"] as? Int, let lines = journal["lines"] as? [Any] else { continue }
                world.append(["canvas": id, "from": from, "next": next, "lines": lines, "dropped": max(0, from - e.logCursor)])
                e.logCursor = next
            }
            world.append(contentsOf: restoreJournal); restoreJournal.removeAll()
            if !world.isEmpty { reply["world"] = world }
        default: break
        }
        return restoreReply(reply)
    }

    struct WorldClock {
        var pending = false
        var settleAt: Double?
        var reply: [String: Any] = [:]
    }
    func clock(settle: Bool) -> WorldClock {
        let world = worlds(["op": "clock", "settle": settle])
        let pending = world.filter { $0["quiescent"] as? Bool == false }
        return WorldClock(pending: settle && !pending.isEmpty,
                          settleAt: pending.compactMap { $0["settleAt"] as? Double }.filter(\.isFinite).max(),
                          reply: world.isEmpty ? [:] : ["world": world.map { $0.filter { ["canvas", "tick", "hash", "quiescent", "error", "assets", "changing"].contains($0.key) } }])
    }
}

extension Agent {
    static func worldRequest(_ request: [String: Any]) -> Bool { request["entity"] != nil || request["world"] as? Bool == true }

    func world(_ request: [String: Any]) -> [String: Any] {
        let id = request["id"] as? UInt32
        let missing: [String: Any] = ["error": "view \(request["id"] ?? "undefined") has no world"]
        guard let id, let e = session.canvases.live(id) else { return missing }
        let op = request["op"] as? String ?? ""
        if op == "screenshot", request["form"] as? String == "save" { return session.canvases.save(e) }
        if op == "focus" {
            guard e.wantsInput else { return ["error": "view \(id)'s surface does not take input"] }
            return ["ok": e.view.focusCanvas()]
        }
        guard ["layout", "state", "tree"].contains(op) else { return ["error": "world does not answer \(op)"] }
        var request = request
        let rect = box(e.view)
        // A `layout` with a point and no entity is the pick (LLP 1041.001 D2): a form, not a ninth name.
        if op == "layout", request["entity"] == nil {
            if let x = request["x"] as? Double { request["x"] = x - rect.minX }
            if let y = request["y"] as? Double { request["y"] = y - rect.minY }
        }
        var reply = session.canvases.agent(id, request) ?? missing
        for key in ["entity", "hit"] {
            if var entity = reply[key] as? [String: Any], var screen = entity["screen"] as? [String: Any] {
                if let x = screen["x"] as? Double { screen["x"] = x + rect.minX }
                if let y = screen["y"] as? Double { screen["y"] = y + rect.minY }
                entity["screen"] = screen
                reply[key] = entity
            }
        }
        return reply
    }

    /// A held key releases its original surface without resolving or focusing a view.
    func releaseCanvasKey(_ request: [String: Any]) -> [String: Any]? {
        guard request["phase"] as? String == "up", let token = request["releaseKey"] as? String else { return nil }
        // A failed down may never have installed its closure; release is still safe.
        return keyReleases.removeValue(forKey: token)?() ?? ["phase": "up", "delivery": "recognized"]
    }

    func canvasType(_ view: NodeView, _ request: [String: Any]) -> [String: Any] {
        guard let e = session.canvases.live(view.id), e.view === view else { return ["error": "view \(view.id) has no world"] }
        guard e.wantsInput else { return ["error": "view \(view.id)'s surface does not take input"] }
        guard let key = request["key"] as? String, let device = KeyCodes.device(key) else { return ["error": "canvas key needs a device code"] }
        let phase = request["phase"] as? String
        guard phase == nil || phase == "down" || phase == "up" else { return ["error": "key: not a phase: \(phase!)"] }
        guard view.focusCanvas() else { return ["error": "view \(view.id) could not take focus"] }
        if phase == "down", let token = request["releaseKey"] as? String, let module = session.canvases.module {
            let surface = e.id
            keyReleases[token] = { [weak self, weak e] in
                let reply: [String: Any] = ["typed": view.id, "key": key, "phase": "up", "delivery": "recognized"]
                // Destruction removes the held device state too. Never send to a replacement.
                guard let self, let e, self.session.canvases.entries[view.id] === e,
                      e.id == surface else { return reply }
                guard self.session.canvases.input(e, module, ["t": "key", "code": device.code, "key": device.key, "down": false, "repeat": false]) else {
                    return ["error": "view \(view.id)'s surface refused key release"]
                }
                return reply
            }
        }
        for step in phase.map({ [$0] }) ?? ["down", "up"] {
            guard session.canvases.input(view, ["t": "key", "code": device.code, "key": device.key, "down": step == "down", "repeat": false]) else {
                return ["error": "view \(view.id)'s surface refused input"]
            }
        }
        var reply: [String: Any] = ["typed": view.id, "key": key, "delivery": "recognized"]
        if let phase { reply["phase"] = phase }
        return reply
    }
}

extension Canvases.Entry {
    func rendered(_ result: UInt32) {
        if result == 3 { presentable = false }
        wants = presentable && result == 1
    }
    func needsFrame(dirty: Bool, editing: Bool = false) -> Bool {
        id != 0 && presentable && (wants || dirty || editing)
    }
}

// ExactKit owns session policy once per process, only after a live surface asks.
private enum CanvasAudio {
    nonisolated(unsafe) static var active = false
    nonisolated(unsafe) static var wanted = false
    nonisolated(unsafe) static var configured = false
    nonisolated(unsafe) static var interrupted = false
    nonisolated(unsafe) static var resumeBlocked = false
    @discardableResult static func activate() -> Bool {
        // NotificationCenter delivers on the posting thread, not necessarily main.
        if !Thread.isMainThread { return DispatchQueue.main.sync { activate() } }
        guard !ExactEnv.agentMode else { return false }
        wanted = true
        guard !active else { return true }
        #if os(iOS)
        do {
            let session = AVAudioSession.sharedInstance()
            if !configured { try session.setCategory(.ambient); configured = true }
            try session.setActive(true)
        } catch { fputs("exact audio session: \(error)\n", stderr); return false }
        #endif
        active = true
        return true
    }
}

/// Every canvas gets notifications even when its session uses the agent clock.
final class CanvasLifecycle: NSObject {
    nonisolated(unsafe) private static let live = NSHashTable<CanvasLifecycle>.weakObjects()
    weak var owner: Canvases?
    private(set) var hidden: Bool
    private var interrupted: Bool
    private var wantsAudio = false
    private var resumeAllowed: Bool
    private var retryFrames = 0
    private let activate: () -> Bool
    init(_ owner: Canvases, activate: @escaping () -> Bool = { CanvasAudio.activate() }) {
        precondition(Thread.isMainThread)
        self.owner = owner
        self.activate = activate
        hidden = !owner.visible
        interrupted = CanvasAudio.interrupted || CanvasAudio.resumeBlocked
        resumeAllowed = !interrupted
        super.init()
        Self.live.add(self)
        let center = NotificationCenter.default
        #if os(macOS)
        for name in [NSApplication.didResignActiveNotification, NSApplication.didHideNotification,
                     NSApplication.didBecomeActiveNotification, NSApplication.didUnhideNotification] {
            center.addObserver(self, selector: #selector(visibilityChanged), name: name, object: nil)
        }
        #else
        for name in [UIApplication.willResignActiveNotification, UIApplication.didEnterBackgroundNotification,
                     UIApplication.willEnterForegroundNotification, UIApplication.didBecomeActiveNotification] {
            center.addObserver(self, selector: #selector(visibilityChanged), name: name, object: nil)
        }
        center.addObserver(self, selector: #selector(interruption(_:)), name: AVAudioSession.interruptionNotification, object: nil)
        #endif
    }
    deinit { NotificationCenter.default.removeObserver(self) }
    private func onMain(_ work: @escaping () -> Void) {
        if Thread.isMainThread { work() } else { DispatchQueue.main.async(execute: work) }
    }
    func deliver(_ id: UInt32) {
        precondition(Thread.isMainThread)
        // Replay the same aggregate that notifications and rendering use.
        refresh(excluding: id)
        owner?.module?.lifecycle?(id, hidden ? 0 : 1)
        if interrupted { owner?.module?.lifecycle?(id, 2) }
    }
    private func send(_ code: UInt32, excluding id: UInt32? = nil) {
        precondition(Thread.isMainThread)
        guard let owner, let module = owner.module else { return }
        for entry in Array(owner.entries.values) where entry.id != 0 && entry.id != id { module.lifecycle?(entry.id, code) }
        if code == 1 || code == 3 { owner.session?.frames.requestCanvas() }
    }
    var needsRetry: Bool {
        wantsAudio && interrupted && !hidden && resumeAllowed
            && !CanvasAudio.interrupted && !CanvasAudio.resumeBlocked && !ExactEnv.agentMode
    }
    func frame() {
        precondition(Thread.isMainThread)
        guard needsRetry else { return }
        if retryFrames > 0 { retryFrames -= 1 }
        if retryFrames == 0 { retryActivation() }
    }
    private func retryActivation(excluding id: UInt32? = nil) {
        guard wantsAudio, !hidden, resumeAllowed,
              !CanvasAudio.interrupted, !CanvasAudio.resumeBlocked else { return }
        if activate() {
            retryFrames = 0
            if interrupted { interrupted = false; send(3, excluding: id) }
        } else {
            retryFrames = 300
            if !interrupted { interrupted = true; send(2, excluding: id) }
            owner?.session?.frames.requestCanvas()
        }
    }
    func gesture() {
        onMain { [weak self] in
            guard let self, !ExactEnv.agentMode else { return }
            if !CanvasAudio.interrupted { Self.allowRecovery(excluding: self) }
            if self.wantsAudio { self.retryActivation() }
        }
    }
    private static func allowRecovery(excluding trigger: CanvasLifecycle) {
        precondition(Thread.isMainThread)
        let blocked = CanvasAudio.resumeBlocked
        CanvasAudio.resumeBlocked = false
        for lifecycle in live.allObjects {
            lifecycle.resumeAllowed = true
            if blocked && lifecycle !== trigger { lifecycle.retryActivation() }
        }
    }
    func requestAudio(userInitiated: Bool = true) {
        onMain { [weak self] in
            guard let self, !ExactEnv.agentMode else { return }
            self.wantsAudio = true
            if userInitiated && !CanvasAudio.interrupted {
                Self.allowRecovery(excluding: self)
            }
            // Automatic surface requests neither bypass no-resume nor the cooldown.
            if userInitiated || self.retryFrames == 0 { self.retryActivation() }
        }
    }
    @objc private func visibilityChanged() {
        // UIKit's "will" notifications precede the applicationState update.
        DispatchQueue.main.async { [weak self] in self?.refresh() }
    }
    func refresh(excluding id: UInt32? = nil) {
        precondition(Thread.isMainThread)
        let next = !(owner?.visible ?? false)
        let becameVisible = hidden && !next
        if next { CanvasAudio.active = false }
        if next != hidden { hidden = next; send(hidden ? 0 : 1, excluding: id) }
        if becameVisible && !CanvasAudio.interrupted {
            Self.allowRecovery(excluding: self)
        }
        if becameVisible || retryFrames == 0 { retryActivation(excluding: id) }
    }
    // Keep the complete interruption transition on main, including session policy.
    func interruption(began: Bool, shouldResume: Bool) {
        onMain { [weak self] in
            guard let self else { return }
            CanvasAudio.interrupted = began
            CanvasAudio.resumeBlocked = !began && !shouldResume
            self.resumeAllowed = !began && shouldResume
            if began {
                CanvasAudio.active = false
                self.retryFrames = 0
                if !self.interrupted { self.interrupted = true; self.send(2) }
            } else if self.resumeAllowed {
                if self.wantsAudio { self.retryActivation() }
                else if self.interrupted { self.interrupted = false; self.send(3) }
            }
        }
    }
    #if os(iOS)
    @objc private func interruption(_ note: Notification) {
        // Decode immutable notification values on the poster; touch no UI/ABI here.
        guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        let options = AVAudioSession.InterruptionOptions(rawValue:
            note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0)
        interruption(began: type == .began, shouldResume: options.contains(.shouldResume))
    }
    #endif
}
