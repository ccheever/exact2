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
    /// The serial publisher preserves acceptance order and keeps asset copying
    /// off the app's first-frame path.
    package static func publishAfterFirstPixel(_ assets: AssetResolver) {
        guard Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") != nil else { return }
        publisher.async {
            do {
                let changed = try publish(assets)
                if changed { try reloadTimelines() }
            } catch { FileHandle.standardError.write(Data("widget generation: \(error)\n".utf8)) }
        }
    }
    // Publishing is the first use of WidgetKit. Its reload API stays in a
    // demand-loaded artifact so the shared runtime brings no SwiftUI to launch.
    private static let reloader: (@convention(c) () -> Void)? = {
        guard let path = Bundle.main.privateFrameworksURL?.appendingPathComponent("ExactWidgetReload.framework/ExactWidgetReload").path,
              let library = dlopen(path, RTLD_NOW | RTLD_LOCAL), let symbol = dlsym(library, "exact_widget_reload") else { return nil }
        return unsafeBitCast(symbol, to: (@convention(c) () -> Void).self)
    }()
    private static func reloadTimelines() throws {
        guard let reloader else { throw HeadlessSession.Failure(description: "widget reload artifact is unavailable") }
        reloader()
    }
    package static func publish(_ assets: AssetResolver) throws -> Bool {
        guard let root, let kinds = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String], !kinds.isEmpty else { return false }
        return try publish(assets, root: root, kinds: kinds, compatibility: compatibility)
    }
    package static func publish(_ assets: AssetResolver, root: URL, kinds: [String], compatibility: String) throws -> Bool {
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

#endif
