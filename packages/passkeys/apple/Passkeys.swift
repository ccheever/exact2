// @exact/passkeys on Apple (LLP 1133.011 D1, D3-D6, D10, D11, D13):
// AuthenticationServices' platform and security-key providers, one
// `ASAuthorizationController` per ceremony, in WebAuthn's JSON forms. The
// app's composed module calls it outside the agent; under the agent the
// runner's substitute answers and no controller is made.
//
// - The options are `PublicKeyCredentialCreationOptionsJSON` and
//   `…RequestOptionsJSON`; the answers `RegistrationResponseJSON` and
//   `AuthenticationResponseJSON`, every buffer base64url. Apple gives a
//   registration's attestation object, not its authenticator data or key,
//   so both are read from it (§5.2.1.1's `getAuthenticatorData()`,
//   `getPublicKey()`, `getPublicKeyAlgorithm()`).
// - Security keys join the sheet only when asked for (D4): `apple.securityKey`,
//   `authenticatorAttachment: "cross-platform"`, `hints` naming
//   `security-key`, or an `allowCredentials` entry with a usb, nfc or ble
//   transport.
// - No answer carries a PRF output (D10): only `prf.enabled` is answered.
//   A lock's user handle (§7 item 3, beginning `exact-lock:`) is never
//   registered, delivered or signalled from here.
// - One ceremony at a time (D9): a newer one cancels a pending conditional
//   `get` (iOS's AutoFill-assisted request), which ends `AbortError`.
// - The hatches (D5) see each request after its words are set; the module
//   sets the fields it owns again after they return.
// - The sheet anchors to the key window. The package context names no
//   session window, so a host showing two sessions anchors to whichever
//   window is key.
import AuthenticationServices
import ExactPackage
import Foundation
import LocalAuthentication
import Security
#if os(macOS)
import AppKit
#else
import UIKit
#endif

private let mark = Data("exact-lock:".utf8)

private func b64(_ data: Data) -> String {
    data.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

private func unb64(_ value: Any?) -> Data? {
    guard var s = value as? String else { return nil }
    s = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    s += String(repeating: "=", count: (4 - s.count % 4) % 4)
    return Data(base64Encoded: s)
}

private func marked(_ handle: Data?) -> Bool { handle.map { $0.starts(with: mark) } ?? false }

public final class PasskeysModule: NSObject, ExactPackage {
    /// The app's access hatches (D5): each registration or assertion
    /// request, after the module's words are set and before the controller
    /// is made.
    public static var registrationRequest: ((ASAuthorizationRequest) -> Void)?
    public static var assertionRequest: ((ASAuthorizationRequest) -> Void)?

    private let context: ExactPackageContext
    /// The ceremony whose sheet is up, and the conditional one a newer ends.
    private var running: [ObjectIdentifier: Ceremony] = [:]
    private var conditional: Ceremony?
    /// Each registration's RP ID by credential id, for `dispose` (D11).
    private var made: [String: String] = [:]

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "getClientCapabilities": reply(capabilities())
            case "create": create(args, reply)
            case "get": get(args, reply)
            case "signalUnknownCredential", "signalAllAcceptedCredentials", "signalCurrentUserDetails": signal(name, args, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "getClientCapabilitiesSync" ? capabilities() : ["error": "NotSupportedError"]
    }

    /// Settings and the credential manager can change while the app is away.
    public func foreground(_ shown: Bool) {
        if shown { context.changed("passkeys.getClientCapabilities") }
    }

    public func dispose(_ ids: [String]) {
        for id in ids {
            let rp = made.removeValue(forKey: id)
            guard let rp, let credential = unb64(id) else { continue }
            if #available(iOS 26.2, macOS 26.2, *) {
                Task { @MainActor [context] in
                    do {
                        try await ASCredentialDataManager().reportUnknownPublicKeyCredential(relyingPartyIdentifier: rp, credentialID: credential)
                        context.log("dispose passkeys \(id): reported unknown")
                    } catch {
                        context.log("dispose passkeys \(id): \(error.localizedDescription)")
                    }
                }
            } else {
                context.log("dispose passkeys \(id): an orphaned passkey for \(rp); this OS has no report for it")
            }
        }
    }

    public func destroy(_ done: @escaping () -> Void) {
        running.values.forEach { $0.controller.cancel() }
        running = [:]
        conditional = nil
        done()
    }

    // MARK: capabilities (D13)

    /// A Mac signs `webcredentials:` only with a profile (D6); iOS always
    /// can, and the platform refuses an RP ID the app is not associated with.
    private var associated: Bool {
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return SecTaskCopyValueForEntitlement(task, "com.apple.developer.associated-domains" as CFString, nil) != nil
        #else
        return true
        #endif
    }

    private func capabilities() -> [String: Any] {
        guard associated else { return ["available": false, "conditionalGet": false, "android": NSNull()] }
        var newer = false, signals = false
        if #available(iOS 18, macOS 15, *) { newer = true }
        if #available(iOS 26.2, macOS 26.2, *) { signals = true }
        #if os(macOS)
        let autofill = false
        #else
        let autofill = true
        #endif
        let verifying = LAContext().canEvaluatePolicy(.deviceOwnerAuthentication, error: nil)
        return [
            "available": true, "conditionalCreate": newer, "conditionalGet": autofill, "hybridTransport": true,
            "passkeyPlatformAuthenticator": true, "userVerifyingPlatformAuthenticator": verifying, "relatedOrigins": false,
            "signalAllAcceptedCredentials": signals, "signalCurrentUserDetails": signals, "signalUnknownCredential": signals,
            "preferImmediatelyAvailableCredentials": true,
            "extensions": ["prf": newer, "largeBlob": true, "credProps": true], "android": NSNull(),
        ]
    }

    // MARK: ceremonies

    private func create(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard context.origin == "app" else { return reply(["error": "NotAllowedError", "message": "person-required"]) }
        guard associated else { return reply(["error": "NotSupportedError", "message": "a Mac build signs no associated domain without a provisioning profile (LLP 1133.011 D6)"]) }
        let o = args["publicKey"] as? [String: Any] ?? [:]
        let user = o["user"] as? [String: Any] ?? [:]
        guard let rp = (o["rp"] as? [String: Any])?["id"] as? String, let challenge = unb64(o["challenge"]),
              let userID = unb64(user["id"]), let name = user["name"] as? String
        else { return reply(["error": "TypeError", "message": "publicKey needs rp.id, challenge, user.id and user.name"]) }
        if marked(userID) { return reply(["error": "NotAllowedError", "message": "a lock's user handle is the lock's alone"]) }
        let selection = o["authenticatorSelection"] as? [String: Any] ?? [:]
        let excluded = (o["excludeCredentials"] as? [[String: Any]] ?? []).compactMap { unb64($0["id"]) }
        let extensions = o["extensions"] as? [String: Any] ?? [:]
        let conditionally = args["mediation"] as? String == "conditional"
        let display = user["displayName"] as? String

        let platform = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: rp)
            .createCredentialRegistrationRequest(challenge: challenge, name: name, userID: userID)
        let own = { (r: ASAuthorizationPlatformPublicKeyCredentialRegistrationRequest) in
            r.challenge = challenge; r.userID = userID; r.name = name
            if #available(iOS 17.4, macOS 13.5, *) { r.excludedCredentials = excluded.map(ASAuthorizationPlatformPublicKeyCredentialDescriptor.init(credentialID:)) }
            if #available(iOS 18, macOS 15, *) { r.requestStyle = conditionally ? .conditional : .standard }
        }
        platform.displayName = display
        if let uv = selection["userVerification"] as? String { platform.userVerificationPreference = .init(rawValue: uv) }
        if let kind = o["attestation"] as? String { platform.attestationPreference = .init(rawValue: kind) }
        if #available(iOS 17, macOS 14, *), let blob = extensions["largeBlob"] as? [String: Any] {
            platform.largeBlob = blob["support"] as? String == "required" ? .supportRequired : .supportPreferred
        }
        if #available(iOS 18, macOS 15, *), let prf = extensions["prf"] as? [String: Any] {
            let eval = prf["eval"] as? [String: Any]
            platform.prf = unb64(eval?["first"]).map { .inputValues(.init(saltInput1: $0, saltInput2: unb64(eval?["second"]))) } ?? .checkForSupport
        }
        own(platform)
        var requests: [ASAuthorizationRequest] = [platform]
        let hints = o["hints"] as? [String] ?? []
        let apple = args["apple"] as? [String: Any] ?? [:]
        if !conditionally, apple["securityKey"] as? Bool == true || selection["authenticatorAttachment"] as? String == "cross-platform" || hints.contains("security-key") {
            let key = ASAuthorizationSecurityKeyPublicKeyCredentialProvider(relyingPartyIdentifier: rp)
                .createCredentialRegistrationRequest(challenge: challenge, displayName: display ?? name, name: name, userID: userID)
            let algorithms = (o["pubKeyCredParams"] as? [[String: Any]] ?? []).compactMap { $0["alg"] as? Int }
            key.credentialParameters = (algorithms.isEmpty ? [-7] : algorithms).map { ASAuthorizationPublicKeyCredentialParameters(algorithm: ASCOSEAlgorithmIdentifier(rawValue: $0)) }
            key.excludedCredentials = excluded.map { ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor(credentialID: $0, transports: ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported) }
            if let resident = selection["residentKey"] as? String { key.residentKeyPreference = .init(rawValue: resident) }
            if let uv = selection["userVerification"] as? String { key.userVerificationPreference = .init(rawValue: uv) }
            if let kind = o["attestation"] as? String { key.attestationPreference = .init(rawValue: kind) }
            requests.append(key)
        }
        if let hatch = Self.registrationRequest {
            for r in requests { hatch(r) }
            if platform.challenge != challenge || platform.userID != userID || platform.name != name {
                context.log("passkeys: the registration hatch changed a field the module owns; set again (LLP 1133.011 D5)")
            }
            own(platform)
        }
        run(requests, conditional: false, autofill: false) { [self] result in
            switch result {
            case .failure(let failure): reply(failure)
            case .success(let credential):
                guard let answer = registration(credential) else { return reply(["error": "UnknownError", "message": "the platform gave no attestation object"]) }
                if let id = answer["id"] as? String { made[id] = rp }
                reply(answer)
            }
        }
    }

    private func get(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard context.origin == "app" else { return reply(["error": "NotAllowedError", "message": "person-required"]) }
        guard associated else { return reply(["error": "NotSupportedError", "message": "a Mac build signs no associated domain without a provisioning profile (LLP 1133.011 D6)"]) }
        let o = args["publicKey"] as? [String: Any] ?? [:]
        guard let rp = o["rpId"] as? String, let challenge = unb64(o["challenge"])
        else { return reply(["error": "TypeError", "message": "publicKey needs rpId and challenge"]) }
        let allow = o["allowCredentials"] as? [[String: Any]] ?? []
        let ids = allow.compactMap { unb64($0["id"]) }
        let extensions = o["extensions"] as? [String: Any] ?? [:]
        let conditionally = args["mediation"] as? String == "conditional"
        #if os(macOS)
        if conditionally { return reply(["error": "NotSupportedError", "message": "macOS has no conditional get"]) }
        #endif
        let platform = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: rp).createCredentialAssertionRequest(challenge: challenge)
        let own = { (r: ASAuthorizationPlatformPublicKeyCredentialAssertionRequest) in
            r.challenge = challenge; r.relyingPartyIdentifier = rp
            r.allowedCredentials = ids.map(ASAuthorizationPlatformPublicKeyCredentialDescriptor.init(credentialID:))
        }
        if let uv = o["userVerification"] as? String { platform.userVerificationPreference = .init(rawValue: uv) }
        if #available(iOS 17, macOS 14, *), let blob = extensions["largeBlob"] as? [String: Any] {
            if blob["read"] as? Bool == true { platform.largeBlob = .read } else if let data = unb64(blob["write"]) { platform.largeBlob = .write(data) }
        }
        if #available(iOS 18, macOS 15, *), let prf = extensions["prf"] as? [String: Any] {
            let values = { (v: Any?) -> ASAuthorizationPublicKeyCredentialPRFAssertionInput.InputValues? in
                let v = v as? [String: Any]
                return unb64(v?["first"]).map { .init(saltInput1: $0, saltInput2: unb64(v?["second"])) }
            }
            let each = (prf["evalByCredential"] as? [String: Any] ?? [:]).reduce(into: [Data: ASAuthorizationPublicKeyCredentialPRFAssertionInput.InputValues]()) { out, pair in
                if let id = unb64(pair.key), let v = values(pair.value) { out[id] = v }
            }
            if let eval = values(prf["eval"]) { platform.prf = .inputValues(eval, perCredentialInputValues: each.isEmpty ? nil : each) }
            else if !each.isEmpty { platform.prf = .perCredentialInputValues(each) }
        }
        own(platform)
        var requests: [ASAuthorizationRequest] = [platform]
        let apple = args["apple"] as? [String: Any] ?? [:]
        let keyTransports = Set(["usb", "nfc", "ble"])
        let wantsKey = apple["securityKey"] as? Bool == true || (o["hints"] as? [String] ?? []).contains("security-key")
            || allow.contains { !keyTransports.isDisjoint(with: $0["transports"] as? [String] ?? []) }
        if !conditionally, wantsKey {
            let key = ASAuthorizationSecurityKeyPublicKeyCredentialProvider(relyingPartyIdentifier: rp).createCredentialAssertionRequest(challenge: challenge)
            key.allowedCredentials = allow.compactMap { c in
                unb64(c["id"]).map { ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor(credentialID: $0, transports: (c["transports"] as? [String])?.map { .init(rawValue: $0) } ?? ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported) }
            }
            if let uv = o["userVerification"] as? String { key.userVerificationPreference = .init(rawValue: uv) }
            requests.append(key)
        }
        if let hatch = Self.assertionRequest {
            for r in requests { hatch(r) }
            if platform.challenge != challenge || platform.relyingPartyIdentifier != rp {
                context.log("passkeys: the assertion hatch changed a field the module owns; set again (LLP 1133.011 D5)")
            }
            own(platform)
        }
        let immediate = args["preferImmediatelyAvailableCredentials"] as? Bool == true
        run(requests, conditional: conditionally, autofill: conditionally, immediate: immediate) { result in
            switch result {
            case .failure(let failure): reply(failure)
            case .success(let credential):
                guard let assertion = credential as? ASAuthorizationPublicKeyCredentialAssertion else { return reply(["error": "UnknownError"]) }
                // A lock's assertion, or a discoverable one with no user handle, is never delivered (§7 item 3).
                if marked(assertion.userID) || (ids.isEmpty && assertion.userID.isEmpty) { return reply(["error": "NotAllowedError"]) }
                reply(Self.assertion(assertion))
            }
        }
    }

    private func signal(_ name: String, _ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let rp = args["rpId"] as? String ?? ""
        let user = unb64(args["userId"])
        if marked(user) { return reply(["error": "NotAllowedError"]) }
        guard #available(iOS 26.2, macOS 26.2, *) else { return reply([:]) } // where the OS has none, nothing to tell
        Task { @MainActor in
            do {
                let manager = ASCredentialDataManager()
                switch name {
                case "signalUnknownCredential":
                    guard let id = unb64(args["credentialId"]) else { return reply(["error": "TypeError"]) }
                    try await manager.reportUnknownPublicKeyCredential(relyingPartyIdentifier: rp, credentialID: id)
                case "signalAllAcceptedCredentials":
                    guard let user else { return reply(["error": "TypeError"]) }
                    let ids = (args["allAcceptedCredentialIds"] as? [String] ?? []).compactMap(unb64)
                    try await manager.reportAllAcceptedPublicKeyCredentials(relyingPartyIdentifier: rp, userHandle: user, acceptedCredentialIDs: ids)
                default:
                    guard let user else { return reply(["error": "TypeError"]) }
                    try await manager.reportPublicKeyCredentialUpdate(relyingPartyIdentifier: rp, userHandle: user, newName: args["name"] as? String ?? "")
                }
                reply([:])
            } catch {
                reply(["error": "UnknownError", "message": error.localizedDescription])
            }
        }
    }

    // MARK: the controller

    private final class Ceremony: NSObject, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
        let controller: ASAuthorizationController
        var done: ((Result<ASAuthorizationCredential, Failure>) -> Void)?
        /// Why the module ended it (a newer ceremony), which the error then reads as.
        var superseded = false
        init(_ requests: [ASAuthorizationRequest]) { controller = ASAuthorizationController(authorizationRequests: requests) }

        func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
            done?(.success(authorization.credential)); done = nil
        }

        func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
            done?(.failure(superseded ? Failure(["error": "AbortError", "message": "a newer ceremony replaced this conditional one"]) : Failure(PasskeysModule.failure(error)))); done = nil
        }

        func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
            #if os(macOS)
            return NSApp.keyWindow ?? NSApp.windows.first ?? ASPresentationAnchor()
            #else
            let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
            return windows.first(where: \.isKeyWindow) ?? windows.first ?? ASPresentationAnchor()
            #endif
        }
    }

    private struct Failure: Error { let json: [String: Any]; init(_ json: [String: Any]) { self.json = json } }
    private enum Outcome { case success(ASAuthorizationCredential), failure([String: Any]) }

    private func run(_ requests: [ASAuthorizationRequest], conditional: Bool, autofill: Bool, immediate: Bool = false,
                     _ done: @escaping (Outcome) -> Void) {
        if let held = self.conditional { held.superseded = true; held.controller.cancel(); self.conditional = nil }
        let c = Ceremony(requests)
        let key = ObjectIdentifier(c)
        running[key] = c
        if conditional { self.conditional = c }
        c.done = { [weak self] result in
            self?.running[key] = nil
            if self?.conditional === c { self?.conditional = nil }
            switch result {
            case .success(let credential): done(.success(credential))
            case .failure(let failure): done(.failure(failure.json))
            }
        }
        c.controller.delegate = c
        c.controller.presentationContextProvider = c
        #if os(iOS)
        if autofill { return c.controller.performAutoFillAssistedRequests() }
        #endif
        if immediate { c.controller.performRequests(options: .preferImmediatelyAvailableCredentials) } else { c.controller.performRequests() }
    }

    /// D1's code for an `ASAuthorizationError`, with Apple's own beside it (D4).
    fileprivate static func failure(_ error: Error) -> [String: Any] {
        let ns = error as NSError
        let words: [Int: String] = [1000: "unknown", 1001: "canceled", 1002: "invalidResponse", 1003: "notHandled", 1004: "failed",
                                    1005: "notInteractive", 1006: "matchedExcludedCredential", 1007: "credentialImport", 1008: "credentialExport"]
        let underlying = ns.userInfo[NSUnderlyingErrorKey] as? NSError
        var apple: [String: Any] = ["code": ns.domain == ASAuthorizationError.errorDomain ? words[ns.code] ?? String(ns.code) : "\(ns.domain) \(ns.code)"]
        if let underlying { apple["underlying"] = ["domain": underlying.domain, "code": underlying.code] }
        let name: String
        switch (ns.domain == ASAuthorizationError.errorDomain ? ns.code : -1) {
        case 1006: name = "InvalidStateError"
        case 1005: return ["error": "NotAllowedError", "message": "person-required", "apple": apple]
        // An RP ID the app is not associated with, or whose association the
        // device could not verify (no file at the domain yet), fails here (D6).
        case 1004 where ["not associated", "webcredentials association"].contains(where: "\(ns.localizedDescription) \(underlying?.localizedDescription ?? "")".contains):
            name = "SecurityError"
        default: name = "NotAllowedError"
        }
        return ["error": name, "message": "\(ns.localizedDescription) (\(ns.domain) \(ns.code))", "apple": apple]
    }

    // MARK: the JSON forms (§5.1.10)

    private func registration(_ credential: ASAuthorizationCredential) -> [String: Any]? {
        guard let r = credential as? ASAuthorizationPublicKeyCredentialRegistration, let object = r.rawAttestationObject,
              case .map(let attestation)? = CBOR.read(object), case .bytes(let authData)? = attestation["authData"] else { return nil }
        let id = b64(r.credentialID)
        var response: [String: Any] = ["clientDataJSON": b64(r.rawClientDataJSON), "attestationObject": b64(object), "authenticatorData": b64(authData)]
        var results: [String: Any] = ["credProps": ["rk": true]]
        var attachment = "cross-platform"
        if let p = r as? ASAuthorizationPlatformPublicKeyCredentialRegistration {
            response["transports"] = ["internal", "hybrid"]
            if #available(iOS 16.6, macOS 13.5, *) { attachment = p.attachment == .platform ? "platform" : "cross-platform" } else { attachment = "platform" }
            if #available(iOS 17, macOS 14, *), let blob = p.largeBlob { results["largeBlob"] = ["supported": blob.isSupported] }
            if #available(iOS 18, macOS 15, *), let prf = p.prf { results["prf"] = ["enabled": prf.isSupported] }
        } else {
            response["transports"] = ["usb", "nfc", "ble"]
        }
        if let (alg, spki) = Self.publicKey(authData) {
            response["publicKeyAlgorithm"] = alg
            if let spki { response["publicKey"] = b64(spki) }
        }
        return ["id": id, "rawId": id, "type": "public-key", "authenticatorAttachment": attachment, "response": response, "clientExtensionResults": results]
    }

    private static func assertion(_ a: ASAuthorizationPublicKeyCredentialAssertion) -> [String: Any] {
        let id = b64(a.credentialID)
        var results: [String: Any] = [:]
        var attachment = "cross-platform"
        if let p = a as? ASAuthorizationPlatformPublicKeyCredentialAssertion {
            if #available(iOS 16.6, macOS 13.5, *) { attachment = p.attachment == .platform ? "platform" : "cross-platform" } else { attachment = "platform" }
            if #available(iOS 17, macOS 14, *), let blob = p.largeBlob {
                switch blob.result {
                case .read(let data): results["largeBlob"] = data.map { ["blob": b64($0)] } ?? [:]
                case .write(let ok): results["largeBlob"] = ["written": ok]
                }
            }
        }
        var response: [String: Any] = ["clientDataJSON": b64(a.rawClientDataJSON), "authenticatorData": b64(a.rawAuthenticatorData), "signature": b64(a.signature)]
        if !a.userID.isEmpty { response["userHandle"] = b64(a.userID) }
        return ["id": id, "rawId": id, "type": "public-key", "authenticatorAttachment": attachment, "response": response, "clientExtensionResults": results]
    }

    /// The attested credential's COSE algorithm and, for an EC2 P-256 or
    /// RSA key, its SubjectPublicKeyInfo (§6.5.1.1, RFC 5480, RFC 8017).
    private static func publicKey(_ authData: Data) -> (Int, Data?)? {
        let d = [UInt8](authData)
        guard d.count > 55, d[32] & 0x40 != 0 else { return nil }
        let length = Int(d[53]) << 8 | Int(d[54])
        guard d.count > 55 + length, case .map(let key)? = CBOR.read(Data(d[(55 + length)...])),
              case .int(let alg)? = key[3] else { return nil }
        if alg == -7, case .bytes(let x)? = key[-2], case .bytes(let y)? = key[-3], x.count == 32, y.count == 32 {
            let prefix: [UInt8] = [0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
                                   0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00, 0x04]
            return (alg, Data(prefix) + x + y)
        }
        return (alg, nil)
    }
}

/// A CBOR reader (RFC 8949) of what an attestation object and a COSE key
/// hold: integers, byte and text strings, arrays and maps.
private enum CBOR: Hashable {
    case int(Int), bytes(Data), text(String), array([CBOR]), map([CBOR: CBOR]), other

    static func read(_ data: Data) -> CBOR? {
        var at = 0
        return item([UInt8](data), &at)
    }

    subscript(key: String) -> CBOR? { if case .map(let m) = self { return m[.text(key)] } else { return nil } }

    private static func item(_ b: [UInt8], _ at: inout Int) -> CBOR? {
        guard at < b.count else { return nil }
        let major = b[at] >> 5, info = Int(b[at] & 31)
        at += 1
        var n = info
        if info >= 24 {
            let size = [24: 1, 25: 2, 26: 4, 27: 8][info] ?? 0
            guard size > 0, at + size <= b.count else { return nil }
            n = b[at..<(at + size)].reduce(0) { $0 << 8 | Int($1) }
            at += size
        }
        switch major {
        case 0: return .int(n)
        case 1: return .int(-1 - n)
        case 2, 3:
            guard at + n <= b.count else { return nil }
            let slice = Data(b[at..<(at + n)]); at += n
            return major == 2 ? .bytes(slice) : .text(String(decoding: slice, as: UTF8.self))
        case 4:
            var out: [CBOR] = []
            for _ in 0..<n { guard let v = item(b, &at) else { return nil }; out.append(v) }
            return .array(out)
        case 5:
            var out: [CBOR: CBOR] = [:]
            for _ in 0..<n { guard let k = item(b, &at), let v = item(b, &at) else { return nil }; out[k] = v }
            return .map(out)
        default: return .other
        }
    }
}

private extension Dictionary where Key == CBOR, Value == CBOR {
    subscript(key: String) -> CBOR? { self[.text(key)] }
    subscript(key: Int) -> CBOR? { self[.int(key)] }
}
