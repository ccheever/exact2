// @exact/local-network on iOS, macOS and tvOS (LLP 1133.036 D1-D4, D8,
// D12, D13): Network framework's `NWBrowser` for the browse and dnssd's
// `DNSServiceResolve` and `DNSServiceGetAddrInfo` for the resolve. Under the
// agent the runner's declared substitute answers and nothing here runs; a
// bound run (`--bind local-network`) browses for real.
//
// - One browse per process and group (type, `includePeerToPeer`), shared by
//   every session that reads it, stopped when the last releases it or
//   leaves (D3); a resolve likewise per (name, type, `includePeerToPeer`).
//   Both pause while the app is in the background and answer afresh on
//   return.
// - A browse answers at its first results or one second after it started,
//   whichever comes first, then each change; instances are merged across
//   interfaces and ordered by name, case-insensitively (D2).
// - The host's fact (D8): `unknown` at launch where the target has local
//   network privacy (iOS on a device, macOS 15 and later), else `granted`;
//   a browse's or resolve's result is `granted` evidence, and
//   `kDNSServiceErr_PolicyDenied` is `denied`. Evidence goes to the host,
//   which keeps the fact the runner's per-hop rule reads (D6), by the
//   `exact.localNetwork` notification. The probe of D8 is not built.
// - Hatches (D13): `parameters` sees each browse's `NWParameters` before the
//   browser is made, and `browser` each `NWBrowser` before it starts.
import dnssd
import ExactPackage
import Foundation
import Network

public final class LocalNetworkModule: ExactPackage {
    /// Given each browse's parameters before its browser is made.
    public static var parameters: ((NWParameters) -> Void)?
    /// Given each browser after it is made, before it starts.
    public static var browser: ((NWBrowser) -> Void)?

    let context: ExactPackageContext
    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            guard !context.agent else { return reply(["error": "NotSupportedError"]) }
            switch name {
            case "localNetworkStatus": reply(LocalNetworkFact.status())
            case "browseServices": LocalNetworkBrowse.read(self, args, reply)
            case "resolveService": LocalNetworkResolve.read(self, args, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard !context.agent else { return ["error": "NotSupportedError"] }
        return name == "localNetworkStatusSync" ? LocalNetworkFact.status() : ["error": "NotSupportedError"]
    }

    public func release(_ name: String) { Self.leave(self) { $0.hasPrefix(name + " ") } }
    public func release(_ name: String, args: [String: Any]) {
        let key = name == "browseServices" ? LocalNetworkBrowse.key(args) : LocalNetworkResolve.key(args)
        Self.leave(self) { $0 == key }
    }

    public func foreground(_ shown: Bool) {
        LocalNetworkBrowse.all.values.forEach { $0.pause(!shown) }
        LocalNetworkResolve.all.values.forEach { $0.pause(!shown) }
    }

    public func destroy(_ done: @escaping () -> Void) {
        Self.leave(self) { _ in true }
        done()
    }

    private static func leave(_ module: LocalNetworkModule, where wanted: (String) -> Bool) {
        let id = ObjectIdentifier(module)
        for (key, b) in LocalNetworkBrowse.all where wanted(key) { b.readers[id] = nil; if b.readers.isEmpty { b.stop(); LocalNetworkBrowse.all[key] = nil } }
        for (key, r) in LocalNetworkResolve.all where wanted(key) { r.readers[id] = nil; if r.readers.isEmpty { r.stop(); LocalNetworkResolve.all[key] = nil } }
    }

    static func error(_ name: String, _ message: String? = nil) -> [String: Any] {
        message.map { ["error": name, "message": $0] } ?? ["error": name]
    }
}

/// The host's local-network fact as this module knows it (D8).
enum LocalNetworkFact {
    static var prompts: Bool = {
        #if targetEnvironment(simulator) || os(tvOS)
        return false
        #elseif os(macOS)
        return ProcessInfo.processInfo.isOperatingSystemAtLeast(OperatingSystemVersion(majorVersion: 15, minorVersion: 0, patchVersion: 0))
        #else
        return true
        #endif
    }()
    static var state = prompts ? "unknown" : "granted"
    /// Every module whose sessions read the status, told of each change.
    static var watchers: [ObjectIdentifier: () -> Void] = [:]

    static func status() -> [String: Any] { ["available": true, "state": state, "android": NSNull()] }

    /// Evidence: `granted` or `denied`, to the host and each session.
    static func report(_ word: String) {
        guard prompts, word != state else { return }
        state = word
        NotificationCenter.default.post(name: Notification.Name("exact.localNetwork"), object: ["state": word] as NSDictionary)
        watchers.values.forEach { $0() }
    }

    static func watch(_ module: LocalNetworkModule) {
        watchers[ObjectIdentifier(module)] = { [weak module] in module?.context.changed("local-network.localNetworkStatus") }
    }

    static let policyDenied = DNSServiceErrorType(kDNSServiceErr_PolicyDenied)
    static let noError = DNSServiceErrorType(kDNSServiceErr_NoError)
}

/// A group's readers: each session's module, told of each change.
class LocalNetworkReaders {
    var readers: [ObjectIdentifier: (module: LocalNetworkModule, topic: String)] = [:]
    func join(_ module: LocalNetworkModule, _ topic: String) {
        readers[ObjectIdentifier(module)] = (module, topic)
        LocalNetworkFact.watch(module)
    }
    func changed() { readers.values.forEach { $0.module.context.changed($0.topic) } }
}

final class LocalNetworkBrowse: LocalNetworkReaders {
    static var all: [String: LocalNetworkBrowse] = [:]
    let type: String, peerToPeer: Bool
    private var browser: NWBrowser?
    private var names: [String] = []
    private var settled = false
    private var failure: [String: Any]?
    private var waiting: [([String: Any]) -> Void] = []
    private var paused = false

    init(type: String, peerToPeer: Bool) { self.type = type; self.peerToPeer = peerToPeer }

    static func key(_ args: [String: Any]) -> String {
        "browseServices \(args["type"] as? String ?? "") \(args["includePeerToPeer"] as? Bool ?? false)"
    }

    static func read(_ module: LocalNetworkModule, _ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let key = key(args)
        let b = all[key] ?? LocalNetworkBrowse(type: args["type"] as? String ?? "", peerToPeer: args["includePeerToPeer"] as? Bool ?? false)
        all[key] = b
        b.join(module, "local-network.browseServices")
        b.answer(reply)
    }

    private func answer(_ reply: @escaping ([String: Any]) -> Void) {
        if let failure { return reply(failure) }
        if settled { return reply(["services": names.map { ["name": $0, "type": type] }]) }
        waiting.append(reply)
        start()
    }

    private func start() {
        guard browser == nil, !paused else { return }
        let parameters = NWParameters()
        LocalNetworkModule.parameters?(parameters)
        parameters.includePeerToPeer = peerToPeer
        let b = NWBrowser(for: .bonjour(type: type, domain: "local."), using: parameters)
        b.browseResultsChangedHandler = { [weak self] results, _ in self?.found(results) }
        b.stateUpdateHandler = { [weak self] state in self?.moved(state) }
        LocalNetworkModule.browser?(b)
        browser = b
        b.start(queue: .main)
        // A quiet network answers empty after a second (D2).
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self, weak b] in
            guard let self, let b, self.browser === b else { return }
            self.settle()
        }
    }

    private func found(_ results: Set<NWBrowser.Result>) {
        var merged = Set<String>()
        for result in results { if case .service(let name, _, _, _) = result.endpoint { merged.insert(name) } }
        names = merged.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        failure = nil
        LocalNetworkFact.report("granted")
        if settled { changed() } else { settle() }
    }

    private func settle() {
        settled = true
        let replies = waiting
        waiting = []
        replies.forEach(answer)
    }

    private func moved(_ state: NWBrowser.State) {
        switch state {
        case .waiting(let error), .failed(let error):
            if case .dns(let code) = error, code == LocalNetworkFact.policyDenied {
                LocalNetworkFact.report("denied")
                fail(LocalNetworkModule.error("NotAllowedError", "local network access is denied"))
            } else if case .failed = state {
                fail(LocalNetworkModule.error("NetworkError", "\(error)"))
            }
        default: break
        }
    }

    private func fail(_ error: [String: Any]) {
        failure = error
        stop()
        let replies = waiting
        waiting = []
        replies.forEach { $0(error) }
        changed()
    }

    func pause(_ hidden: Bool) {
        guard hidden != paused else { return }
        paused = hidden
        if hidden { stop() } else { settled = false; failure = nil; changed() }
    }

    func stop() {
        browser?.cancel()
        browser = nil
    }
}

final class LocalNetworkResolve: LocalNetworkReaders {
    static var all: [String: LocalNetworkResolve] = [:]
    let name: String, type: String, peerToPeer: Bool
    private var service: DNSServiceRef?
    private var lookup: DNSServiceRef?
    private var record: (hostName: String, port: Int, txt: [[String: Any]])?
    private var addresses: [String] = []
    private var waiting: [([String: Any]) -> Void] = []
    private var paused = false

    init(name: String, type: String, peerToPeer: Bool) { self.name = name; self.type = type; self.peerToPeer = peerToPeer }

    static func key(_ args: [String: Any]) -> String {
        "resolveService \(args["type"] as? String ?? "") \(args["includePeerToPeer"] as? Bool ?? false) \(args["name"] as? String ?? "")"
    }

    static func read(_ module: LocalNetworkModule, _ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let key = key(args)
        let r = all[key] ?? LocalNetworkResolve(name: args["name"] as? String ?? "", type: args["type"] as? String ?? "",
                                    peerToPeer: args["includePeerToPeer"] as? Bool ?? false)
        all[key] = r
        r.join(module, "local-network.resolveService")
        if let info = r.info() { return reply(info) }
        r.waiting.append(reply)
        r.start()
        let timeout = (args["timeout"] as? Double ?? 5000) / 1000
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout) { [weak r] in
            guard let r, r.info() == nil else { return }
            r.finish(LocalNetworkModule.error("NotFoundError"))
        }
    }

    /// The answer once the platform has the SRV and TXT records and an address (D2).
    private func info() -> [String: Any]? {
        guard let record, !addresses.isEmpty else { return nil }
        let host = record.hostName.hasSuffix(".") ? String(record.hostName.dropLast()) : record.hostName
        return ["name": name, "type": type, "host": host, "hostName": record.hostName, "port": record.port,
                "addresses": addresses, "txt": record.txt]
    }

    private func start() {
        guard service == nil, !paused else { return }
        let flags = DNSServiceFlags(peerToPeer ? kDNSServiceFlagsIncludeP2P : 0)
        let me = Unmanaged.passUnretained(self).toOpaque()
        var ref: DNSServiceRef?
        let code = DNSServiceResolve(&ref, flags, 0, name, type, "local.", localNetworkResolved, me)
        guard code == LocalNetworkFact.noError, let ref else { return failed(code) }
        DNSServiceSetDispatchQueue(ref, .main)
        service = ref
    }

    fileprivate func took(_ hostName: String, _ port: Int, _ txt: [[String: Any]]) {
        let moved = record?.hostName != hostName
        record = (hostName, port, txt)
        LocalNetworkFact.report("granted")
        guard moved else { return answered() }
        addresses = []
        lookup.map(DNSServiceRefDeallocate)
        lookup = nil
        var ref: DNSServiceRef?
        let me = Unmanaged.passUnretained(self).toOpaque()
        let code = DNSServiceGetAddrInfo(&ref, 0, 0, DNSServiceProtocol(kDNSServiceProtocol_IPv4 | kDNSServiceProtocol_IPv6),
                                         hostName, localNetworkAddressed, me)
        guard code == LocalNetworkFact.noError, let ref else { return failed(code) }
        DNSServiceSetDispatchQueue(ref, .main)
        lookup = ref
    }

    fileprivate func took(address: String, added: Bool) {
        if added, !addresses.contains(address) { addresses.append(address) }
        if !added { addresses.removeAll { $0 == address } }
        answered()
    }

    private func answered() {
        guard let info = info() else { return }
        if waiting.isEmpty { return changed() }
        finish(info)
    }

    fileprivate func failed(_ code: DNSServiceErrorType) {
        if code == LocalNetworkFact.policyDenied {
            LocalNetworkFact.report("denied")
            return finish(LocalNetworkModule.error("NotAllowedError", "local network access is denied"))
        }
        finish(LocalNetworkModule.error("NetworkError", "dnssd error \(code)"))
    }

    private func finish(_ answer: [String: Any]) {
        let replies = waiting
        waiting = []
        replies.forEach { $0(answer) }
        if answer["error"] != nil { stop(); record = nil; addresses = [] }
    }

    func pause(_ hidden: Bool) {
        guard hidden != paused else { return }
        paused = hidden
        if hidden { stop() } else { record = nil; addresses = []; changed() }
    }

    func stop() {
        lookup.map(DNSServiceRefDeallocate)
        service.map(DNSServiceRefDeallocate)
        lookup = nil
        service = nil
    }

    /// A TXT record's entries (RFC 6763 §6): keys lower-cased, a value
    /// decoded as UTF-8 with U+FFFD for invalid sequences, none without `=`.
    static func entries(_ bytes: UnsafeBufferPointer<UInt8>) -> [[String: Any]] {
        var out: [[String: Any]] = [], at = 0
        while at < bytes.count {
            let length = Int(bytes[at]), end = min(bytes.count, at + 1 + length)
            let entry = Array(bytes[(at + 1)..<end])
            at = end
            guard !entry.isEmpty else { continue }
            let equals = entry.firstIndex(of: UInt8(ascii: "="))
            let key = String(decoding: entry[..<(equals ?? entry.count)], as: UTF8.self).lowercased()
            guard !key.isEmpty, !out.contains(where: { $0["key"] as? String == key }) else { continue }
            let value: Any = equals.map { String(decoding: entry[($0 + 1)...], as: UTF8.self) } ?? NSNull()
            out.append(["key": key, "value": value])
        }
        return out.sorted { ($0["key"] as? String ?? "") < ($1["key"] as? String ?? "") }
    }
}

private let localNetworkResolved: DNSServiceResolveReply = { _, _, _, code, _, host, port, length, txt, context in
    guard let context else { return }
    let r = Unmanaged<LocalNetworkResolve>.fromOpaque(context).takeUnretainedValue()
    guard code == LocalNetworkFact.noError, let host else { return r.failed(code) }
    let entries = txt.map { LocalNetworkResolve.entries(UnsafeBufferPointer(start: $0, count: Int(length))) } ?? []
    r.took(String(cString: host), Int(UInt16(bigEndian: port)), entries)
}

private let localNetworkAddressed: DNSServiceGetAddrInfoReply = { _, flags, _, code, _, address, _, context in
    guard let context else { return }
    let r = Unmanaged<LocalNetworkResolve>.fromOpaque(context).takeUnretainedValue()
    guard code == LocalNetworkFact.noError, let address else { return code == LocalNetworkFact.noError ? () : r.failed(code) }
    var text = [CChar](repeating: 0, count: Int(NI_MAXHOST))
    guard getnameinfo(address, socklen_t(address.pointee.sa_len), &text, socklen_t(text.count), nil, 0, NI_NUMERICHOST) == 0 else { return }
    let written = String(cString: text)
    // An IPv6 link-local address carries a zone no URL can (D2).
    guard !written.lowercased().hasPrefix("fe80:") else { return }
    r.took(address: written, added: flags & DNSServiceFlags(kDNSServiceFlagsAdd) != 0)
}
