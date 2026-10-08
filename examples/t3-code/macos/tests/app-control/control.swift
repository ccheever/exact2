import Darwin
import Foundation
import XCTest

// The address (ports packages/shared/src/desktopAppControl.test.ts's Unix case by name, and holds the
// macOS vectors desktop-activation.test.ts checks with Node's SHA-256), the protocol's shapes, and
// the app-level control: when the socket listens, and how a request reaches a window and back.
final class AppControlAddressTests: XCTestCase {
    func test_keepsUnixSocketPathsShortAndSeparatesDesktopStateDirectories() {
        named("keeps Unix socket paths short and separates desktop state directories") {
            let first = T3AppControlAddress.resolve(stateDir: "/home/user/\(String(repeating: "long/", count: 40))userdata", tempDir: "/tmp", userId: 1000)
            let second = T3AppControlAddress.resolve(stateDir: "/home/user/.t3/other/userdata", tempDir: "/tmp", userId: 1000)
            XCTAssertEqual(first.directory, "/tmp/t3code-1000")
            XCTAssertLessThan(first.address.utf8.count, 108)
            XCTAssertNotEqual(first.address, second.address)
        }
    }

    func test_theMacOSVectorsTheBunTestShares() {
        let lane = T3AppControlAddress.resolve(stateDir: "/Users/someone/.t3/userdata", tempDir: "/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T", userId: 501)
        XCTAssertEqual(lane.directory, "/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T/t3code-501")
        XCTAssertEqual(lane.address, "/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T/t3code-501/073e69d4582c50be74c1c2d0.sock")
        XCTAssertLessThan(lane.address.utf8.count, 104)
        XCTAssertEqual(T3AppControlAddress.resolve(stateDir: "/Users/someone/.t3/userdata", tempDir: "/tmp", userId: nil).directory, "/tmp/t3code-073e69d4582c")
        // Node's os.tmpdir() and path.join, which the CLI computes with.
        XCTAssertEqual(T3AppControlAddress.nodeTmpdir(["TMPDIR": "/var/folders/x/T/"]), "/var/folders/x/T")
        XCTAssertEqual(T3AppControlAddress.nodeTmpdir(["TMP": "/a/"]), "/a")
        XCTAssertEqual(T3AppControlAddress.nodeTmpdir([:]), "/tmp")
        XCTAssertEqual(T3AppControlAddress.normalize("/Users/me/lane//t3-home/./x/../userdata/"), "/Users/me/lane/t3-home/userdata")
    }
}

final class AppControlProtocolTests: XCTestCase {
    func test_requestShape() {
        XCTAssertNotNil(T3ActivationProtocol.request(request("r")))
        for (key, value) in [("version", 2 as Any), ("version", true as Any), ("requestId", "" as Any), ("type", "open-file" as Any),
                             ("workspaceRoot", 7 as Any), ("platform", "freebsd" as Any)] {
            var raw = request("r"); raw[key] = value
            XCTAssertNil(T3ActivationProtocol.request(raw), "\(key)=\(value)")
        }
        XCTAssertEqual(T3ActivationProtocol.requestId(fromUnknown: ["requestId": "  "]), "invalid-request")
        XCTAssertEqual(T3ActivationProtocol.requestId(fromUnknown: ["requestId": "abc"]), "abc")
        XCTAssertEqual(T3ActivationProtocol.requestId(fromUnknown: [1, 2]), "invalid-request")
    }

    func test_responseShapeAndTheTimeoutSeam() {
        XCTAssertNotNil(T3ActivationProtocol.response(success(request("r"))))
        XCTAssertNotNil(T3ActivationProtocol.response(T3ActivationProtocol.failure("r", "project-create-failed", "No.")))
        XCTAssertNil(T3ActivationProtocol.response(T3ActivationProtocol.failure("r", "made-up", "No.")))
        XCTAssertNil(T3ActivationProtocol.response(["version": 1, "requestId": "r", "ok": true, "projectId": "p"]))
        XCTAssertNil(T3ActivationProtocol.response(["version": 1, "requestId": "r", "ok": 1, "projectId": "p", "threadId": "t"]))
        XCTAssertEqual(T3AppControl.requestTimeout(env: [:], packaged: false), 15)
        XCTAssertEqual(T3AppControl.requestTimeout(env: ["T3_ACTIVATION_TIMEOUT_MS": "400"], packaged: false), 0.4)
        XCTAssertEqual(T3AppControl.requestTimeout(env: ["T3_ACTIVATION_TIMEOUT_MS": "400"], packaged: true), 15) // development flavor only
    }
}

final class AppControlLifecycleTests: XCTestCase {
    private final class Window {}
    private final class Topics { let lock = NSLock(); var list: [String] = []; func add(_ topic: String) { lock.lock(); list.append(topic); lock.unlock() }; var all: [String] { lock.lock(); defer { lock.unlock() }; return list } }
    private final class Backend { let lock = NSLock(); var status: [String: Any] = [:]; var observers = 0, released = 0; var changed: () -> Void = {}
        func set(_ value: [String: Any]) { lock.lock(); status = value; lock.unlock(); changed() }
        var current: [String: Any] { lock.lock(); defer { lock.unlock() }; return status } }

    /// A control over a fake backend status and a private temp dir; `activations` counts window activations.
    private func control(_ root: String, backend: Backend, activations: @escaping () -> Void = {}) -> T3AppControl {
        let control = T3AppControl()
        control.environment = { ["TMPDIR": root + "/"] }
        control.backendStatus = { backend.current }
        control.observeBackend = { _, _, changed in backend.observers += 1; backend.changed = changed }
        control.stopObservingBackend = { _ in backend.released += 1 }
        control.activateWindow = activations
        control.packaged = { false }
        control.log = { _ in }
        return control
    }
    private func settle(_ control: T3AppControl) { control.queue.sync {} }
    private func address(_ root: String, home: String) -> String { T3AppControlAddress.resolve(stateDir: home + "/userdata", tempDir: root, userId: getuid()).address }

    func test_listensWhileTheEmbeddedServerIsWantedAndClosesWithTheLastWindow() {
        let root = shortRoot(), home = root + "/t3-home", backend = Backend()
        let control = control(root, backend: backend), first = Window(), second = Window()
        backend.status = ["state": "starting", "enabled": true]
        control.attach(first, dataRoot: URL(fileURLWithPath: root), changed: { _ in })
        XCTAssertEqual(control.presentation()["listening"] as? Bool, false) // no T3 home yet
        backend.set(["state": "starting", "enabled": true, "t3Home": home]); settle(control)
        XCTAssertEqual(control.presentation()["address"] as? String, address(root, home: home))
        XCTAssertTrue(exists(address(root, home: home)))
        control.attach(second, dataRoot: URL(fileURLWithPath: root), changed: { _ in })
        XCTAssertEqual(backend.observers, 1)
        // The Local environment switched off (the stopgap): no socket, as the reference's relaunch has none.
        backend.set(["state": "stopped", "enabled": false, "t3Home": home]); settle(control)
        XCTAssertEqual(control.presentation()["listening"] as? Bool, false)
        XCTAssertFalse(exists(address(root, home: home)))
        backend.set(["state": "starting", "enabled": true, "t3Home": home]); settle(control)
        XCTAssertTrue(exists(address(root, home: home)))
        control.detach(first)
        XCTAssertTrue(exists(address(root, home: home)))
        XCTAssertEqual(backend.released, 0)
        control.detach(second) // the last window: the socket goes, and the backend is let go of
        XCTAssertFalse(exists(address(root, home: home)))
        XCTAssertEqual(backend.released, 1)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_aRefusedBuildHasNoSocketAndABindFailureIsLoggedWhileTheAppGoesOn() {
        let root = shortRoot(), backend = Backend(), window = Window()
        let control = control(root, backend: backend)
        var logged: [String] = []
        control.log = { logged.append($0) }
        backend.status = ["state": "refused", "enabled": true, "t3Home": root + "/home"]
        control.attach(window, dataRoot: URL(fileURLWithPath: root), changed: { _ in })
        XCTAssertEqual(control.presentation()["listening"] as? Bool, false)
        // A temp directory too long for a Unix socket path.
        control.environment = { ["TMPDIR": root + "/" + String(repeating: "d", count: 90)] }
        backend.set(["state": "starting", "enabled": true, "t3Home": root + "/home"]); settle(control)
        XCTAssertEqual(control.presentation()["listening"] as? Bool, false)
        XCTAssertTrue((control.presentation()["error"] as? String ?? "").contains("too long"))
        XCTAssertTrue(logged.contains { $0.hasPrefix("desktop app control socket unavailable") })
        control.detach(window)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_handsARequestToTheReadyWindowAndAnswersTheCommandLine() throws {
        let root = shortRoot(), home = root + "/h", backend = Backend(), window = Window(), topics = Topics()
        var activated = 0
        let control = control(root, backend: backend, activations: { activated += 1 })
        backend.status = ["state": "ready", "enabled": true, "t3Home": home]
        control.attach(window, dataRoot: URL(fileURLWithPath: root), changed: topics.add)
        let socket = address(root, home: home)
        var answer: [String: Any]?
        let answered = expectation(description: "answered")
        DispatchQueue.global().async { answer = exchange(socket, request("req-1")); answered.fulfill() }
        // Not ready: the request waits, the window was brought forward, nothing is handed.
        let deadline = Date().addingTimeInterval(2)
        while (control.presentation()["pending"] as? Int ?? 0) == 0, Date() < deadline { usleep(5_000) }
        XCTAssertEqual(activated, 1)
        XCTAssertEqual(control.status(for: window)["dispatched"] as? String, "")
        control.setReady(window, ready: true, token: "page-1")
        XCTAssertEqual(topics.all, ["t3.activation"])
        XCTAssertEqual(control.status(for: window)["dispatched"] as? String, "req-1")
        XCTAssertEqual(control.handedRequest(for: window, requestId: "req-1")?["workspaceRoot"] as? String, NSTemporaryDirectory() + "project")
        XCTAssertNil(control.handedRequest(for: window, requestId: "req-other"))
        XCTAssertEqual(control.complete(["version": 1, "requestId": "req-1", "ok": true]), "The activation response is invalid.")
        XCTAssertNil(control.complete(success(request("req-1"))))
        wait(for: [answered], timeout: 3)
        XCTAssertEqual(answer?["projectId"] as? String, "project-1")
        XCTAssertEqual(control.status(for: window)["dispatched"] as? String, "")
        XCTAssertNil(control.handedRequest(for: window, requestId: "req-1"))
        control.detach(window)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_aReloadedWindowFailsWhatTheOldPageWasHanded() {
        let root = shortRoot(), home = root + "/h", backend = Backend(), window = Window(), topics = Topics()
        let control = control(root, backend: backend)
        backend.status = ["state": "ready", "enabled": true, "t3Home": home]
        control.attach(window, dataRoot: URL(fileURLWithPath: root), changed: topics.add)
        control.setReady(window, ready: true, token: "page-1")
        var answer: [String: Any]?
        let answered = expectation(description: "answered")
        DispatchQueue.global().async { answer = exchange(self.address(root, home: home), request("req-2")); answered.fulfill() }
        let deadline = Date().addingTimeInterval(2)
        while control.status(for: window)["dispatched"] as? String != "req-2", Date() < deadline { usleep(5_000) }
        control.setReady(window, ready: true, token: "page-2") // the page reloaded and is ready again
        wait(for: [answered], timeout: 3)
        XCTAssertEqual(answer?["code"] as? String, "renderer-unavailable")
        XCTAssertEqual(answer?["message"] as? String, "The T3 Code window closed before it opened the project.")
        control.detach(window)
        try? FileManager.default.removeItem(atPath: root)
    }

    func test_aClosedWindowFailsItsHandedRequestAndANotReadyOneQueues() {
        let root = shortRoot(), home = root + "/h", backend = Backend(), first = Window(), second = Window()
        let control = control(root, backend: backend)
        backend.status = ["state": "ready", "enabled": true, "t3Home": home]
        control.attach(first, dataRoot: URL(fileURLWithPath: root), changed: { _ in })
        control.attach(second, dataRoot: URL(fileURLWithPath: root), changed: { _ in })
        control.setReady(first, ready: true, token: "a")
        var answer: [String: Any]?
        let answered = expectation(description: "answered")
        DispatchQueue.global().async { answer = exchange(self.address(root, home: home), request("req-3")); answered.fulfill() }
        let deadline = Date().addingTimeInterval(2)
        while control.status(for: first)["dispatched"] as? String != "req-3", Date() < deadline { usleep(5_000) }
        control.windowClosing(first) // its window began to close mid-request (before the session's teardown)
        wait(for: [answered], timeout: 3)
        XCTAssertEqual(answer?["code"] as? String, "renderer-unavailable")
        XCTAssertEqual(answer?["message"] as? String, "The T3 Code window closed before it opened the project.")
        control.setReady(first, ready: true, token: "a") // a late report from the closing page counts for nothing
        XCTAssertEqual(control.presentation()["windowReady"] as? Bool, false)
        control.detach(first)
        // The other window, not ready (a disconnected primary): the next request waits for it.
        var queued: [String: Any]?
        let later = expectation(description: "later")
        DispatchQueue.global().async { queued = exchange(self.address(root, home: home), request("req-4")); later.fulfill() }
        let wait = Date().addingTimeInterval(2)
        while (control.presentation()["pending"] as? Int ?? 0) == 0, Date() < wait { usleep(5_000) }
        XCTAssertEqual(control.status(for: second)["dispatched"] as? String, "")
        control.setReady(second, ready: true, token: "b")
        XCTAssertEqual(control.status(for: second)["dispatched"] as? String, "req-4")
        XCTAssertNil(control.complete(T3ActivationProtocol.failure("req-4", "project-create-failed", "T3 Code could not add the project.")))
        self.wait(for: [later], timeout: 3)
        XCTAssertEqual(queued?["code"] as? String, "project-create-failed")
        control.detach(second)
        try? FileManager.default.removeItem(atPath: root)
    }
}

/// DesktopWindow.activate's window (fix-misc-batch, #298 bug 7): with the window minimized, `t3 app` timed out and
/// the window stayed in the Dock, because the choice filtered on `canBecomeMain`, which AppKit answers false for a
/// window that is not visible.
final class AppControlActivationTargetTests: XCTestCase {
    private final class Stand: T3ActivationWindow {
        let name: String, isVisible: Bool, isMiniaturized: Bool, isActivationDocument: Bool
        init(_ name: String, visible: Bool = false, minimized: Bool = false, document: Bool = true) {
            self.name = name; isVisible = visible; isMiniaturized = minimized; isActivationDocument = document
        }
    }

    func test_aMinimizedWindowIsTheOneRestored() {
        let panel = Stand("snapshot flash", visible: true, document: false), docked = Stand("T3 Code", minimized: true)
        XCTAssertTrue(T3AppControl.activationTarget(main: nil, windows: [panel, docked]) === docked, "the Dock's window, not a visible panel")
        let shown = Stand("second", visible: true)
        XCTAssertTrue(T3AppControl.activationTarget(main: nil, windows: [docked, shown]) === shown, "a window on screen comes first")
        XCTAssertTrue(T3AppControl.activationTarget(main: docked, windows: [shown]) === docked, "the main window when there is one")
        XCTAssertNil(T3AppControl.activationTarget(main: nil, windows: [panel, Stand("closed")]))
    }

    func test_whatAppKitSaysOfAWindowItHidesOrPanels() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        // Not on screen: AppKit will not let it become main, which is why the old filter never found a minimized window.
        XCTAssertFalse(window.isVisible)
        XCTAssertFalse(window.canBecomeMain)
        XCTAssertTrue(window.isActivationDocument)
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 20, height: 20), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        XCTAssertFalse(panel.isActivationDocument)
        XCTAssertTrue(T3AppControl.activationTarget(main: nil, windows: [panel, window]) == nil, "neither is on screen or in the Dock")
    }
}
