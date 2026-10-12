import Foundation
import XCTest

// Ported from T3 Code (MIT, see LICENSE-T3), reference 1e2ecbd975:
// apps/desktop/src/shell/DesktopShellEnvironment.test.ts (its seven darwin cases and the probe-failure case) and
// apps/desktop/src/app/DesktopObservability.test.ts (the backend child output cases).
// Change from the reference: bun/vitest over Effect there; XCTest over the native ports
// (T3LocalShellEnvironment.swift, T3LocalLog.swift) here, with the original names.

private func envOutput(_ values: KeyValuePairs<String, String>) -> String {
    values.flatMap { ["__T3CODE_ENV_\($0.key)_START__", $0.value, "__T3CODE_ENV_\($0.key)_END__"] }.joined(separator: "\n")
}

final class LocalShellEnvironmentTests: XCTestCase {
    func test_hydrates_PATH_and_missing_SSH_AUTH_SOCK_from_the_login_shell_on_macOS() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/Users/test/.local/bin:/usr/bin"]
        var commands: [String] = []
        T3LocalShellEnvironment.install(into: &env) { command, args, _ in
            commands.append(command)
            XCTAssertEqual(args.first, "-ilc")
            return envOutput(["PATH": "/opt/homebrew/bin:/usr/bin", "SSH_AUTH_SOCK": "/tmp/secretive.sock", "HOMEBREW_PREFIX": "/opt/homebrew"])
        }
        XCTAssertEqual(commands, ["/bin/zsh"])
        XCTAssertEqual(env["PATH"], "/opt/homebrew/bin:/usr/bin:/Users/test/.local/bin")
        XCTAssertEqual(env["SSH_AUTH_SOCK"], "/tmp/secretive.sock")
        XCTAssertEqual(env["HOMEBREW_PREFIX"], "/opt/homebrew")
    }

    func test_preserves_inherited_POSIX_values_when_present() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin", "SSH_AUTH_SOCK": "/tmp/inherited.sock"]
        T3LocalShellEnvironment.install(into: &env) { _, _, _ in envOutput(["PATH": "/opt/homebrew/bin:/usr/bin", "SSH_AUTH_SOCK": "/tmp/login-shell.sock"]) }
        XCTAssertEqual(env["PATH"], "/opt/homebrew/bin:/usr/bin")
        XCTAssertEqual(env["SSH_AUTH_SOCK"], "/tmp/inherited.sock")
    }

    func test_hydrates_the_locale_from_the_login_shell_on_macOS() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin"]
        T3LocalShellEnvironment.install(into: &env) { _, _, _ in envOutput(["PATH": "/opt/homebrew/bin:/usr/bin", "LANG": "de_DE.UTF-8"]) }
        XCTAssertEqual(env["LANG"], "de_DE.UTF-8")
    }

    func test_preserves_an_inherited_locale_over_the_login_shell_on_macOS() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin", "LANG": "en_US.UTF-8"]
        T3LocalShellEnvironment.install(into: &env) { _, _, _ in envOutput(["PATH": "/opt/homebrew/bin:/usr/bin", "LANG": "de_DE.UTF-8"]) }
        XCTAssertEqual(env["LANG"], "en_US.UTF-8")
    }

    func test_does_not_mix_login_shell_locale_categories_into_an_inherited_locale() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin", "LANG": "en_US.UTF-8"]
        T3LocalShellEnvironment.install(into: &env) { _, _, _ in envOutput(["PATH": "/opt/homebrew/bin:/usr/bin", "LC_ALL": "de_DE.UTF-8"]) }
        XCTAssertEqual(env["LANG"], "en_US.UTF-8")
        XCTAssertNil(env["LC_ALL"])
    }

    func test_falls_back_to_a_UTF_8_LC_CTYPE_when_no_locale_is_available_on_macOS() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin"]
        T3LocalShellEnvironment.install(into: &env) { _, _, _ in envOutput(["PATH": "/opt/homebrew/bin:/usr/bin"]) }
        XCTAssertNil(env["LANG"])
        XCTAssertNil(env["LC_ALL"])
        XCTAssertEqual(env["LC_CTYPE"], "en_US.UTF-8")
    }

    func test_falls_back_to_launchctl_PATH_on_macOS_when_shell_probing_does_not_return_one() {
        var env = ["SHELL": "/opt/homebrew/bin/nu", "PATH": "/usr/bin"]
        var commands: [String] = []
        T3LocalShellEnvironment.install(into: &env) { command, _, _ in
            commands.append(command)
            return command == "/bin/launchctl" ? "/opt/homebrew/bin:/usr/bin" : ""
        }
        XCTAssertEqual(commands, ["/opt/homebrew/bin/nu", "/bin/zsh", "/bin/launchctl"])
        XCTAssertEqual(env["PATH"], "/opt/homebrew/bin:/usr/bin")
    }

    /// The reference runs this case on linux; the logic is the same on macOS: a probe that fails
    /// leaves the inherited values (the reference logs the probe's name and argument count, never
    /// its arguments, and goes on), and launchctl is asked for PATH.
    func test_logs_command_failures_with_safe_probe_context_and_the_exact_cause() {
        var env = ["SHELL": "/bin/zsh", "PATH": "/usr/bin"]
        var calls: [(String, Int)] = []
        T3LocalShellEnvironment.install(into: &env) { command, args, _ in calls.append((command, args.count)); return "" }
        XCTAssertEqual(calls.map { $0.0 }, ["/bin/zsh", "/bin/launchctl"])
        XCTAssertEqual(calls.map { $0.1 }, [2, 2])
        XCTAssertEqual(env["PATH"], "/usr/bin")
    }

    func testTheRealLoginShellIsAskedWithTheReferenceMarkers() {
        let command = T3LocalShellEnvironment.captureCommand(["PATH"])
        XCTAssertEqual(command, "printf '%s\\n' '__T3CODE_ENV_PATH_START__'; printenv PATH || true; printf '%s\\n' '__T3CODE_ENV_PATH_END__'")
        let output = T3LocalShellEnvironment.runCommand("/bin/zsh", ["-ilc", command], 5)
        XCTAssertNotNil(T3LocalShellEnvironment.extract(output, ["PATH"])["PATH"], "zsh -ilc answers within 5 s")
    }
}

final class LocalOutputLogTests: XCTestCase {
    private func temporaryLog() -> (T3LocalFileOutputLog, URL) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-desktop-backend-output-log-test-\(UUID().uuidString)/userdata/logs")
        return (T3LocalFileOutputLog(logDirectory: directory, runId: "test-run"), directory.appendingPathComponent("server-child.log"))
    }
    private func records(_ path: URL) -> [[String: Any]] {
        ((try? String(contentsOf: path, encoding: .utf8)) ?? "").split(separator: "\n").map { (try? JSONSerialization.jsonObject(with: Data($0.utf8))) as? [String: Any] ?? [:] }
    }

    func test_buffers_backend_child_output_and_persists_it_only_when_a_failure_is_reported() {
        let (log, path) = temporaryLog()
        log.beginSession(details: "pid=123 port=3773 cwd=/repo")
        log.writeOutputChunk(.stdout, Data("hello server\n".utf8))
        XCTAssertFalse(FileManager.default.fileExists(atPath: path.path))
        log.persistFailure(details: "code=1")
        log.beginSession(details: "pid=456")
        log.writeOutputChunk(.stderr, Data("normal shutdown\n".utf8))
        log.discardSession()
        let lines = records(path)
        XCTAssertEqual(lines.count, 3)
        let start = lines[0], output = lines[1], end = lines[2]
        let a = { (record: [String: Any]) in record["annotations"] as? [String: Any] ?? [:] }
        XCTAssertEqual(start["message"] as? String, "backend child process failure output start")
        XCTAssertEqual(start["level"] as? String, "ERROR")
        XCTAssertEqual(a(start)["component"] as? String, "desktop-backend-child")
        XCTAssertEqual(a(start)["runId"] as? String, "test-run")
        XCTAssertEqual(a(start)["instanceId"] as? String, "primary")
        XCTAssertEqual(a(start)["phase"] as? String, "START")
        XCTAssertEqual(a(start)["details"] as? String, "pid=123 port=3773 cwd=/repo")
        XCTAssertEqual(output["message"] as? String, "backend child process output")
        XCTAssertEqual(output["level"] as? String, "INFO")
        XCTAssertEqual(a(output)["stream"] as? String, "stdout")
        XCTAssertEqual(a(output)["text"] as? String, "hello server\n")
        XCTAssertEqual(end["message"] as? String, "backend child process failure output end")
        XCTAssertEqual(end["level"] as? String, "ERROR")
        XCTAssertEqual(a(end)["phase"] as? String, "END")
        XCTAssertEqual(a(end)["details"] as? String, "code=1")
        XCTAssertEqual(start["fiberId"] as? String, "#backend-child")
    }

    func test_keeps_buffering_output_after_a_non_terminal_failure_snapshot() {
        let (log, path) = temporaryLog()
        log.beginSession(details: "pid=123")
        log.writeOutputChunk(.stdout, Data("before timeout\n".utf8))
        log.persistFailureSnapshot(details: "readiness timeout")
        log.writeOutputChunk(.stderr, Data("after timeout\n".utf8))
        log.persistFailure(details: "code=1")
        let lines = records(path)
        XCTAssertTrue(lines.contains { ($0["annotations"] as? [String: Any])?["text"] as? String == "after timeout\n" })
        XCTAssertEqual((lines.last?["annotations"] as? [String: Any])?["details"] as? String, "code=1")
    }

    func test_retains_only_the_last_mebibyte_of_backend_child_output() {
        let (log, path) = temporaryLog()
        var output = Data(repeating: UInt8(ascii: "x"), count: 1024 * 1024 + 128)
        output.replaceSubrange(0..<128, with: Data(repeating: UInt8(ascii: "y"), count: 128))
        log.beginSession(details: "pid=123")
        log.writeOutputChunk(.stderr, output)
        log.persistFailure(details: "code=1")
        let text = (records(path)[1]["annotations"] as? [String: Any])?["text"] as? String ?? ""
        XCTAssertEqual(text.utf8.count, 1024 * 1024)
        XCTAssertFalse(text.contains("y"))
    }

    func test_bounds_the_number_of_retained_backend_child_output_chunks() {
        let (log, path) = temporaryLog()
        log.beginSession(details: "pid=123")
        for index in 0..<300 { log.writeOutputChunk(.stderr, Data([UInt8(index % 128)])) }
        log.persistFailure(details: "code=1")
        XCTAssertEqual(records(path).count, 258)
    }

    func test_advances_a_retained_output_offset_instead_of_repeatedly_copying_a_full_head_chunk() {
        var session = T3LocalOutputSession(runId: "r", startDetails: "")
        session = session.appending(.stdout, Data(repeating: 1, count: T3LocalOutputSession.maxBytes))
        session = session.appending(.stdout, Data(repeating: 2, count: 10))
        XCTAssertEqual(session.chunks.count, 2)
        XCTAssertEqual(session.chunks[0].offset, 10)
        XCTAssertEqual(session.byteLength, T3LocalOutputSession.maxBytes)
    }

    func testTheLogRotatesAtTenMebibytesKeepingTen() {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-rotate-\(UUID().uuidString)")
        let file = T3LocalRotatingFile(path: directory.appendingPathComponent("server-child.log"), maxBytes: 100, maxFiles: 3)
        for _ in 0..<10 { file.write(Data(repeating: 65, count: 60)) }
        let names = Set((try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? [])
        XCTAssertEqual(names, ["server-child.log", "server-child.log.1", "server-child.log.2", "server-child.log.3"])
        let defaults = T3LocalRotatingFile(path: directory.appendingPathComponent("defaults/server-child.log"))
        XCTAssertEqual(defaults.maxBytes, 10 * 1024 * 1024)
        XCTAssertEqual(defaults.maxFiles, 10)
    }
}
