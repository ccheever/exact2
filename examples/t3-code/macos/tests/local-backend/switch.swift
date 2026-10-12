import AppKit
import Foundation
import XCTest

// 20261005-local-primary-environment: the Local environment switch's stopgap (decision U4; exact2
// cannot relaunch, issue X45 / #122), the primary's descriptor in the status, the switch read at
// launch (desktop-settings.json since decision U7: settings.swift; the failed-start test starts from an
// old t3-code.json, which the attach carries over), and `handleFatalStartupError` (T3LocalFatal.swift). Each test runs its own
// T3LocalBackend over a real child (a fake `t3` shell script on a lane port) with an HTTP double for
// the token exchange and the descriptor; readiness answers at once (FakeProber).
private final class LocalServer: URLProtocol, @unchecked Sendable {
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let body: [String: Any] = request.url!.path == "/oauth/token"
            ? ["access_token": "local-bearer", "token_type": "Bearer"]
            : ["environmentId": "local-env", "label": "Lane Mac", "serverVersion": "0.0.46-nightly.20261005.2667"]
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["content-type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: body))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

final class LocalSwitchTests: XCTestCase {
    private final class Owner {}
    private let owner = Owner()

    private func scratch(_ name: String) -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-switch-\(name)-\(UUID().uuidString)", isDirectory: true)
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }
    /// A runtime folder whose `t3` runs `body` after `--version`.
    private func runtime(_ body: String) -> URL {
        let directory = scratch("runtime"), script = directory.appendingPathComponent("t3")
        try! "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 't3 v0.0.46-nightly.20261005.2667'; exit 0; fi\n\(body)\n".write(to: script, atomically: true, encoding: .utf8)
        try! FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
        return directory
    }
    private func backend(runtime: URL, port: Int = 16897, dataRoot: URL, answers: Bool = true) -> T3LocalBackend {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [LocalServer.self]
        let backend = T3LocalBackend(session: URLSession(configuration: configuration))
        let home = scratch("home")
        backend.environment = { ["T3_LOCAL_HOME": home.path, "T3_LOCAL_PORT": "\(port)", "T3_LOCAL_RUNTIME_DIR": runtime.path, "PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.serverEnvironment = { $0 }
        backend.makeProber = { let prober = FakeProber(); if !answers { prober.fallback = nil }; return prober }
        backend.fatal = { stage, message in XCTFail("unexpected fatal \(stage): \(message)") }
        return backend
    }
    private func until(_ seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { if condition() { return true }; Thread.sleep(forTimeInterval: 0.02) }
        return condition()
    }
    private func setEnabled(_ backend: T3LocalBackend, _ enabled: Bool, timeout: TimeInterval = 15) -> (String?, TimeInterval) {
        let done = DispatchSemaphore(value: 0), started = Date()
        var failure: String?
        backend.setEnabled(enabled) { failure = $0; done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "setEnabled(\(enabled)) did not answer")
        return (failure, Date().timeIntervalSince(started))
    }
    private func alive(_ pid: Int) -> Bool { kill(pid_t(pid), 0) == 0 }

    func testTurningOffStopsAServerThatIgnoresSIGTERMWithinTheBound() {
        // The double ignores SIGTERM: the stop waits 2 s, then SIGKILL (the reference's 2 s grace).
        // Readiness answers at once (FakeProber), so the stop could reach the shell before its `trap`
        // ran and end it on the SIGTERM (~1 run in 5). The double marks the trap; the stop waits for it.
        let trapped = scratch("trap").appendingPathComponent("trapped")
        let backend = backend(runtime: runtime("trap '' TERM\n: > '\(trapped.path)'\nwhile :; do sleep 0.1; done"), dataRoot: scratch("data"))
        backend.attach(owner, dataRoot: scratch("data"), changed: { _ in })
        XCTAssertTrue(until(10) { backend.statusValue()["state"] as? String == "ready" && backend.statusValue()["bearerReady"] as? Bool == true }, "\(backend.statusValue())")
        XCTAssertTrue(until(5) { backend.statusValue()["environmentId"] as? String == "local-env" }, "the descriptor names the primary")
        XCTAssertEqual(backend.statusValue()["label"] as? String, "Lane Mac")
        let pid = backend.statusValue()["pid"] as? Int ?? 0
        XCTAssertTrue(pid > 1 && alive(pid))
        XCTAssertTrue(until(5) { FileManager.default.fileExists(atPath: trapped.path) }, "the double ignores SIGTERM")
        let (failure, elapsed) = setEnabled(backend, false)
        XCTAssertNil(failure)
        XCTAssertGreaterThanOrEqual(elapsed, 1.9, "SIGTERM is ignored for 2 s")
        XCTAssertLessThan(elapsed, 5.5, "the stop is bounded")
        XCTAssertTrue(until(2) { !self.alive(pid) }, "the process is gone")
        XCTAssertEqual(backend.statusValue()["lastExit"] as? String, "signal=9", "SIGKILL ended it")
        XCTAssertEqual(backend.statusValue()["state"] as? String, "stopped")
        XCTAssertEqual(backend.statusValue()["enabled"] as? Bool, false)
        // Turning it on again starts a new process and answers once it is ready.
        let (again, _) = setEnabled(backend, true)
        XCTAssertNil(again)
        let next = backend.statusValue()["pid"] as? Int ?? 0
        XCTAssertTrue(next > 1 && next != pid && alive(next), "a new pid")
        XCTAssertEqual(backend.statusValue()["state"] as? String, "ready")
        XCTAssertEqual(backend.statusValue()["enabled"] as? Bool, true)
        backend.detach(owner)
        XCTAssertTrue(until(6) { !self.alive(next) })
    }

    func testAFailedStartAnswersWhyAndLeavesTheServerStopped() {
        let data = scratch("data")
        try! #"{"version":1,"localEnvironmentEnabled":false}"#.write(to: data.appendingPathComponent("t3-code.json"), atomically: true, encoding: .utf8)
        let backend = backend(runtime: runtime("exit 1"), dataRoot: data, answers: false)
        backend.attach(owner, dataRoot: data, changed: { _ in })
        // Switched off at launch: nothing starts.
        XCTAssertTrue(until(3) { backend.statusValue()["enabled"] as? Bool == false }, "\(backend.statusValue())")
        Thread.sleep(forTimeInterval: 0.3)
        XCTAssertEqual(backend.statusValue()["state"] as? String, "stopped")
        XCTAssertTrue(backend.statusValue()["pid"] == nil || backend.statusValue()["pid"] is NSNull)
        let (failure, _) = setEnabled(backend, true)
        XCTAssertEqual(failure, "The local server stopped before it was ready (code=1).")
        XCTAssertTrue(until(6) { backend.statusValue()["state"] as? String == "stopped" }, "no restart ladder after a failed switch-on: \(backend.statusValue())")
        backend.detach(owner)
    }

    func testARefusedDevelopmentBuildCannotBeTurnedOn() {
        let backend = T3LocalBackend()
        backend.environment = { ["PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.attach(owner, dataRoot: scratch("data"), changed: { _ in })
        XCTAssertTrue(until(3) { backend.statusValue()["state"] as? String == "refused" })
        let (failure, _) = setEnabled(backend, true)
        XCTAssertEqual(failure, T3LocalPolicy.developmentMissing)
        backend.detach(owner)
    }

    // MARK: handleFatalStartupError

    func testPortExhaustionIsFatalWithTheStageAndTheMessageThenQuits() {
        let resources = scratch("resources"), home = scratch("packaged-home")
        try! #"{"flavor":"packaged"}"#.write(to: resources.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        let backend = T3LocalBackend()
        backend.environment = { ["T3CODE_HOME": home.path, "PATH": "/usr/bin:/bin"] }
        backend.resources = { resources }
        backend.canListen = { _, _ in false }
        let fatal = expectation(description: "fatal")
        var recorded: [String] = []
        backend.fatal = { stage, message in recorded = [stage, message]; fatal.fulfill() }
        backend.attach(owner, dataRoot: scratch("data"), changed: { _ in })
        wait(for: [fatal], timeout: 5)
        XCTAssertEqual(recorded, ["bootstrap", "No desktop backend port is available on hosts 127.0.0.1, 0.0.0.0, :: between 3773 and 65535."])
        XCTAssertEqual(backend.statusValue()["state"] as? String, "failed")
        XCTAssertFalse(FileManager.default.fileExists(atPath: home.appendingPathComponent("runtime").path), "nothing unpacked before the port")
        backend.detach(owner)
    }

    func testTheFatalHandlerAlertsOnceWithTheReferenceTextThenQuits() {
        T3LocalFatal.reset()
        var events: [String] = []
        let previousAlert = T3LocalFatal.alert, previousQuit = T3LocalFatal.quit
        defer { T3LocalFatal.alert = previousAlert; T3LocalFatal.quit = previousQuit; T3LocalFatal.reset() }
        T3LocalFatal.alert = { title, detail in events.append("alert \(title) | \(detail)") }
        T3LocalFatal.quit = { events.append("quit") }
        T3LocalFatal.handle(stage: "bootstrap", message: "No desktop backend port is available.")
        T3LocalFatal.handle(stage: "bootstrap", message: "A second report while quitting.")
        XCTAssertEqual(events, ["alert T3 Code failed to start | Stage: bootstrap\nNo desktop backend port is available.", "quit"])
    }

    func testReturnAndEscapeBothDismissTheFatalAlert() {
        _ = NSApplication.shared
        for (key, code, characters) in [("Return", UInt16(36), "\r"), ("Escape", UInt16(53), "\u{1b}")] {
            let started = Date()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                guard let window = NSApp.modalWindow,
                      let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                   windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters,
                                                   isARepeat: false, keyCode: code) else { return }
                NSApp.postEvent(event, atStart: false)
            }
            // A stuck alert would hang here: a watchdog stops the modal after 5 s and the test fails.
            var timedOut = false
            DispatchQueue.main.asyncAfter(deadline: .now() + 5) { if NSApp.modalWindow != nil { timedOut = true; NSApp.abortModal() } }
            T3LocalFatal.present(title: T3LocalFatal.title, detail: T3LocalFatal.detail(stage: "bootstrap", message: key))
            XCTAssertFalse(timedOut, "\(key) did not dismiss the alert")
            XCTAssertLessThan(Date().timeIntervalSince(started), 4, key)
        }
    }
}
