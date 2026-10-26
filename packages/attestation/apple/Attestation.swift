// @exact/attestation on Apple (LLP 1133.028 D1-D6): App Attest, for the
// app's own server. Under the agent it asks DeviceCheck for nothing and
// answers `exact-substitute` tokens from the session's private store
// (AgentAttestation.swift, §7); `--bind attestation` asks DeviceCheck.
//
// - `clientData` is hashed here, SHA-256 of its UTF-8 bytes, so a server
//   hashes the same bytes whichever platform called (D1).
// - One key per account per device (D3): the first `attest` for an account
//   generates and attests a key, each later one asserts with it. The key id
//   and whether Apple attested it are kept in Application Support, excluded
//   from backup, so a restore that cannot bring the key back does not bring
//   its id back either.
// - `serverUnavailable` keeps the key, to be attested again with the same
//   client data; any other `attestKey` error discards it. `invalidKey` from
//   an assertion means the Secure Enclave lost the key: a new one is
//   generated and attested in the same call.
// - A dropped attestation (`dispose`, its `attested` handle) discards its
//   key: no server saw it. Every call is replied to, even after `cancel`.
// - Calls for one account run one at a time, since an assertion's counter
//   must grow past the last the server saw; accounts run in parallel.
import CryptoKit
import DeviceCheck
import ExactPackage
import Foundation

public final class AttestationModule: ExactPackage {
    let context: ExactPackageContext
    private lazy var agent = AgentAttestation(context: context)
    /// Each account's calls waiting behind the one running.
    private var queues: [String: [(@escaping () -> Void) -> Void]] = [:]
    private var running: Set<String> = []

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("attestation: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let account = args["account"] as? String ?? ""
        switch name {
        case "attestationStatus": reply(context.agent ? agent.status() : Self.status())
        case "attest":
            guard let data = args["clientData"] as? String else { return reply(["error": "TypeError", "message": "clientData is a string"]) }
            let hash = Data(SHA256.hash(data: Data(data.utf8)))
            serial(account) { [self] done in
                let answer: ([String: Any]) -> Void = { reply($0); done() }
                context.agent ? agent.attest(account, hash, answer) : attest(account, hash, answer)
            }
        case "prepareAttestation":
            serial(account) { [self] done in
                let answer: ([String: Any]) -> Void = { reply($0); done() }
                context.agent ? agent.prepare(account, answer) : prepare(account, answer)
            }
        case "resetAttestation":
            serial(account) { [self] done in
                if context.agent { agent.reset(account) } else { Keys.forget(account) }
                say("reset \(account.isEmpty ? "the unnamed account" : "an account")")
                reply([:])
                done()
            }
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard name == "attestationStatus" || name == "attestationStatusSync" else { return ["error": "NotSupportedError"] }
        return context.agent ? agent.status() : Self.status()
    }

    public func preferred(_ values: [String: Any]) { agent.preferred(values) }

    /// An attestation whose reply the runner dropped: its key goes, since no
    /// server saw it and Apple does not say a key may be attested twice (D3).
    public func dispose(_ ids: [String]) {
        for id in ids {
            say("dispose: the dropped attestation's key is discarded")
            if context.agent { agent.discard(keyId: id) } else { Keys.discard(keyId: id) }
        }
    }

    /// Runs `work` for `account` once the account's earlier calls are done.
    private func serial(_ account: String, _ work: @escaping (@escaping () -> Void) -> Void) {
        queues[account, default: []].append(work)
        pump(account)
    }

    private func pump(_ account: String) {
        guard !running.contains(account), var queue = queues[account], !queue.isEmpty else { return }
        let next = queue.removeFirst()
        queues[account] = queue
        running.insert(account)
        next { [weak self] in
            DispatchQueue.main.async {
                self?.running.remove(account)
                self?.pump(account)
            }
        }
    }

    // MARK: App Attest (D3)

    static func status() -> [String: Any] {
        let supported = DCAppAttestService.shared.isSupported
        let apple: Any = environment().map { ["environment": $0] } ?? NSNull()
        return ["available": supported, "provider": supported ? "app-attest" : "none", "apple": supported ? apple : NSNull()]
    }

    /// The environment the build's provisioning profile signs for: a
    /// development profile allows a debugger, a distribution one does not.
    /// None without a profile (a simulator, an ad-hoc Mac build).
    static func environment() -> String? {
        #if os(macOS)
        let url = Bundle.main.bundleURL.appendingPathComponent("Contents/embedded.provisionprofile")
        #else
        let url = Bundle.main.bundleURL.appendingPathComponent("embedded.mobileprovision")
        #endif
        guard let data = try? Data(contentsOf: url),
              let start = data.range(of: Data("<?xml".utf8)), let end = data.range(of: Data("</plist>".utf8)),
              let plist = try? PropertyListSerialization.propertyList(from: data[start.lowerBound..<end.upperBound], format: nil) as? [String: Any],
              let entitlements = plist["Entitlements"] as? [String: Any] else { return nil }
        return entitlements["get-task-allow"] as? Bool == true ? "development" : "production"
    }

    private func prepare(_ account: String, _ reply: @escaping ([String: Any]) -> Void) {
        let service = DCAppAttestService.shared
        guard service.isSupported else { return reply(Self.failure(.featureUnsupported)) }
        guard Keys.read()[account] == nil else { return reply([:]) }
        service.generateKey { keyId, error in
            DispatchQueue.main.async {
                guard let keyId else { return reply(Self.failure(error)) }
                Keys.write(account, Keys.Key(id: keyId, attested: false))
                reply([:])
            }
        }
    }

    private func attest(_ account: String, _ hash: Data, _ reply: @escaping ([String: Any]) -> Void) {
        let service = DCAppAttestService.shared
        guard service.isSupported else { return reply(Self.failure(.featureUnsupported)) }
        guard let key = Keys.read()[account] else {
            return service.generateKey { [self] keyId, error in
                DispatchQueue.main.async { [self] in
                    guard let keyId else { return reply(Self.failure(error)) }
                    Keys.write(account, Keys.Key(id: keyId, attested: false))
                    attestKey(account, keyId, hash, reply)
                }
            }
        }
        guard key.attested else { return attestKey(account, key.id, hash, reply) }
        service.generateAssertion(key.id, clientDataHash: hash) { [self] assertion, error in
            DispatchQueue.main.async { [self] in
                if let assertion {
                    say("asserted")
                    return reply(["format": "apple-appattest-assertion", "token": Self.base64url(assertion), "keyId": key.id, "attested": NSNull()])
                }
                guard (error as? DCError)?.code == .invalidKey else { return reply(Self.failure(error)) }
                // The Enclave no longer has the key: a new one, attested now.
                say("invalidKey: a new key is generated and attested")
                Keys.forget(account)
                attest(account, hash, reply)
            }
        }
    }

    private func attestKey(_ account: String, _ keyId: String, _ hash: Data, _ reply: @escaping ([String: Any]) -> Void) {
        DCAppAttestService.shared.attestKey(keyId, clientDataHash: hash) { [self] object, error in
            DispatchQueue.main.async { [self] in
                if let object {
                    Keys.write(account, Keys.Key(id: keyId, attested: true))
                    say("attested a new key")
                    return reply(["format": "apple-appattest", "token": Self.base64url(object), "keyId": keyId, "attested": keyId])
                }
                // Apple: retry `serverUnavailable` with the same key; discard it after any other error.
                if (error as? DCError)?.code != .serverUnavailable { Keys.forget(account) }
                reply(Self.failure(error))
            }
        }
    }

    static func failure(_ error: Error?) -> [String: Any] {
        guard let error = error as? DCError else { return ["error": "OperationError", "message": error?.localizedDescription ?? "no key"] }
        return failure(error.code, error.localizedDescription)
    }

    static func failure(_ code: DCError.Code, _ message: String = "") -> [String: Any] {
        let (name, word): (String, String) = switch code {
        case .featureUnsupported: ("NotSupportedError", "featureUnsupported")
        case .serverUnavailable: ("NetworkError", "serverUnavailable")
        case .invalidInput: ("OperationError", "invalidInput")
        case .invalidKey: ("InvalidStateError", "invalidKey")
        default: ("OperationError", "unknownSystemFailure")
        }
        return ["error": name, "message": message, "apple": ["code": word]]
    }

    static func base64url(_ data: Data) -> String {
        data.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

/// The key ids App Attest made, per account (D3): a file in Application
/// Support excluded from backup. Main thread.
enum Keys {
    struct Key: Codable { var id: String; var attested: Bool }

    private static var file: URL? {
        guard let support = try? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        else { return nil }
        // A Mac app outside the sandbox shares this directory with every app.
        let dir = support.appendingPathComponent(Bundle.main.bundleIdentifier ?? "exact", isDirectory: true)
        return dir.appendingPathComponent("exact-attestation-keys.json")
    }

    static func read() -> [String: Key] {
        guard let file, let data = try? Data(contentsOf: file) else { return [:] }
        return (try? JSONDecoder().decode([String: Key].self, from: data)) ?? [:]
    }

    static func write(_ account: String, _ key: Key?) {
        var all = read()
        all[account] = key
        guard var file, let data = try? JSONEncoder().encode(all) else { return }
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? data.write(to: file, options: .atomic)
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? file.setResourceValues(values)
    }

    static func forget(_ account: String) { write(account, nil) }

    static func discard(keyId: String) {
        for (account, key) in read() where key.id == keyId { forget(account) }
    }
}
