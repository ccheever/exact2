// @exact/attestation's agent path (LLP 1133.028 §7): D3's lifecycle over
// synthetic keys in the session's private store, with no call to
// DeviceCheck. A token is `exact-substitute`, the base64url of a JSON object
// naming the client data's SHA-256, the key id, the kind and the counter,
// which `packages/attestation/test/verify.mjs` checks.
//
// The agent's words: `attestationStatus` is laid over this path's reading,
// `available` with App Attest; `fail` makes the next call fail with that
// name (a `NetworkError` keeps the key, as `serverUnavailable` does); `delay`
// holds each answer that long on the agent's clock, so a drive can supersede
// a call whose answer is in flight.
import ExactPackage
import Foundation
import Security

final class AgentAttestation {
    private let context: ExactPackageContext
    private struct Key { var id: String; var attested: Bool; var counter: Int }
    private var keys: [String: Key] = [:]
    private var overlay: [String: Any] = [:]
    private var fail: String?
    private var lastFail: String?
    private var delay: Double = 0

    init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("attestation: \(line)") }

    func preferred(_ values: [String: Any]) {
        overlay = values["attestationStatus"] as? [String: Any] ?? [:]
        let word = values["fail"] as? String
        // A new word arms one failure; the same word again does not.
        if let word, !word.isEmpty, word != lastFail { fail = word }
        lastFail = word
        delay = (values["delay"] as? NSNumber)?.doubleValue ?? 0
    }

    func status() -> [String: Any] {
        var out: [String: Any] = ["available": true, "provider": "app-attest", "apple": ["environment": "development"]]
        for (k, v) in overlay {
            if var inner = out[k] as? [String: Any], let over = v as? [String: Any] {
                inner.merge(over) { $1 }
                out[k] = inner
            } else {
                out[k] = v
            }
        }
        if out["available"] as? Bool == false { out["provider"] = "none"; out["apple"] = NSNull() }
        return out
    }

    private var available: Bool { status()["available"] as? Bool ?? false }

    func prepare(_ account: String, _ reply: @escaping ([String: Any]) -> Void) {
        guard available else { return later(reply, AttestationModule.failure(.featureUnsupported)) }
        if let failed = takeFailure() { return later(reply, failed) }
        if keys[account] == nil { keys[account] = Key(id: Self.newId(), attested: false, counter: 0) }
        later(reply, [:])
    }

    func attest(_ account: String, _ hash: Data, _ reply: @escaping ([String: Any]) -> Void) {
        guard available else { return later(reply, AttestationModule.failure(.featureUnsupported)) }
        var key = keys[account] ?? Key(id: Self.newId(), attested: false, counter: 0)
        keys[account] = key
        if let failed = takeFailure() {
            // As Apple's errors do (D3): a network failure keeps the key, any
            // other discards one not yet attested.
            if failed["error"] as? String != "NetworkError", !key.attested { keys[account] = nil }
            say("failed \(failed["error"] ?? "")")
            return later(reply, failed)
        }
        let attesting = !key.attested
        if !attesting { key.counter += 1 }
        key.attested = true
        keys[account] = key
        let token: [String: Any] = ["clientDataHash": AttestationModule.base64url(hash), "keyId": key.id,
                                    "kind": attesting ? "attestation" : "assertion", "counter": key.counter]
        let bytes = (try? JSONSerialization.data(withJSONObject: token, options: [.sortedKeys])) ?? Data()
        say(attesting ? "attested a new key" : "asserted (counter \(key.counter))")
        later(reply, ["format": "exact-substitute", "token": AttestationModule.base64url(bytes), "keyId": key.id,
                      "attested": attesting ? key.id : NSNull()])
    }

    func reset(_ account: String) { keys[account] = nil }

    func discard(keyId: String) {
        for (account, key) in keys where key.id == keyId { keys[account] = nil }
    }

    private func takeFailure() -> [String: Any]? {
        guard let name = fail else { return nil }
        fail = nil
        let code = ["NotSupportedError": "featureUnsupported", "NetworkError": "serverUnavailable", "OperationError": "unknownSystemFailure"][name]
        return ["error": name, "message": "the agent's failure", "apple": code.map { ["code": $0] } ?? NSNull()]
    }

    /// `answer` after `delay` ms of the agent's clock, polled, since nothing
    /// tells an implementation that the clock moved.
    private func later(_ reply: @escaping ([String: Any]) -> Void, _ answer: [String: Any]) {
        guard delay > 0 else { return reply(answer) }
        let due = context.now() + delay
        func wait() {
            if context.now() >= due { return reply(answer) }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: wait)
        }
        wait()
    }

    private static func newId() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64EncodedString()
    }
}
