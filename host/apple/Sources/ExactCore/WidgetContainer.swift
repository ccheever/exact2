// @ref LLP 1126 D8 — the app publishes, an extension pins and only reads.
import Foundation
import CryptoKit
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
    /// and hashing off the app's first-frame path.
    package static func publishAfterFirstPixel(_ assets: AssetResolver) {
        guard Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") != nil else { return }
        publisher.async {
            do {
                try publish(assets)
                #if EXACT_WIDGETS
                WidgetCenter.shared.reloadAllTimelines()
                #endif
            } catch { FileHandle.standardError.write(Data("widget generation: \(error)\n".utf8)) }
        }
    }
    package static func publish(_ assets: AssetResolver) throws {
        guard let root, let kinds = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetKinds") as? [String], !kinds.isEmpty else { return }
        let names = try assets.widgetNames()
        var payload: [(String, Data)] = []
        for name in names.sorted() {
            guard let bytes = try assets.delivery(name) else { throw CocoaError(.fileReadNoSuchFile) }
            payload.append((name, bytes))
        }
        for kind in kinds where !names.contains("plans/\(kind).plan") { throw HeadlessSession.Failure(description: "generation lacks widget \(kind)") }
        payload.append(("compatibility", Data(compatibility.utf8)))
        var hash = SHA256()
        for (name, bytes) in payload { hash.update(data: Data("\(name.utf8.count):\(name):\(bytes.count):".utf8)); hash.update(data: bytes) }
        let digest = hash.finalize().map { String(format: "%02x", $0) }.joined()
        let base = root.appendingPathComponent("widget-generations", isDirectory: true)
        let final = base.appendingPathComponent(digest, isDirectory: true)
        let fm = FileManager.default
        try fm.createDirectory(at: base, withIntermediateDirectories: true)
        if !fm.fileExists(atPath: final.path) {
            let staging = base.appendingPathComponent(".\(UUID().uuidString)", isDirectory: true)
            try fm.createDirectory(at: staging, withIntermediateDirectories: false)
            defer { try? fm.removeItem(at: staging) }
            for (name, bytes) in payload {
                let file = staging.appendingPathComponent(name)
                try fm.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
                try bytes.write(to: file)
            }
            try fm.moveItem(at: staging, to: final)
        }
        try Data(digest.utf8).write(to: base.appendingPathComponent("current"), options: .atomic)
    }
}
