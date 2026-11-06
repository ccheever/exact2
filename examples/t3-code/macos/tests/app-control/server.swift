import Darwin
import Foundation
import XCTest

// The control socket. The first four port apps/desktop/src/app/DesktopAppActivation.test.ts (T3 Code,
// MIT, see LICENSE-T3; reference 1e2ecbd975) by name; the rest cover the rest of
// startDesktopAppControlServer's contract: the invalid answers, the line deadline, the modes, and
// the watch that binds the address again on its own.
final class AppControlServerTests: XCTestCase {
    func test_roundtripsARequestAndRemovesItsSocketOnShutdown() throws {
        try named("roundtrips a request and removes its socket on shutdown") {
            let root = shortRoot(), target = target(root)
            var received: [[String: Any]] = []
            let server = try startServer(target, handle: { request, done in received.append(request); done(success(request)) })
            let response = exchange(target.address, request("request-1"))
            XCTAssertEqual(received.count, 1)
            XCTAssertEqual(response?["ok"] as? Bool, true)
            XCTAssertEqual(response?["requestId"] as? String, "request-1")
            closeServer(server)
            XCTAssertFalse(exists(target.address))
            try? FileManager.default.removeItem(atPath: root)
        }
    }

    func test_cancelsAQueuedRequestWhenTheClientDisconnects() throws {
        try named("cancels a queued request when the client disconnects") {
            let root = shortRoot(), target = target(root)
            let canceled = expectation(description: "canceled")
            var canceledId = ""
            let server = try startServer(target, handle: { _, _ in }, cancel: { id in canceledId = id; canceled.fulfill() })
            let fd = try XCTUnwrap(connectClient(target.address))
            send(fd, String(decoding: try JSONSerialization.data(withJSONObject: request("request-canceled")), as: UTF8.self) + "\n")
            close(fd)
            wait(for: [canceled], timeout: 3)
            XCTAssertEqual(canceledId, "request-canceled")
            closeServer(server)
            try? FileManager.default.removeItem(atPath: root)
        }
    }

    // Two apps can share one state dir, such as nightly and a preview build.
    func test_keepsANewerAppsSocketWhenAnOlderAppOnTheSameStateDirQuits() throws {
        try named("keeps a newer app's socket when an older app on the same state dir quits") {
            let root = shortRoot(), target = target(root)
            let older = try startServer(target)
            let newer = try startServer(target)
            closeServer(older)
            let response = exchange(target.address, request("after-quit"))
            XCTAssertEqual(response?["ok"] as? Bool, true)
            XCTAssertEqual(response?["requestId"] as? String, "after-quit")
            closeServer(newer)
            try? FileManager.default.removeItem(atPath: root)
        }
    }

    func test_bindsItsAddressAgainAfterTheSocketFileIsRemoved() throws {
        try named("binds its address again after the socket file is removed") {
            let root = shortRoot(), target = target(root)
            let server = try startServer(target)
            unlink(target.address)
            try serverQueue.sync { try server.reclaim() }
            let response = exchange(target.address, request("reclaimed"))
            XCTAssertEqual(response?["ok"] as? Bool, true)
            XCTAssertEqual(response?["requestId"] as? String, "reclaimed")
            closeServer(server)
            try? FileManager.default.removeItem(atPath: root)
        }
    }

    func test_answersInvalidLinesWithInvalidRequest() throws {
        let root = shortRoot(), target = target(root)
        var handled = 0
        let server = try startServer(target, handle: { request, done in handled += 1; done(success(request)) })
        let notJson = exchangeRaw(target.address, "{nope\n")
        XCTAssertEqual(notJson?["code"] as? String, "invalid-request")
        XCTAssertEqual(notJson?["requestId"] as? String, "invalid-request")
        XCTAssertEqual(notJson?["message"] as? String, "The desktop app request is not valid JSON.")
        var wrong = request("request-wrong"); wrong["type"] = "open-file"
        let invalid = exchange(target.address, wrong)
        XCTAssertEqual(invalid?["ok"] as? Bool, false)
        XCTAssertEqual(invalid?["requestId"] as? String, "request-wrong")
        XCTAssertEqual(invalid?["message"] as? String, "The desktop app request is invalid.")
        XCTAssertEqual(exchangeRaw(target.address, "[1,2]\n")?["requestId"] as? String, "invalid-request")
        XCTAssertEqual(handled, 0)
        closeServer(server)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_answersATooLargeRequestWithInvalidRequest() throws {
        let root = shortRoot(), target = target(root)
        let server = try startServer(target)
        let fd = try XCTUnwrap(connectClient(target.address))
        let big = String(repeating: "x", count: 70 * 1024)
        DispatchQueue.global().async { send(fd, big) }
        let line = readLine(fd).flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
        close(fd)
        XCTAssertEqual(line?["code"] as? String, "invalid-request")
        XCTAssertEqual(line?["message"] as? String, "The desktop app request is too large.")
        closeServer(server)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_dropsAClientThatSendsNoLineInTime() throws {
        let root = shortRoot(), target = target(root)
        let saved = T3AppControlServer.requestLineTimeout
        T3AppControlServer.requestLineTimeout = 0.2
        defer { T3AppControlServer.requestLineTimeout = saved }
        let server = try startServer(target)
        let fd = try XCTUnwrap(connectClient(target.address))
        send(fd, "{\"version\":1")
        let started = Date()
        XCTAssertNil(readLine(fd)) // closed without an answer
        XCTAssertLessThan(Date().timeIntervalSince(started), 3)
        close(fd)
        closeServer(server)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_ownsAPrivateDirectoryAndAPrivateSocket() throws {
        let root = shortRoot(), target = target(root)
        let server = try startServer(target)
        XCTAssertEqual(mode(target.directory), 0o700)
        XCTAssertEqual(mode(target.address), 0o600)
        var info = stat(); lstat(target.address, &info)
        XCTAssertEqual(info.st_mode & S_IFMT, S_IFSOCK)
        XCTAssertEqual((try? FileManager.default.contentsOfDirectory(atPath: target.directory))?.filter { $0.hasSuffix(".tmp") }, [])
        closeServer(server)
        // A symlinked socket directory is refused, as is one that is not a directory.
        let other = shortRoot(), elsewhere = shortRoot(), linked = T3AppControlAddress.resolve(stateDir: other + "/userdata", tempDir: other, userId: getuid())
        try FileManager.default.createSymbolicLink(atPath: linked.directory, withDestinationPath: elsewhere)
        XCTAssertThrowsError(try startServer(linked)) { XCTAssertTrue("\($0)".contains("is not a directory")) }
        for path in [root, other, elsewhere] { try? FileManager.default.removeItem(atPath: path) }
    }

    func test_bindsAgainOnItsOwnWithinASecondWhenTheFileVanishes() throws {
        let root = shortRoot(), target = target(root)
        let server = try startServer(target)
        unlink(target.address)
        let deadline = Date().addingTimeInterval(1)
        while !exists(target.address), Date() < deadline { usleep(10_000) }
        XCTAssertTrue(exists(target.address))
        XCTAssertEqual(exchange(target.address, request("watched"))?["ok"] as? Bool, true)
        closeServer(server)
        XCTAssertFalse(exists(target.address))
        try? FileManager.default.removeItem(atPath: root)
    }
}

// The broker. Ports apps/desktop/src/app/DesktopAppActivationBroker.test.ts by name, plus the
// duplicate id and the shutdown answers.
final class ActivationBrokerTests: XCTestCase {
    private let request: [String: Any] = ["version": 1, "requestId": "request-1", "type": "open-workspace", "workspaceRoot": "/workspace/project", "platform": "linux"]
    /// A clock in the test's hands: the scheduled timeouts, fired only when the test says so.
    private final class ManualClock {
        var scheduled: [(seconds: TimeInterval, fire: () -> Void, cancelled: Bool)] = []
        func schedule(_ seconds: TimeInterval, _ fire: @escaping () -> Void) -> () -> Void {
            let index = scheduled.count
            scheduled.append((seconds, fire, false))
            return { [weak self] in self?.scheduled[index].cancelled = true }
        }
        func advance(_ seconds: TimeInterval) { for entry in scheduled where !entry.cancelled && entry.seconds <= seconds { entry.fire() } }
    }
    private func broker(_ clock: ManualClock = ManualClock(), activate: @escaping () -> Void = {}) -> T3ActivationBroker {
        T3ActivationBroker(requestTimeout: 1, activate: activate, schedule: clock.schedule)
    }
    private final class Box { var value: [String: Any]? }

    func test_focusesImmediatelyAndWaitsForRendererReadiness() {
        named("focuses immediately and waits for renderer readiness") {
            var activated = 0, sent: [[String: Any]] = []
            let broker = broker(activate: { activated += 1 }), response = Box()
            broker.request(request) { response.value = $0 }
            XCTAssertEqual(activated, 1)
            XCTAssertTrue(sent.isEmpty)
            broker.registerRenderer { sent.append($0) }
            XCTAssertEqual(sent.first?["requestId"] as? String, "request-1")
            broker.complete(success(request))
            XCTAssertEqual(response.value?["ok"] as? Bool, true)
            XCTAssertEqual(response.value?["projectId"] as? String, "project-1")
            broker.close()
        }
    }

    func test_failsAnInFlightRequestWhenTheRendererGoesAway() {
        named("fails an in-flight request when the renderer goes away") {
            let broker = broker(), response = Box()
            broker.registerRenderer { _ in }
            broker.request(request) { response.value = $0 }
            broker.clearRenderer()
            XCTAssertEqual(response.value?["ok"] as? Bool, false)
            XCTAssertEqual(response.value?["code"] as? String, "renderer-unavailable")
            XCTAssertEqual(response.value?["message"] as? String, "The T3 Code window closed before it opened the project.")
            broker.close()
        }
    }

    func test_queuesRequestsAfterUnsubscribeUntilANewRendererRegisters() {
        named("queues requests after unsubscribe until a new renderer registers") {
            var previous: [[String: Any]] = [], next: [[String: Any]] = []
            let broker = broker(), response = Box()
            broker.registerRenderer { previous.append($0) }
            broker.clearRenderer()
            broker.request(request) { response.value = $0 }
            XCTAssertTrue(previous.isEmpty)
            XCTAssertTrue(next.isEmpty)
            broker.registerRenderer { next.append($0) }
            XCTAssertEqual(next.first?["requestId"] as? String, "request-1")
            broker.complete(success(request))
            XCTAssertEqual(response.value?["ok"] as? Bool, true)
            broker.close()
        }
    }

    func test_removesAQueuedRequestWhenItsCliConnectionCloses() {
        named("removes a queued request when its CLI connection closes") {
            var sent: [[String: Any]] = []
            let broker = broker(), response = Box()
            broker.request(request) { response.value = $0 }
            broker.cancel("request-1")
            broker.registerRenderer { sent.append($0) }
            XCTAssertEqual(response.value?["ok"] as? Bool, false)
            XCTAssertEqual(response.value?["code"] as? String, "renderer-unavailable")
            XCTAssertEqual(response.value?["message"] as? String, "The command closed before T3 Code was ready.")
            XCTAssertTrue(sent.isEmpty)
            broker.close()
        }
    }

    func test_neverSendsACanceledRequestThatWasQueuedBehindAnotherRequest() {
        named("never sends a canceled request that was queued behind another request") {
            var sent: [[String: Any]] = []
            let broker = broker(), first = Box(), second = Box()
            broker.registerRenderer { sent.append($0) }
            var secondRequest = request; secondRequest["requestId"] = "request-2"
            broker.request(request) { first.value = $0 }
            broker.request(secondRequest) { second.value = $0 }
            XCTAssertEqual(sent.count, 1)
            XCTAssertEqual(sent.last?["requestId"] as? String, "request-1")
            broker.cancel("request-2")
            broker.complete(success(request))
            XCTAssertEqual(first.value?["ok"] as? Bool, true)
            XCTAssertEqual(second.value?["ok"] as? Bool, false)
            XCTAssertEqual(sent.count, 1)
            broker.close()
        }
    }

    func test_timesOutARequestWithoutPolling() {
        named("times out a request without polling") {
            let clock = ManualClock()
            let broker = broker(clock), response = Box()
            broker.request(request) { response.value = $0 }
            // One deadline, scheduled once: nothing polls.
            XCTAssertEqual(clock.scheduled.map(\.seconds), [1])
            clock.advance(1)
            XCTAssertEqual(response.value?["ok"] as? Bool, false)
            XCTAssertEqual(response.value?["code"] as? String, "request-timeout")
            XCTAssertEqual(response.value?["message"] as? String, "The desktop app did not finish opening the project in time.")
            broker.close()
        }
    }

    func test_aDuplicateIdIsInvalidAndShutdownAnswersEveryRequest() {
        let clock = ManualClock()
        let broker = broker(clock), first = Box(), duplicate = Box(), late = Box()
        broker.request(request) { first.value = $0 }
        broker.request(request) { duplicate.value = $0 }
        XCTAssertEqual(duplicate.value?["code"] as? String, "invalid-request")
        XCTAssertEqual(duplicate.value?["message"] as? String, "The request id is already in use.")
        XCTAssertNil(first.value)
        broker.close()
        XCTAssertEqual(first.value?["message"] as? String, "T3 Code is shutting down.")
        XCTAssertEqual(first.value?["code"] as? String, "renderer-unavailable")
        XCTAssertTrue(clock.scheduled.allSatisfy(\.cancelled))
        broker.request(request) { late.value = $0 }
        XCTAssertEqual(late.value?["message"] as? String, "T3 Code is shutting down.")
    }
}
