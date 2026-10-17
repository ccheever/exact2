import Foundation
import XCTest

// A terminal's attach stream rides the transport beside the app's streams
// (T3Transport+Terminal.swift). Its chunks never reach the app's event inbox, each is acknowledged only
// when the receiver says the buffer holds it, terminal streams do not count toward the 16 app streams,
// and a lost socket ends them all. Served by FixtureSocket (reconnect.swift), a loopback WebSocket peer.
final class TerminalStreamTests: XCTestCase {
    private func wait(_ seconds: TimeInterval) { RunLoop.current.run(until: Date().addingTimeInterval(seconds)) }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if condition() { return true }; wait(0.02) }
        return condition()
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any]) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        return result
    }
    private func connected(_ socket: FixtureSocket) -> T3Transport {
        FixtureHTTP.healthy()
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [FixtureHTTP.self]
        let transport = T3Transport(persistent: false, configuration: config, random: { 0 }, signals: false, changed: { _ in })
        let opened = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "credential": "pairing"])
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        return transport
    }
    private final class Sink: @unchecked Sendable {
        let lock = NSLock()
        var values: [[String: Any]] = [], held: [() -> Void] = [], ended: [(T3Failure?, Bool)] = []
        func add(_ new: [Any], _ ack: @escaping () -> Void) { lock.lock(); values += new.compactMap { $0 as? [String: Any] }; held.append(ack); lock.unlock() }
        func release() { lock.lock(); let all = held; held = []; lock.unlock(); all.forEach { $0() } }
        func end(_ failure: T3Failure?, _ lost: Bool) { lock.lock(); ended.append((failure, lost)); lock.unlock() }
        var count: Int { lock.lock(); defer { lock.unlock() }; return values.count }
        var endings: [(T3Failure?, Bool)] { lock.lock(); defer { lock.unlock() }; return ended }
    }
    private func attach(_ transport: T3Transport, _ sink: Sink, terminal: String = "term-1") -> (String?, T3Failure?) {
        let done = DispatchSemaphore(value: 0)
        var result: (String?, T3Failure?) = (nil, nil)
        transport.terminalAttach(payload: ["threadId": "thread-1", "terminalId": terminal, "cwd": "/repo"], receive: { sink.add($0, $1) },
                                 ended: { sink.end($0, $1) }, opened: { result = ($0, $1); done.signal() })
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        return result
    }

    func testOutputSkipsTheInboxAndIsAcknowledgedOnlyOnceHeld() throws {
        let socket = try FixtureSocket()
        socket.answer = { request in request["tag"] as? String == "terminal.attach" ? [] : [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        let transport = connected(socket); defer { transport.destroy() }
        let sink = Sink()
        let (id, failure) = attach(transport, sink)
        XCTAssertNil(failure)
        let streamId = try XCTUnwrap(id)
        XCTAssertTrue(until(2) { socket.requests("terminal.attach").first?["id"] as? String == streamId })
        XCTAssertEqual(socket.requests("terminal.attach").first?["payload"] as? [String: String], ["threadId": "thread-1", "terminalId": "term-1", "cwd": "/repo"])
        socket.send(["_tag": "Chunk", "requestId": streamId, "values": [["type": "snapshot", "snapshot": ["history": "$ "]]]])
        for index in 0..<11 { socket.send(["_tag": "Chunk", "requestId": streamId, "values": [["type": "output", "threadId": "thread-1", "terminalId": "term-1", "data": "line \(index)\r\n"]]]) }
        XCTAssertTrue(until(3) { sink.count == 12 }, "chunks received: \(sink.count)")
        wait(0.2)
        XCTAssertEqual(socket.frames(tagged: "Ack").count, 0, "nothing is acknowledged before the receiver holds it")
        sink.release()
        XCTAssertTrue(until(3) { socket.frames(tagged: "Ack").count == 12 }, "acks: \(socket.frames(tagged: "Ack").count)")
        XCTAssertTrue(socket.frames(tagged: "Ack").allSatisfy { $0["requestId"] as? String == streamId })
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let inbox = (perform(transport, ["op": "events", "after": 0, "generation": generation])["value"] as? [String: Any])?["events"] as? [Any] ?? []
        XCTAssertTrue(inbox.isEmpty, "terminal output must not enter the app inbox: \(inbox)")
        // The server ends the stream with a failure: the receiver hears it; nothing is retried.
        socket.send(["_tag": "Exit", "requestId": streamId, "exit": ["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "EnvironmentAuthorizationError", "message": "The authenticated token is missing required scope: terminal:operate."]]]]])
        XCTAssertTrue(until(2) { !sink.endings.isEmpty })
        XCTAssertEqual(sink.endings.first?.0?.message, "The authenticated token is missing required scope: terminal:operate.")
        wait(0.4)
        XCTAssertEqual(socket.requests("terminal.attach").count, 1)
    }

    /// The reference's RPC client refuses no request for the number already pending, so a
    /// user's write queued behind slow reads (a snooze chosen from a native menu) still goes out.
    func testPendingRequestsHaveNoCapAsInTheReference() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] } // the server holds every reply
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let lock = NSLock()
        var refused: [String] = []
        let note: ([String: Any]) -> Void = { reply in
            guard let error = reply["error"] as? [String: Any], error["kind"] as? String != "Timeout" else { return }
            lock.lock(); refused.append("\(error)"); lock.unlock()
        }
        for index in 0..<80 {
            transport.perform(["op": "request", "method": "vcs.refreshStatus", "payload": ["cwd": "/repo/\(index)"], "generation": generation, "timeout": 5], completion: note)
        }
        transport.perform(["op": "request", "method": "orchestration.dispatchCommand", "payload": ["type": "thread.snooze"], "generation": generation, "timeout": 5], completion: note)
        XCTAssertTrue(until(3) { socket.requests("vcs.refreshStatus").count == 80 && socket.requests("orchestration.dispatchCommand").count == 1 },
                      "sent: \(socket.requests("vcs.refreshStatus").count) reads, \(socket.requests("orchestration.dispatchCommand").count) writes")
        lock.lock(); let failures = refused; lock.unlock()
        XCTAssertEqual(failures, [], "no request is refused for the number pending")
    }

    /// A shared read joins an identical one still pending, as the reference's query atoms share
    /// one request per input. One request reaches the server and every caller gets its reply. Another
    /// payload is sent on its own, and a request that is not shared ends the sharing for its workspace.
    func testSharedReadsJoinAnIdenticalPendingOne() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] } // the server holds every reply
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let lock = NSLock()
        var replies: [[String: Any]] = []
        let note: ([String: Any]) -> Void = { reply in lock.lock(); replies.append(reply); lock.unlock() }
        for _ in 0..<5 {
            transport.perform(["op": "request", "method": "vcs.listRefs", "payload": ["cwd": "/repo", "limit": 50], "generation": generation, "share": true], completion: note)
        }
        transport.perform(["op": "request", "method": "vcs.listRefs", "payload": ["cwd": "/other", "limit": 50], "generation": generation, "share": true], completion: note)
        XCTAssertTrue(until(3) { socket.requests("vcs.listRefs").count >= 2 })
        wait(0.3)
        XCTAssertEqual(socket.requests("vcs.listRefs").count, 2, "five identical shared reads are one request")
        // A request that is not shared, naming /repo (as a branch switch does): a later shared read of
        // /repo is sent anew, and /other's still joins.
        transport.perform(["op": "request", "method": "vcs.switchRef", "payload": ["cwd": "/repo", "refName": "feature"], "generation": generation], completion: { _ in })
        transport.perform(["op": "request", "method": "vcs.listRefs", "payload": ["limit": 50, "cwd": "/repo"], "generation": generation, "share": true], completion: { _ in })
        transport.perform(["op": "request", "method": "vcs.listRefs", "payload": ["cwd": "/other", "limit": 50], "generation": generation, "share": true], completion: { _ in })
        XCTAssertTrue(until(3) { socket.requests("vcs.listRefs").count >= 3 })
        wait(0.3)
        XCTAssertEqual(socket.requests("vcs.listRefs").count, 3, "after the write, /repo is read again; /other still joins")
        let first = try XCTUnwrap(socket.requests("vcs.listRefs").first?["id"] as? String)
        socket.send(["_tag": "Exit", "requestId": first, "exit": ["_tag": "Success", "value": ["refs": [] as [Any], "totalCount": 0]]])
        XCTAssertTrue(until(3) { lock.lock(); defer { lock.unlock() }; return replies.filter { $0["ok"] as? Bool == true }.count == 5 },
                      "every joined caller gets the reply: \(replies)")
    }

    /// A shared reply too large for one native reply (over 512 KB) comes back as a native transfer.
    /// Each joined caller gets its own, so one reader's release never strands another's.
    func testJoinedCallersEachReadTheirOwnLargeReply() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] } // the server holds the reply until the test sends it
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let lock = NSLock()
        var replies: [[String: Any]] = []
        for _ in 0..<2 {
            transport.perform(["op": "request", "method": "vcs.listRefs", "payload": ["cwd": "/repo"], "generation": generation, "share": true]) { reply in
                lock.lock(); replies.append(reply); lock.unlock()
            }
        }
        XCTAssertTrue(until(3) { socket.requests("vcs.listRefs").count == 1 })
        wait(0.2)
        XCTAssertEqual(socket.requests("vcs.listRefs").count, 1, "the second read joined the first")
        let id = try XCTUnwrap(socket.requests("vcs.listRefs").first?["id"] as? String)
        let name = String(repeating: "r", count: 600 * 1024)
        socket.send(["_tag": "Exit", "requestId": id, "exit": ["_tag": "Success", "value": ["refs": [["name": name]], "totalCount": 1]]])
        XCTAssertTrue(until(3) { lock.lock(); defer { lock.unlock() }; return replies.count == 2 })
        lock.lock(); let both = replies; lock.unlock()
        // Each caller reads its transfer to the end and releases it, as protocol.ts bridgeReply does, one after the other.
        for (caller, reply) in both.enumerated() {
            let transfer = try XCTUnwrap((reply["value"] as? [String: Any])?["_nativeTransfer"] as? [String: Any], "caller \(caller): \(reply)")
            let transferId = try XCTUnwrap(transfer["id"] as? String), parts = try XCTUnwrap(transfer["parts"] as? Int)
            var text = ""
            for index in 0..<parts {
                let chunk = perform(transport, ["op": "readChunk", "id": transferId, "index": index, "generation": generation])
                XCTAssertEqual(chunk["ok"] as? Bool, true, "caller \(caller), chunk \(index): \(chunk)")
                text += ((chunk["value"] as? [String: Any])?["text"] as? String) ?? ""
            }
            _ = perform(transport, ["op": "releaseChunk", "id": transferId, "generation": generation])
            let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
            XCTAssertEqual(((value?["refs"] as? [[String: Any]])?.first?["name"] as? String)?.count, name.count, "caller \(caller) read the whole reply")
        }
    }

    func testTerminalStreamsDoNotCountTowardTheSixteenAppStreamsAndEndWithTheSocket() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] }
        let transport = connected(socket); defer { transport.destroy() }
        let generation = perform(transport, ["op": "status"])["generation"] as! Int
        let sinks = (0..<20).map { _ in Sink() }
        for (index, sink) in sinks.enumerated() { XCTAssertNotNil(attach(transport, sink, terminal: "term-\(index + 1)").0) }
        for index in 0..<16 {
            let reply = perform(transport, ["op": "subscribe", "key": "app-\(index)", "method": "subscribeServerConfig", "payload": [:], "generation": generation])
            XCTAssertEqual(reply["ok"] as? Bool, true, "app stream \(index): \(reply)")
        }
        let seventeenth = perform(transport, ["op": "subscribe", "key": "app-16", "method": "subscribeServerConfig", "payload": [:], "generation": generation])
        XCTAssertEqual((seventeenth["error"] as? [String: Any])?["kind"] as? String, "Busy", "the app's own cap is unchanged")
        XCTAssertTrue(until(3) { socket.requests("terminal.attach").count == 20 }, "attach requests: \(socket.requests("terminal.attach").count)")
        socket.dropAll()
        XCTAssertTrue(until(5) { sinks.allSatisfy { $0.endings.first?.1 == true } }, "a lost socket ends every attach stream")
    }

    func testProviderInstanceIsPreservedAcrossSessionReattach() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] }
        let transport = connected(socket); defer { transport.destroy() }
        let sessions = T3TerminalSessions(transport: transport)
        let view = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        defer { view.destroy() }
        _ = sessions.bind(view, environment: "fixture", thread: "thread-1", terminal: "term-1", cwd: "/repo", worktreePath: "", env: [:], providerInstance: "codex-custom")
        XCTAssertTrue(until(2) { socket.requests("terminal.attach").count == 1 })
        XCTAssertEqual((socket.requests("terminal.attach").first?["payload"] as? [String: Any])?["providerInstanceId"] as? String, "codex-custom")
        socket.dropAll()
        XCTAssertTrue(until(8) { socket.requests("terminal.attach").count == 2 })
        XCTAssertEqual((socket.requests("terminal.attach").last?["payload"] as? [String: Any])?["providerInstanceId"] as? String, "codex-custom", "a fresh server shell must resolve the same provider environment")
    }

    func testQueuedOldStreamCannotChangeAReplacementWithTheSameTerminalID() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] }
        let transport = connected(socket); defer { transport.destroy() }
        let sessions = T3TerminalSessions(transport: transport)
        let oldView = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        let newView = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 2), agent: true)
        defer { oldView.destroy(); newView.destroy() }
        let old = sessions.bind(oldView, environment: "fixture", thread: "thread-1", terminal: "term-5", cwd: "/repo", worktreePath: "", env: [:])
        XCTAssertTrue(until(2) { old.streamId != nil })
        let id = try XCTUnwrap(old.streamId)
        sessions.retain([old.key]); sessions.unbind(oldView, from: old)
        // Queue old delivery while main is occupied by an unmount/remount. The callbacks must
        // still acknowledge old output, but may not look up and mutate the replacement by key.
        transport.queue.sync {
            let stream = transport.terminalStreams[id]!
            stream.receive([["type": "output", "data": "old lifetime"]]) {}
            stream.receive([["type": "closed"]]) {}
            stream.ended(T3Failure(kind: "OldStream", message: "old lifetime failed"), false)
        }
        sessions.retain([])
        let replacement = sessions.bind(newView, environment: "fixture", thread: "thread-1", terminal: "term-5", cwd: "/repo", worktreePath: "", env: [:])
        XCTAssertFalse(replacement === old)
        XCTAssertTrue(until(2) { replacement.streamId != nil })
        XCTAssertEqual(replacement.version, 0)
        XCTAssertEqual(replacement.output.text, "")
        XCTAssertEqual(replacement.chunks, 0)
        XCTAssertNil(replacement.error)
        XCTAssertFalse(replacement.failed)
    }

    func testLateResizeCannotResizeAReplacementWithTheSameTerminalID() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] }
        let transport = connected(socket); defer { transport.destroy() }
        let sessions = T3TerminalSessions(transport: transport)
        XCTAssertTrue(until(2) { sessions.reaching("fixture") === transport })
        let oldView = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        let newView = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 2), agent: true)
        defer { oldView.destroy(); newView.destroy() }
        let old = sessions.bind(oldView, environment: "fixture", thread: "thread-1", terminal: "term-5", cwd: "/repo", worktreePath: "", env: [:])
        sessions.resize(old, cols: 100, rows: 30)
        XCTAssertTrue(until(2) { socket.requests("terminal.resize").count == 1 })
        let held = try XCTUnwrap(socket.requests("terminal.resize").first?["id"])
        sessions.resize(old, cols: 120, rows: 40)
        sessions.retain([old.key]); sessions.unbind(oldView, from: old); sessions.retain([])
        let replacement = sessions.bind(newView, environment: "fixture", thread: "thread-1", terminal: "term-5", cwd: "/repo", worktreePath: "", env: [:])
        XCTAssertFalse(old === replacement)
        sessions.resize(replacement, cols: 80, rows: 24)
        XCTAssertTrue(until(2) { socket.requests("terminal.resize").count == 2 })
        socket.send(["_tag": "Exit", "requestId": held, "exit": ["_tag": "Success", "value": [:]]])
        wait(0.2)
        XCTAssertEqual(socket.requests("terminal.resize").count, 2, "old pending grid must never be sent to the replacement terminal")
    }

    func testDisconnectedResizeIsAppliedOnAttachWithoutAnotherFit() throws {
        let socket = try FixtureSocket()
        socket.answer = { request in [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        FixtureHTTP.healthy()
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [FixtureHTTP.self]
        let transport = T3Transport(persistent: false, configuration: config, signals: false, changed: { _ in })
        defer { transport.destroy() }
        let sessions = T3TerminalSessions(transport: transport)
        let view = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        defer { view.destroy() }
        let session = sessions.bind(view, environment: "fixture", thread: "thread-1", terminal: "term-1", cwd: "", worktreePath: "", env: [:])
        sessions.resize(session, cols: 120, rows: 40)
        // A queue barrier ensures the disconnected RPC answered; then drain its main-queue callback.
        _ = perform(transport, ["op": "status"])
        wait(0.05)
        XCTAssertEqual(session.lastSize.cols, 0, "a refused grid must not be recorded as applied")
        let opened = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "credential": "pairing"])
        XCTAssertEqual(opened["ok"] as? Bool, true)
        XCTAssertTrue(until(2) { sessions.reaching("fixture") === transport })
        // Deliver the attach snapshot through the production reducer, without another resize event.
        sessions.apply(["type": "snapshot", "snapshot": ["history": "", "status": "running"]], to: session)
        XCTAssertTrue(until(2) { session.lastSize.cols == 120 && session.lastSize.rows == 40 })
        XCTAssertEqual(socket.requests("terminal.resize").count, 1)
        XCTAssertEqual((socket.requests("terminal.resize").first?["payload"] as? [String: Any])?["rows"] as? Int, 40)
        sessions.resize(session, cols: 120, rows: 40)
        _ = perform(transport, ["op": "status"])
        wait(0.05)
        XCTAssertEqual(socket.requests("terminal.resize").count, 1, "successful identical fits stay deduplicated")
    }

    func testResizeFailureCanRetryAndInflightChangesKeepOnlyTheLatestGrid() throws {
        let socket = try FixtureSocket()
        socket.answer = { _ in [] } // Hold each resize until the test releases its reply.
        let transport = connected(socket); defer { transport.destroy() }
        let sessions = T3TerminalSessions(transport: transport)
        XCTAssertTrue(until(2) { sessions.reaching("fixture") === transport })
        let view = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        defer { view.destroy() }
        let session = sessions.bind(view, environment: "fixture", thread: "thread-1", terminal: "term-1", cwd: "", worktreePath: "", env: [:])
        sessions.resize(session, cols: 100, rows: 30)
        XCTAssertTrue(until(2) { socket.requests("terminal.resize").count == 1 })
        let failed = try XCTUnwrap(socket.requests("terminal.resize").first?["id"])
        socket.send(["_tag": "Exit", "requestId": failed, "exit": ["_tag": "Failure", "cause": [["_tag": "Fail", "error": ["_tag": "TerminalError", "message": "resize failed"]]]]])
        wait(0.1)
        XCTAssertEqual(session.lastSize.cols, 0)
        XCTAssertEqual(socket.requests("terminal.resize").count, 1, "failure must not start a retry loop")
        sessions.resize(session, cols: 100, rows: 30)
        XCTAssertTrue(until(2) { socket.requests("terminal.resize").count == 2 }, "the same failed grid must be retryable")
        sessions.resize(session, cols: 110, rows: 35)
        sessions.resize(session, cols: 120, rows: 40)
        let retry = try XCTUnwrap(socket.requests("terminal.resize").last?["id"])
        socket.send(["_tag": "Exit", "requestId": retry, "exit": ["_tag": "Success", "value": [:]]])
        XCTAssertTrue(until(2) { socket.requests("terminal.resize").count == 3 })
        let latest = try XCTUnwrap(socket.requests("terminal.resize").last)
        XCTAssertEqual((latest["payload"] as? [String: Any])?["cols"] as? Int, 120)
        socket.send(["_tag": "Exit", "requestId": latest["id"]!, "exit": ["_tag": "Success", "value": [:]]])
        XCTAssertTrue(until(2) { session.lastSize.cols == 120 && session.lastSize.rows == 40 })
        XCTAssertEqual(socket.requests("terminal.resize").count, 3)
    }

    /// A session goes to its own environment's server, as the reference's terminal
    /// commands take the thread's `environmentId`. Two loopback servers are environments A and B; the focused
    /// transport reaches A, then B (the clone opens B's thread by moving the focus there), and each environment's
    /// background transport carries its sessions while another is the focus. A session is never attached, written
    /// or resized on another environment's server.
    func testSessionsGoToTheirOwnEnvironmentsServerAcrossAFocusChange() throws {
        let a = try FixtureSocket(), b = try FixtureSocket()
        a.answer = { request in request["tag"] as? String == "terminal.attach" ? [] : [["_tag": "Exit", "requestId": request["id"]!, "exit": ["_tag": "Success", "value": [:]]]] }
        b.answer = a.answer
        let portA = Int(a.port)
        FixtureHTTP.reset { request in
            let environment = request.url?.port == portA ? "env-a" : "env-b"
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, ["environmentId": environment, "label": environment, "orchestrationProtocolVersion": 2, "capabilities": ["connectionProbe": true]])
            case "/oauth/token": return (200, ["access_token": "fixture-access", "token_type": "Bearer"])
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read", "orchestration:operate", "terminal:operate"]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "fixture-ticket"])
            default: return (404, [:])
            }
        }
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [FixtureHTTP.self]
        let make = { T3Transport(persistent: false, configuration: config, random: { 0 }, signals: false, changed: { _ in }) }
        let connect = { (transport: T3Transport, socket: FixtureSocket) in
            let opened = self.perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "credential": "pairing"])
            XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        }
        let threads = { (socket: FixtureSocket, method: String) in socket.requests(method).compactMap { ($0["payload"] as? [String: Any])?["threadId"] as? String } }
        let focused = make(), backgroundB = make(), backgroundA = make()
        defer { focused.destroy(); backgroundB.destroy(); backgroundA.destroy() }
        connect(focused, a)
        let sessions = T3TerminalSessions(transport: focused)
        let via = { (session: T3TerminalSession) in sessions.status.first { $0["key"] as? String == session.key }?["via"] as? String }
        XCTAssertTrue(until(2) { sessions.reaching("env-a") === focused })
        let viewA = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        let viewB = T3TerminalView(props: [:], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 2), agent: true)
        defer { viewA.destroy(); viewB.destroy() }
        let sessionA = sessions.bind(viewA, environment: "env-a", thread: "a-1", terminal: "term-1", cwd: "/a", worktreePath: "", env: [:])
        let sessionB = sessions.bind(viewB, environment: "env-b", thread: "b-1", terminal: "term-1", cwd: "/b", worktreePath: "", env: [:])
        XCTAssertTrue(until(2) { sessionA.streamId != nil })
        wait(0.3)
        XCTAssertEqual(threads(a, "terminal.attach"), ["a-1"], "B's terminal is never started on A's server")
        XCTAssertNil(sessionB.streamId, "B's terminal waits for B's server")
        sessions.write("pwd\r", to: sessionB, from: viewB)
        XCTAssertEqual(sessionB.writeFailures, 1)
        XCTAssertTrue(threads(a, "terminal.write").isEmpty)
        // B is a background environment: its own transport carries its terminal.
        sessions.observe(backgroundB)
        connect(backgroundB, b)
        XCTAssertTrue(until(3) { threads(b, "terminal.attach") == ["b-1"] }, "\(threads(b, "terminal.attach"))")
        XCTAssertTrue(until(2) { sessionB.streamId != nil })
        sessions.write("echo b\r", to: sessionB, from: viewB)
        sessions.resize(sessionB, cols: 90, rows: 20)
        XCTAssertTrue(until(2) { threads(b, "terminal.write") == ["b-1"] && threads(b, "terminal.resize") == ["b-1"] })
        XCTAssertTrue(threads(a, "terminal.write").isEmpty && threads(a, "terminal.resize").isEmpty)
        // The focus moves to B (settings-b-fleet.ts focusFleetThread: B's background transport stops, the focused one reconnects there).
        backgroundB.destroy()
        connect(focused, b)
        XCTAssertTrue(until(3) { threads(b, "terminal.attach") == ["b-1", "b-1"] }, "\(threads(b, "terminal.attach"))")
        XCTAssertTrue(until(2) { sessions.reaching("env-b") === focused && sessionB.streamId != nil })
        wait(0.3)
        XCTAssertFalse(threads(b, "terminal.attach").contains("a-1"), "A's terminal is never started on B's server")
        XCTAssertNil(sessionA.streamId, "A's terminal waits for A's server")
        XCTAssertEqual(via(sessionA), "", "A's ended stream is not reported on the transport it was opened on")
        XCTAssertEqual(via(sessionB), "focused")
        // A is now a background environment: its terminal attaches there again.
        sessions.observe(backgroundA)
        connect(backgroundA, a)
        XCTAssertTrue(until(3) { threads(a, "terminal.attach") == ["a-1", "a-1"] }, "\(threads(a, "terminal.attach"))")
        XCTAssertTrue(until(2) { sessionA.streamId != nil && sessions.reaching("env-a") === backgroundA })
        sessions.write("echo a\r", to: sessionA, from: viewA)
        XCTAssertTrue(until(2) { threads(a, "terminal.write") == ["a-1"] })
        XCTAssertFalse(threads(b, "terminal.write").contains("a-1"))
        XCTAssertEqual(sessions.status.map { $0["via"] as? String ?? "" }.sorted(), ["background", "focused"])
    }
}
