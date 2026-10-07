// App-local inclusion of native terminal ownership at887b249; type names and platform guard adapted.
// @ref llp/1106.007-mobile-terminal.decision.md#stream-ownership
#if os(iOS)
import Foundation

/// The drawer's terminal sessions (task terminal-drawer), on the main thread. T3 Code 1e2ecbd975 (MIT
/// reference, see LICENSE-T3) keeps one attach subscription per mounted terminal viewport and folds its
/// events with `applyTerminalAttachStreamEvent` (packages/client-runtime/src/state/terminalSession.ts,
/// terminal.ts `attach`); the viewport writes what is new since its cursor (ThreadTerminalDrawer.tsx
/// TerminalViewport) and sends input and grid sizes with `terminal.write` / `terminal.resize`.
/// Changes from the reference, for exact2:
/// - The stream and its buffer are native: chunks go from the socket (T3Transport+Terminal.swift) to the
///   session's `T3MobileTerminalOutput` to the bound `t3-terminal` views, never through the TypeScript inbox.
///   A chunk is acknowledged once the buffer holds it.
/// - A session lives while a view shows it or while the drawer keeps its thread mounted (`retain`, the
///   reference's MAX_HIDDEN_MOUNTED_TERMINAL_THREADS hidden threads plus the active one), so a hidden
///   thread keeps its stream and buffer without a web view (the S2 budget); its view replays the buffer
///   when it shows again. An unretained session without views detaches after a short grace.
/// - A lost socket ends every attach stream; the sessions attach again on the next socket (the
///   reference's durable subscription waits for the next session the same way). A stream that fails
///   (an authorization error, a bad cwd) is not retried until the drawer shows the terminal again.
/// - Writes are one RPC per input, as in the reference: a paste over 65,536 characters is refused by
///   the server and shown as `[terminal] <message>` (decision U19: match the reference).
/// - Resizes are latest-wins per session (the reference's `concurrency: latest` scheduler).
final class T3MobileTerminalSession {
    let key: String, threadId: String, terminalId: String
    var cwd = "", worktreePath = "", providerInstance = "", env: [String: String] = [:]
    var output = T3MobileTerminalOutput()
    /// TerminalBufferState: status, error, version and lifecycle version.
    var status = "closed", error: String? = nil, version = 0, lifecycleVersion = 0
    var label = "", hasRunningSubprocess = false
    var streamId: String? = nil, attaching = false, failed = false
    var readOnly = false
    var retained = false
    fileprivate var views: [WeakView] = []
    fileprivate var detachWork: DispatchWorkItem?
    fileprivate var resizing = false, pendingSize: (cols: Int, rows: Int)? = nil
    fileprivate var desiredSize: (cols: Int, rows: Int)? = nil
    var lastSize = (cols: 0, rows: 0)
    var chunks = 0, acknowledged = 0, writes = 0, writeFailures = 0, attaches = 0
    init(key: String, threadId: String, terminalId: String) { self.key = key; self.threadId = threadId; self.terminalId = terminalId }
    var liveViews: [T3MobileTerminalView] { views.compactMap(\.view) }
}

fileprivate struct WeakView { weak var view: T3MobileTerminalView? }

/// The module that owns the sessions (T3Module+Terminal.swift). A protocol, so the XCTests that leave out
/// T3Module*.swift still compile the terminal view.
protocol T3MobileTerminalSessionOwner: AnyObject { var terminalSessions: T3MobileTerminalSessions { get } }

final class T3MobileTerminalSessions {
    private static var registries: [ObjectIdentifier: T3MobileTerminalSessions] = [:]
    /// The sessions of one transport (the focused connection; the drawer's thread is always on it).
    static func of(_ transport: T3Transport) -> T3MobileTerminalSessions {
        if let found = registries[ObjectIdentifier(transport)] { return found }
        let made = T3MobileTerminalSessions(transport: transport)
        registries[ObjectIdentifier(transport)] = made
        return made
    }
    static var all: [T3MobileTerminalSessions] { Array(registries.values) }

    private weak var transport: T3Transport?
    private var connectedEnvironment = ""
    private(set) var sessions: [String: T3MobileTerminalSession] = [:]
    private var retainedKeys: Set<String> = []
    private static var attachGeneration = 0
    /// Seconds an unretained session keeps its stream after its last view goes (a thread switch's remount).
    var detachGrace: TimeInterval = 2

    init(transport: T3Transport) {
        self.transport = transport
        transport.terminalConnection = { [weak self, weak transport] connected in
            let environment = transport?.descriptor["environmentId"] as? String ?? ""
            DispatchQueue.main.async { self?.connectionChanged(connected, environment: environment) }
        }
        transport.queue.async { [weak self, weak transport] in
            guard let transport else { return }
            let connected = transport.state == "connected", environment = transport.descriptor["environmentId"] as? String ?? ""
            DispatchQueue.main.async { self?.connectionChanged(connected, environment: environment) }
        }
    }

    /// `JSON.stringify([environmentId, threadId, terminalId])`, the reference's terminal session key.
    static func key(environment: String, thread: String, terminal: String) -> String {
        T3MobileTerminalView.json([environment, thread, terminal])
    }

    /// Provider Account input shares the focused transport, but has its own serial queue in the view.
    func providerAuthCall(_ payload: [String: Any], done: @escaping (T3Failure?) -> Void) {
        guard let transport else { return done(T3Failure(kind: "Closed", message: "The server is not connected.")) }
        transport.terminalCall("provider.auth.respond", payload: payload) { error in
            DispatchQueue.main.async { done(error) }
        }
    }

    // MARK: Views

    func bind(_ view: T3MobileTerminalView, environment: String, thread: String, terminal: String, cwd: String, worktreePath: String, env: [String: String], providerInstance: String = "", readOnly: Bool = false) -> T3MobileTerminalSession {
        let key = Self.key(environment: environment, thread: thread, terminal: terminal)
        let session = sessions[key] ?? T3MobileTerminalSession(key: key, threadId: thread, terminalId: terminal)
        sessions[key] = session
        session.cwd = cwd; session.worktreePath = worktreePath; session.env = env; session.providerInstance = providerInstance; session.readOnly = readOnly
        session.views.removeAll { $0.view == nil || $0.view === view }
        session.views.append(WeakView(view: view))
        session.detachWork?.cancel(); session.detachWork = nil
        session.retained = session.retained || retainedKeys.contains(key)
        // A terminal shown again tries a failed attach once more (the reference remounts its viewport).
        if session.failed { session.failed = false }
        attach(session)
        return session
    }

    func unbind(_ view: T3MobileTerminalView, from session: T3MobileTerminalSession) {
        session.views.removeAll { $0.view == nil || $0.view === view }
        guard session.liveViews.isEmpty, !session.retained else { return }
        let work = DispatchWorkItem { [weak self, weak session] in
            guard let self, let session, session.liveViews.isEmpty, !session.retained else { return }
            self.drop(session)
        }
        session.detachWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + detachGrace, execute: work)
    }

    /// The drawer's mounted threads' terminals (terminal-drawer-view.ts): these keep their streams without a view.
    func retain(_ keys: Set<String>) {
        retainedKeys = keys
        for session in sessions.values {
            session.retained = keys.contains(session.key)
            if !session.retained, session.liveViews.isEmpty, session.detachWork == nil { drop(session) }
        }
    }

    private func drop(_ session: T3MobileTerminalSession) {
        guard sessions[session.key] === session else { return }
        session.detachWork?.cancel(); session.detachWork = nil
        if let id = session.streamId { transport?.terminalDetach(id) }
        session.streamId = nil
        sessions[session.key] = nil
    }

    // MARK: Stream

    private func connectionChanged(_ connected: Bool, environment: String) {
        guard connected else { connectedEnvironment = ""; return }
        connectedEnvironment = environment
        // Reconnecting another saved environment must never reattach the preceding server's PTYs.
        for session in Array(sessions.values) where environmentOf(session.key) != environment { drop(session) }
        for session in sessions.values where session.streamId == nil && !session.failed && (session.retained || !session.liveViews.isEmpty) { attach(session) }
    }

    private func attach(_ session: T3MobileTerminalSession) {
        guard let transport, !connectedEnvironment.isEmpty, environmentOf(session.key) == connectedEnvironment, session.streamId == nil, !session.attaching, !session.failed, (session.readOnly || !session.cwd.isEmpty) else { return }
        session.attaching = true
        // nextTerminalAttachSeedState: a reinstalled stream never reuses an old renderer's cursor.
        Self.attachGeneration += 1
        session.output = T3MobileTerminalOutput(); session.output.generation = Self.attachGeneration
        session.status = "closed"; session.error = nil; session.version = 0; session.lifecycleVersion = 0
        session.lastSize = (cols: 0, rows: 0)
        session.attaches += 1
        var payload: [String: Any] = ["threadId": session.threadId, "terminalId": session.terminalId, "cwd": session.cwd,
                                      "worktreePath": session.worktreePath.isEmpty ? NSNull() : session.worktreePath]
        if session.readOnly { payload = ["threadId": session.threadId, "terminalId": session.terminalId] }
        if !session.readOnly && !session.env.isEmpty { payload["env"] = session.env }
        if !session.providerInstance.isEmpty { payload["providerInstanceId"] = session.providerInstance }
        // A closed terminal ID may be reused before queued delivery from its old stream drains.
        let key = session.key, generation = session.output.generation
        transport.terminalAttach(payload: payload, receive: { [weak self, weak session] values, acknowledge in
            DispatchQueue.main.async {
                if let self, let session, self.sessions[key] === session, session.output.generation == generation {
                    session.chunks += 1
                    for value in values { if let event = value as? [String: Any] { self.apply(event, to: session) } }
                    session.acknowledged += 1
                    self.changed(key)
                }
                acknowledge() // only now: the buffer holds the chunk
            }
        }, ended: { [weak self, weak session] failure, lost in
            DispatchQueue.main.async {
                guard let self, let session, self.sessions[key] === session, session.output.generation == generation else { return }
                session.streamId = nil; session.attaching = false
                if let failure {
                    // useAttachedTerminalSession: an attach error shows as the session's error.
                    session.failed = true
                    session.status = "error"; session.error = failure.message; session.version += 1
                    self.changed(key)
                } else if lost {
                    // The next socket attaches again (connectionChanged).
                } else {
                    // The server ended the stream (the session closed).
                    session.failed = true
                }
            }
        }, opened: { [weak self, weak session] id, failure in
            DispatchQueue.main.async {
                guard let self, let session, self.sessions[key] === session, session.output.generation == generation else { if let id { transport.terminalDetach(id) }; return }
                session.attaching = false
                if let id { session.streamId = id; return }
                // Not connected yet: the connection's return attaches. Any other refusal shows as the error.
                if let failure, failure.kind != "Disconnected" {
                    session.failed = true
                    session.status = "error"; session.error = failure.message; session.version += 1
                    self.changed(key)
                }
            }
        })
    }

    /// applyTerminalAttachStreamEvent (terminalSession.ts), on the native buffer.
    func apply(_ event: [String: Any], to session: T3MobileTerminalSession) {
        switch event["type"] as? String {
        case "snapshot", "restarted":
            let snapshot = event["snapshot"] as? [String: Any] ?? [:]
            let lifecycle = event["type"] as? String == "restarted" || session.version != 0 ? session.lifecycleVersion + 1 : session.lifecycleVersion
            session.output.reset(snapshot["history"] as? String ?? "")
            session.status = snapshot["status"] as? String ?? "running"
            session.error = nil; session.version += 1; session.lifecycleVersion = lifecycle
            if let label = snapshot["label"] as? String { session.label = label }
            // A fit while disconnected may have failed. Reapply the current grid once the
            // server has attached, even when the view has not resized again.
            if let size = session.desiredSize { resize(session, cols: size.cols, rows: size.rows) }
        case "output":
            session.output.append(event["data"] as? String ?? "")
            if session.status == "closed" { session.status = "running" }
            session.error = nil; session.version += 1
        case "cleared":
            session.output.reset(""); session.error = nil; session.version += 1
        case "exited":
            session.status = "exited"; session.error = nil; session.version += 1
        case "closed":
            session.status = "closed"; session.error = nil; session.version += 1
        case "error":
            session.status = "error"; session.error = event["message"] as? String ?? "Terminal error"; session.version += 1
        case "activity":
            session.hasRunningSubprocess = event["hasRunningSubprocess"] as? Bool ?? false
            if let label = event["label"] as? String { session.label = label }
        default: break
        }
    }

    private func changed(_ key: String) {
        guard let session = sessions[key] else { return }
        for view in session.liveViews { view.sessionChanged(session) }
    }

    // MARK: Input

    /// handleData: one `terminal.write` per input; a failure is written into the terminal that sent it.
    func write(_ data: String, to session: T3MobileTerminalSession, from view: T3MobileTerminalView, done: @escaping () -> Void = {}) {
        guard !data.isEmpty, !session.readOnly, sessions[session.key] === session, environmentOf(session.key) == connectedEnvironment, let transport else { done(); return }
        session.writes += 1
        transport.terminalCall("terminal.write", payload: ["threadId": session.threadId, "terminalId": session.terminalId, "data": data]) { [weak view] failure in
            DispatchQueue.main.async {
                if let failure {
                    session.writeFailures += 1
                    view?.systemMessage(failure.message.isEmpty ? "Terminal write failed" : failure.message)
                }
                done()
            }
        }
    }

    /// onResize → `terminal.resize`, latest-wins while one is in flight (cols 1–1000, rows 1–500).
    func resize(_ session: T3MobileTerminalSession, cols: Int, rows: Int) {
        guard !session.readOnly, sessions[session.key] === session, environmentOf(session.key) == connectedEnvironment, cols > 0, rows > 0 else { return }
        let size = (cols: min(cols, 1000), rows: min(rows, 500))
        session.desiredSize = size
        if session.resizing { session.pendingSize = size; return }
        if size == session.lastSize { return } // the page's ready size and its first fit are often the same
        guard let transport else { return }
        session.resizing = true
        transport.terminalCall("terminal.resize", payload: ["threadId": session.threadId, "terminalId": session.terminalId, "cols": size.cols, "rows": size.rows]) { [weak self] failure in
            DispatchQueue.main.async {
                guard let self, self.sessions[session.key] === session else { return }
                session.resizing = false
                if failure == nil { session.lastSize = size }
                if let next = session.pendingSize {
                    session.pendingSize = nil
                    if next != session.lastSize { self.resize(session, cols: next.cols, rows: next.rows) }
                }
            }
        }
    }

    private func environmentOf(_ key: String) -> String {
        ((try? JSONSerialization.jsonObject(with: Data(key.utf8))) as? [String])?.first ?? ""
    }
    func destroy() {
        for session in Array(sessions.values) { drop(session) }
        retainedKeys.removeAll(); connectedEnvironment = ""
        transport?.terminalConnection = nil
        if let transport { Self.registries.removeValue(forKey: ObjectIdentifier(transport)) }
    }

    // MARK: Status (state.presentation.terminalSessions)

    var status: [[String: Any]] {
        sessions.values.sorted { $0.key < $1.key }.map { session in
            ["key": session.key, "threadId": session.threadId, "terminalId": session.terminalId, "status": session.status,
             "error": session.error ?? "", "label": session.label, "attached": session.streamId != nil, "retained": session.retained,
             "views": session.liveViews.count, "retainedBytes": session.output.retainedBytes, "chunks": session.chunks,
             "acknowledged": session.acknowledged, "writes": session.writes, "writeFailures": session.writeFailures,
             "attaches": session.attaches, "cols": session.lastSize.cols, "rows": session.lastSize.rows, "cwd": session.cwd,
             "tail": String((session.output.chunks.last?.data ?? "").suffix(160))]
        }
    }
}
#endif
