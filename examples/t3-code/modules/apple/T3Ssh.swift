// Settings → Connections → Add Environment → SSH (lane settings-b), after the
// desktop app's DesktopSshEnvironment and @t3tools/ssh: hosts come from
// ~/.ssh/config (Host aliases, Include globs) and known_hosts; an alias resolves
// through `ssh -G`; connecting reuses the remote T3 server (or starts `t3 serve`
// there), forwards a loopback port with `ssh -N -L`, issues a pairing token with
// `t3 auth pairing create --json`, and the environment then pairs like any other.
// Tunnels are remembered by origin and reopened at launch, so a saved SSH
// environment reconnects on the same loopback port.
//
// Agent runs never read the real ~/.ssh or reach a real host: discovery reads
// only T3_SSH_HOME and connecting requires the T3_SSH_COMMAND test double.
import Foundation
import CryptoKit
import Darwin

final class T3Ssh: @unchecked Sendable {
    private let agent: Bool
    private let environment = ProcessInfo.processInfo.environment
    private let queue = DispatchQueue(label: "com.exact.t3code.ssh", qos: .userInitiated, attributes: .concurrent)
    private let lock = NSLock()
    private var tunnels: [String: Process] = [:]
    private var memoryTargets: [String: [String: Any]] = [:]
    private var alive = true
    private let targetsKey = "t3.ssh.targets"
    static let defaultRemotePort = 3773
    static let readyTimeout: TimeInterval = 20

    init(agent: Bool) {
        self.agent = agent
        // A saved SSH environment's transport retries its loopback origin; reopening the
        // tunnel in the background lets that retry land (DesktopSshEnvironment reconnect).
        if !agent { queue.async { [weak self] in self?.reopenSaved() } }
    }

    /// Where ~/.ssh is read from: the user's home, or in agent runs only T3_SSH_HOME.
    var discoveryHome: String? {
        if let home = environment["T3_SSH_HOME"], !home.isEmpty { return home }
        return agent ? nil : NSHomeDirectory()
    }
    /// The ssh executable: T3_SSH_COMMAND (a test double) or /usr/bin/ssh outside agent runs.
    var sshCommand: String? {
        if let command = environment["T3_SSH_COMMAND"], !command.isEmpty { return command }
        return agent ? nil : "/usr/bin/ssh"
    }

    func perform(_ request: [String: Any], completion: @escaping ([String: Any]) -> Void) {
        let op = request["op"] as? String ?? ""
        queue.async { [self] in
            do {
                switch op {
                case "sshHosts":
                    var value: [String: Any] = ["hosts": discover(), "available": discoveryHome != nil]
                    value["targets"] = savedTargets()
                    completion(Self.success(value))
                case "sshResolve":
                    completion(Self.success(try resolve(alias: request["alias"] as? String ?? "")))
                case "sshConnect":
                    completion(Self.success(try connect(Self.target(request), pair: request["pair"] as? Bool ?? true)))
                case "sshForget":
                    forget(origin: request["origin"] as? String ?? "")
                    completion(Self.success(["forgotten": true]))
                default:
                    completion(Self.failure(T3Failure(kind: "Arguments", message: "Unknown SSH operation.")))
                }
            } catch { completion(Self.failure(error)) }
        }
    }

    func destroy() {
        lock.lock(); alive = false; let all = Array(tunnels.values); tunnels.removeAll(); lock.unlock()
        for process in all where process.isRunning { process.terminate() }
    }

    // MARK: Discovery (@t3tools/ssh config.ts discoverSshHosts)

    func discover() -> [[String: Any]] {
        guard let home = discoveryHome else { return [] }
        let ssh = (home as NSString).appendingPathComponent(".ssh")
        var visited = Set<String>()
        let aliases = Self.configAliases((ssh as NSString).appendingPathComponent("config"), home: home, visited: &visited)
        let known = Self.knownHosts((try? String(contentsOfFile: (ssh as NSString).appendingPathComponent("known_hosts"), encoding: .utf8)) ?? "")
        var seen = Set<String>(), hosts: [[String: Any]] = []
        for (alias, source) in aliases.map({ ($0, "ssh-config") }) + known.map({ ($0, "known-hosts") }) where !seen.contains(alias) {
            seen.insert(alias)
            var host: [String: Any] = ["alias": alias, "hostname": alias, "source": source]
            host["username"] = NSNull(); host["port"] = NSNull()
            hosts.append(host)
        }
        return hosts
    }

    static func hasPattern(_ value: String) -> Bool { value.contains("*") || value.contains("?") || value.hasPrefix("!") }

    static func configAliases(_ path: String, home: String, visited: inout Set<String>) -> [String] {
        let resolved = (path as NSString).standardizingPath
        guard !visited.contains(resolved), let raw = try? String(contentsOfFile: resolved, encoding: .utf8) else { return [] }
        visited.insert(resolved)
        var aliases = Set<String>()
        for line in raw.components(separatedBy: .newlines) {
            let stripped = (line.firstIndex(of: "#").map { String(line[..<$0]) } ?? line).trimmingCharacters(in: .whitespaces)
            if stripped.isEmpty { continue }
            let parts = stripped.replacingOccurrences(of: "=", with: " ").split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
            guard let directive = parts.first?.lowercased() else { continue }
            if directive == "include" {
                for pattern in parts.dropFirst() {
                    let expanded = pattern.hasPrefix("~") ? home + pattern.dropFirst() : pattern
                    let absolute = expanded.hasPrefix("/") ? expanded : ((home as NSString).appendingPathComponent(".ssh") as NSString).appendingPathComponent(expanded)
                    for included in expandGlob(absolute) { aliases.formUnion(configAliases(included, home: home, visited: &visited)) }
                }
                continue
            }
            guard directive == "host" else { continue }
            for alias in parts.dropFirst() where !alias.isEmpty && !hasPattern(alias) { aliases.insert(alias) }
        }
        return aliases.sorted()
    }

    static func expandGlob(_ pattern: String) -> [String] {
        guard pattern.contains("*") || pattern.contains("?") else { return FileManager.default.fileExists(atPath: pattern) ? [pattern] : [] }
        let directory = (pattern as NSString).deletingLastPathComponent, base = (pattern as NSString).lastPathComponent
        let escaped = NSRegularExpression.escapedPattern(for: base).replacingOccurrences(of: "\\*", with: ".*").replacingOccurrences(of: "\\?", with: ".")
        guard let matcher = try? NSRegularExpression(pattern: "^\(escaped)$"),
              let entries = try? FileManager.default.contentsOfDirectory(atPath: directory) else { return [] }
        return entries.filter { matcher.firstMatch(in: $0, range: NSRange($0.startIndex..., in: $0)) != nil }.sorted().map { (directory as NSString).appendingPathComponent($0) }
    }

    static func knownHosts(_ raw: String) -> [String] {
        var hostnames = Set<String>()
        for line in raw.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty || trimmed.hasPrefix("#") { continue }
            let fields = trimmed.split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
            let hostField = trimmed.hasPrefix("@") ? (fields.count > 1 ? fields[1] : "") : (fields.first ?? "")
            if hostField.isEmpty || hostField.hasPrefix("|") { continue }
            for raw in hostField.split(separator: ",").map(String.init) {
                var host = raw
                if host.hasPrefix("["), let close = host.firstIndex(of: "]") { host = String(host[host.index(after: host.startIndex)..<close]) }
                else if let first = host.firstIndex(of: ":"), first == host.lastIndex(of: ":") { host = String(host[..<first]) }
                host = host.trimmingCharacters(in: .whitespaces)
                if !host.isEmpty && !hasPattern(host) { hostnames.insert(host) }
            }
        }
        return hostnames.sorted()
    }

    // MARK: Resolution (command.ts resolveSshTarget / parseSshResolveOutput)

    func resolve(alias raw: String) throws -> [String: Any] {
        let alias = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !alias.isEmpty, alias.count <= 255 else { throw T3Failure(kind: "Arguments", message: "SSH host alias is required.") }
        var fallback: [String: Any] = ["alias": alias, "hostname": alias]
        fallback["username"] = NSNull(); fallback["port"] = NSNull()
        guard let command = sshCommand else { throw T3Failure(kind: "Unavailable", message: "SSH is unavailable in this run.") }
        guard let result = try? run(command, configArgs() + ["-G", alias], stdin: nil, timeout: 10), result.status == 0 else { return fallback }
        return Self.parseResolve(alias: alias, stdout: result.stdout)
    }

    static func parseResolve(alias: String, stdout: String) -> [String: Any] {
        var values: [String: String] = [:]
        for line in stdout.components(separatedBy: .newlines) {
            let parts = line.trimmingCharacters(in: .whitespaces).split(separator: " ", maxSplits: 1).map(String.init)
            if parts.count == 2, values[parts[0]] == nil { values[parts[0]] = parts[1].trimmingCharacters(in: .whitespaces) }
        }
        var target: [String: Any] = ["alias": alias, "hostname": values["hostname"].flatMap { $0.isEmpty ? nil : $0 } ?? alias]
        if let user = values["user"], !user.isEmpty { target["username"] = user } else { target["username"] = NSNull() }
        if let port = values["port"].flatMap({ Int($0) }) { target["port"] = port } else { target["port"] = NSNull() }
        return target
    }

    private func configArgs() -> [String] {
        // Agent runs point ssh at the isolated config, never ~/.ssh/config.
        guard agent, let home = discoveryHome else { return [] }
        let config = ((home as NSString).appendingPathComponent(".ssh") as NSString).appendingPathComponent("config")
        return FileManager.default.fileExists(atPath: config) ? ["-F", config] : ["-F", "/dev/null"]
    }

    // MARK: Connection (tunnel.ts ensureEnvironment)

    struct Target { let alias: String; let hostname: String; let username: String?; let port: Int? }

    static func target(_ request: [String: Any]) throws -> Target {
        let alias = (request["alias"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        let hostname = (request["hostname"] as? String ?? alias).trimmingCharacters(in: .whitespaces)
        let username = (request["username"] as? String).flatMap { $0.trimmingCharacters(in: .whitespaces).isEmpty ? nil : $0.trimmingCharacters(in: .whitespaces) }
        let port = request["port"] as? Int
        guard !(alias.isEmpty && hostname.isEmpty), alias.count <= 255, hostname.count <= 255 else { throw T3Failure(kind: "Arguments", message: "SSH host or alias is required.") }
        let unsafe = CharacterSet(charactersIn: " \t\n\r;&|`$<>\"'\\")
        guard (alias + hostname + (username ?? "")).rangeOfCharacter(from: unsafe) == nil, !alias.hasPrefix("-"), !hostname.hasPrefix("-") else {
            throw T3Failure(kind: "Arguments", message: "SSH host or alias is not valid.")
        }
        if let port, !(1...65_535).contains(port) { throw T3Failure(kind: "Arguments", message: "SSH port must be between 1 and 65535.") }
        return Target(alias: alias.isEmpty ? hostname : alias, hostname: hostname.isEmpty ? alias : hostname, username: username, port: port)
    }

    static func connectionKey(_ target: Target) -> String { "\(target.alias)\u{0}\(target.hostname)\u{0}\(target.username ?? "")\u{0}\(target.port.map(String.init) ?? "")" }
    static func stateKey(_ target: Target) -> String { String(SHA256.hash(data: Data(connectionKey(target).utf8)).map { String(format: "%02x", $0) }.joined().prefix(16)) }
    static func hostSpec(_ target: Target) -> String { target.username.map { "\($0)@\(target.alias)" } ?? target.alias }
    private func baseArgs(_ target: Target) -> [String] {
        configArgs() + ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"] + (target.port.map { ["-p", String($0)] } ?? [])
    }
    /// A stable loopback port per target, so the saved origin survives relaunches.
    static func preferredPort(_ target: Target) -> Int {
        let digest = SHA256.hash(data: Data(connectionKey(target).utf8)).prefix(2).reduce(0) { $0 << 8 | Int($1) }
        return 41_000 + digest % 8_000
    }

    func connect(_ target: Target, pair: Bool) throws -> [String: Any] {
        guard let command = sshCommand else { throw T3Failure(kind: "Unavailable", message: "SSH is unavailable in this run.") }
        let key = Self.connectionKey(target)
        lock.lock(); let existing = tunnels[key]; lock.unlock()
        var origin = ""
        var remote: (port: Int, kind: String) = (Self.defaultRemotePort, "external")
        if let existing, existing.isRunning, let port = Self.forwardedPort(existing), Self.httpReady(port, timeout: 2) {
            origin = "http://127.0.0.1:\(port)"
        } else {
            existing?.terminate()
            remote = try launchRemote(command, target)
            var local = Self.preferredPort(target)
            for _ in 0..<64 where !Self.portFree(local) { local = 41_000 + (local - 41_000 + 1) % 8_000 }
            let process = try openTunnel(command, target, local: local, remote: remote.port)
            lock.lock(); tunnels[key] = process; let stillAlive = alive; lock.unlock()
            if !stillAlive { process.terminate(); throw T3Failure(kind: "Closed", message: "The window was closed.") }
            origin = "http://127.0.0.1:\(local)"
        }
        remember(origin: origin, target: target)
        var value: [String: Any] = ["origin": origin, "alias": target.alias, "remotePort": remote.port, "serverKind": remote.kind]
        value["credential"] = pair ? try issuePairingToken(command, target) : ""
        return value
    }

    private func launchRemote(_ command: String, _ target: Target) throws -> (port: Int, kind: String) {
        let result = try run(command, baseArgs(target) + [Self.hostSpec(target), "sh", "-l", "-s", "--", Self.stateKey(target)], stdin: Self.launchScript, timeout: 60)
        guard result.status == 0 else { throw T3Failure(kind: "Ssh", message: Self.sshError(result.stderr, fallback: "SSH could not start T3 Code on \(target.alias).")) }
        guard let json = Self.jsonObject(result.stdout), let port = json["remotePort"] as? Int, (1...65_535).contains(port) else {
            throw T3Failure(kind: "Ssh", message: "SSH launch did not return a remote port.")
        }
        return (port, json["serverKind"] as? String ?? "managed")
    }

    private func openTunnel(_ command: String, _ target: Target, local: Int, remote: Int) throws -> Process {
        let process = Process(), errors = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        process.arguments = baseArgs(target) + ["-o", "ExitOnForwardFailure=yes", "-o", "ControlMaster=no", "-o", "ControlPath=none", "-o", "ControlPersist=no",
                                                "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3", "-n", "-N", "-L", "\(local):127.0.0.1:\(remote)", Self.hostSpec(target)]
        process.standardInput = FileHandle.nullDevice; process.standardOutput = FileHandle.nullDevice; process.standardError = errors
        try process.run()
        let deadline = Date().addingTimeInterval(Self.readyTimeout)
        while Date() < deadline {
            if !process.isRunning {
                let stderr = String(data: errors.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
                throw T3Failure(kind: "Ssh", message: Self.sshError(stderr, fallback: "SSH tunnel exited unexpectedly for \(target.alias) (exit \(process.terminationStatus))."))
            }
            if Self.httpReady(local, timeout: 1) { return process }
            Thread.sleep(forTimeInterval: 0.2)
        }
        process.terminate()
        throw T3Failure(kind: "Ssh", message: "The T3 server on \(target.alias) did not answer through the SSH tunnel.")
    }

    private func issuePairingToken(_ command: String, _ target: Target) throws -> String {
        let result = try run(command, baseArgs(target) + [Self.hostSpec(target), "sh", "-l", "-s"], stdin: Self.pairingScript, timeout: 60) // login shell: t3 on the remote PATH
        guard result.status == 0 else { throw T3Failure(kind: "Ssh", message: Self.sshError(result.stderr, fallback: "SSH pairing failed on \(target.alias).")) }
        guard Self.lastLine(result.stdout) != nil else { throw T3Failure(kind: "Ssh", message: "SSH pairing did not return a credential.") }
        guard let json = Self.jsonObject(result.stdout) else { throw T3Failure(kind: "Ssh", message: "SSH pairing returned unparseable output.") }
        guard let credential = json["credential"] as? String, !credential.trimmingCharacters(in: .whitespaces).isEmpty else {
            throw T3Failure(kind: "Ssh", message: "SSH pairing command returned an invalid credential.")
        }
        return credential
    }

    // MARK: Saved targets

    private func savedTargets() -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        if agent { return memoryTargets }
        return UserDefaults.standard.dictionary(forKey: targetsKey) ?? [:]
    }
    private func remember(origin: String, target: Target) {
        var entry: [String: Any] = ["alias": target.alias, "hostname": target.hostname]
        if let username = target.username { entry["username"] = username }
        if let port = target.port { entry["port"] = port }
        lock.lock(); defer { lock.unlock() }
        if agent { memoryTargets[origin] = entry; return }
        var all = UserDefaults.standard.dictionary(forKey: targetsKey) ?? [:]
        all[origin] = entry
        UserDefaults.standard.set(all, forKey: targetsKey)
    }
    private func forget(origin: String) {
        lock.lock()
        if agent { memoryTargets.removeValue(forKey: origin) } else {
            var all = UserDefaults.standard.dictionary(forKey: targetsKey) ?? [:]
            all.removeValue(forKey: origin); UserDefaults.standard.set(all, forKey: targetsKey)
        }
        let matching = tunnels.filter { Self.forwardedPort($0.value).map { "http://127.0.0.1:\($0)" } == origin }
        for (key, process) in matching { tunnels.removeValue(forKey: key); process.terminate() }
        lock.unlock()
    }
    private func reopenSaved() {
        for (origin, value) in savedTargets() {
            guard let entry = value as? [String: Any], let target = try? Self.target(entry) else { continue }
            if let port = URL(string: origin)?.port, Self.httpReady(port, timeout: 1) { continue }
            _ = try? connect(target, pair: false)
        }
    }

    // MARK: Processes and probes

    struct Output { let status: Int32; let stdout: String; let stderr: String }

    private func run(_ command: String, _ arguments: [String], stdin: String?, timeout: TimeInterval) throws -> Output {
        let process = Process(), output = Pipe(), errors = Pipe(), input = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        process.arguments = arguments
        process.standardOutput = output; process.standardError = errors
        process.standardInput = stdin == nil ? FileHandle.nullDevice : input
        do { try process.run() } catch { throw T3Failure(kind: "Ssh", message: "SSH could not be started: \(error.localizedDescription)") }
        if let stdin {
            input.fileHandleForWriting.write(Data(stdin.utf8))
            try? input.fileHandleForWriting.close()
        }
        var stdoutData = Data(), stderrData = Data()
        let group = DispatchGroup()
        group.enter(); DispatchQueue.global().async { stdoutData = output.fileHandleForReading.readDataToEndOfFile(); group.leave() }
        group.enter(); DispatchQueue.global().async { stderrData = errors.fileHandleForReading.readDataToEndOfFile(); group.leave() }
        if group.wait(timeout: .now() + timeout) == .timedOut {
            process.terminate()
            throw T3Failure(kind: "Ssh", message: "SSH did not finish within \(Int(timeout)) seconds.")
        }
        process.waitUntilExit()
        return Output(status: process.terminationStatus, stdout: String(data: stdoutData, encoding: .utf8) ?? "", stderr: String(data: stderrData, encoding: .utf8) ?? "")
    }

    static func forwardedPort(_ process: Process) -> Int? {
        guard let arguments = process.arguments, let index = arguments.firstIndex(of: "-L"), index + 1 < arguments.count else { return nil }
        return arguments[index + 1].split(separator: ":").first.flatMap { Int($0) }
    }

    static func portFree(_ port: Int) -> Bool {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { return false }
        defer { close(socket) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = in_port_t(UInt16(port).bigEndian)
        address.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        let bound = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        return bound == 0
    }

    static func httpReady(_ port: Int, timeout: TimeInterval) -> Bool {
        guard let url = URL(string: "http://127.0.0.1:\(port)/") else { return false }
        var request = URLRequest(url: url, timeoutInterval: timeout)
        request.httpMethod = "GET"
        let done = DispatchSemaphore(value: 0)
        var ready = false
        let task = URLSession(configuration: .ephemeral).dataTask(with: request) { _, response, _ in
            if let http = response as? HTTPURLResponse, http.statusCode < 500 { ready = true }
            done.signal()
        }
        task.resume()
        _ = done.wait(timeout: .now() + timeout + 0.5)
        return ready
    }

    /// decodeRemoteJsonOutput: the whole output, else its last line, else the last `{…}` block.
    static func jsonObject(_ stdout: String) -> [String: Any]? {
        let parse = { (text: String) in (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any] }
        let trimmed = stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        if let whole = parse(trimmed) { return whole }
        if let line = lastLine(stdout), let parsed = parse(line) { return parsed }
        if let start = trimmed.range(of: "\n{", options: .backwards)?.lowerBound { return parse(String(trimmed[start...])) }
        return nil
    }
    static func lastLine(_ text: String) -> String? {
        text.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.last { !$0.isEmpty }
    }
    /// normalizeSshErrorMessage: the last meaningful stderr line, tokens redacted.
    static func sshError(_ stderr: String, fallback: String) -> String {
        let redacted = stderr.replacingOccurrences(of: #""(access_token|bearerToken|credential|pairingToken|token)"\s*:\s*"[^"]+""#, with: "\"$1\":\"[redacted]\"", options: .regularExpression)
        let lines = redacted.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty && !$0.hasPrefix("Warning: Permanently added") }
        guard let last = lines.last else { return fallback }
        return String(last.prefix(400))
    }

    static func success(_ value: [String: Any]) -> [String: Any] { ["ok": true, "generation": 0, "value": value] }
    static func failure(_ error: Error) -> [String: Any] {
        let failure = error as? T3Failure
        let detail: [String: Any] = ["kind": failure?.kind ?? "Ssh", "message": failure?.message ?? error.localizedDescription, "uncertain": false]
        return ["ok": false, "generation": 0, "error": detail]
    }

    // The remote side: reuse the default server (its userdata/server-runtime.json names a live
    // loopback port) or start `t3 serve` on the first free port from 3773, then print
    // {"remotePort","serverKind"} (tunnel.ts REMOTE_LAUNCH_SCRIPT, without the archive installer).
    static let launchScript = #"""
set -eu
STATE_DIR="$HOME/.t3/ssh-launch/${1:-default}"
DEFAULT_SERVER_HOME="$HOME/.t3"
RUNTIME_FILE="$DEFAULT_SERVER_HOME/userdata/server-runtime.json"
LOG_FILE="$STATE_DIR/server.log"
mkdir -p "$STATE_DIR"
probe() {
  if command -v curl >/dev/null 2>&1; then curl -s -o /dev/null -m 1 "http://127.0.0.1:$1/"; return $?; fi
  if command -v wget >/dev/null 2>&1; then wget -q -O /dev/null -T 1 "http://127.0.0.1:$1/"; return $?; fi
  return 1
}
wait_ready() {
  WAIT_COUNT=0
  while [ "$WAIT_COUNT" -lt "$2" ]; do
    if probe "$1"; then return 0; fi
    WAIT_COUNT=$((WAIT_COUNT + 1)); sleep 0.5
  done
  return 1
}
if [ -f "$RUNTIME_FILE" ]; then
  REMOTE_PORT="$(sed -n 's/.*"port"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$RUNTIME_FILE" | head -n 1)"
  if [ -n "$REMOTE_PORT" ] && wait_ready "$REMOTE_PORT" 4; then
    printf '{"remotePort":%s,"serverKind":"external"}\n' "$REMOTE_PORT"; exit 0
  fi
fi
if ! command -v t3 >/dev/null 2>&1; then
  printf 'T3 Code is not installed on this host. Install it (npx t3), then add the environment again.\n' >&2; exit 1
fi
REMOTE_PORT=3773
while probe "$REMOTE_PORT"; do
  REMOTE_PORT=$((REMOTE_PORT + 1))
  if [ "$REMOTE_PORT" -gt 3972 ]; then printf 'Failed to find an available port on the remote host.\n' >&2; exit 1; fi
done
nohup env T3CODE_NO_BROWSER=1 t3 serve --host 127.0.0.1 --port "$REMOTE_PORT" --base-dir "$DEFAULT_SERVER_HOME" >>"$LOG_FILE" 2>&1 < /dev/null &
printf '%s\n' "$!" >"$STATE_DIR/pid"
if ! wait_ready "$REMOTE_PORT" 40; then
  printf 'Remote T3 server did not become ready on 127.0.0.1:%s.\n' "$REMOTE_PORT" >&2
  tail -n 20 "$LOG_FILE" >&2 2>/dev/null || true
  exit 1
fi
printf '{"remotePort":%s,"serverKind":"managed"}\n' "$REMOTE_PORT"
"""#

    // tunnel.ts REMOTE_PAIRING_SCRIPT: a pairing token from the remote's default server home.
    static let pairingScript = #"""
set -eu
t3 auth pairing create --base-dir "$HOME/.t3" --json
"""#
}
