// @exact/local-auth on Apple (LLP 1133.010 D1, D3, D8, D10-D13):
// LocalAuthentication's sheet, answered in the interface's shapes. The app's
// composed module calls it outside the agent; under the agent the runner's
// substitute answers and no context is made, unless the drive binds it
// (`--bind local-auth`).
//
// - `authenticate` evaluates `deviceOwnerAuthentication` unless the call
//   says otherwise (D3): `biometricOnly`, or `apple.policy`, which beats it.
// - The status's `state` is the Face ID permission (D12). Apple gives no
//   call that tells `prompt` from `granted`, so the module keeps, in its own
//   defaults, whether an evaluation that could use Face ID has completed.
// - The authenticator entry (D13): the host's presence backend asks it to
//   check the person for an item; it evaluates the policy the item's line
//   names (biometrics for `biometry-*`, else the device owner), which shows
//   the sheet, and the host reads the item with the context it returns, which
//   the keychain accepts without asking again. D6's
//   `evaluateAccessControl(_:operation:localizedReason:)` fails on the iOS
//   27 simulator with LAError -1020 before any sheet, so the access control
//   is not evaluated itself; a `device-credential` item read after a face
//   asks for the passcode in the keychain's own sheet.
// - Tier 3 (D11): `LocalAuthModule.hatch` receives each call's context after
//   the module's words are set, before evaluation. The module sets what it
//   owns again after the hatch returns (`interactionNotAllowed`, the reuse
//   duration), so a hatch cannot weaken a check, and a development build
//   names the change (`log`).
import ExactPackage
import Foundation
import LocalAuthentication
import Security

public final class LocalAuthModule: ExactPackage, ExactAuthenticator {
    /// The app's access hatch (D11): each call's context, before evaluation.
    public static var hatch: ((LAContext) -> Void)?

    private let changed: (String) -> Void
    private let log: (String) -> Void
    private let now: () -> Double
    private let context: ExactPackageContext
    /// Contexts whose sheet is up, invalidated if the session ends first.
    private var evaluating: [ObjectIdentifier: LAContext] = [:]
    /// The status last answered, to announce a change on return (D12).
    private var said: [String: Any]?
    /// An evaluation that could use Face ID has completed (D12).
    private static let usedKey = "exact.local-auth.faceID"
    private var faceIDUsed: Bool {
        get { UserDefaults.standard.bool(forKey: Self.usedKey) }
        set { UserDefaults.standard.set(newValue, forKey: Self.usedKey) }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        changed = context.changed
        log = context.log
        now = context.now
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "localAuthStatus": reply(status())
            case "authenticate": authenticate(args, origin: context.origin, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "localAuthStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    /// The person can change Face ID's permission or the passcode in Settings
    /// while the app is away: asked again on return (D12).
    public func foreground(_ shown: Bool) {
        guard shown, let said else { return }
        if !NSDictionary(dictionary: status()).isEqual(to: said) { changed("local-auth.localAuthStatus") }
    }

    public func destroy(_ done: @escaping () -> Void) {
        evaluating.values.forEach { $0.invalidate() }
        evaluating = [:]
        done()
    }

    // MARK: the status

    private func biometryWord(_ type: LABiometryType) -> String {
        switch type {
        case .none: return "none"
        case .touchID: return "touchID"
        case .faceID: return "faceID"
        default:
            if #available(iOS 17, macOS 14, *), type == .opticID { return "opticID" }
            return "other"
        }
    }

    private func status() -> [String: Any] {
        let c = LAContext()
        var ownerError: NSError?, biometricError: NSError?
        let owner = c.canEvaluatePolicy(.deviceOwnerAuthentication, error: &ownerError)
        let biometric = c.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &biometricError)
        let type = c.biometryType
        // Only a passcode that is not set says there is none; a lockout leaves it set.
        let credential = owner || ownerError.map { LAError.Code(rawValue: $0.code) != .passcodeNotSet } ?? true
        let state: String
        if type == .faceID {
            let refused = !biometric && biometricError.map { LAError.Code(rawValue: $0.code) == .biometryNotAvailable } == true
            state = refused ? "denied" : faceIDUsed ? "granted" : "prompt"
        } else {
            state = "granted"
        }
        var apple: [String: Any] = ["biometryType": type.rawValue]
        if #available(iOS 18, macOS 15, *), let hash = c.domainState.biometry.stateHash { apple["domainState"] = hash.base64EncodedString() }
        let value: [String: Any] = [
            "available": owner, "biometry": biometryWord(type), "deviceCredential": credential, "state": state,
            "presenceSecrets": credential && keychainKeepsLocked, "apple": apple, "android": NSNull(),
        ]
        said = value
        return value
    }

    /// The host's keychain keeps locked items: iOS's always; a Mac's only
    /// with a provisioned keychain group (D7). The runner's own answer
    /// replaces this on every asynchronous read.
    private var keychainKeepsLocked: Bool {
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return SecTaskCopyValueForEntitlement(task, "keychain-access-groups" as CFString, nil) != nil
        #else
        return true
        #endif
    }

    // MARK: the check

    /// `apple.policy` by Apple's name, where this OS has it (D10).
    private func policy(_ word: String?, biometricOnly: Bool) -> LAPolicy {
        switch word {
        case "deviceOwnerAuthentication": return .deviceOwnerAuthentication
        case "deviceOwnerAuthenticationWithBiometrics": return .deviceOwnerAuthenticationWithBiometrics
        default: break
        }
        #if os(macOS)
        if word == "deviceOwnerAuthenticationWithWatch" { return .deviceOwnerAuthenticationWithWatch }
        if word == "deviceOwnerAuthenticationWithBiometricsOrWatch" { return .deviceOwnerAuthenticationWithBiometricsOrWatch }
        #endif
        if #available(iOS 18, macOS 15, *) {
            if word == "deviceOwnerAuthenticationWithCompanion" { return .deviceOwnerAuthenticationWithCompanion }
            if word == "deviceOwnerAuthenticationWithBiometricsOrCompanion" { return .deviceOwnerAuthenticationWithBiometricsOrCompanion }
        }
        return biometricOnly ? .deviceOwnerAuthenticationWithBiometrics : .deviceOwnerAuthentication
    }

    /// A context with the call's words, then the hatch, then the module's own
    /// fields set again (D11).
    private func made(_ apple: [String: Any], origin: String) -> LAContext {
        let c = LAContext()
        if let title = apple["localizedFallbackTitle"] as? String { c.localizedFallbackTitle = title }
        if let title = apple["localizedCancelTitle"] as? String { c.localizedCancelTitle = title }
        // Touch ID's reuse of the device's own unlock is a person's to use (D12).
        let reuse = origin == "app" ? min(apple["touchIDAuthenticationAllowableReuseDuration"] as? Double ?? 0, LATouchIDAuthenticationMaximumAllowableReuseDuration) : 0
        c.touchIDAuthenticationAllowableReuseDuration = reuse
        c.interactionNotAllowed = false
        Self.hatch?(c)
        if c.interactionNotAllowed || c.touchIDAuthenticationAllowableReuseDuration != reuse {
            log("local-auth: the context hatch changed interactionNotAllowed or the reuse duration, which the module owns; set again (LLP 1133.010 D11)")
        }
        c.interactionNotAllowed = false
        c.touchIDAuthenticationAllowableReuseDuration = reuse
        return c
    }

    /// D1's code for a LocalAuthentication error, with Apple's own beside it.
    private func failure(_ error: Error?, _ c: LAContext) -> [String: Any] {
        let ns = error as NSError?
        let code = ns.flatMap { LAError.Code(rawValue: $0.code) }
        let word: String
        switch code {
        case .userCancel?, .userFallback?: word = "USER_CANCEL"
        case .systemCancel?, .appCancel?, .invalidContext?: word = "SYSTEM_CANCEL"
        case .authenticationFailed?: word = "AUTHENTICATION_FAILED"
        case .biometryLockout?: word = "LOCKOUT"
        case .notInteractive?: return ["error": "PERMISSION_DENIED", "message": "person-required"]
        // Face ID refused for the app reads as not available (D12).
        case .biometryNotAvailable? where c.biometryType == .faceID && faceIDRefused(): word = "PERMISSION_DENIED"
        default: word = "NOT_AVAILABLE"
        }
        let message = ns.map { "\($0.localizedDescription) (\($0.domain) \($0.code))" } ?? word
        return ["error": word, "message": message, "apple": ["code": ns?.code ?? 0]]
    }

    private func faceIDRefused() -> Bool {
        var error: NSError?
        let ok = LAContext().canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
        return !ok && error.map { LAError.Code(rawValue: $0.code) == .biometryNotAvailable } == true
    }

    /// The evaluation ended: a Face ID one counts as used (D12).
    private func ended(_ c: LAContext) {
        evaluating[ObjectIdentifier(c)] = nil
        if c.biometryType == .faceID, !faceIDUsed {
            faceIDUsed = true
            changed("local-auth.localAuthStatus")
        }
    }

    private func authenticate(_ args: [String: Any], origin: String, _ reply: @escaping ([String: Any]) -> Void) {
        // Presence is not delegable (D12); the runner refuses first, and this holds without it.
        guard origin == "app" else { return reply(["error": "PERMISSION_DENIED", "message": "person-required"]) }
        let apple = args["apple"] as? [String: Any] ?? [:]
        let chosen = policy(apple["policy"] as? String, biometricOnly: args["biometricOnly"] as? Bool ?? false)
        let c = made(apple, origin: origin)
        evaluating[ObjectIdentifier(c)] = c
        c.evaluatePolicy(chosen, localizedReason: args["reason"] as? String ?? "") { ok, error in
            DispatchQueue.main.async { [self] in
                ended(c)
                c.invalidate()
                reply(ok ? ["timestamp": Date().timeIntervalSince1970 * 1000, "android": NSNull()] : failure(error, c))
            }
        }
    }

    public func evaluate(accessControl: SecAccessControl, reason: String, policy: String, origin: String,
                         reply: @escaping (AnyObject?, [String: Any]?) -> Void) {
        guard origin == "app" else { return reply(nil, ["error": "PERMISSION_DENIED", "message": "person-required"]) }
        let c = made([:], origin: origin)
        evaluating[ObjectIdentifier(c)] = c
        let biometric = policy.hasPrefix("biometry-")
        c.evaluatePolicy(biometric ? .deviceOwnerAuthenticationWithBiometrics : .deviceOwnerAuthentication, localizedReason: reason) { ok, error in
            DispatchQueue.main.async { [self] in
                ended(c)
                if ok { return reply(c, nil) }
                c.invalidate()
                reply(nil, failure(error, c))
            }
        }
    }
}
