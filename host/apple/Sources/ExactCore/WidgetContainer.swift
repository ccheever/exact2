// @ref LLP 1126 D8 — the app publishes, an extension pins and only reads.
import Foundation
#if EXACT_WIDGET_RELOAD
import WidgetKit
@_cdecl("exact_widget_reload")
public func exactWidgetReload() { WidgetCenter.shared.reloadAllTimelines() }
#else
import CommonCrypto

public enum WidgetContainer {
    public static var appID: String? { Bundle.main.object(forInfoDictionaryKey: "ExactAppIdentifier") as? String }
    public static var root: URL? {
        guard let group = Bundle.main.object(forInfoDictionaryKey: "ExactAppGroup") as? String else { return nil }
        return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
    }
    /// Called before any session. Only widget-enabled binaries carry these keys.
    public static func configure() {
        guard let root, let appID else { return }
        setenv("EXACT_APP_GROUP_ROOT", root.path, 1)
        setenv("EXACT_APP_GROUP_APP", appID, 1)
        if Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") != nil {
            setenv("EXACT_WIDGET_WRITER", appID, 1)
        }
    }
    public static var updateBase: URL? { root?.appendingPathComponent("Library/Application Support", isDirectory: true) }
    private static var compatibility: String {
        let object = (try? JSONSerialization.jsonObject(with: Runtime.bakedCompat())) as? [String: Any]
        return object?["id"] as? String ?? ""
    }
    /// No lock, selection, repair or network access: read the pointer once and
    /// keep its immutable directory for this entire invocation. Stage 1 keeps
    /// all published generations; reclamation needs cross-process leases first.
    public static func pin() throws -> URL {
        guard let root else { throw HeadlessSession.Failure(description: "widget App Group is unavailable") }
        let base = root.appendingPathComponent("widget-generations", isDirectory: true)
        let name = try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8)
        guard name.count == 64, name.allSatisfy({ $0.isHexDigit }) else { throw CocoaError(.fileReadCorruptFile) }
        let generation = base.appendingPathComponent(name, isDirectory: true)
        guard try String(contentsOf: generation.appendingPathComponent("compatibility"), encoding: .utf8) == compatibility else {
            throw HeadlessSession.Failure(description: "widget generation belongs to another compatibility cohort")
        }
        return generation
    }
    private static let publisher = DispatchQueue(label: "exact.widget-generation", qos: .utility)
    private static var typescript: Bool { Bundle.main.object(forInfoDictionaryKey: "ExactWidgetTypeScript") as? Bool == true }
    private static let statusLock = NSLock()
    private static var lastFailure: String?
    public static var publicationFailure: String? {
        statusLock.lock(); defer { statusLock.unlock() }; return lastFailure
    }
    private static func report(_ error: Error?) {
        statusLock.lock(); lastFailure = error.map(String.init(describing:)); statusLock.unlock()
        if let error { FileHandle.standardError.write(Data("widget generation: \(error)\n".utf8)) }
    }
    private static var storeObserver: WidgetStoreObserver?
    /// Installed by the running app, never by the extension.
    package static func observeStore(foreground: Notification.Name, changed: @escaping () -> Void) {
        guard storeObserver == nil, let root, let appID,
              Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") != nil else { return }
        storeObserver = WidgetStoreObserver(root: root, name: appID + ".widget-store", foreground: foreground, changed: changed)
    }
    @MainActor package static func reloadAfterStoreChange() {
        do { try reloadTimelines() } catch { report(error) }
    }
    /// Copy assets off the first-frame path; first-use framework loading and
    /// WidgetCenter's call run on main. A failed signal stays pending on disk.
    package static func publishAfterFirstPixel(_ assets: AssetResolver) {
        guard let root, let kinds = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String], !kinds.isEmpty else { return }
        publisher.async {
            do {
                guard try publish(assets, root: root, kinds: kinds, compatibility: compatibility, typescript: typescript) else { return }
                DispatchQueue.main.async {
                    do { try signal(root: root) { try reloadTimelines() }; report(nil) }
                    catch { report(error) }
                }
            } catch { report(error) }
        }
    }
    private typealias Reload = @convention(c) () -> Void
    @MainActor private static var reloader: (path: String, call: Reload)?
    @MainActor package static func reloadTimelines(artifact: URL? = Bundle.main.privateFrameworksURL?.appendingPathComponent("ExactWidgetReload.framework/ExactWidgetReload")) throws {
        precondition(Thread.isMainThread)
        guard let path = artifact?.path else { throw HeadlessSession.Failure(description: "widget reload: app has no frameworks directory") }
        if reloader?.path != path {
            _ = dlerror()
            guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
                throw HeadlessSession.Failure(description: "widget reload: \(dlerror().map { String(cString: $0) } ?? "dlopen failed"): \(path)")
            }
            _ = dlerror()
            guard let symbol = dlsym(library, "exact_widget_reload") else {
                let reason = dlerror().map { String(cString: $0) } ?? "missing exact_widget_reload"
                dlclose(library)
                throw HeadlessSession.Failure(description: "widget reload: \(reason): \(path)")
            }
            reloader = (path, unsafeBitCast(symbol, to: Reload.self))
        }
        reloader?.call()
    }
    @MainActor package static func signal(root: URL, reload: () throws -> Void) throws {
        precondition(Thread.isMainThread)
        let base = root.appendingPathComponent("widget-generations")
        let digest = try Data(contentsOf: base.appendingPathComponent("current"))
        let signalled = base.appendingPathComponent("signalled")
        if (try? Data(contentsOf: signalled)) == digest { return }
        try reload()
        try digest.write(to: signalled, options: .atomic)
        FileHandle.standardError.write(Data("widget reload: signalled \(String(decoding: digest, as: UTF8.self))\n".utf8))
    }
    package static func publish(_ assets: AssetResolver) throws -> Bool {
        guard let root, let kinds = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String], !kinds.isEmpty else { return false }
        return try publish(assets, root: root, kinds: kinds, compatibility: compatibility, typescript: typescript)
    }
    package static func publish(_ assets: AssetResolver, root: URL, kinds: [String], compatibility: String, typescript: Bool = false) throws -> Bool {
        // A signed bake or accepted delivery identifies the bytes already. An
        // unchanged launch reads only current, without opening any asset.
        let stamp = Data((compatibility + "\n" + assets.publicationID).utf8)
        var hash = [UInt8](repeating: 0, count: Int(CC_SHA256_DIGEST_LENGTH))
        _ = stamp.withUnsafeBytes { CC_SHA256($0.baseAddress, CC_LONG(stamp.count), &hash) }
        let digest = hash.map { String(format: "%02x", $0) }.joined()
        let base = root.appendingPathComponent("widget-generations", isDirectory: true)
        let current = base.appendingPathComponent("current")
        if (try? String(contentsOf: current, encoding: .utf8)) == digest {
            return (try? String(contentsOf: base.appendingPathComponent("signalled"), encoding: .utf8)) != digest
        }
        let final = base.appendingPathComponent(digest, isDirectory: true)
        let fm = FileManager.default
        try fm.createDirectory(at: base, withIntermediateDirectories: true)
        if !fm.fileExists(atPath: final.path) {
            let names = try assets.widgetNames()
            for kind in kinds where !names.contains("plans/\(kind).plan") { throw HeadlessSession.Failure(description: "generation lacks widget \(kind)") }
            if typescript && !["typescript/app.hbc", "typescript/app.module.json"].allSatisfy(names.contains) {
                throw HeadlessSession.Failure(description: "widget generation lacks the TypeScript module pair")
            }
            let staging = base.appendingPathComponent(".\(UUID().uuidString)", isDirectory: true)
            try fm.createDirectory(at: staging, withIntermediateDirectories: false)
            defer { try? fm.removeItem(at: staging) }
            for name in names {
                guard let bytes = try assets.delivery(name) else { throw CocoaError(.fileReadNoSuchFile) }
                let file = staging.appendingPathComponent(name)
                try fm.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
                try bytes.write(to: file)
            }
            try Data(compatibility.utf8).write(to: staging.appendingPathComponent("compatibility"))
            try fm.moveItem(at: staging, to: final)
        }
        try Data(digest.utf8).write(to: current, options: .atomic)
        return true
    }
}

/// Both a Darwin ping and foreground take this same counter check. The
/// notification carries no state; coalesced or missed pings are harmless.
package final class WidgetStoreObserver {
    private let counter: URL
    private let name: String
    private let changed: () -> Void
    private var generation: UInt64?
    private var foregroundToken: NSObjectProtocol?
    package init(root: URL, name: String, foreground: Notification.Name, changed: @escaping () -> Void) {
        counter = root.appendingPathComponent("widget-store-generation")
        self.name = name
        self.changed = changed
        foregroundToken = NotificationCenter.default.addObserver(forName: foreground, object: nil, queue: .main) { [weak self] _ in self?.check() }
        CFNotificationCenterAddObserver(CFNotificationCenterGetDarwinNotifyCenter(), Unmanaged.passUnretained(self).toOpaque(), { _, observer, _, _, _ in
            guard let observer else { return }
            let owner = Unmanaged<WidgetStoreObserver>.fromOpaque(observer).takeUnretainedValue()
            DispatchQueue.main.async { [weak owner] in owner?.check() }
        }, name as CFString, nil, .deliverImmediately)
    }
    deinit {
        if let foregroundToken { NotificationCenter.default.removeObserver(foregroundToken) }
        CFNotificationCenterRemoveObserver(CFNotificationCenterGetDarwinNotifyCenter(), Unmanaged.passUnretained(self).toOpaque(), CFNotificationName(name as CFString), nil)
    }
    package func check() {
        precondition(Thread.isMainThread)
        guard let text = try? String(contentsOf: counter, encoding: .utf8),
              let next = UInt64(text), next != generation else { return }
        generation = next
        changed()
    }
}

#endif
