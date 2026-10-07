// The embedded server's environment (20261005-embedded-server-runtime), after T3 Code's desktop
// app (MIT, see LICENSE-T3; reference 1e2ecbd975: apps/desktop/src/shell/DesktopShellEnvironment.ts
// `installPosixEnvironment`, and backend/DesktopBackendConfiguration.ts `DESKTOP_BACKEND_ENV_NAMES`).
//
// An app opened from Finder inherits launchd's short PATH, so the server would not find the
// provider CLIs. The login shell is asked once (`$SHELL -ilc`, 5 s; then /bin/zsh), launchctl's
// PATH only when no shell answered (2 s), and the values are merged into the environment the
// server starts with. Difference: the reference installs them into its own process (every child
// inherits them); this module applies them to the server's environment only.
import Foundation

enum T3LocalShellEnvironment {
    static let loginShellNames = ["PATH", "DBUS_SESSION_BUS_ADDRESS", "DISPLAY", "LANG", "LC_ALL", "LC_CTYPE", "SSH_AUTH_SOCK",
                                  "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "XDG_CONFIG_HOME", "XDG_CURRENT_DESKTOP",
                                  "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE", "WAYLAND_DISPLAY"]
    static let localeNames = ["LANG", "LC_ALL", "LC_CTYPE"]
    static let fallbackLcCtype = "en_US.UTF-8"
    static let loginShellTimeout: TimeInterval = 5
    static let launchctlTimeout: TimeInterval = 2
    /// `DESKTOP_BACKEND_ENV_NAMES`: removed from the server's environment (the envelope carries them).
    static let backendEnvNames = ["T3CODE_PORT", "T3CODE_MODE", "T3CODE_NO_BROWSER", "T3CODE_HOST", "T3CODE_DESKTOP_WS_URL",
                                  "T3CODE_DESKTOP_LAN_ACCESS", "T3CODE_DESKTOP_LAN_HOST", "T3CODE_DESKTOP_HTTPS_ENDPOINTS",
                                  "T3CODE_TAILSCALE_SERVE", "T3CODE_TAILSCALE_SERVE_PORT"]

    /// Runs `command args` and returns its stdout ("" on failure or after `timeout`).
    typealias Runner = (_ command: String, _ args: [String], _ timeout: TimeInterval) -> String

    static func startMarker(_ name: String) -> String { "__T3CODE_ENV_\(name)_START__" }
    static func endMarker(_ name: String) -> String { "__T3CODE_ENV_\(name)_END__" }

    static func captureCommand(_ names: [String]) -> String {
        names.map { "printf '%s\\n' '\(startMarker($0))'; printenv \($0) || true; printf '%s\\n' '\(endMarker($0))'" }.joined(separator: "; ")
    }

    static func extract(_ output: String, _ names: [String]) -> [String: String] {
        var environment: [String: String] = [:]
        for name in names {
            guard let start = output.range(of: startMarker(name)), let end = output.range(of: endMarker(name), range: start.upperBound..<output.endIndex) else { continue }
            var value = String(output[start.upperBound..<end.lowerBound])
            if value.hasPrefix("\r\n") { value.removeFirst(2) } else if value.hasPrefix("\n") { value.removeFirst() }
            if value.hasSuffix("\r\n") { value.removeLast(2) } else if value.hasSuffix("\n") { value.removeLast() }
            if !value.isEmpty { environment[name] = value }
        }
        return environment
    }

    static func trimmed(_ value: String?) -> String? {
        guard let value = value?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else { return nil }
        return value
    }

    static func mergePaths(_ values: [String?]) -> String? {
        var entries: [String] = [], seen = Set<String>()
        for value in values.compactMap({ $0 }) {
            for entry in value.split(separator: ":", omittingEmptySubsequences: false) {
                let sanitized = entry.trimmingCharacters(in: .whitespaces)
                if sanitized.isEmpty { continue }
                let key = sanitized.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                if key.isEmpty || seen.contains(key) { continue }
                seen.insert(key); entries.append(sanitized)
            }
        }
        return entries.isEmpty ? nil : entries.joined(separator: ":")
    }

    static func loginShellCandidates(_ env: [String: String]) -> [String] {
        var seen = Set<String>(), candidates: [String] = []
        for candidate in [trimmed(env["SHELL"]), "/bin/zsh"].compactMap({ $0 }) where !seen.contains(candidate) {
            seen.insert(candidate); candidates.append(candidate)
        }
        return candidates
    }

    /// `installPosixEnvironment` for darwin, applied to `env`.
    static func install(into env: inout [String: String], run: Runner) {
        var shell: [String: String] = [:]
        for candidate in loginShellCandidates(env) {
            shell.merge(extract(run(candidate, ["-ilc", captureCommand(loginShellNames)], loginShellTimeout), loginShellNames)) { $1 }
            if shell["PATH"] != nil { break }
        }
        let launchctlPath = shell["PATH"] == nil ? trimmed(run("/bin/launchctl", ["getenv", "PATH"], launchctlTimeout)) : nil
        if let merged = mergePaths([trimmed(shell["PATH"]) ?? launchctlPath, trimmed(env["PATH"] ?? env["Path"] ?? env["path"])]) {
            env["PATH"] = merged
        }
        if env["SSH_AUTH_SOCK"]?.isEmpty ?? true, let value = shell["SSH_AUTH_SOCK"] { env["SSH_AUTH_SOCK"] = value }
        for name in ["DBUS_SESSION_BUS_ADDRESS", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE"] {
            if let value = shell[name] { env[name] = value }
        }
        for name in ["DISPLAY", "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY"] {
            if env[name]?.isEmpty ?? true, let value = shell[name] { env[name] = value }
        }
        // One precedence group: LC_ALL can override an inherited LANG or LC_CTYPE.
        if localeNames.allSatisfy({ trimmed(env[$0]) == nil }) {
            for name in localeNames { if let value = trimmed(shell[name]) { env[name] = value } }
            if localeNames.allSatisfy({ trimmed(env[$0]) == nil }) { env["LC_CTYPE"] = fallbackLcCtype }
        }
    }

    /// The server's environment: the app's, the login shell's values, without the envelope's
    /// variables, and with telemetry off (spec; issue X39).
    static func serverEnvironment(base: [String: String], run: Runner) -> [String: String] {
        var env = base
        install(into: &env, run: run)
        for name in backendEnvNames { env.removeValue(forKey: name) }
        env["T3CODE_TELEMETRY_ENABLED"] = "false"
        return env
    }

    /// The production runner: stdin closed, stdout read, SIGTERM at the timeout (then SIGKILL after 1 s).
    static func runCommand(_ command: String, _ args: [String], _ timeout: TimeInterval) -> String {
        let process = Process(), output = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        process.arguments = args
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice
        let done = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in done.signal() }
        do { try process.run() } catch { return "" }
        var data = Data()
        let reader = DispatchQueue(label: "com.exact.t3code.local.shell-env")
        let read = DispatchSemaphore(value: 0)
        reader.async { data = output.fileHandleForReading.readDataToEndOfFile(); read.signal() }
        if done.wait(timeout: .now() + timeout) == .timedOut {
            process.terminate()
            if done.wait(timeout: .now() + 1) == .timedOut { kill(process.processIdentifier, SIGKILL); done.wait() }
            _ = read.wait(timeout: .now() + 1)
            return ""
        }
        _ = read.wait(timeout: .now() + 1)
        return String(decoding: data, as: UTF8.self)
    }
}
