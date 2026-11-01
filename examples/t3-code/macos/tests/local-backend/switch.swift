import AppKit
import Foundation
import XCTest

// The Local environment switch's stopgap (exact2
// cannot relaunch, issues/closed/20261009-native-process-relaunch.md, #122), the primary's descriptor in the status, the switch read at
// launch (desktop-settings.json: settings.swift; the failed-start test starts from an
// old t3-code.json, which the attach carries over), a runtime pruned while the server is stopped, and
// `handleFatalStartupError` (T3LocalFatal.swift). Each test runs its own
// T3LocalBackend over a real child (a fake `t3` shell script on a fixed T3_LOCAL_PORT) with an HTTP double for
// the token exchange and the descriptor; readiness answers at once (FakeProber).
private final class LocalServer: URLProtocol, @unchecked Sendable {
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let body: [String: Any] = request.url!.path == "/oauth/token"
            ? ["access_token": "local-bearer", "token_type": "Bearer"]
            : ["environmentId": "local-env", "label": "Lane Mac", "serverVersion": "0.1.6"]
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
        try! "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 't3 v0.1.6'; exit 0; fi\n\(body)\n".write(to: script, atomically: true, encoding: .utf8)
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
        backend.serverEnvironment = { ($0, true) }
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
        // The double ignores SIGTERM: the stop waits 5 s, then SIGKILL (the reference's 5 s grace).
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
        XCTAssertGreaterThanOrEqual(elapsed, 4.9, "SIGTERM is ignored for 5 s")
        XCTAssertLessThan(elapsed, 5.5, "the stop is bounded")
        XCTAssertEqual(backend.statusValue()["state"] as? String, "stopped")
        XCTAssertEqual(backend.statusValue()["enabled"] as? Bool, false)
        // The bounded stop can answer before the old process's exit callback. Turning it back on
        // immediately waits for that teardown and the new process's readiness, without failing the start.
        let (again, _) = setEnabled(backend, true)
        XCTAssertNil(again)
        XCTAssertTrue(until(2) { !self.alive(pid) }, "the old process is gone")
        XCTAssertEqual(backend.statusValue()["lastExit"] as? String, "killed by SIGKILL", "SIGKILL ended it")
        let next = backend.statusValue()["pid"] as? Int ?? 0
        XCTAssertTrue(next > 1 && next != pid && alive(next), "a new pid")
        XCTAssertEqual(backend.statusValue()["state"] as? String, "ready")
        XCTAssertEqual(backend.statusValue()["enabled"] as? Bool, true)
        backend.detach(owner)
        XCTAssertTrue(until(6) { !self.alive(next) })
    }

    /// `t3 update` and a starting background service prune a runtime no process runs (74541bab17,
    /// pinnedRuntime.ts `prunePinnedRuntimes`): the next start unpacks the bundled archive again.
    func testARuntimePrunedWhileTheServerIsStoppedIsUnpackedAgainAtTheNextStart() {
        let fixture = Fixture(t3: "while :; do sleep 0.1; done"), data = scratch("data")
        let backend = backend(runtime: scratch("unused"), port: 16866, dataRoot: data)
        backend.environment = { ["T3_LOCAL_HOME": fixture.home.path, "T3_LOCAL_PORT": "16866", "PATH": "/usr/bin:/bin"] }
        backend.resources = { fixture.bundleDir.deletingLastPathComponent() }
        backend.startTimeout = 10
        backend.attach(owner, dataRoot: data, changed: { _ in })
        XCTAssertTrue(until(10) { backend.statusValue()["state"] as? String == "ready" }, "\(backend.statusValue())")
        let paths = T3LocalRuntime.paths(home: fixture.home, version: fixture.version)
        XCTAssertTrue(T3LocalRuntime.installed(paths, version: fixture.version))
        XCTAssertNil(setEnabled(backend, false).0)
        // The prune renames the runtime out of the way, then removes it.
        let pruned = fixture.versions.appendingPathComponent(".pruned-\(fixture.version)")
        try! FileManager.default.moveItem(at: paths.versionDir, to: pruned)
        try! FileManager.default.removeItem(at: pruned)
        XCTAssertNil(setEnabled(backend, true).0)
        XCTAssertEqual(backend.statusValue()["state"] as? String, "ready")
        XCTAssertTrue(T3LocalRuntime.installed(paths, version: fixture.version))
        let pid = backend.statusValue()["pid"] as? Int ?? 0
        backend.detach(owner)
        XCTAssertTrue(until(6) { !self.alive(pid) })
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

    func testALegacyMarkerAloneCannotAdmitADevelopmentBackend() {
        let resources = scratch("legacy-resources"), home = scratch("unadmitted-home")
        try! #"{"flavor":"packaged"}"#.write(to: resources.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        let backend = T3LocalBackend()
        backend.environment = { ["T3CODE_HOME": home.path, "T3CODE_PORT": "16497", "PATH": "/usr/bin:/bin"] }
        backend.bundleInfo = { [:] }
        backend.resources = { resources }
        backend.attach(owner, dataRoot: scratch("data"), changed: { _ in })
        XCTAssertTrue(until(3) { backend.statusValue()["state"] as? String == "refused" })
        XCTAssertEqual(backend.statusValue()["refused"] as? String, T3LocalPolicy.developmentMissing)
        XCTAssertFalse(FileManager.default.fileExists(atPath: home.appendingPathComponent("userdata").path))
        backend.detach(owner)
    }

    // MARK: handleFatalStartupError

    func testPortExhaustionIsFatalWithTheStageAndTheMessageThenQuits() {
        let resources = scratch("resources"), home = scratch("packaged-home")
        let backend = T3LocalBackend()
        backend.bundleInfo = { ["ExactProtocolHandlers": [["protocol": "t3code", "url": "/open?u=%s"]],
                               "CFBundleURLTypes": [["CFBundleURLSchemes": ["t3code"]]]] }
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
