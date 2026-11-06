// The embedded server's environment, after T3 Code's desktop
// app (MIT, see LICENSE-T3; reference 5d3d78ab93: apps/desktop/src/shell/DesktopShellEnvironment.ts
// `installPosixEnvironment` and `make`, backend/DesktopBackendConfiguration.ts `DESKTOP_BACKEND_ENV_NAMES`,
// packages/shared/src/shell.ts `buildEnvironmentCaptureCommand`, `readEnvironmentFromLoginShell`,
// `applyShellEnvironment`, and packages/contracts/src/resourceTelemetry.ts `ShellEnvironmentPatch`).
//
// An app opened from Finder inherits launchd's short PATH, so the server would not find the
// provider CLIs. The login shell is asked (`$SHELL -ilc`, 5 s; then /bin/zsh), launchctl's
// PATH only when no shell answered (2 s), and the values are merged into the environment the
// server runs with. The server does not wait for the shell (T3LocalShellDiscovery): it starts with
// the values the last probe cached, and the probe's answer reaches it later on its telemetry descriptor.
// Differences: the reference installs the values into its own process (every child inherits them);
// this module applies them to the server's environment only. The probe runs in a new session with
// stderr discarded, as the shared reader's does; the desktop's stays in the app's session. The
// reference also reads T3CODE_TELEMETRY_ENABLED from the shell; here telemetry is always off (spec),
// so it is not read, and every snapshot says "false".
import Foundation

enum T3LocalShellEnvironment {
    static let loginShellNames = ["PATH", "DBUS_SESSION_BUS_ADDRESS", "DISPLAY", "LANG", "LC_ALL", "LC_CTYPE", "SSH_AUTH_SOCK",
                                  "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "XDG_CONFIG_HOME", "XDG_CURRENT_DESKTOP",
                                  "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE", "WAYLAND_DISPLAY",
                                  "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS", "T3CODE_GITHUB_APP_CLIENT_ID", "__MISE_DIFF", "JAVA_HOME"]
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

    /// `buildEnvironmentCaptureCommand`: `printenv` in POSIX syntax, or Nu syntax for a shell whose
    /// executable name starts with "nu".
    static func captureCommand(_ shell: String, _ names: [String]) -> String {
        let nushell = (shell as NSString).lastPathComponent.lowercased().hasPrefix("nu")
        return names.map { name in
            guard nushell else { return "printf '%s\\n' '\(startMarker(name))'; printenv \(name) || true; printf '%s\\n' '\(endMarker(name))'" }
            // Without a configured conversion, Nu expands home paths only in a list PATH.
            let defaultValue = name == "PATH"
                ? "if ($env.PATH? | describe | str starts-with 'list') { $env.PATH | each {|p| if ($p | str starts-with '~') { "
                    + "$p | path expand --no-symlink } else { $p } } | str join (char esep) } else { $env.PATH? | default '' }"
                : "$env.\(name)? | default ''"
            let value = "if $env.\(name)? == null { '' } else if $env.ENV_CONVERSIONS?.\(name)?.to_string? != null { "
                + "$env.\(name) | do $env.ENV_CONVERSIONS.\(name).to_string $env.\(name) } else { \(defaultValue) }"
            return "print '\(startMarker(name))'; print (\(value)); print '\(endMarker(name))'"
        }.joined(separator: "; ")
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

    /// `installPosixEnvironment` for darwin, applied to `env`. True when a login shell returned a PATH (only such a
    /// probe is cached, and marks its snapshot `discovered`).
    @discardableResult static func install(into env: inout [String: String], run: Runner) -> Bool {
        var shell: [String: String] = [:]
        for candidate in loginShellCandidates(env) {
            shell.merge(extract(run(candidate, ["-ilc", captureCommand(candidate, loginShellNames)], loginShellTimeout), loginShellNames)) { $1 }
            if shell["PATH"] != nil { break }
        }
        let launchctlPath = shell["PATH"] == nil ? trimmed(run("/bin/launchctl", ["getenv", "PATH"], launchctlTimeout)) : nil
        if let merged = mergePaths([trimmed(shell["PATH"]) ?? launchctlPath, trimmed(env["PATH"] ?? env["Path"] ?? env["path"])]) {
            env["PATH"] = merged
        }
        if env["SSH_AUTH_SOCK"]?.isEmpty ?? true, let value = shell["SSH_AUTH_SOCK"] { env["SSH_AUTH_SOCK"] = value }
        // __MISE_DIFF is mise's record of the PATH entries it added. It must travel with the shell's PATH, or
        // `mise x` can rebuild PATH behind a same-named wrapper (`exec mise x claude -- claude`) that re-runs itself.
        for name in ["DBUS_SESSION_BUS_ADDRESS", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE", "__MISE_DIFF"] {
            if let value = shell[name] { env[name] = value }
        }
        // The CA trust names: without them a Node or OpenSSL with no bundled roots (a shared-OpenSSL node on
        // PATH) cannot verify TLS, so provider updaters and agents fail to reach their registries.
        for name in ["DISPLAY", "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY",
                     "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS", "JAVA_HOME", "T3CODE_GITHUB_APP_CLIENT_ID"] {
            if env[name]?.isEmpty ?? true, let value = shell[name] { env[name] = value }
        }
        // One precedence group: LC_ALL can override an inherited LANG or LC_CTYPE.
        if localeNames.allSatisfy({ trimmed(env[$0]) == nil }) {
            for name in localeNames { if let value = trimmed(shell[name]) { env[name] = value } }
            if localeNames.allSatisfy({ trimmed(env[$0]) == nil }) { env["LC_CTYPE"] = fallbackLcCtype }
        }
        return trimmed(shell["PATH"]) != nil
    }

    /// The server's environment: `env` (the app's, with the shell's values) without the envelope's variables,
    /// with telemetry off (spec), and marked as waiting for the shell (`T3CODE_DESKTOP_SHELL_DISCOVERY_PENDING`,
    /// which the server clears when a discovered snapshot arrives).
    static func serverEnvironment(_ env: [String: String]) -> [String: String] {
        var env = env
        for name in backendEnvNames { env.removeValue(forKey: name) }
        env["T3CODE_TELEMETRY_ENABLED"] = "false"
        env["T3CODE_DESKTOP_SHELL_DISCOVERY_PENDING"] = "1"
        return env
    }

    /// `ShellEnvironmentPatch`: every name a snapshot carries, each with a value or null (removed).
    static let patchNames = ["PATH", "DBUS_SESSION_BUS_ADDRESS", "DISPLAY", "LANG", "LC_ALL", "LC_CTYPE", "SSH_AUTH_SOCK", "HOMEBREW_PREFIX",
                             "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "XDG_CONFIG_HOME", "XDG_CURRENT_DESKTOP", "XDG_DATA_HOME", "XDG_RUNTIME_DIR",
                             "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE", "WAYLAND_DISPLAY", "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS",
                             "T3CODE_TELEMETRY_ENABLED", "T3CODE_GITHUB_APP_CLIENT_ID", "__MISE_DIFF", "JAVA_HOME", "FNM_DIR", "FNM_MULTISHELL_PATH"]
    /// `CACHED_ENV_NAMES`: what the cache keeps of a probe that returned a PATH.
    static let cachedNames = ["PATH", "__MISE_DIFF", "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY", "JAVA_HOME",
                              "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS"]

    /// `env`'s values for the patch names; an absent name is null. Telemetry stays off (spec).
    static func patch(of env: [String: String]) -> [String: String] {
        var values = env.filter { patchNames.contains($0.key) }
        values["T3CODE_TELEMETRY_ENABLED"] = "false"
        return values
    }

    /// `applyShellEnvironment` with a full patch: each patch name takes the patch's value or is removed.
    static func apply(_ patch: [String: String], to env: inout [String: String]) {
        for name in patchNames { env[name] = patch[name] }
    }

    /// The cache key: platform, home, SHELL and ZDOTDIR, as the JSON array the reference encodes.
    static func cacheKey(_ env: [String: String]) -> String {
        let parts = ["darwin", env["HOME"] ?? NSHomeDirectory(), env["SHELL"] ?? "", env["ZDOTDIR"] ?? ""]
        let data = (try? JSONSerialization.data(withJSONObject: parts, options: [.withoutEscapingSlashes])) ?? Data()
        return String(decoding: data, as: UTF8.self)
    }

    /// A cache document of `key` written by the last probe that returned a PATH: the patch it gives at once, the
    /// inherited value winning over a cached one (PATH merged with it, mise's record kept with the cached PATH).
    /// Nil when the document is missing, malformed or another key's.
    static func restore(_ data: Data?, key: String, inherited: [String: String]) -> [String: String]? {
        guard let data, let document = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              document["version"] as? Int == 1, document["key"] as? String == key,
              let environment = document["environment"] as? [String: Any],
              patchNames.allSatisfy({ environment[$0] == nil || environment[$0] is String || environment[$0] is NSNull }) else { return nil }
        var baseline = inherited
        for name in cachedNames {
            guard let value = environment[name] as? String else { continue }
            if name != "PATH" && name != "__MISE_DIFF" && inherited[name] != nil { continue }
            baseline[name] = name == "PATH" ? mergePaths([value, trimmed(inherited["PATH"] ?? inherited["Path"] ?? inherited["path"])]) ?? value : value
        }
        return patch(of: baseline)
    }

    /// The cache document for a probe that returned a PATH: `env`'s cached names.
    static func cacheDocument(key: String, env: [String: String]) -> Data {
        let environment = env.filter { cachedNames.contains($0.key) && !$0.value.isEmpty }
        let document: [String: Any] = ["version": 1, "key": key, "environment": environment]
        return (try? JSONSerialization.data(withJSONObject: document, options: [.withoutEscapingSlashes, .sortedKeys])) ?? Data()
    }

    /// `DesktopShellEnvironmentSnapshot`, one NDJSON line.
    static func snapshotLine(_ patch: [String: String], discovered: Bool) -> Data {
        let environment = Dictionary(uniqueKeysWithValues: patchNames.map { ($0, patch[$0].map { $0 as Any } ?? NSNull()) })
        let message: [String: Any] = ["version": 1, "type": "shellEnvironment", "discovered": discovered, "platform": "darwin", "environment": environment]
        var line = (try? JSONSerialization.data(withJSONObject: message, options: [.withoutEscapingSlashes, .sortedKeys])) ?? Data()
        line.append(0x0A)
        return line
    }

    /// The production runner (`readEnvironmentFromLoginShell`'s `detached: true, stdio: ["ignore", "pipe",
    /// "ignore"]`): a new session, so job control in an interactive shell finds no controlling terminal to
    /// stop it on; stdin from /dev/null, stdout read, stderr discarded; SIGTERM at the timeout (then SIGKILL
    /// after 1 s). "" on failure. A shell stuck in the kernel survives SIGKILL, so the probe does not wait
    /// for its exit (DesktopShellEnvironment.ts `runCommandOutput`, reference 322f6544dfad).
    static func runCommand(_ command: String, _ args: [String], _ timeout: TimeInterval) -> String {
        runCommand(command, args, timeout, signal: { kill($0, $1) })
    }

    /// `runCommand` with the signal sender a test replaces.
    static func runCommand(_ command: String, _ args: [String], _ timeout: TimeInterval, signal: (pid_t, Int32) -> Int32) -> String {
        var fds: [Int32] = [-1, -1]
        guard pipe(&fds) == 0 else { return "" }
        var actions: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_adddup2(&actions, fds[1], 1)
        posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0)
        var attributes: posix_spawnattr_t?
        posix_spawnattr_init(&attributes)
        defer { posix_spawnattr_destroy(&attributes) }
        // CLOEXEC_DEFAULT: the child gets only the three descriptors above.
        posix_spawnattr_setflags(&attributes, Int16(POSIX_SPAWN_SETSID | POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_SETSIGMASK))
        var signals = sigset_t()
        sigfillset(&signals); posix_spawnattr_setsigdefault(&attributes, &signals)
        sigemptyset(&signals); posix_spawnattr_setsigmask(&attributes, &signals)
        let argv = ([command] + args).map { strdup($0) } + [nil]
        defer { argv.forEach { free($0) } }
        var pid: pid_t = 0
        let spawned = posix_spawn(&pid, command, &actions, &attributes, argv, environ)
        close(fds[1])
        guard spawned == 0 else { close(fds[0]); return "" }
        let reader = DispatchQueue(label: "com.exact.t3code.local.shell-env")
        let drained = DispatchSemaphore(value: 0), done = DispatchSemaphore(value: 0)
        var data = Data()
        reader.async {
            var buffer = [UInt8](repeating: 0, count: 16_384)
            while true {
                let count = read(fds[0], &buffer, buffer.count)
                if count > 0 { data.append(buffer, count: count) } else if count == 0 || errno != EINTR { break }
            }
            close(fds[0]); drained.signal()
        }
        DispatchQueue.global().async {
            var status: Int32 = 0
            while waitpid(pid, &status, 0) == -1 && errno == EINTR {}
            done.signal()
        }
        if done.wait(timeout: .now() + timeout) == .timedOut {
            _ = signal(pid, SIGTERM)
            if done.wait(timeout: .now() + 1) == .timedOut { _ = signal(pid, SIGKILL) }
            _ = drained.wait(timeout: .now() + 1)
            return ""
        }
        // A descendant that keeps stdout open leaves the output unread.
        guard drained.wait(timeout: .now() + 1) == .success else { return "" }
        return String(decoding: data, as: UTF8.self)
    }
}

/// `DesktopShellEnvironment.make`: the shell's values without holding the server's start. `start()` restores the
/// cache (`<T3 home>/userdata/shell-environment-cache.json`) when its key matches and probes the login shell in
/// the background; each result is a snapshot every server receives on its telemetry descriptor
/// (T3LocalTelemetryChannel), the latest replayed to a new one. The first result is published even when the
/// probe failed, the later ones only when it returned a PATH, which is also when the cache is written.
final class T3LocalShellDiscovery: @unchecked Sendable {
    let inherited: [String: String]
    let cacheURL: URL
    private let run: T3LocalShellEnvironment.Runner
    private let queue = DispatchQueue(label: "com.exact.t3code.local.shell-discovery", qos: .userInitiated)
    private let lock = NSLock()
    private var started = false
    private var latest: (patch: [String: String], line: Data)?
    private var listeners: [Int: (Data) -> Void] = [:]
    private var nextListener = 0
    /// Signalled when the first probe has finished (tests).
    let probed = DispatchSemaphore(value: 0)

    init(inherited: [String: String], stateDir: URL, run: @escaping T3LocalShellEnvironment.Runner) {
        self.inherited = inherited; self.run = run
        cacheURL = stateDir.appendingPathComponent("shell-environment-cache.json")
    }

    /// `installIntoProcess`: at most once; returns at once.
    func start() {
        lock.lock(); let first = !started; started = true; lock.unlock()
        guard first else { return }
        let key = T3LocalShellEnvironment.cacheKey(inherited)
        if let cached = T3LocalShellEnvironment.restore(try? Data(contentsOf: cacheURL), key: key, inherited: inherited) { publish(cached, discovered: false) }
        queue.async { [self] in
            refresh(key: key)
            probed.signal()
        }
    }

    /// The inherited environment with the latest snapshot applied.
    func environment() -> [String: String] {
        var env = inherited
        lock.lock(); let patch = latest?.patch; lock.unlock()
        if let patch { T3LocalShellEnvironment.apply(patch, to: &env) }
        return env
    }

    /// Registers `listener` for later snapshots and returns the latest one, under one lock.
    func subscribe(_ listener: @escaping (Data) -> Void) -> (replay: Data?, cancel: () -> Void) {
        lock.lock(); defer { lock.unlock() }
        let id = nextListener; nextListener += 1
        listeners[id] = listener
        return (latest?.line, { [weak self] in self?.lock.lock(); self?.listeners[id] = nil; self?.lock.unlock() })
    }

    private func publish(_ patch: [String: String], discovered: Bool) {
        let line = T3LocalShellEnvironment.snapshotLine(patch, discovered: discovered)
        lock.lock(); latest = (patch, line); let targets = Array(listeners.values); lock.unlock()
        for listener in targets { listener(line) }
    }

    /// From the inherited baseline, so a cached value the shell no longer sets does not survive.
    private func refresh(key: String) {
        var fresh = inherited
        let prepared = T3LocalShellEnvironment.install(into: &fresh, run: run)
        lock.lock(); let none = latest == nil; lock.unlock()
        if prepared || none { publish(T3LocalShellEnvironment.patch(of: fresh), discovered: prepared) }
        guard prepared else { return }
        let directory = cacheURL.deletingLastPathComponent(), staging = cacheURL.appendingPathExtension("tmp")
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            guard FileManager.default.createFile(atPath: staging.path, contents: T3LocalShellEnvironment.cacheDocument(key: key, env: fresh), attributes: [.posixPermissions: 0o600]),
                  rename(staging.path, cacheURL.path) == 0 else { return }
        } catch {}
    }
}
