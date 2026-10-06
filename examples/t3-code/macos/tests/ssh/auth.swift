import Foundation
import AppKit
import XCTest

// SSH password prompts and remote Open (T3SshAuth.swift, T3RemoteEditors.swift). The
// attempt loop runs against fake-ssh.sh, a double that refuses BatchMode and wants
// FAKE_SSH_PASSWORD through the askpass helper; no real host, no ~/.ssh.
final class SshAuthTests: XCTestCase {
    /// fake-ssh.sh beside this file (the recipe compiles it by its path from the repository root).
    private var double: String { URL(fileURLWithPath: "\(#filePath)").deletingLastPathComponent().appendingPathComponent("fake-ssh.sh").path }
    private var authLog = ""

    override func setUp() {
        authLog = NSTemporaryDirectory() + "t3-ssh-auth-\(UUID().uuidString).log"
        setenv("FAKE_SSH_AUTH_LOG", authLog, 1); setenv("FAKE_SSH_PASSWORD", "right-pass", 1)
        try? "right-pass".write(toFile: authLog + ".password", atomically: true, encoding: .utf8)
        setenv("FAKE_SSH_PASSWORD_FILE", authLog + ".password", 1)
    }
    override func tearDown() { unsetenv("FAKE_SSH_PASSWORD"); unsetenv("FAKE_SSH_AUTH_LOG"); unsetenv("FAKE_SSH_PASSWORD_FILE"); try? FileManager.default.removeItem(atPath: authLog); try? FileManager.default.removeItem(atPath: authLog + ".password") }

    /// One `ssh … box true` run through the auth loop, as launchRemote and the pairing run do.
    private func attempt(_ ssh: T3Ssh, _ target: T3Ssh.Target) throws -> String {
        try ssh.withAuth(target) { auth -> String in
            let result = try ssh.run(double, ["-o", "BatchMode=\(auth.batch ? "yes" : "no")", "-o", "ConnectTimeout=10", T3Ssh.hostSpec(target), "true"], stdin: nil, timeout: 10, environment: auth.environment)
            guard result.status == 0 else { throw T3Failure(kind: "Ssh", message: T3Ssh.sshError(result.stderr, fallback: "failed")) }
            return "ok"
        }
    }
    /// The dialog's side: wait for the next request and answer it with `password` (nil cancels).
    private func answer(_ prompts: T3SshPrompts, _ passwords: [String?], seen: @escaping ([[String: Any]]) -> Void = { _ in }) -> Thread {
        let thread = Thread {
            var requests: [[String: Any]] = []
            for password in passwords {
                var request: [String: Any]?
                for _ in 0..<200 {
                    if let next = prompts.state()["request"] as? [String: Any], !requests.contains(where: { $0["requestId"] as? String == next["requestId"] as? String }) { request = next; break }
                    Thread.sleep(forTimeInterval: 0.02)
                }
                guard let request, let id = request["requestId"] as? String else { break }
                requests.append(request)
                prompts.readSecret = { _ in password }
                _ = prompts.resolve(id: id, answer: password == nil ? "cancel" : "submit")
            }
            seen(requests)
        }
        thread.start()
        return thread
    }
    private func log() -> [String] { ((try? String(contentsOfFile: authLog, encoding: .utf8)) ?? "").split(separator: "\n").map(String.init) }

    func testDetectsSshAuthFailuresFromCommonPermissionDeniedMessages() {
        XCTAssertTrue(T3SshAuth.isAuthFailure("julius@100.65.180.100: Permission denied (publickey,password,keyboard-interactive)."))
        XCTAssertTrue(T3SshAuth.isAuthFailure("Permission denied (publickey)."))
        XCTAssertFalse(T3SshAuth.isAuthFailure("Connection timed out"))
        XCTAssertFalse(T3SshAuth.isAuthFailure("mkdir: Permission denied"))
    }

    func testCreatesAskpassEnvForCachedPasswordPrompts() throws {
        let parent = NSTemporaryDirectory() + "t3-ssh-askpass-test-\(UUID().uuidString)"
        try FileManager.default.createDirectory(atPath: parent, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: parent) }
        let helper = try T3SshAuth.makeAskpassHelper(in: parent)
        XCTAssertTrue(helper.hasSuffix("/t3code-ssh-askpass/ssh-askpass.sh"))
        XCTAssertTrue(((helper as NSString).deletingLastPathComponent as NSString).deletingLastPathComponent.contains("t3code-ssh-runtime-"))
        let env = T3SshAuth.childEnvironment(base: [:], interactive: true, askpass: helper, secret: "super-secret")
        XCTAssertEqual(env["SSH_ASKPASS"], helper); XCTAssertEqual(env["SSH_ASKPASS_REQUIRE"], "force")
        XCTAssertEqual(env["T3_SSH_AUTH_SECRET"], "super-secret"); XCTAssertEqual(env["DISPLAY"], "t3code")
        XCTAssertTrue(try String(contentsOfFile: helper, encoding: .utf8).contains(#"printf "%s\n" "$T3_SSH_AUTH_SECRET""#))
        XCTAssertEqual((try FileManager.default.attributesOfItem(atPath: helper)[.posixPermissions] as? NSNumber)?.intValue, 0o700)
        XCTAssertEqual(T3SshAuth.childEnvironment(base: ["A": "1"], interactive: false, askpass: helper, secret: "x"), ["A": "1"], "a batch run gets no helper")
    }

    func testWrongThenRightPasswordConnectsAndTheSecretIsReusedForThatTarget() throws {
        let ssh = T3Ssh(agent: true, promptsAvailable: true)
        defer { ssh.destroy() }
        let target = try T3Ssh.target(["alias": "box", "username": "me"])
        var requests: [[String: Any]] = []
        let dialog = answer(ssh.prompts, ["wrong-pass", "right-pass"]) { requests = $0 }
        XCTAssertEqual(try attempt(ssh, target), "ok")
        while dialog.isExecuting { Thread.sleep(forTimeInterval: 0.01) }
        XCTAssertEqual(requests.count, 2, "two prompts")
        XCTAssertEqual(requests.first?["prompt"] as? String, "Enter the SSH password for me@box.")
        XCTAssertEqual(requests.first?["destination"] as? String, "box"); XCTAssertEqual(requests.first?["username"] as? String, "me")
        XCTAssertEqual(requests.map { $0["attempt"] as? Int ?? 0 }, [1, 2])
        XCTAssertEqual(log(), ["batch refused box", "password refused box", "password accepted box"])
        // The next run of the same target uses the kept secret: no prompt, no batch attempt.
        XCTAssertEqual(try attempt(ssh, target), "ok")
        XCTAssertEqual(log().last, "password accepted box"); XCTAssertEqual(log().count, 4)
        XCTAssertTrue(ssh.prompts.state()["request"] is NSNull)
        XCTAssertFalse(log().joined().contains("right-pass"), "the double never logs the secret")
    }

    func testAFailedSecretIsDroppedAndCancelSaysSo() throws {
        let ssh = T3Ssh(agent: true, promptsAvailable: true)
        defer { ssh.destroy() }
        let target = try T3Ssh.target(["alias": "box"])
        let first = answer(ssh.prompts, ["right-pass"])
        XCTAssertEqual(try attempt(ssh, target), "ok")
        while first.isExecuting { Thread.sleep(forTimeInterval: 0.01) }
        try "rotated".write(toFile: authLog + ".password", atomically: true, encoding: .utf8) // the host's password changes
        let cancel = answer(ssh.prompts, [nil])
        XCTAssertThrowsError(try attempt(ssh, target)) { XCTAssertEqual(($0 as? T3Failure)?.message, "SSH authentication cancelled for box.") }
        while cancel.isExecuting { Thread.sleep(forTimeInterval: 0.01) }
        // The rotated secret is gone: the next run starts with BatchMode again and asks.
        let again = answer(ssh.prompts, ["rotated"])
        let before = log().count
        XCTAssertEqual(try attempt(ssh, target), "ok")
        while again.isExecuting { Thread.sleep(forTimeInterval: 0.01) }
        XCTAssertEqual(Array(log()[before...]), ["batch refused box", "password accepted box"])
    }

    func testTwoPromptsAtMostThenSshsOwnMessage() throws {
        let ssh = T3Ssh(agent: true, promptsAvailable: true)
        defer { ssh.destroy() }
        let dialog = answer(ssh.prompts, ["nope", "still-nope", "never-asked"])
        XCTAssertThrowsError(try attempt(ssh, try T3Ssh.target(["alias": "box"]))) { XCTAssertEqual(($0 as? T3Failure)?.message, "box: Permission denied (publickey,password,keyboard-interactive).") }
        Thread.sleep(forTimeInterval: 0.3); dialog.cancel()
        XCTAssertEqual(log().filter { $0.hasPrefix("password") }.count, 2)
    }

    func testWithoutAPromptServiceTheRefusalIsFinal() throws {
        let ssh = T3Ssh(agent: true)
        defer { ssh.destroy() }
        // Without a dialog the reference runs ssh interactively with no secret (askpass fails) and keeps ssh's message.
        XCTAssertThrowsError(try attempt(ssh, try T3Ssh.target(["alias": "box"]))) { XCTAssertEqual(($0 as? T3Failure)?.message, "box: Permission denied (publickey,password,keyboard-interactive).") }
        XCTAssertEqual(log(), ["password refused box"])
    }

    func testQueueExpiryAndWindowClose() {
        var clock = 1_000.0
        let prompts = T3SshPrompts(available: true, timeout: 0.3, clock: { clock })
        // Expiry: the request stays shown, marked expired; Continue says so; Dismiss removes it.
        XCTAssertEqual(prompts.request(destination: "box", username: nil, prompt: "Enter the SSH password for box.", attempt: 1), .timedOut)
        let expired = prompts.state()["request"] as? [String: Any]
        XCTAssertEqual(expired?["expired"] as? Bool, true); XCTAssertEqual(expired?["expiresAt"] as? Double, 1_300)
        let id = expired?["requestId"] as? String ?? ""
        XCTAssertEqual(prompts.resolve(id: id, answer: "submit").message, "SSH password prompt expired. Try connecting again.")
        XCTAssertEqual((prompts.state()["request"] as? [String: Any])?["error"] as? String, "SSH password prompt expired. Try connecting again.")
        XCTAssertTrue(prompts.resolve(id: id, answer: "dismiss").ok)
        XCTAssertTrue(prompts.state()["request"] is NSNull)
        XCTAssertEqual(T3SshPrompts.message(.timedOut, destination: "box"), "SSH authentication timed out for box.")
        // FIFO: a second request waits behind the first; the window's end fails both.
        clock = 2_000
        let long = T3SshPrompts(available: true, timeout: 30, clock: { clock })
        var outcomes: [T3SshPrompts.Outcome] = []
        let lock = NSLock(), group = DispatchGroup()
        for name in ["first", "second"] {
            group.enter()
            DispatchQueue.global().async { let outcome = long.request(destination: name, username: nil, prompt: name, attempt: 1); lock.lock(); outcomes.append(outcome); lock.unlock(); group.leave() }
            Thread.sleep(forTimeInterval: 0.05)
        }
        XCTAssertEqual((long.state()["request"] as? [String: Any])?["destination"] as? String, "first")
        XCTAssertEqual(long.state()["queued"] as? Int, 1)
        long.closeAll(windowClosed: true)
        XCTAssertEqual(group.wait(timeout: .now() + 2), .success)
        XCTAssertEqual(outcomes, [.windowClosed, .windowClosed])
        XCTAssertEqual(T3SshPrompts.message(.windowClosed, destination: "box"), "SSH authentication was cancelled because the app window closed.")
        XCTAssertEqual(long.request(destination: "late", username: nil, prompt: "late", attempt: 1), .windowClosed)
    }

    func testApplicationTerminationClosesPromptsWithoutWindowTeardown() {
        let ssh = T3Ssh(agent: true, promptsAvailable: true)
        NotificationCenter.default.post(name: NSApplication.willTerminateNotification, object: nil)
        XCTAssertEqual(ssh.prompts.request(destination: "late", username: nil, prompt: "late", attempt: 1), .windowClosed)
        ssh.destroy() // Session teardown may still follow the notification.
    }

    func testSecureFieldMasksForgetsAndRefusesCopy() throws {
        let events = ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 0)
        let instance = T3SshPasswordField(props: ["request": "req-1", "prompt": "Enter the SSH password for box."], events: events)
        let view = try XCTUnwrap(instance.view as? NSSecureTextField)
        XCTAssertEqual(view.accessibilityLabel(), "Enter the SSH password for box.")
        try instance.agentInput(.text("typed-secret"))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 80), styleMask: [.titled], backing: .buffered, defer: false)
        view.frame = NSRect(x: 10, y: 10, width: 200, height: 24); window.contentView?.addSubview(view)
        window.makeFirstResponder(view)
        let editor = try XCTUnwrap(view.currentEditor() as? NSTextView)
        editor.selectAll(nil)
        XCTAssertFalse(editor.validateMenuItem(NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")), "copy refused")
        XCTAssertFalse(editor.validateMenuItem(NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")), "cut refused")
        XCTAssertEqual(T3SshPasswordFields.take("req-1"), "typed-secret")
        XCTAssertEqual(view.stringValue, "", "the field forgets the password once read")
        XCTAssertEqual(editor.string, "", "the active field editor must forget it too")
        try instance.setProps(["request": "req-2", "prompt": "Try again."])
        editor.insertText("replacement", replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertEqual(T3SshPasswordFields.take("req-2"), "replacement", "a retry must not append to the previous password")
        try instance.agentInput(.text("again"))
        instance.destroy()
        XCTAssertEqual(view.stringValue, ""); XCTAssertNil(T3SshPasswordFields.take("req-1"))
        XCTAssertThrowsError(try instance.agentInput(.key("a", phase: nil)))
    }

    func testListsOnlyRemoteCapableEditorsThatResolve() throws {
        let root = NSTemporaryDirectory() + "t3-editors-\(UUID().uuidString)"
        let bin = root + "/bin", apps = root + "/Applications"
        let make = { (path: String) in
            try FileManager.default.createDirectory(atPath: (path as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
            FileManager.default.createFile(atPath: path, contents: Data("#!/bin/sh\n".utf8), attributes: [.posixPermissions: 0o755])
        }
        defer { try? FileManager.default.removeItem(atPath: root) }
        try make(bin + "/cursor"); try make(bin + "/idea") // idea is not remote-capable
        try make(apps + "/Visual Studio Code.app/Contents/Resources/app/bin/code")
        try make(apps + "/Zed.app/Contents/MacOS/cli")
        try FileManager.default.createDirectory(atPath: apps + "/VSCodium.app/Contents", withIntermediateDirectories: true) // no CLI inside
        XCTAssertEqual(T3RemoteEditors.probe(path: bin, appRoots: [apps]), ["cursor", "vscode", "zed"])
        XCTAssertEqual(T3RemoteEditors.probe(path: "", appRoots: [root + "/none"]), [], "nothing resolves: the client falls back to VS Code")
    }

    func testSafeExternalUrls() {
        XCTAssertEqual(T3RemoteEditors.safeExternalUrl("https://example.com/path"), "https://example.com/path")
        XCTAssertEqual(T3RemoteEditors.safeExternalUrl("vscode://vscode-remote/ssh-remote+example.com/home/user/project"), "vscode://vscode-remote/ssh-remote+example.com/home/user/project")
        XCTAssertEqual(T3RemoteEditors.safeExternalUrl("zed://ssh/example.com/home/user/project"), "zed://ssh/example.com/home/user/project")
        XCTAssertEqual(T3RemoteEditors.safeExternalUrl("zed://ssh/example.com/"), "zed://ssh/example.com/")
        for refused in ["zed://extension/attacker", "vscode://ssh/example.com/home/user/project", "vscode://user@vscode-remote/ssh-remote+example.com/home/user/project",
                        "vscode://:secret@vscode-remote/ssh-remote+example.com/home/user/project", "zed://ssh/user@example.com/home/user/project", "file:///etc/passwd",
                        "vscode://ms-python.python/some-command?argument=attacker", "vscode://vscode-remote/ssh-remote+"] {
            XCTAssertNil(T3RemoteEditors.safeExternalUrl(refused), refused)
        }
        XCTAssertNil(T3RemoteEditors.safeExternalUrl(42))
    }
}
