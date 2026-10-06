import Foundation
import XCTest

// Task terminal-drawer: a terminal's attach stream rides the transport beside the app's streams
// (T3Transport+Terminal.swift). Its chunks never reach the app's event inbox, each is acknowledged only
// when the receiver says the buffer holds it, terminal streams do not count toward the 16 app streams,
// and a lost socket ends them all. Served by R3Socket (r3.swift), a loopback WebSocket peer.
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
    private func connected(_ socket: R3Socket) -> T3Transport {
        R3HTTP.healthy()
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
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
        let socket = try R3Socket()
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

    func testTerminalStreamsDoNotCountTowardTheSixteenAppStreamsAndEndWithTheSocket() throws {
        let socket = try R3Socket()
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
}
