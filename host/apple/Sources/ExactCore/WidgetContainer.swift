// @ref LLP 1126 D8 — the app publishes, an extension pins and only reads.
import Foundation
import CommonCrypto
#if EXACT_WIDGETS
import WidgetKit
#endif

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
            _ = storeObserver
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
    private static var storeGeneration = ""
    private static let storeObserver: Void = {
        guard let appID else { return }
        let name = appID + ".widget-store"
        CFNotificationCenterAddObserver(CFNotificationCenterGetDarwinNotifyCenter(), nil, { _, _, _, _, _ in
            publisher.async {
                guard let root, let next = try? String(contentsOf: root.appendingPathComponent("widget-store-generation"), encoding: .utf8), next != storeGeneration else { return }
                storeGeneration = next
                #if EXACT_WIDGETS
                WidgetCenter.shared.reloadAllTimelines()
                #endif
            }
        }, name as CFString, nil, .deliverImmediately)
    }()
    /// The serial publisher preserves acceptance order and keeps asset copying
    /// off the app's first-frame path.
    package static func publishAfterFirstPixel(_ assets: AssetResolver) {
        guard Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") != nil else { return }
        publisher.async {
            do {
                let changed = try publish(assets)
                #if EXACT_WIDGETS
                if changed { WidgetCenter.shared.reloadAllTimelines() }
                #endif
            } catch { FileHandle.standardError.write(Data("widget generation: \(error)\n".utf8)) }
        }
    }
    package static func publish(_ assets: AssetResolver) throws -> Bool {
        guard let root, let kinds = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String], !kinds.isEmpty else { return false }
        return try publish(assets, root: root, kinds: kinds, compatibility: compatibility, typescript: Bundle.main.object(forInfoDictionaryKey: "ExactWidgetTypeScript") as? Bool == true)
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
        if (try? String(contentsOf: current, encoding: .utf8)) == digest { return false }
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
