#if os(macOS) || os(iOS)
import CryptoKit
import Darwin
import Foundation
import XCTest
@testable import ExactKit

// The dev client across its server's restarts (LLP 1023 D3, LLP 1122 D4),
// against a loopback stand-in for host/web/dev.mjs: a dropped stream, or a
// server restarted on the same port, is subscribed and registered again
// under the build's id, and the pin the channel holds for it comes back on
// the new stream; a `rebuilt` frame or a generation for another program
// takes no generation, but the host keeps its stream and registration and
// takes the next generation for its own program.
final class PlanURLReconnectTests: XCTestCase {
    private static let program = String(repeating: "a", count: 64), other = String(repeating: "b", count: 64)

    #if os(macOS)
    func testAHostSubscribesAndRegistersAgainAcrossADroppedStreamAndARestart() throws {
        try XCTSkipUnless(BuildChannel.enabled, "a production bake has no build channel")
        let server = try DevServerStub()
        defer { server.stop() }
        try server.start(program: Self.program, plan: Data("plan one".utf8))
        let host = Host.open(server.page)
        defer { host.close() }
        settle { host.applied.count == 1 && server.registers(in: server.epoch).count == 1 }
        let first = try XCTUnwrap(server.registers(in: server.epoch).first)
        XCTAssertEqual(first.body["id"] as? String, host.id)
        // Its `plan` may be null: the stream opens before the payloads are
        // fetched, so the first register can go before the first apply.
        XCTAssertEqual(host.applied, [server.identity(seq: 1)])
        // A pin from the design tool reaches the build, which reports it.
        let revision = server.pin(host.id, ["scheme": "dark"])
        settle { replayed(host, server, on: first.nonce, revision: revision) }
        XCTAssertEqual(host.pins(on: first.nonce).last?["pin"] as? [String: String], ["scheme": "dark"])

        // The stream dropped with the server up: a new stream, a register on
        // its nonce under the same id, and the pin the server holds for that
        // id replayed there (it is the server that re-asserts the pin; the
        // host only registers as itself again).
        server.dropStreams()
        settle { server.registers(in: server.epoch).count == 2 && replayed(host, server, on: server.registers(in: server.epoch)[1].nonce, revision: revision) }
        let second = try XCTUnwrap(server.registers(in: server.epoch).dropFirst().first)
        XCTAssertNotEqual(second.nonce, first.nonce)
        XCTAssertEqual(second.body["id"] as? String, host.id)
        XCTAssertEqual(server.opened(in: server.epoch), 2)
        XCTAssertEqual(host.pins(on: second.nonce).last?["pin"] as? [String: String], ["scheme": "dark"])

        // The server stopped, down until the host's retry has been refused
        // (the port reset each connection meanwhile), and started again on
        // the same port (a new process: a new epoch, its sequence from 1)
        // with the same plan: subscribed and registered again, the pin
        // replayed, and the generation it already runs not fetched again.
        let before = server.epoch
        server.stop()
        try server.startRefusing()
        settle { server.refused > 0 }
        server.stop()
        try server.start(program: Self.program, plan: Data("plan one".utf8))
        XCTAssertNotEqual(server.epoch, before)
        settle { server.registers(in: server.epoch).count == 1 && replayed(host, server, on: server.registers(in: server.epoch)[0].nonce, revision: revision) }
        let third = try XCTUnwrap(server.registers(in: server.epoch).first)
        XCTAssertEqual(third.body["id"] as? String, host.id)
        XCTAssertEqual(third.body["plan"] as? String, server.identity(seq: 1))
        XCTAssertEqual(server.opened(in: server.epoch), 1)
        XCTAssertEqual(host.pins(on: third.nonce).last?["pin"] as? [String: String], ["scheme": "dark"])
        // The status that acknowledges the replayed pin follows the register.
        let kinds = server.reports.filter { $0.nonce == third.nonce }.map(\.kind)
        XCTAssertEqual(kinds.first, "register")
        XCTAssertEqual(host.applied.count, 1)
        XCTAssertFalse(server.requests(in: server.epoch).contains { $0.hasSuffix("/app.plan") })
        XCTAssertNil(host.connection?.terminal)
    }

    func testAnotherProgramTakesNoGenerationAndTheHostStaysSubscribedAndRegistered() throws {
        try XCTSkipUnless(BuildChannel.enabled, "a production bake has no build channel")
        let server = try DevServerStub()
        defer { server.stop() }
        try server.start(program: Self.program, plan: Data("plan one".utf8))
        let host = Host.open(server.page)
        defer { host.close() }
        settle { host.applied.count == 1 && server.registers(in: server.epoch).count == 1 }
        let first = try XCTUnwrap(server.registers(in: server.epoch).first)

        // The server rebuilt the native code, then announces a generation for
        // the new program: neither is taken. The pin sent after them, in the
        // stream's order, still reaches the build and its status is taken on
        // the same nonce: the stream stayed open and the host registered.
        server.push(["rebuilt": 1])
        settle { host.connection?.terminal != nil }
        let foreign = server.publish(program: Self.other, plan: Data("plan two".utf8))
        let revision = server.pin(host.id, ["contrast": "more"])
        settle { host.pins(on: first.nonce).contains { $0["revision"] as? Int == revision } && server.statuses(on: first.nonce).last == revision }
        XCTAssertFalse(server.requests(in: server.epoch).contains { $0.contains("/\(server.epoch)/\(foreign)/") }, "a generation for another program is not fetched")
        XCTAssertEqual(host.applied.count, 1)
        XCTAssertNotNil(host.connection?.terminal)
        XCTAssertEqual(server.opened(in: server.epoch), 1)

        // Restarted on the other program (the gate's case): its envelope is
        // read and refused, nothing fetched, but the host subscribes and
        // registers again, and its pin is replayed.
        server.stop()
        try server.start(program: Self.other, plan: Data("plan two".utf8))
        settle { server.registers(in: server.epoch).count == 1 && replayed(host, server, on: server.registers(in: server.epoch)[0].nonce, revision: revision) }
        let again = try XCTUnwrap(server.registers(in: server.epoch).first)
        XCTAssertEqual(again.body["id"] as? String, host.id)
        XCTAssertEqual(host.pins(on: again.nonce).last?["pin"] as? [String: String], ["contrast": "more"])
        XCTAssertTrue(server.requests(in: server.epoch).contains { $0.hasSuffix("/1/exact.json") })
        XCTAssertFalse(server.requests(in: server.epoch).contains { $0.hasSuffix("/app.plan") })
        XCTAssertEqual(host.applied.count, 1)
        XCTAssertNotNil(host.connection?.terminal)

        // Back on the host's program: its next generation is taken, on the
        // stream it kept, and the rebuild status clears.
        let own = server.publish(program: Self.program, plan: Data("plan three".utf8))
        settle { host.applied.count == 2 }
        XCTAssertEqual(host.applied.last, server.identity(seq: own))
        XCTAssertNil(host.connection?.terminal)
        XCTAssertEqual(server.opened(in: server.epoch), 1)
    }

    #endif

    /// A loop whose producers are starting answers its dev status, not a
    /// plan (dtool2's pass 3 #8, theme2's condition c): the host says it once
    /// however often it asks, keeps asking, and takes the generation once
    /// there is one, the status gone; a start that failed is said once too,
    /// as the failure it is.
    func testAStartingLoopIsSaidOnceAndThePlanThatFollowsIsTaken() throws {
        let server = try DevServerStub()
        defer { server.stop() }
        try server.start(program: Self.program, plan: Data("plan one".utf8))
        server.status = ("503 Service Unavailable", #"{"exact":1,"status":"preparing","message":"building the module producer"}"#)
        var said: [String] = [], shown: [PlanURL.DevStatus?] = []
        let host = Host.open(server.page) { connection in
            connection.log = { said.append($0) }
            connection.onDevStatus = { shown.append($0) }
        }
        defer { host.close() }
        settle { server.requests(in: server.epoch).filter { $0 == "GET /" }.count >= 3 }
        XCTAssertEqual(said.filter { $0.contains("building the module producer") }, ["dev loop: building the module producer"])
        XCTAssertFalse(said.contains { $0.contains("cannot reach") }, "\(said)")
        XCTAssertEqual(host.connection?.devStatus, .preparing("building the module producer"))
        XCTAssertEqual(host.applied, [])
        server.status = nil
        settle { host.applied.count == 1 }
        XCTAssertNil(host.connection?.devStatus)
        XCTAssertEqual(shown, [.preparing("building the module producer"), nil])

        // A start that failed: its error, said once, and asked again later.
        server.status = ("500 Internal Server Error", #"{"exact":1,"status":"failed","message":"the resident loop exited 1: error: could not compile"}"#)
        host.connection?.reload()
        settle { host.connection?.devStatus == .failed("the resident loop exited 1: error: could not compile") }
        settle { server.requests(in: server.epoch).filter { $0 == "GET /" }.count >= 6 }
        XCTAssertEqual(said.filter { $0.contains("failed to start") }.count, 1, "\(said)")
    }

    /// The pin `revision` replayed on stream `nonce`: a pin frame the host
    /// took on that stream itself, and the status it sent for it, after the
    /// one its hello sent (which names the revision it held already), taken
    /// there.
    private func replayed(_ host: Host, _ server: DevServerStub, on nonce: String, revision: Int) -> Bool {
        host.pins(on: nonce).contains { $0["revision"] as? Int == revision }
            && server.statuses(on: nonce).count >= 2 && server.statuses(on: nonce).last == revision
    }

    /// Runs the main run loop, where the connection's callbacks land, until
    /// `predicate` holds; the deadline only stops a test whose event can
    /// never come.
    private func settle(file: StaticString = #file, line: UInt = #line, _ predicate: () -> Bool) {
        let hang = Date(timeIntervalSinceNow: 20)
        while !predicate() && Date() < hang { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertTrue(predicate(), file: file, line: line)
    }
}

/// The app's side of a connection, as Session's `connect` and
/// `ExactApp.channel` answer it: the first program asked about is pinned, a
/// generation applied is the one it runs, a hello registers under the
/// process's id with a status after, and a pin is taken and its revision
/// reported.
private final class Host {
    let id = "macos-reconnect-test"
    var connection: PlanURL?
    private var program: String?
    private var running: String?
    private(set) var applied: [String] = []
    /// Each pin frame taken, with the nonce of the stream it came on.
    private var pins: [(nonce: String?, frame: [String: Any])] = []
    func pins(on nonce: String) -> [[String: Any]] { pins.filter { $0.nonce == nonce }.map(\.frame) }
    private var revision: Int?

    static func open(_ page: String, _ configure: (PlanURL) -> Void = { _ in }) -> Host {
        let host = Host()
        host.connection = PlanURL.open(page, acceptProgram: { [weak host] program in
            guard let host else { return false }
            if let old = host.program { return old == program }
            host.program = program
            return true
        }, current: { [weak host] in host?.running }, plan: { [weak host] in host?.running }, channel: { [weak host] in host?.channel($0) }, apply: { [weak host] candidate, _, _ in
            host?.running = candidate.identity
            host?.applied.append(candidate.identity)
            return true
        })
        host.connection.map(configure)
        return host
    }

    func close() { connection?.close() }

    private func channel(_ frame: [String: Any]) {
        if frame["hello"] as? Bool == true {
            connection?.report(["kind": "register", "id": id, "host": BuildChannel.host, "capabilities": ["pin"], "design": NSNull()])
        } else if frame["kind"] as? String == "pin" {
            pins.append((connection?.nonce, frame))
            revision = (frame["revision"] as? NSNumber)?.intValue
        } else {
            return
        }
        connection?.report(["kind": "status", "revision": revision ?? NSNull()])
    }
}

/// A loopback stand-in for the dev server (host/web/dev.mjs) as PlanURL
/// reads it: the page and its envelope link, each generation's envelope and
/// plan, the `/__dev` stream (a hello with a nonce, then announcements and
/// commands), and `/__dev/report`, which answers a register with the pin it
/// holds for the build's id, as scripts/tamber-builds.mjs does. `stop` drops
/// every connection and the listener; `start` listens again on the same
/// port as a new process: a new epoch, its sequence from 1. Its pins outlive
/// a restart, as a channel kept by a process the restart does not take down.
final class DevServerStub: @unchecked Sendable {
    struct Report { let epoch: String; let status: Int; let body: [String: Any]
        var kind: String? { body["kind"] as? String }
        var nonce: String { body["nonce"] as? String ?? "" }
    }
    private struct Generation { let program: String; let plan: Data; let sha: String; let identity: String }

    private let lock = NSLock()
    private var listener: Int32 = -1
    private var stopping = false
    private var accepting: DispatchSemaphore?
    /// Accepted sockets a worker still owns, and the workers: `stop` shuts
    /// the sockets and drains the workers before a restart can follow.
    private var clients = Set<Int32>()
    private let workers = DispatchGroup()
    private var refusing = false
    private var refusedCount = 0
    /// Connections reset while the server was down (`startRefusing`).
    var refused: Int { lock.lock(); defer { lock.unlock() }; return refusedCount }
    private(set) var port: UInt16 = 0
    private var currentEpoch = ""
    private var generations: [Generation] = []
    private var streams: [String: Int32] = [:]
    private var builds: [String: String] = [:]
    private var pins: [String: (revision: Int, pin: Any)] = [:]
    private var revisions = 0
    private var seen: [(epoch: String, line: String)] = []
    private var reported: [Report] = []
    private var subscribed: [String] = []

    var page: String { "http://127.0.0.1:\(port)/" }
    /// A dev status the page answers instead of itself (a loop whose producers start).
    var status: (line: String, body: String)? {
        get { lock.lock(); defer { lock.unlock() }; return statusAnswer }
        set { lock.lock(); statusAnswer = newValue; lock.unlock() }
    }
    private var statusAnswer: (line: String, body: String)?
    var epoch: String { lock.lock(); defer { lock.unlock() }; return currentEpoch }
    var reports: [Report] { lock.lock(); defer { lock.unlock() }; return reported }
    func registers(in epoch: String) -> [Report] { reports.filter { $0.epoch == epoch && $0.kind == "register" && $0.status == 200 } }
    /// The revisions the statuses taken on `nonce` name, in order (nil for
    /// none).
    func statuses(on nonce: String) -> [Int?] { reports.filter { $0.nonce == nonce && $0.kind == "status" && $0.status == 200 }.map { $0.body["revision"] as? Int } }
    func requests(in epoch: String) -> [String] { lock.lock(); defer { lock.unlock() }; return seen.filter { $0.epoch == epoch }.map(\.line) }
    func opened(in epoch: String) -> Int { lock.lock(); defer { lock.unlock() }; return subscribed.filter { $0 == epoch }.count }
    func identity(seq: Int) -> String { lock.lock(); defer { lock.unlock() }; return generations[seq - 1].identity }

    init() throws {}

    func start(program: String, plan: Data) throws {
        try listen(refusing: false) {
            currentEpoch = UUID().uuidString.lowercased()
            generations = []
        }
        _ = add(program: program, plan: plan)
    }

    /// The port down, but watched: each connection is reset unanswered, as
    /// a refused one fails, and counted.
    func startRefusing() throws { try listen(refusing: true) {} }

    private func listen(refusing: Bool, _ reset: () -> Void) throws {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { throw NSError(domain: "socket", code: 1) }
        var reuse: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &reuse, socklen_t(MemoryLayout<Int32>.size))
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size); address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1"); address.sin_port = port.bigEndian
        let bound = withUnsafePointer(to: &address) { p in
            p.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        }
        guard bound == 0, Darwin.listen(fd, 16) == 0 else { close(fd); throw NSError(domain: "listen", code: Int(errno)) }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &address) { p in p.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) } }
        // Polled, so `stop` ends the loop before the descriptor is closed and
        // its number can be another listener's.
        _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        lock.lock()
        port = UInt16(bigEndian: address.sin_port)
        listener = fd
        stopping = false
        self.refusing = refusing
        reset()
        lock.unlock()
        let done = DispatchSemaphore(value: 0)
        accepting = done
        DispatchQueue.global().async { [self] in
            while true {
                lock.lock(); let stop = stopping; lock.unlock()
                if stop { break }
                var ready = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
                guard poll(&ready, 1, 20) > 0 else { continue }
                let client = accept(fd, nil, nil)
                guard client >= 0 else { continue }
                lock.lock()
                if self.refusing {
                    refusedCount += 1
                    lock.unlock()
                    var reset = linger(l_onoff: 1, l_linger: 0)
                    setsockopt(client, SOL_SOCKET, SO_LINGER, &reset, socklen_t(MemoryLayout<linger>.size))
                    close(client)
                    continue
                }
                clients.insert(client)
                lock.unlock()
                // Blocking, as the listener's O_NONBLOCK is inherited.
                _ = fcntl(client, F_SETFL, fcntl(client, F_GETFL) & ~O_NONBLOCK)
                workers.enter()
                DispatchQueue.global().async { self.serve(client); self.workers.leave() }
            }
            done.signal()
        }
    }

    func stop() {
        lock.lock(); stopping = true; lock.unlock()
        accepting?.wait()
        accepting = nil
        // A worker blocked reading wakes; one about to open a stream sees
        // `stopping` and closes instead.
        lock.lock()
        for fd in clients { shutdown(fd, SHUT_RDWR) }
        lock.unlock()
        workers.wait()
        lock.lock()
        if listener >= 0 { close(listener); listener = -1 }
        lock.unlock()
        dropStreams()
    }

    /// A worker is done with its socket: it leaves the set and is closed
    /// under the lock, so `stop` never shuts a reused descriptor.
    private func finish(_ client: Int32) {
        lock.lock(); defer { lock.unlock() }
        clients.remove(client)
        close(client)
    }

    /// Every open stream ends, as when the server's process goes.
    func dropStreams() {
        lock.lock(); defer { lock.unlock() }
        for fd in streams.values { shutdown(fd, SHUT_RDWR); close(fd) }
        streams.removeAll()
        builds.removeAll()
    }

    /// A frame on every open stream.
    func push(_ frame: [String: Any]) {
        lock.lock(); defer { lock.unlock() }
        for fd in streams.values { write(frame, to: fd) }
    }

    /// The design tool's `pin` for build `id`: held for it and sent to the
    /// stream it registered on. Its revision.
    @discardableResult
    func pin(_ id: String, _ pin: [String: Any]) -> Int {
        lock.lock(); defer { lock.unlock() }
        revisions += 1
        pins[id] = (revisions, pin)
        for (nonce, build) in builds where build == id { if let fd = streams[nonce] { write(["kind": "pin", "revision": revisions, "pin": pin], to: fd) } }
        return revisions
    }

    /// A new generation, announced on every stream. Its sequence.
    func publish(program: String, plan: Data) -> Int {
        let seq = add(program: program, plan: plan)
        lock.lock()
        let frame = announcement(seq)
        lock.unlock()
        push(frame)
        return seq
    }

    private func add(program: String, plan: Data) -> Int {
        let sha = SHA256.hash(data: plan).map { String(format: "%02x", $0) }.joined()
        // The envelope's identity, as PlanURL and the server compute it: the
        // canonical manifest of a plan with no assets.
        let canonical = "{\"assets\":[],\"plan\":{\"bytes\":\(plan.count),\"sha256\":\"\(sha)\"}}"
        let identity = SHA256.hash(data: Data(canonical.utf8)).map { String(format: "%02x", $0) }.joined()
        lock.lock(); defer { lock.unlock() }
        generations.append(Generation(program: program, plan: plan, sha: sha, identity: identity))
        return generations.count
    }

    /// The latest or `seq`'s announcement, as hello and the stream carry it.
    private func announcement(_ seq: Int) -> [String: Any] {
        let g = generations[seq - 1]
        return ["epoch": currentEpoch, "program": g.program, "seq": seq, "generation": g.identity, "digest": g.sha,
                "envelope": "/__dev/generation/\(currentEpoch)/\(seq)/exact.json"]
    }

    private func serve(_ client: Int32) {
        var timeout = timeval(tv_sec: 5, tv_usec: 0), noPipe: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
        guard let request = read(client) else { finish(client); return }
        let (method, path, body) = request
        lock.lock()
        seen.append((currentEpoch, "\(method) \(path)"))
        let latest = generations.count
        lock.unlock()
        let parts = path.split(separator: "/").map(String.init)
        switch (method, path) {
        case ("GET", "/") where status != nil:
            let answer = status!
            respond(client, answer.line, PlanURL.devStatusType, Data(answer.body.utf8))
        case ("GET", "/"):
            lock.lock(); let envelope = announcement(latest)["envelope"] as! String; lock.unlock()
            let html = "<!doctype html><html><head><link rel=\"alternate\" type=\"\(PlanURL.envelopeType)\" href=\"\(envelope)\"></head></html>"
            respond(client, "200 OK", "text/html", Data(html.utf8))
        case ("GET", "/__dev"):
            let nonce = (0..<16).map { _ in String(format: "%02x", UInt8.random(in: 0...255)) }.joined()
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nTransfer-Encoding: chunked\r\n\r\n"
            lock.lock(); defer { lock.unlock() }
            clients.remove(client)
            guard !stopping, send(Data(head.utf8), to: client) else { close(client); return }
            var hello = announcement(latest)
            hello["hello"] = true
            hello["nonce"] = nonce
            streams[nonce] = client
            subscribed.append(currentEpoch)
            write(hello, to: client)
            return
        case ("POST", "/__dev/report"):
            report(client, body)
        case ("GET", _) where parts.count == 5 && parts[0] == "__dev" && parts[1] == "generation":
            lock.lock()
            let g = parts[2] == currentEpoch ? Int(parts[3]).flatMap { (1...generations.count).contains($0) ? generations[$0 - 1] : nil } : nil
            let dev = g.map { _ in announcement(Int(parts[3])!) }
            lock.unlock()
            guard let g, let dev else { respond(client, "404 Not Found", "text/plain", Data()); break }
            if parts[4] == "exact.json" {
                let envelope: [String: Any] = ["exact": 1, "assets": [] as [Any],
                                               "plan": ["url": "app.plan", "sha256": g.sha, "bytes": g.plan.count],
                                               "dev": ["epoch": dev["epoch"]!, "program": g.program, "seq": dev["seq"]!, "generation": g.identity, "events": "/__dev"]]
                respond(client, "200 OK", PlanURL.envelopeType, try! JSONSerialization.data(withJSONObject: envelope))
            } else if parts[4] == "app.plan" {
                respond(client, "200 OK", "application/octet-stream", g.plan)
            } else {
                respond(client, "404 Not Found", "text/plain", Data())
            }
        default:
            // `/__dev/design` among them: no live table here.
            respond(client, "404 Not Found", "text/plain", Data())
        }
        finish(client)
    }

    /// As tamber-builds.mjs: a nonce no open stream holds is 403, a report
    /// before a register 409; a register is answered on its stream with the
    /// pin held for its id (revision 0 and null when there is none).
    private func report(_ client: Int32, _ data: Data) {
        let body = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
        lock.lock()
        let nonce = body["nonce"] as? String ?? ""
        let status: Int
        if streams[nonce] == nil {
            status = 403
        } else if body["kind"] as? String == "register", let id = body["id"] as? String {
            builds[nonce] = id
            let held = pins[id]
            write(["kind": "pin", "revision": held?.revision ?? 0, "pin": held?.pin ?? NSNull()], to: streams[nonce]!)
            status = 200
        } else {
            status = builds[nonce] == nil ? 409 : 200
        }
        reported.append(Report(epoch: currentEpoch, status: status, body: body))
        lock.unlock()
        respond(client, status == 200 ? "200 OK" : "\(status) Refused", "application/json", Data("{}".utf8))
    }

    /// One request: its method, path and body (to its Content-Length).
    private func read(_ client: Int32) -> (String, String, Data)? {
        var buffer = [UInt8](repeating: 0, count: 4096), request = Data()
        var end: Range<Data.Index>?
        while end == nil, request.count < 65536 {
            let count = recv(client, &buffer, buffer.count, 0)
            if count <= 0 { return nil }
            request.append(contentsOf: buffer.prefix(count))
            end = request.range(of: Data("\r\n\r\n".utf8))
        }
        guard let end else { return nil }
        let head = String(decoding: request[..<end.lowerBound], as: UTF8.self).components(separatedBy: "\r\n")
        let line = head[0].split(separator: " ").map(String.init)
        guard line.count >= 2 else { return nil }
        let length = head.dropFirst().compactMap { field -> Int? in
            let pair = field.split(separator: ":", maxSplits: 1)
            return pair.count == 2 && pair[0].lowercased() == "content-length" ? Int(pair[1].trimmingCharacters(in: .whitespaces)) : nil
        }.first ?? 0
        var body = Data(request[end.upperBound...])
        while body.count < length {
            let count = recv(client, &buffer, buffer.count, 0)
            if count <= 0 { return nil }
            body.append(contentsOf: buffer.prefix(count))
        }
        return (line[0], line[1].components(separatedBy: "?")[0], body)
    }

    private func respond(_ client: Int32, _ status: String, _ type: String, _ body: Data) {
        let head = "HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
        _ = send(Data(head.utf8) + body, to: client)
    }

    /// One SSE frame as one chunk. The caller holds the lock.
    private func write(_ frame: [String: Any], to fd: Int32) {
        let event = Data("data: ".utf8) + (try! JSONSerialization.data(withJSONObject: frame)) + Data("\n\n".utf8)
        _ = send(Data(String(event.count, radix: 16).utf8) + Data("\r\n".utf8) + event + Data("\r\n".utf8), to: fd)
    }

    private func send(_ data: Data, to fd: Int32) -> Bool {
        data.withUnsafeBytes { bytes in
            var offset = 0
            while offset < bytes.count {
                let sent = Darwin.send(fd, bytes.baseAddress!.advanced(by: offset), bytes.count - offset, 0)
                if sent <= 0 { return false }
                offset += sent
            }
            return true
        }
    }
}
#endif
