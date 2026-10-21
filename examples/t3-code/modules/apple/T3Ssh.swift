// Settings → Connections → Add Environment → SSH, after the
// desktop app's DesktopSshEnvironment and @t3tools/ssh: hosts come from
// ~/.ssh/config (Host aliases and their HostName, Include globs) and known_hosts; an alias resolves
// through `ssh -G`; connecting reuses the remote T3 server (or starts `t3 serve`
// there), forwards a loopback port with `ssh -N -L`, issues a pairing token with
// `t3 auth pairing create --json`, and the environment then pairs like any other.
// Tunnels are remembered by origin and reopened at launch, so a saved SSH
// environment reconnects on the same loopback port.
//
// Agent runs never read the real ~/.ssh or reach a real host: discovery reads
// only T3_SSH_HOME and connecting requires the T3_SSH_COMMAND test double.
//
// Password hosts (tunnel.ts runWithSshAuthAttempt, at 1e2ecbd975): each run starts with
// BatchMode=yes; a refused key asks for a password (T3SshAuth.swift, two prompts at most)
// and the run is repeated with the askpass helper. The password is kept in memory for the
// target's later runs and dropped when it fails.
import Foundation
import AppKit
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
    let prompts: T3SshPrompts
    private var secrets: [String: String] = [:] // authSecrets: by connection key, memory only
    private var askpass: String?
    private let targetsKey = "t3.ssh.targets"
    static let defaultRemotePort = 3773
    static let readyTimeout: TimeInterval = 20

    /// `promptsAvailable`: a window can show the password dialog (the module); the AppKit tests have none.
    init(agent: Bool, promptsAvailable: Bool = false, changed: @escaping (String) -> Void = { _ in }) {
        self.agent = agent
        let timeout = agent ? (ProcessInfo.processInfo.environment["T3_SSH_PROMPT_TIMEOUT_MS"].flatMap(Double.init).map { $0 / 1000 } ?? T3SshPrompts.defaultTimeout) : T3SshPrompts.defaultTimeout // agent-only expiry seam
        prompts = T3SshPrompts(available: promptsAvailable, timeout: timeout, changed: { changed("t3.ssh-prompt") })
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
                case "sshPromptState":
                    completion(Self.success(prompts.state()))
                case "sshPromptResolve":
                    let result = prompts.resolve(id: request["requestId"] as? String ?? "", answer: request["answer"] as? String ?? "cancel")
                    completion(result.ok ? Self.success(["resolved": true]) : Self.failure(T3Failure(kind: "Prompt", message: result.message)))
                case "sshForget":
                    forget(origin: request["origin"] as? String ?? "")
                    completion(Self.success(["forgotten": true]))
                default:
                    completion(Self.failure(T3Failure(kind: "Arguments", message: "Unknown SSH operation.")))
                }
            } catch { completion(Self.failure(error)) }
        }
    }

    /// The module's teardown (T3Module.destroy): since exact2 #200 the host destroys every session
    /// at quit and when the last window closes, so this runs on every orderly termination.
    func destroy() {
        prompts.closeAll(windowClosed: true) // "SSH authentication was cancelled because the app window closed."
        lock.lock(); alive = false; let all = Array(tunnels.values); tunnels.removeAll(); secrets.removeAll(); let helper = askpass; askpass = nil; lock.unlock()
        for process in all where process.isRunning { process.terminate() }
        if let helper { try? FileManager.default.removeItem(atPath: ((helper as NSString).deletingLastPathComponent as NSString).deletingLastPathComponent) }
    }

    // MARK: Authentication (tunnel.ts runWithSshAuthAttempt / handleSshAuthFailure / promptForPassword)

    struct Auth { let batch: Bool; let environment: [String: String]? }

    private func askpassHelper() throws -> String {
        lock.lock(); defer { lock.unlock() }
        if let askpass { return askpass }
        do { let path = try T3SshAuth.makeAskpassHelper(); askpass = path; return path }
        catch { throw T3Failure(kind: "Ssh", message: "Failed to prepare SSH authentication helpers.") }
    }

    /// The options for one attempt: no secret yet runs with BatchMode=yes while a dialog can ask
    /// (interactive askpass without a secret otherwise); a secret runs interactively through the helper.
    func authOptions(secret: String?) throws -> Auth {
        if secret == nil && prompts.available { return Auth(batch: true, environment: nil) }
        return Auth(batch: false, environment: T3SshAuth.childEnvironment(base: environment, interactive: true, askpass: try askpassHelper(), secret: secret))
    }

    /// runWithSshAuth: the operation, asked again with a password after an authentication failure.
    func withAuth<T>(_ target: Target, _ operation: (Auth) throws -> T) throws -> T {
        let key = Self.connectionKey(target)
        lock.lock(); var secret = secrets[key]; lock.unlock()
        var promptCount = 0
        while true {
            do { return try operation(try authOptions(secret: secret)) }
            catch {
                let message = (error as? T3Failure)?.message ?? error.localizedDescription
                guard T3SshAuth.isAuthFailure(message), prompts.available else { throw error }
                if secret != nil { lock.lock(); secrets.removeValue(forKey: key); lock.unlock() }
                if promptCount >= 2 { throw error }
                promptCount += 1
                let password = try promptForPassword(target, attempt: promptCount)
                lock.lock(); secrets[key] = password; lock.unlock()
                secret = password
            }
        }
    }

    private func promptForPassword(_ target: Target, attempt: Int) throws -> String {
        let hostSpec = Self.hostSpec(target)
        let destination = target.alias.isEmpty ? target.hostname : target.alias
        let outcome = prompts.request(destination: destination, username: target.username, prompt: "Enter the SSH password or verification code for \(hostSpec).", attempt: attempt)
        if case .password(let password) = outcome { return password }
        throw T3Failure(kind: "Ssh", message: T3SshPrompts.message(outcome, destination: destination))
    }

    // MARK: Discovery (@t3tools/ssh config.ts discoverSshHosts, at 74541bab17)

    /// Every concrete `Host` alias with the `HostName` OpenSSH would use for it (the alias when that is uncertain),
    /// then the known_hosts names that are neither an alias nor a configured hostname, sorted by alias.
    func discover() -> [[String: Any]] {
        guard let home = discoveryHome else { return [] }
        let ssh = (home as NSString).appendingPathComponent(".ssh")
        let config = Self.readConfig((ssh as NSString).appendingPathComponent("config"), home: home)
        let known = Self.knownHosts((try? String(contentsOfFile: (ssh as NSString).appendingPathComponent("known_hosts"), encoding: .utf8)) ?? "")
        var discovered: [String: [String: Any]] = [:], configured = Set<String>()
        for alias in config.aliases {
            let hostname = Self.resolveConfiguredHostname(alias, config.rules)
            if let hostname { configured.insert(hostname.lowercased()) }
            discovered[alias] = ["alias": alias, "hostname": hostname ?? alias, "username": NSNull(), "port": NSNull(), "source": "ssh-config"]
        }
        for hostname in known where discovered[hostname] == nil && !configured.contains(hostname.lowercased()) {
            discovered[hostname] = ["alias": hostname, "hostname": hostname, "username": NSNull(), "port": NSNull(), "source": "known-hosts"]
        }
        // `left.alias.localeCompare(right.alias)`.
        return discovered.keys.sorted { $0.localizedCompare($1) == .orderedAscending }.map { discovered[$0]! }
    }

    static func hasPattern(_ value: String) -> Bool { value.contains("*") || value.contains("?") || value.hasPrefix("!") }

    /// parseSshConfigLine, after OpenSSH readconf: the keyword may be followed by one optional "=", then the
    /// arguments split like misc.c argv_split() with quotes, the escapes \\ \" \' and an unquoted "\ ", and "#"
    /// starting a comment only at the start of a token. `args` is nil for an unterminated quote, which leaves
    /// hostname resolution uncertain.
    static func parseConfigLine(_ line: String) -> (directive: String, args: [String]?)? {
        let characters = Array(line)
        var index = 0
        while index < characters.count, characters[index].isWhitespace { index += 1 }
        var keyword = ""
        while index < characters.count, !characters[index].isWhitespace, characters[index] != "=" { keyword.append(characters[index]); index += 1 }
        if keyword.isEmpty { return nil }
        while index < characters.count, characters[index].isWhitespace { index += 1 }
        if index < characters.count, characters[index] == "=" { index += 1 }
        while index < characters.count, characters[index].isWhitespace { index += 1 }
        var args: [String] = [], token: String?, quote: Character?
        while index < characters.count {
            let char = characters[index], next: Character? = index + 1 < characters.count ? characters[index + 1] : nil
            if char == "\\", let next, next == "\"" || next == "'" || next == "\\" || (quote == nil && next == " ") {
                token = (token ?? "") + String(next); index += 1
            } else if let open = quote {
                if char == open { quote = nil } else { token = (token ?? "") + String(char) }
            } else if char == "\"" || char == "'" {
                quote = char; token = token ?? ""
            } else if char.isWhitespace {
                if let finished = token { args.append(finished); token = nil }
            } else if char == "#" && token == nil {
                break
            } else {
                token = (token ?? "") + String(char)
            }
            index += 1
        }
        if quote != nil { return (keyword.lowercased(), nil) }
        if let token { args.append(token) }
        return (keyword.lowercased(), args)
    }

    /// A `Host` line's patterns, or nil after a `Match` (its condition is not evaluated here).
    typealias HostCondition = [String]?
    struct HostNameRule { let conditions: [HostCondition]; let hostname: String? }

    static func globMatches(_ pattern: String, _ value: String) -> Bool {
        let escaped = NSRegularExpression.escapedPattern(for: pattern).replacingOccurrences(of: "\\*", with: ".*").replacingOccurrences(of: "\\?", with: ".")
        guard let matcher = try? NSRegularExpression(pattern: "^\(escaped)$") else { return false }
        return matcher.firstMatch(in: value, range: NSRange(value.startIndex..., in: value)) != nil
    }

    static func matchesHostPatterns(_ alias: String, _ patterns: [String]) -> Bool {
        var matched = false
        for pattern in patterns {
            let negated = pattern.hasPrefix("!"), hostPattern = negated ? String(pattern.dropFirst()) : pattern
            let matches = hostPattern == "*" || (hasPattern(hostPattern) ? globMatches(hostPattern, alias) : hostPattern == alias)
            if !matches { continue }
            if negated { return false }
            matched = true
        }
        return matched
    }

    /// resolveConfiguredHostname: SSH takes the first applicable value, so an uncertain earlier one blocks later fallbacks.
    static func resolveConfiguredHostname(_ alias: String, _ rules: [HostNameRule]) -> String? {
        for rule in rules {
            if rule.conditions.contains(where: { condition in condition.map { !matchesHostPatterns(alias, $0) } ?? false }) { continue }
            guard !rule.conditions.contains(where: { $0 == nil }), let hostname = rule.hostname else { return nil }
            if hostname.range(of: "%(?!h)", options: .regularExpression) != nil || hostname.contains("$") { return nil }
            return hostname.replacingOccurrences(of: "%h", with: alias)
        }
        return nil
    }

    /// collectSshConfigAliasesFromFile over the whole Include tree.
    static func readConfig(_ path: String, home: String) -> (aliases: Set<String>, rules: [HostNameRule]) {
        var visited = Set<String>(), aliases = Set<String>(), rules: [HostNameRule] = []
        collectConfig(path, home: home, visited: &visited, aliases: &aliases, rules: &rules, inherited: [])
        return (aliases, rules)
    }

    /// The config's concrete `Host` aliases, sorted (the tests' view of discovery's first half).
    static func configAliases(_ path: String, home: String, visited: inout Set<String>) -> [String] {
        var aliases = Set<String>(), rules: [HostNameRule] = []
        collectConfig(path, home: home, visited: &visited, aliases: &aliases, rules: &rules, inherited: [])
        return aliases.sorted()
    }

    private static func collectConfig(_ path: String, home: String, visited: inout Set<String>, aliases: inout Set<String>, rules: inout [HostNameRule], inherited: [HostCondition]) {
        let resolved = (path as NSString).standardizingPath
        if visited.contains(resolved) {
            // Read each file once, but never reuse its rules under a different Include context.
            rules.append(HostNameRule(conditions: inherited, hostname: nil))
            return
        }
        guard let raw = try? String(contentsOfFile: resolved, encoding: .utf8) else { return }
        visited.insert(resolved)
        var condition: HostCondition = ["*"]
        for line in raw.components(separatedBy: .newlines) {
            guard let parsed = parseConfigLine(line) else { continue }
            let (directive, args) = parsed
            let conditions = inherited + [condition]
            switch directive {
            case "host":
                condition = args
                for alias in args ?? [] where !alias.isEmpty && !hasPattern(alias) { aliases.insert(alias) }
            case "match":
                condition = nil
            case "hostname":
                rules.append(HostNameRule(conditions: conditions, hostname: args?.count == 1 ? args![0] : nil))
            case "include":
                guard let args else { rules.append(HostNameRule(conditions: conditions, hostname: nil)); continue }
                for pattern in args {
                    let expanded = pattern == "~" || pattern.hasPrefix("~/") ? home + pattern.dropFirst() : pattern
                    let absolute = expanded.hasPrefix("/") ? expanded : ((home as NSString).appendingPathComponent(".ssh") as NSString).appendingPathComponent(expanded)
                    // Tokens, environment variables, character classes, `~user` and globbed directories are not expanded here.
                    if pattern.range(of: "%|\\$\\{|\\[", options: .regularExpression) != nil || (pattern.hasPrefix("~") && expanded == pattern)
                        || hasPattern((absolute as NSString).deletingLastPathComponent) {
                        rules.append(HostNameRule(conditions: conditions, hostname: nil))
                        continue
                    }
                    for included in expandGlob(absolute) {
                        collectConfig(included, home: home, visited: &visited, aliases: &aliases, rules: &rules, inherited: conditions)
                    }
                }
            default:
                continue
            }
        }
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
    private func baseArgs(_ target: Target, _ auth: Auth) -> [String] {
        configArgs() + ["-o", "BatchMode=\(auth.batch ? "yes" : "no")", "-o", "ConnectTimeout=10"] + (target.port.map { ["-p", String($0)] } ?? [])
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
            remote = try withAuth(target) { try launchRemote(command, target, $0) }
            var local = Self.preferredPort(target)
            for _ in 0..<64 where !Self.portFree(local) { local = 41_000 + (local - 41_000 + 1) % 8_000 }
            let process = try withAuth(target) { try openTunnel(command, target, local: local, remote: remote.port, $0) }
            lock.lock(); tunnels[key] = process; let stillAlive = alive; lock.unlock()
            if !stillAlive { process.terminate(); throw T3Failure(kind: "Closed", message: "The window was closed.") }
            origin = "http://127.0.0.1:\(local)"
        }
        remember(origin: origin, target: target)
        var value: [String: Any] = ["origin": origin, "alias": target.alias, "remotePort": remote.port, "serverKind": remote.kind]
        value["credential"] = pair ? try withAuth(target) { try issuePairingToken(command, target, $0) } : ""
        return value
    }

    private func launchRemote(_ command: String, _ target: Target, _ auth: Auth) throws -> (port: Int, kind: String) {
        let result = try run(command, baseArgs(target, auth) + [Self.hostSpec(target), "sh", "-l", "-s", "--", Self.stateKey(target)], stdin: Self.launchScript, timeout: 60, environment: auth.environment)
        guard result.status == 0 else { throw T3Failure(kind: "Ssh", message: Self.sshError(result.stderr, fallback: "SSH could not start T3 Code on \(target.alias).")) }
        guard let json = Self.jsonObject(result.stdout), let port = json["remotePort"] as? Int, (1...65_535).contains(port) else {
            throw T3Failure(kind: "Ssh", message: "SSH launch did not return a remote port.")
        }
        return (port, json["serverKind"] as? String ?? "managed")
    }

    private func openTunnel(_ command: String, _ target: Target, local: Int, remote: Int, _ auth: Auth) throws -> Process {
        let process = Process(), errors = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        if let environment = auth.environment { process.environment = environment }
        process.arguments = baseArgs(target, auth) + ["-o", "ExitOnForwardFailure=yes", "-o", "ControlMaster=no", "-o", "ControlPath=none", "-o", "ControlPersist=no",
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

    private func issuePairingToken(_ command: String, _ target: Target, _ auth: Auth) throws -> String {
        let result = try run(command, baseArgs(target, auth) + [Self.hostSpec(target), "sh", "-l", "-s"], stdin: Self.pairingScript, timeout: 60, environment: auth.environment) // login shell: t3 on the remote PATH
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

    func run(_ command: String, _ arguments: [String], stdin: String?, timeout: TimeInterval, environment: [String: String]? = nil) throws -> Output {
        let process = Process(), output = Pipe(), errors = Pipe(), input = Pipe()
        process.executableURL = URL(fileURLWithPath: command)
        if let environment { process.environment = environment }
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

    static func loopbackAddress(_ port: Int) -> sockaddr_in {
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = in_port_t(UInt16(port).bigEndian)
        address.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        return address
    }

    /// sshHelper.ts pick-port: the port binds on loopback and nothing answers there. macOS lets the bind
    /// succeed while another process listens on *:<port>, so the connect check is needed too.
    static func portFree(_ port: Int) -> Bool { canBind(port) && !answersOnLoopback(port) }

    static func canBind(_ port: Int) -> Bool {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { return false }
        defer { close(socket) }
        var address = loopbackAddress(port)
        let bound = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        return bound == 0
    }

    /// A connection to 127.0.0.1:<port> is accepted, or still pending after `timeout` (both count as taken).
    static func answersOnLoopback(_ port: Int, timeout: TimeInterval = 1) -> Bool {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { return false }
        defer { close(socket) }
        _ = fcntl(socket, F_SETFL, fcntl(socket, F_GETFL) | O_NONBLOCK)
        var address = loopbackAddress(port)
        let result = withUnsafePointer(to: &address) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        if result == 0 { return true }
        guard errno == EINPROGRESS else { return false }
        var descriptor = pollfd(fd: socket, events: Int16(POLLOUT), revents: 0)
        let ready = poll(&descriptor, 1, Int32(timeout * 1000))
        if ready == 0 { return true }
        var failure: Int32 = 0
        var length = socklen_t(MemoryLayout<Int32>.size)
        guard ready > 0, getsockopt(socket, SOL_SOCKET, SO_ERROR, &failure, &length) == 0 else { return false }
        return failure == 0
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
    // loopback port), wait for a server this script started that is still running, or start
    // `t3 serve` on the first port from 3773 that nothing answers on, then print
    // {"remotePort","serverKind"} (tunnel.ts REMOTE_LAUNCH_SCRIPT at 9c161462c9, without the archive
    // installer; the arguments arrive as "$1", so no value is substituted into the script).
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
# Anything that accepts or holds a loopback connection takes the port, HTTP or not (curl's 7 is a
# refused connection; a timeout counts as taken).
answers() {
  if command -v curl >/dev/null 2>&1; then
    curl -s -o /dev/null -m 1 "http://127.0.0.1:$1/" && return 0
    [ "$?" -ne 7 ]
    return
  fi
  probe "$1"
}
# kill -0 also succeeds for a server that exited but was not reaped yet, so
# a zombie does not count as running. Without ps to tell, trust kill -0.
server_running() {
  [ -n "$1" ] && kill -0 "$1" 2>/dev/null || return 1
  case "$(ps -o stat= -p "$1" 2>/dev/null || true)" in
    *Z*) return 1 ;;
  esac
}
wait_ready() {
  WAIT_COUNT=0
  while [ "$WAIT_COUNT" -lt "$2" ]; do
    if probe "$1"; then return 0; fi
    WAIT_COUNT=$((WAIT_COUNT + 1)); sleep 0.5
  done
  return 1
}
REMOTE_PORT=""
if [ -f "$RUNTIME_FILE" ]; then
  REMOTE_PORT="$(sed -n 's/.*"port"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$RUNTIME_FILE" | head -n 1)"
  if [ -n "$REMOTE_PORT" ] && wait_ready "$REMOTE_PORT" 4; then
    printf '{"remotePort":%s,"serverKind":"external"}\n' "$REMOTE_PORT"; exit 0
  fi
fi
# Reconnecting owns the tunnel, not the lifetime of a server already running: one this script
# started that is alive but slow is waited for and left running, never replaced.
MANAGED_PID="$(cat "$STATE_DIR/pid" 2>/dev/null || true)"
if server_running "$MANAGED_PID"; then
  MANAGED_PORT="$(cat "$STATE_DIR/port" 2>/dev/null || true)"
  [ -n "$MANAGED_PORT" ] || MANAGED_PORT="$REMOTE_PORT"
  if [ -n "$MANAGED_PORT" ] && wait_ready "$MANAGED_PORT" 40; then
    printf '{"remotePort":%s,"serverKind":"managed"}\n' "$MANAGED_PORT"; exit 0
  fi
  printf 'Remote T3 server is still running but did not become ready. Retry the connection; the server was left running.\n' >&2
  exit 1
fi
if ! command -v t3 >/dev/null 2>&1; then
  printf 'T3 Code is not installed on this host. Install it (npx t3), then add the environment again.\n' >&2; exit 1
fi
REMOTE_PORT=3773
while answers "$REMOTE_PORT"; do
  REMOTE_PORT=$((REMOTE_PORT + 1))
  if [ "$REMOTE_PORT" -gt 3972 ]; then printf 'Failed to find an available port on the remote host.\n' >&2; exit 1; fi
done
nohup env T3CODE_NO_BROWSER=1 t3 serve --host 127.0.0.1 --port "$REMOTE_PORT" --base-dir "$DEFAULT_SERVER_HOME" >>"$LOG_FILE" 2>&1 < /dev/null &
printf '%s\n' "$!" >"$STATE_DIR/pid"
printf '%s\n' "$REMOTE_PORT" >"$STATE_DIR/port"
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
