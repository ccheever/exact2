// Locked secrets' keychain and the person's check (LLP 1133.010 D6, D8,
// D13). The Rust backend (`host/apple/src/presence_secrets.rs`) owns every
// move and calls the keychain through the hook installed here: one
// generic-password item per name and generation, its service the app's
// store service and `.presence`, its account `<name>#<generation>`, its
// access control the grant line's policy. None of those calls shows the
// person anything.
//
// The check is the batch's `presence` op: the app module's authenticator
// entry (`@exact/local-auth`'s implementation) evaluates the item's access
// control, which shows the platform's sheet; this then reads the item with
// the evaluated context off the main thread, since `SecItemCopyMatching`
// blocks its caller, invalidates the context, and answers on the store's
// completion path (`exact_presence`), never the module route. Under the
// agent with the implementation bound (`--bind local-auth`) the op names no
// service: the drive's own store holds the value, and only the check runs.
//
// The context crosses as an object and is invalidated through the runtime,
// so ExactKit links no LocalAuthentication: only an app whose module
// composes the implementation does (LLP 1133.010 D16).
import CExact
import Foundation
import Security

/// The keychain primitives the Rust backend calls (`exact_on_presence_keychain`).
enum PresenceKeychain {
    private static let installed: Void = { exact_on_presence_keychain(presenceKeychainHook) }()
    /// Once a process, before the first boot reconciles the backend.
    static func install() { _ = installed }

    /// The access control a presence line's words ask for (D6, D10).
    static func accessControl(_ words: String) -> SecAccessControl? {
        let w = words.split(separator: " ").map(String.init)
        var flags: SecAccessControlCreateFlags
        switch w.first {
        case "biometry-any": flags = .biometryAny
        case "biometry-current-set": flags = .biometryCurrentSet
        case "device-credential": flags = .devicePasscode
        default: flags = .userPresence
        }
        var alternatives: SecAccessControlCreateFlags = []
        #if os(macOS)
        if w.contains("apple.orWatch") { alternatives.insert(.watch) }
        #endif
        #if !os(tvOS)
        if w.contains("apple.orCompanion"), #available(iOS 18, macOS 15, *) { alternatives.insert(.companion) }
        #endif
        if !alternatives.isEmpty { flags.formUnion(alternatives.union(.or)) }
        let accessible = w.contains("apple.accessible=whenUnlockedThisDeviceOnly")
            ? kSecAttrAccessibleWhenUnlockedThisDeviceOnly : kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
        return SecAccessControlCreateWithFlags(nil, accessible, flags, nil)
    }

    /// Whether this build keeps locked items: iOS's keychain is the data
    /// protection one; a Mac's only with a keychain access group, which a
    /// provisioning profile authorizes (D7). tvOS keeps none (D15).
    static var capable: Bool {
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return ["keychain-access-groups", "com.apple.application-identifier"].contains {
            SecTaskCopyValueForEntitlement(task, $0 as CFString, nil) != nil
        }
        #elseif os(tvOS)
        return false
        #else
        return true
        #endif
    }

    private static func query(_ service: String, _ account: String? = nil) -> [CFString: Any] {
        var q: [CFString: Any] = [kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecUseDataProtectionKeychain: true]
        if let account { q[kSecAttrAccount] = account }
        return q
    }

    /// The hook's ops (exact.h): a status and the bytes it answers.
    fileprivate static func run(_ op: UInt32, _ request: [String: Any], _ value: Data) -> (Int32, Data) {
        let service = request["service"] as? String ?? "", account = request["account"] as? String
        switch op {
        case 1:
            guard let control = accessControl(request["policy"] as? String ?? "") else { return failed("the access control could not be made") }
            var q = query(service, account)
            q[kSecValueData] = value
            q[kSecAttrAccessControl] = control
            return done(SecItemAdd(q as CFDictionary, nil))
        case 2:
            let status = SecItemDelete(query(service, account) as CFDictionary)
            return status == errSecItemNotFound ? (1, Data()) : done(status)
        case 3:
            // A protected item's attributes also require authentication on
            // a phone. Probe one indexed account without permitting a sheet.
            guard let account else { return failed("an existence check needs an account") }
            var q = query(service, account)
            q[kSecUseAuthenticationUI] = kSecUseAuthenticationUIFail
            let status = SecItemCopyMatching(q as CFDictionary, nil)
            if status == errSecItemNotFound { return (1, Data()) }
            if status == errSecInteractionNotAllowed { return (0, Data()) }
            return done(status)
        case 4:
            let status = SecItemDelete(query(service) as CFDictionary)
            return status == errSecItemNotFound ? (0, Data()) : done(status)
        case 5:
            return (capable ? 0 : 1, Data())
        default:
            return failed("keychain op \(op) is not one this host knows")
        }
    }

    private static func done(_ status: OSStatus) -> (Int32, Data) {
        status == errSecSuccess ? (0, Data()) : failed(message(status))
    }

    private static func failed(_ message: String) -> (Int32, Data) { (-1, Data(message.utf8)) }

    static func message(_ status: OSStatus) -> String {
        "keychain \(status): \(SecCopyErrorMessageString(status, nil) as String? ?? "unknown")"
    }

    /// The item, read with the context the sheet evaluated, off the main
    /// thread: a value, nothing (the platform dropped it, D6), or a failure.
    static func read(service: String, account: String, context: AnyObject) -> [String: Any] {
        var q = query(service, account)
        q[kSecReturnData] = true
        q[kSecMatchLimit] = kSecMatchLimitOne
        q[kSecUseAuthenticationContext] = context
        var found: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &found)
        switch status {
        case errSecSuccess:
            guard let data = found as? Data else { return ["status": "read"] }
            return ["status": "read", "value": String(decoding: data, as: UTF8.self)]
        case errSecItemNotFound: return ["status": "read"]
        case errSecUserCanceled: return ["status": "failed", "code": "USER_CANCEL", "message": message(status)]
        default: return ["status": "failed", "code": "AUTHENTICATION_FAILED", "message": message(status)]
        }
    }
}

/// `exact_on_presence_keychain`'s hook, on the runtime's thread.
private let presenceKeychainHook: ExactPresenceKeychainFn = { op, json, len, value, valueLen, reply, done in
    let request = json.flatMap { try? JSONSerialization.jsonObject(with: Data(bytes: $0, count: len)) as? [String: Any] } ?? [:]
    let bytes = value.map { Data(bytes: $0, count: valueLen) } ?? Data()
    let (status, out) = PresenceKeychain.run(op, request, bytes)
    out.withUnsafeBytes { done?(reply, status, $0.bindMemory(to: UInt8.self).baseAddress, out.count) }
}

/// One check's continuation, through the module's `done`.
private final class PresenceCheck {
    let then: (AnyObject?, [String: Any]?) -> Void
    init(_ then: @escaping (AnyObject?, [String: Any]?) -> Void) { self.then = then }
}

/// The module's answer to `authenticate`, from any thread.
private let presenceDone: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void = { ctx, context, error, len in
    guard let ctx else { return }
    let check = Unmanaged<PresenceCheck>.fromOpaque(ctx).takeRetainedValue()
    let object = context.map { Unmanaged<AnyObject>.fromOpaque($0).takeRetainedValue() }
    let failure = error.flatMap { try? JSONSerialization.jsonObject(with: Data(bytes: $0, count: len)) as? [String: Any] }
    DispatchQueue.main.async { check.then(object, failure) }
}

/// Ends an evaluated context's authorization (`LAContext.invalidate()`),
/// by the runtime, so ExactKit links no LocalAuthentication.
private func invalidate(_ context: AnyObject) {
    let selector = NSSelectorFromString("invalidate")
    if context.responds(to: selector) { _ = context.perform(selector) }
}

/// The authenticator entry's type (NativeModule.swift's table, offset 232).
typealias PresenceAuthenticateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?,
                                                   (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void)?) -> Void

extension ExactSession {
    /// A batch's `presence` op: the platform's check, then the item's read.
    func presenceOp(_ op: [String: Any]) {
        guard let call = (op["call"] as? NSNumber)?.uint64Value else { return }
        let answer = { [weak self] (fields: [String: Any]) in
            var fields = fields
            fields["call"] = call
            self?.runtime.presence(fields)
        }
        let failed = { [weak self] (code: String, message: String) in
            self?.runtime.log("presence \(op["name"] as? String ?? ""): the platform's check failed \(code): \(message)")
            answer(["status": "failed", "code": code, "message": message])
        }
        let policy = op["policy"] as? String ?? ""
        guard let control = PresenceKeychain.accessControl(policy) else {
            return failed("NOT_AVAILABLE", "this device cannot make the access control for \(policy)")
        }
        guard let (module, entry) = natives.authenticator() else {
            return failed("NotSupportedError", "this build's module serves no authenticator (LLP 1133.010 D13)")
        }
        let service = op["service"] as? String, account = op["account"] as? String
        let check = PresenceCheck { context, error in
            guard let context else {
                return failed(error?["error"] as? String ?? "UnknownError", error?["message"] as? String ?? "")
            }
            guard let service, let account else {
                invalidate(context)
                return answer(["status": "passed"])
            }
            DispatchQueue.global(qos: .userInitiated).async {
                let read = PresenceKeychain.read(service: service, account: account, context: context)
                invalidate(context)
                DispatchQueue.main.async { answer(read) }
            }
        }
        let request: [String: Any] = ["reason": op["reason"] as? String ?? "", "policy": policy, "origin": op["origin"] as? String ?? "app"]
        let json = (try? JSONSerialization.data(withJSONObject: request)) ?? Data()
        let ctx = Unmanaged.passRetained(check).toOpaque()
        json.withUnsafeBytes { entry(module, $0.bindMemory(to: UInt8.self).baseAddress, json.count, Unmanaged.passUnretained(control).toOpaque(), ctx, presenceDone) }
    }
}
