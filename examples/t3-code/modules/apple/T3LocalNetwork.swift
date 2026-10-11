// Network facts for "This machine" (20261005-this-machine-network-access item 2), after T3 Code
// (MIT, see LICENSE-T3; reference 1e2ecbd975): apps/desktop/src/backend/DesktopNetworkInterfaces.ts
// (`os.networkInterfaces()`), packages/tailscale/src/tailscale.ts (`readTailscaleStatus`: `tailscale
// status --json`, 1.5 s, the stderr diagnostics that never quote the CLI, `probeTailscaleHttpsEndpoint`:
// 2xx on `/.well-known/t3/environment` within 2.5 s) and DesktopServerExposure.ts (the status read is
// cached 60 s: on the App Store build each spawn can raise macOS's "Other apps" prompt).
//
// TypeScript asks for the facts (`localNetworkFacts`) and computes the exposure state and endpoints
// (server-exposure.ts). This answers at once from the caches and refreshes in the background; when a
// background read lands it announces `t3.local`, so a read never waits on the CLI or the network.
// At launch the bind host comes from desktop-settings.json and the interfaces here (`T3LocalExposure`), the
// same rule as resolveRuntimeState: a network request with no LAN and no Tailscale IPv4 binds loopback.
import Foundation
import Darwin

/// One `os.networkInterfaces()` address.
struct T3NetworkAddress: Equatable {
    let address: String
    let family: String
    let isInternal: Bool
}

/// DesktopNetworkInterfaces.
enum T3NetworkInterfaces {
    /// DesktopNetworkInterfacesReadError: the platform and the cause, a message without the cause's text.
    struct ReadError: Error {
        let platform: String
        let cause: Error
        var message: String { "Failed to read desktop network interfaces on \(platform)." }
    }

    /// `read`: the addresses by interface name, or a ReadError carrying the failure.
    static func read(platform: String = "darwin", _ source: () throws -> [String: [T3NetworkAddress]] = system) throws -> [String: [T3NetworkAddress]] {
        do { return try source() } catch { throw ReadError(platform: platform, cause: error) }
    }

    /// libuv's uv_interface_addresses: interfaces up and running, IPv4 and IPv6 only, `internal` for loopback.
    static func system() throws -> [String: [T3NetworkAddress]] {
        var head: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&head) == 0, let first = head else { throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno)) }
        defer { freeifaddrs(head) }
        var result: [String: [T3NetworkAddress]] = [:]
        var cursor: UnsafeMutablePointer<ifaddrs>? = first
        while let entry = cursor?.pointee {
            defer { cursor = entry.ifa_next }
            let flags = Int32(entry.ifa_flags)
            guard flags & IFF_UP != 0, flags & IFF_RUNNING != 0, let address = entry.ifa_addr else { continue }
            let family = Int32(address.pointee.sa_family)
            guard family == AF_INET || family == AF_INET6 else { continue }
            var buffer = [CChar](repeating: 0, count: Int(INET6_ADDRSTRLEN))
            let text: String?
            if family == AF_INET {
                text = address.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { pointer in
                    var value = pointer.pointee.sin_addr
                    return inet_ntop(AF_INET, &value, &buffer, socklen_t(buffer.count)).map { String(cString: $0) }
                }
            } else {
                text = address.withMemoryRebound(to: sockaddr_in6.self, capacity: 1) { pointer in
                    var value = pointer.pointee.sin6_addr
                    return inet_ntop(AF_INET6, &value, &buffer, socklen_t(buffer.count)).map { String(cString: $0) }
                }
            }
            guard let text else { continue }
            result[String(cString: entry.ifa_name), default: []].append(T3NetworkAddress(address: text, family: family == AF_INET ? "IPv4" : "IPv6", isInternal: flags & IFF_LOOPBACK != 0))
        }
        return result
    }

    static func json(_ interfaces: [String: [T3NetworkAddress]]) -> [String: Any] {
        interfaces.mapValues { $0.map { ["address": $0.address, "family": $0.family, "internal": $0.isInternal] } }
    }

    /// isTailscaleIpv4Address: 100.64.0.0/10.
    static func isTailscaleIpv4(_ address: String) -> Bool {
        let parts = address.split(separator: ".", omittingEmptySubsequences: false).map { Int($0) }
        guard parts.count == 4, parts.allSatisfy({ ($0 ?? -1) >= 0 && ($0 ?? 256) <= 255 }) else { return false }
        return parts[0] == 100 && (64...127).contains(parts[1]!)
    }
    /// A usable LAN IPv4 (resolveLanAdvertisedHost): not internal, not 127.x, 169.254.x or Tailscale.
    static func hasLanIpv4(_ interfaces: [String: [T3NetworkAddress]]) -> Bool {
        interfaces.values.contains { $0.contains { !$0.isInternal && $0.family == "IPv4" && !$0.address.hasPrefix("127.") && !$0.address.hasPrefix("169.254.") && !isTailscaleIpv4($0.address) } }
    }
    static func hasTailscaleIpv4(_ interfaces: [String: [T3NetworkAddress]]) -> Bool {
        interfaces.values.contains { $0.contains { !$0.isInternal && $0.family == "IPv4" && isTailscaleIpv4($0.address) } }
    }
}

/// The exposure part of the bootstrap envelope (`host`, `tailscaleServeEnabled`, `tailscaleServePort`).
struct T3LocalExposure: Equatable {
    var host = "127.0.0.1"
    var tailscaleServeEnabled = false
    var tailscaleServePort = 443

    static func normalizedPort(_ value: Any?) -> Int {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return 443 }
        let double = number.doubleValue
        return double.rounded() == double && double >= 1 && double <= 65_535 ? Int(double) : 443
    }

    /// At launch (configureFromSettings): the desktop settings' `serverExposureMode`, `tailscaleServeEnabled`
    /// and `tailscaleServePort` (T3DesktopSettings.swift); a network request binds 0.0.0.0 only when a LAN or
    /// Tailscale IPv4 (or an explicit T3CODE_DESKTOP_LAN_HOST) exists, else it falls back to loopback, keeping
    /// the preference.
    static func atLaunch(settings: T3DesktopSettings, interfaces: [String: [T3NetworkAddress]], lanHostOverride: String?) -> T3LocalExposure {
        var exposure = T3LocalExposure()
        exposure.tailscaleServeEnabled = settings.tailscaleServeEnabled
        exposure.tailscaleServePort = settings.tailscaleServePort
        if settings.serverExposureMode == "network-accessible",
           lanHostOverride != nil || T3NetworkInterfaces.hasLanIpv4(interfaces) || T3NetworkInterfaces.hasTailscaleIpv4(interfaces) {
            exposure.host = "0.0.0.0"
        }
        return exposure
    }
}

/// readTailscaleStatus over a runner (the tests replace it).
enum T3TailscaleCLI {
    static let statusTimeoutMs = 1_500.0
    static let probeTimeoutMs = 2_500.0

    struct Status: Equatable {
        var magicDnsName: String?
        var tailnetIpv4Addresses: [String]
    }

    /// The failure kinds; none carries the CLI's text (stderr can hold auth keys and node names).
    enum Failure: Error, Equatable {
        case spawn(subcommand: String, argumentCount: Int)
        case exit(subcommand: String, argumentCount: Int, exitCode: Int32, stdoutLength: Int, stderrLength: Int, diagnostic: String?)
        case timeout(subcommand: String, argumentCount: Int, timeoutMs: Int)
        case parse

        var message: String {
            switch self {
            case let .spawn(subcommand, _): return "Failed to spawn tailscale \(subcommand)."
            case let .exit(subcommand, _, exitCode, _, _, _): return "tailscale \(subcommand) exited with code \(exitCode)."
            case let .timeout(subcommand, _, timeoutMs): return "tailscale \(subcommand) timed out after \(timeoutMs)ms."
            case .parse: return "Failed to decode tailscale status JSON."
            }
        }
    }

    struct Output { var code: Int32; var stdout: Data; var stderr: Data }
    enum RunError: Error { case spawn, timeout }
    /// Runs `tailscale args`, answering once on any thread.
    typealias Runner = (_ args: [String], _ timeoutMs: Double, _ done: @escaping (Result<Output, RunError>) -> Void) -> Void

    /// TailscaleStderrDiagnostic: matched most specific first; anything else is "unknown"; empty stderr none.
    static func diagnostic(_ stderr: String) -> String? {
        if stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return nil }
        let patterns: [(String, String)] = [("handler does not exist", "no-existing-handler"), ("not logged in|logged out|needs? login", "not-logged-in"),
                                            ("permission denied|access denied|must be root|operation not permitted", "permission-denied")]
        for (pattern, label) in patterns where stderr.range(of: pattern, options: [.regularExpression, .caseInsensitive]) != nil { return label }
        return "unknown"
    }

    /// parseTailscaleStatus.
    static func parseStatus(_ data: Data) throws -> Status {
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw Failure.parse }
        if let selfValue = object["Self"], !(selfValue is [String: Any]) { throw Failure.parse }
        let me = object["Self"] as? [String: Any] ?? [:]
        let dnsName = (me["DNSName"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\\.$", with: "", options: .regularExpression)
        let ips = (me["TailscaleIPs"] as? [Any] ?? []).compactMap { $0 as? String }.filter(T3NetworkInterfaces.isTailscaleIpv4)
        return Status(magicDnsName: dnsName?.isEmpty == false ? dnsName : nil, tailnetIpv4Addresses: ips)
    }

    static func readStatus(runner: Runner, timeoutMs: Double = statusTimeoutMs, _ done: @escaping (Result<Status, Failure>) -> Void) {
        let args = ["status", "--json"]
        runner(args, timeoutMs) { result in
            switch result {
            case .failure(.spawn): done(.failure(.spawn(subcommand: "status", argumentCount: args.count)))
            case .failure(.timeout): done(.failure(.timeout(subcommand: "status", argumentCount: args.count, timeoutMs: Int(timeoutMs))))
            case let .success(output):
                guard output.code == 0 else {
                    return done(.failure(.exit(subcommand: "status", argumentCount: args.count, exitCode: output.code, stdoutLength: output.stdout.count,
                                               stderrLength: output.stderr.count, diagnostic: diagnostic(String(decoding: output.stderr, as: UTF8.self)))))
                }
                do { done(.success(try parseStatus(output.stdout))) } catch { done(.failure(.parse)) }
            }
        }
    }

    /// `tailscale` from the login shell's PATH (the desktop installs it into its environment), never a shell.
    static func processRunner(environment: @escaping () -> [String: String]) -> Runner {
        return { args, timeoutMs, done in
            DispatchQueue.global(qos: .utility).async {
                let env = environment()
                let executable = (env["PATH"] ?? "").split(separator: ":").map { URL(fileURLWithPath: String($0)).appendingPathComponent("tailscale").path }
                    .first { FileManager.default.isExecutableFile(atPath: $0) }
                guard let executable else { return done(.failure(.spawn)) }
                let process = Process(), stdout = Pipe(), stderr = Pipe()
                process.executableURL = URL(fileURLWithPath: executable)
                process.arguments = args
                process.environment = env
                process.standardInput = FileHandle.nullDevice
                process.standardOutput = stdout
                process.standardError = stderr
                final class Once { let lock = NSLock(); var given = false
                    func take() -> Bool { lock.lock(); defer { lock.unlock() }; if given { return false }; given = true; return true } }
                let once = Once()
                do { try process.run() } catch { return done(.failure(.spawn)) }
                DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(Int(timeoutMs))) {
                    guard once.take() else { return }
                    if process.isRunning { process.terminate() }
                    done(.failure(.timeout))
                }
                // Both pipes drain together, so a chatty stderr cannot stall stdout.
                var err = Data()
                let drained = DispatchGroup()
                drained.enter()
                DispatchQueue.global(qos: .utility).async { err = stderr.fileHandleForReading.readDataToEndOfFile(); drained.leave() }
                let out = stdout.fileHandleForReading.readDataToEndOfFile()
                drained.wait()
                process.waitUntilExit()
                guard once.take() else { return }
                done(.success(Output(code: process.terminationStatus, stdout: out, stderr: err)))
            }
        }
    }
}

/// The cached facts behind `localNetworkFacts`.
final class T3LocalNetwork: @unchecked Sendable {
    static let shared = T3LocalNetwork()
    static let statusTtl: TimeInterval = 60
    static let probeFreshness: TimeInterval = 30

    private let lock = NSLock()
    private var status: (value: T3TailscaleCLI.Status?, at: Date)?
    private var statusReading = false
    private var probes: [String: (reachable: Bool, at: Date)] = [:]
    private var probing = Set<String>()
    private var generation = 0

    // Seams (tests replace them).
    var interfaces: () throws -> [String: [T3NetworkAddress]] = { try T3NetworkInterfaces.read() }
    var environment: () -> [String: String] = { ProcessInfo.processInfo.environment }
    lazy var runner: T3TailscaleCLI.Runner = T3TailscaleCLI.processRunner(environment: { [unowned self] in self.shellEnvironment() })
    var prober: T3LocalProber = T3LocalHTTPProber()
    var now: () -> Date = { Date() }
    /// Announces `t3.local` after a background read changed what the facts say.
    var changed: () -> Void = {}

    private var shellEnv: [String: String]?
    private func shellEnvironment() -> [String: String] {
        lock.lock(); if let shellEnv { lock.unlock(); return shellEnv }; lock.unlock()
        var env = environment()
        T3LocalShellEnvironment.install(into: &env, run: T3LocalShellEnvironment.runCommand)
        lock.lock(); shellEnv = env; lock.unlock()
        return env
    }

    /// DesktopConfig: T3CODE_DESKTOP_LAN_HOST and T3CODE_DESKTOP_HTTPS_ENDPOINTS.
    var lanHostOverride: String? { T3LocalShellEnvironment.trimmed(environment()["T3CODE_DESKTOP_LAN_HOST"]) }
    var httpsEndpointUrls: [String] {
        (environment()["T3CODE_DESKTOP_HTTPS_ENDPOINTS"] ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    /// The restart (a new process in the reference) reads everything afresh.
    func invalidate() {
        lock.lock(); status = nil; probes = [:]; generation += 1; lock.unlock()
    }

    /// `localNetworkFacts {tailscale, probe, refresh}`: interfaces now; the status and the probe from
    /// their caches (`read: false` until the first answer), refreshed in the background when stale.
    func facts(tailscale wantsStatus: Bool, probe url: String, refresh: Bool) -> [String: Any] {
        var value: [String: Any] = ["lanHostOverride": lanHostOverride ?? "", "httpsEndpointUrls": httpsEndpointUrls]
        do { value["interfaces"] = T3NetworkInterfaces.json(try interfaces()) }
        catch { value["interfaces"] = [String: Any](); value["interfacesError"] = (error as? T3NetworkInterfaces.ReadError)?.message ?? "Failed to read desktop network interfaces on darwin." }
        let now = now()
        lock.lock()
        let cached = status, cachedProbe = url.isEmpty ? nil : probes[url]
        let readStatus = wantsStatus && !statusReading && (cached == nil || now.timeIntervalSince(cached!.at) >= Self.statusTtl)
        if readStatus { statusReading = true }
        let readProbe = !url.isEmpty && !probing.contains(url) && (cachedProbe == nil || refresh || now.timeIntervalSince(cachedProbe!.at) >= Self.probeFreshness)
        if readProbe { probing.insert(url) }
        let epoch = generation
        lock.unlock()
        if wantsStatus {
            value["tailscale"] = ["read": cached != nil, "magicDnsName": cached?.value?.magicDnsName ?? NSNull(), "tailnetIpv4Addresses": (cached?.value?.tailnetIpv4Addresses ?? []) as [String]]
        }
        if !url.isEmpty { value["probe"] = ["url": url, "read": cachedProbe != nil, "reachable": cachedProbe?.reachable ?? false] }
        if readStatus {
            T3TailscaleCLI.readStatus(runner: runner) { [self] result in
                lock.lock()
                statusReading = false
                // A failed read answers null (`Effect.orElseSucceed(() => null)`) and is cached like a success.
                let next = (try? result.get())
                let differs = status?.value != next || status == nil
                if epoch == generation { status = (next, self.now()) }
                lock.unlock()
                if case let .failure(failure) = result { FileHandle.standardError.write(Data("t3.local: \(failure.message)\n".utf8)) }
                if differs { changed() }
            }
        }
        if readProbe, let target = URL(string: "/.well-known/t3/environment", relativeTo: URL(string: url)) {
            // `timeoutOption(2_500)` is a deadline for the whole request; URLSession's own timeout only
            // bounds silence between bytes, so the first of the answer and the deadline counts.
            let once = T3Once()
            let settle = { [self] (reachable: Bool) in
                guard once.claim() else { return }
                lock.lock()
                probing.remove(url)
                let differs = probes[url]?.reachable != reachable || probes[url] == nil
                if epoch == generation { probes[url] = (reachable, self.now()) }
                lock.unlock()
                if differs { changed() }
            }
            prober.probe(target.absoluteURL, timeoutMs: T3TailscaleCLI.probeTimeoutMs, settle)
            DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + T3TailscaleCLI.probeTimeoutMs / 1000) { settle(false) }
        } else if readProbe {
            lock.lock(); probing.remove(url); probes[url] = (false, now); lock.unlock()
        }
        return value
    }
}

/// The first caller wins (a probe's answer or its deadline).
final class T3Once: @unchecked Sendable {
    private let lock = NSLock()
    private var done = false
    func claim() -> Bool { lock.lock(); defer { lock.unlock() }; if done { return false }; done = true; return true }
}
