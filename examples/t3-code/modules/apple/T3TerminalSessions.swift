#if os(macOS)
import Foundation

/// The drawer's terminal sessions, on the main thread. T3 Code 1e2ecbd975 (MIT
/// reference, see LICENSE-T3) keeps one attach subscription per mounted terminal viewport and folds its
/// events with `applyTerminalAttachStreamEvent` (packages/client-runtime/src/state/terminalSession.ts,
/// terminal.ts `attach`); the viewport writes what is new since its cursor (ThreadTerminalDrawer.tsx
/// TerminalViewport) and sends input and grid sizes with `terminal.write` / `terminal.resize`.
/// Changes from the reference, for exact2:
/// - The stream and its buffer are native: chunks go from the module's link to the environment
///   (T3Link.swift) to the session's `T3TerminalOutput` to the bound `t3-terminal` views, never through
///   the data module. A chunk is acknowledged once the buffer holds it.
/// - A session lives while a view shows it or while the drawer keeps its thread mounted (`retain`, the
///   reference's MAX_HIDDEN_MOUNTED_TERMINAL_THREADS hidden threads plus the active one), so a hidden
///   thread keeps its stream and buffer without a web view; its view replays the buffer
///   when it shows again. An unretained session without views detaches after a short grace.
/// - A lost socket ends every attach stream; the sessions attach again once the data module's connection
///   is back (the reference's durable subscription waits for the next session the same way). A stream that fails
///   (an authorization error, a bad cwd) is not retried until the drawer shows the terminal again.
/// - Writes are one RPC per input, as in the reference: a paste over 65,536 characters is refused by
///   the server and shown as `[terminal] <message>`.
/// - Resizes are latest-wins per session (the reference's `concurrency: latest` scheduler).
/// - A session goes to its own environment's server, as the reference's terminal commands take the
///   thread's `environmentId`: the link to that environment, which the data module hands over while it
///   is connected (focused or in the background). A session whose environment has no link waits for one,
///   never attaching to another server.
final class T3TerminalSession {
    let key: String, environment: String, threadId: String, terminalId: String
    var cwd = "", worktreePath = "", providerInstance = "", env: [String: String] = [:]
    var output = T3TerminalOutput()
    /// TerminalBufferState: status, error, version and lifecycle version.
    var status = "closed", error: String? = nil, version = 0, lifecycleVersion = 0
    var label = "", hasRunningSubprocess = false
    var streamId: String? = nil, attaching = false, failed = false
    var retained = false
    /// The link the current stream is opening or open on (the stream ends with that link's socket);
    /// nil once it ends or fails to open, so the status's `via` names only a stream the session has.
    fileprivate weak var streamLink: T3Link?
    fileprivate var views: [WeakView] = []
    fileprivate var detachWork: DispatchWorkItem?
    fileprivate var resizing = false, pendingSize: (cols: Int, rows: Int)? = nil
    fileprivate var desiredSize: (cols: Int, rows: Int)? = nil
    var lastSize = (cols: 0, rows: 0)
    var chunks = 0, acknowledged = 0, writes = 0, writeFailures = 0, attaches = 0
    init(key: String, environment: String, threadId: String, terminalId: String) {
        self.key = key; self.environment = environment; self.threadId = threadId; self.terminalId = terminalId
    }
    var liveViews: [T3TerminalView] { views.compactMap(\.view) }
}

fileprivate struct WeakView { weak var view: T3TerminalView? }

/// The module that owns the sessions (T3Module+Terminal.swift). A protocol, so the XCTests that leave out
/// T3Module*.swift still compile the terminal view.
protocol T3TerminalSessionOwner: AnyObject { var terminalSessions: T3TerminalSessions { get } }

final class T3TerminalSessions {
    private static var registries: [ObjectIdentifier: T3TerminalSessions] = [:]
    /// The module's sessions, by its links.
    static func of(_ links: T3Links) -> T3TerminalSessions {
        if let found = registries[ObjectIdentifier(links)] { return found }
        let made = T3TerminalSessions(links: links)
        registries[ObjectIdentifier(links)] = made
        return made
    }
    static var all: [T3TerminalSessions] { Array(registries.values) }

    private weak var links: T3Links?
    private(set) var sessions: [String: T3TerminalSession] = [:]
    private var retainedKeys: Set<String> = []
    private static var attachGeneration = 0
    /// Seconds an unretained session keeps its stream after its last view goes (a thread switch's remount).
    var detachGrace: TimeInterval = 2

    init(links: T3Links) {
        self.links = links
        links.changed.append { [weak self] environment, link in self?.linkChanged(environment, link) }
    }

    /// The link that reaches an environment now, while the data module keeps it connected.
    func reaching(_ environment: String) -> T3Link? { links?.link(environment: environment) }

    /// `JSON.stringify([environmentId, threadId, terminalId])`, the reference's terminal session key.
    static func key(environment: String, thread: String, terminal: String) -> String {
        T3TerminalView.json([environment, thread, terminal])
    }

    /// Provider Account input goes to the focused environment, but has its own serial queue in the view.
    func providerAuthCall(_ payload: [String: Any], done: @escaping (T3Failure?) -> Void) {
        guard let link = links?.focusedLink else { return done(T3Failure(kind: "Closed", message: "The server is not connected.")) }
        link.terminalCall("provider.auth.respond", payload: payload) { error in
            DispatchQueue.main.async { done(error) }
        }
    }

    // MARK: Views

    func bind(_ view: T3TerminalView, environment: String, thread: String, terminal: String, cwd: String, worktreePath: String, env: [String: String], providerInstance: String = "") -> T3TerminalSession {
        let key = Self.key(environment: environment, thread: thread, terminal: terminal)
        let session = sessions[key] ?? T3TerminalSession(key: key, environment: environment, threadId: thread, terminalId: terminal)
        sessions[key] = session
        session.cwd = cwd; session.worktreePath = worktreePath; session.env = env; session.providerInstance = providerInstance
        session.views.removeAll { $0.view == nil || $0.view === view }
        session.views.append(WeakView(view: view))
        session.detachWork?.cancel(); session.detachWork = nil
        session.retained = session.retained || retainedKeys.contains(key)
        // A terminal shown again tries a failed attach once more (the reference remounts its viewport).
        if session.failed { session.failed = false }
        attach(session)
        return session
    }

    func unbind(_ view: T3TerminalView, from session: T3TerminalSession) {
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

    private func drop(_ session: T3TerminalSession) {
        guard sessions[session.key] === session else { return }
        session.detachWork?.cancel(); session.detachWork = nil
        if let id = session.streamId { session.streamLink?.terminalDetach(id) }
        session.streamId = nil; session.streamLink = nil
        sessions[session.key] = nil
    }

    // MARK: Stream

    /// The data module connected an environment (or reconnected it: its sessions attach there) or let it go (its
    /// streams have ended with the link's socket).
    private func linkChanged(_ environment: String, _ link: T3Link?) {
        DispatchQueue.main.async { [weak self] in
            guard let self, link != nil, self.reaching(environment) != nil else { return }
            for session in self.sessions.values where session.environment == environment && session.streamId == nil && !session.failed
                && (session.retained || !session.liveViews.isEmpty) { self.attach(session) }
        }
    }

    private func attach(_ session: T3TerminalSession) {
        guard let link = reaching(session.environment), session.streamId == nil, !session.attaching, !session.failed,
              !session.cwd.isEmpty else { return }
        session.attaching = true
        session.streamLink = link
        // nextTerminalAttachSeedState: a reinstalled stream never reuses an old renderer's cursor.
        Self.attachGeneration += 1
        session.output = T3TerminalOutput(); session.output.generation = Self.attachGeneration
        session.status = "closed"; session.error = nil; session.version = 0; session.lifecycleVersion = 0
        session.lastSize = (cols: 0, rows: 0)
        session.attaches += 1
        var payload: [String: Any] = ["threadId": session.threadId, "terminalId": session.terminalId, "cwd": session.cwd,
                                      "worktreePath": session.worktreePath.isEmpty ? NSNull() : session.worktreePath]
        if !session.env.isEmpty { payload["env"] = session.env }
        if !session.providerInstance.isEmpty { payload["providerInstanceId"] = session.providerInstance }
        // A closed terminal ID may be reused before queued delivery from its old stream drains.
        let key = session.key, generation = session.output.generation
        link.terminalAttach(payload: payload, receive: { [weak self, weak session] values, acknowledge in
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
                session.streamId = nil; session.attaching = false; session.streamLink = nil
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
                guard let self, let session, self.sessions[key] === session, session.output.generation == generation else { if let id { link.terminalDetach(id) }; return }
                session.attaching = false
                if let id { session.streamId = id; return }
                session.streamLink = nil
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
    func apply(_ event: [String: Any], to session: T3TerminalSession) {
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

    /// `TerminalWriteInput`'s `data` bound (contracts terminal.ts: `isMaxLength(65_536)`, UTF-16 units).
    static let maxWriteLength = 65_536
    /// What the reference shows for a write its client refuses to encode (effect's sync encode, SchemaParser.ts).
    static let schemaRefusal = "Schema validation failed"

    /// handleData: one `terminal.write` per input; a failure is written into the terminal that sent it.
    /// The reference's client encodes the input first, so data over the bound is refused before anything
    /// is sent (a 70,000-character paste reads "Schema validation failed").
    func write(_ data: String, to session: T3TerminalSession, from view: T3TerminalView) {
        guard !data.isEmpty else { return }
        session.writes += 1
        if data.utf16.count > Self.maxWriteLength {
            session.writeFailures += 1
            return view.systemMessage(Self.schemaRefusal)
        }
        guard let link = reaching(session.environment) else {
            session.writeFailures += 1
            return view.systemMessage("The server is not connected.")
        }
        link.terminalCall("terminal.write", payload: ["threadId": session.threadId, "terminalId": session.terminalId, "data": data]) { [weak view] failure in
            guard let failure else { return }
            DispatchQueue.main.async {
                session.writeFailures += 1
                view?.systemMessage(failure.message.isEmpty ? "Terminal write failed" : failure.message)
            }
        }
    }

    /// onResize → `terminal.resize`, latest-wins while one is in flight (cols 1–1000, rows 1–500).
    func resize(_ session: T3TerminalSession, cols: Int, rows: Int) {
        guard sessions[session.key] === session, cols > 0, rows > 0 else { return }
        let size = (cols: min(cols, 1000), rows: min(rows, 500))
        session.desiredSize = size
        if session.resizing { session.pendingSize = size; return }
        if size == session.lastSize { return } // the page's ready size and its first fit are often the same
        guard let link = reaching(session.environment) else { return } // applied on the next attach's snapshot
        session.resizing = true
        link.terminalCall("terminal.resize", payload: ["threadId": session.threadId, "terminalId": session.terminalId, "cols": size.cols, "rows": size.rows]) { [weak self] failure in
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

    // MARK: Status (state.presentation.terminalSessions)

    var status: [[String: Any]] {
        sessions.values.sorted { $0.key < $1.key }.map { session in
            ["key": session.key, "environment": session.environment, "via": session.streamLink == nil ? "" : session.streamLink === links?.focusedLink ? "focused" : "background",
             "threadId": session.threadId, "terminalId": session.terminalId, "status": session.status,
             "error": session.error ?? "", "label": session.label, "attached": session.streamId != nil, "retained": session.retained,
             "views": session.liveViews.count, "retainedBytes": session.output.retainedBytes, "chunks": session.chunks,
             "acknowledged": session.acknowledged, "writes": session.writes, "writeFailures": session.writeFailures,
             "attaches": session.attaches, "cols": session.lastSize.cols, "rows": session.lastSize.rows, "cwd": session.cwd,
             "tail": String((session.output.chunks.last?.data ?? "").suffix(160))]
        }
    }
}
#endif
