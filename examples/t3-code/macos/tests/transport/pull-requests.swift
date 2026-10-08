import Foundation
import XCTest

// 20261005-pr-code-tab: the pull request diff's narrow POST (T3Transport+PullRequests.swift) against
// the R3 fixture server (r3.swift): the allow-listed body, the bearer, the reply, the refusals, and an
// identical read joining the one still out.
final class PullRequestDiffTransportTests: XCTestCase {
    private final class Seen: @unchecked Sendable {
        private let lock = NSLock(); private var list: [(method: String, body: [String: Any], authorization: String)] = []
        func add(_ entry: (method: String, body: [String: Any], authorization: String)) { lock.lock(); list.append(entry); lock.unlock() }
        var all: [(method: String, body: [String: Any], authorization: String)] { lock.lock(); defer { lock.unlock() }; return list }
    }
    private static func body(_ request: URLRequest) -> [String: Any] {
        var data = request.httpBody ?? Data()
        if data.isEmpty, let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let read = stream.read(&buffer, maxLength: buffer.count); if read <= 0 { break }; data.append(buffer, count: read) }
        }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }
    /// A healthy fixture whose diff endpoint answers one slice (after `delay`) and records each request.
    private func serve(_ seen: Seen, delay: TimeInterval = 0) {
        R3HTTP.reset { request in
            switch request.url!.path {
            case "/.well-known/t3/environment": return (200, ["environmentId": "fixture", "label": "Fixture", "orchestrationProtocolVersion": 2, "capabilities": ["connectionProbe": true]])
            case "/oauth/token": return (200, ["access_token": "fixture-access", "token_type": "Bearer"])
            case "/api/auth/session": return (200, ["authenticated": true, "scopes": ["orchestration:read", "orchestration:operate"]])
            case "/api/auth/websocket-ticket": return (200, ["ticket": "fixture-ticket"])
            case "/api/pull-requests/diff":
                seen.add((request.httpMethod ?? "", Self.body(request), request.value(forHTTPHeaderField: "Authorization") ?? ""))
                if delay > 0 { Thread.sleep(forTimeInterval: delay) }
                return (200, ["patch": "diff --git a/a.txt b/a.txt\n", "truncated": false, "nextCursor": "100", "omittedFileStats": []])
            default: return (404, [:])
            }
        }
    }
    private func perform(_ transport: T3Transport, _ request: [String: Any], timeout: TimeInterval = 5) -> [String: Any] {
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any] = [:]
        transport.perform(request) { result = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "\(request["op"] ?? "") did not answer")
        return result
    }
    private func connected(_ socket: R3Socket) -> (T3Transport, Int) {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [R3HTTP.self]
        let transport = T3Transport(persistent: false, configuration: config, signals: false, changed: { _ in })
        let opened = perform(transport, ["op": "connect", "origin": "http://127.0.0.1:\(socket.port)", "credential": "pairing"])
        XCTAssertEqual((opened["value"] as? [String: Any])?["state"] as? String, "connected", "\(opened)")
        return (transport, opened["generation"] as? Int ?? 0)
    }
    private let reference: [String: Any] = ["projectId": "project-1", "host": "github.com", "repository": "acme/web", "number": 114]

    func testThePullRequestDiffPostsOnlyItsFieldsWithTheBearerAndAnswersTheSlice() throws {
        let socket = try R3Socket(), seen = Seen()
        serve(seen)
        let (transport, generation) = connected(socket); defer { transport.destroy() }
        let answer = perform(transport, ["op": "prDiff", "generation": generation, "payload": reference.merging(["cursor": "100", "commit": "abc1234"]) { $1 }])
        XCTAssertEqual(answer["ok"] as? Bool, true, "\(answer)")
        XCTAssertEqual((answer["value"] as? [String: Any])?["nextCursor"] as? String, "100")
        XCTAssertEqual(seen.all.count, 1)
        XCTAssertEqual(seen.all.first?.method, "POST")
        XCTAssertEqual(seen.all.first?.authorization, "Bearer fixture-access")
        XCTAssertEqual(seen.all.first?.body as NSDictionary?, ["projectId": "project-1", "host": "github.com", "repository": "acme/web", "number": 114, "cursor": "100", "commit": "abc1234"] as NSDictionary)
    }

    func testThePullRequestDiffRefusesAnythingButItsFields() throws {
        let socket = try R3Socket(), seen = Seen()
        serve(seen)
        let (transport, generation) = connected(socket); defer { transport.destroy() }
        for payload: [String: Any] in [reference.merging(["path": "/api/other"]) { $1 }, reference.merging(["number": 0]) { $1 }, reference.merging(["number": true]) { $1 },
                                       reference.merging(["cursor": "  "]) { $1 }, ["repository": "acme/web", "number": 1]] {
            let refused = perform(transport, ["op": "prDiff", "generation": generation, "payload": payload])
            XCTAssertEqual(refused["ok"] as? Bool, false, "\(payload)")
            XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "Arguments")
        }
        XCTAssertEqual((perform(transport, ["op": "prDiff", "generation": generation + 1, "payload": reference])["error"] as? [String: Any])?["kind"] as? String, "stale")
        // The read-only HTTP interface stays read-only.
        XCTAssertEqual(perform(transport, ["op": "http", "path": "/api/pull-requests/diff", "method": "POST", "body": "{}", "generation": generation])["ok"] as? Bool, false)
        XCTAssertEqual(seen.all.count, 0)
    }

    func testAnIdenticalDiffReadJoinsTheOneStillOut() throws {
        let socket = try R3Socket(), seen = Seen()
        serve(seen, delay: 0.3)
        let (transport, generation) = connected(socket); defer { transport.destroy() }
        let first = DispatchSemaphore(value: 0), second = DispatchSemaphore(value: 0)
        var answers: [[String: Any]] = []
        let lock = NSLock()
        transport.perform(["op": "prDiff", "generation": generation, "payload": reference]) { reply in lock.lock(); answers.append(reply); lock.unlock(); first.signal() }
        transport.perform(["op": "prDiff", "generation": generation, "payload": reference]) { reply in lock.lock(); answers.append(reply); lock.unlock(); second.signal() }
        XCTAssertEqual(first.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(second.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(seen.all.count, 1, "the second read joined the first")
        XCTAssertEqual(answers.compactMap { $0["ok"] as? Bool }, [true, true])
    }
}
