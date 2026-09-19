import Foundation

/// One complete generation's resolver. Stored bytes are verified once and
/// retained; an absent name never falls through to another generation.
public final class AssetResolver {
    let root: URL
    private let names: [String]?
    private let read: ((String) throws -> Data?)?
    var isComplete: Bool { names != nil }
    private var cache: [String: Data] = [:]
    private var files: [String: URL] = [:]
    private var directory: URL?
    private(set) var refusal: String?

    /// A complete provider never falls through to the embedded directory.
    public init(root: URL, names: [String], read: @escaping (String) throws -> Data?) {
        self.root = root; self.names = names; self.read = read
    }

    /// Own verified payloads without retaining texture heap buffers. The private
    /// files live exactly as long as this generation and can be read after loss.
    init(root: URL, verified: [String: Data]) throws {
        self.root = root; names = Array(verified.keys); read = nil
        for (name, bytes) in verified {
            guard Self.validAssetName(name.hasPrefix("assets/") ? String(name.dropFirst(7)) : name) else {
                throw CocoaError(.fileReadInvalidFileName)
            }
            if name.hasSuffix(".tex") { _ = try materialize(name, bytes) }
            else { cache[name] = bytes }
        }
    }

    private func materialize(_ name: String, _ bytes: Data) throws -> URL {
        if directory == nil {
            let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-generation-\(UUID().uuidString)", isDirectory: true)
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            directory = dir
        }
        let filename = Data(name.utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        let url = directory!.appendingPathComponent(filename)
        try bytes.write(to: url, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: 0o400], ofItemAtPath: url.path)
        files[name] = url
        return url
    }

    init(root: URL) {
        self.root = root; names = nil; read = nil
    }

    deinit { if let directory { try? FileManager.default.removeItem(at: directory) } }

    func bytes(_ name: String) -> Data? {
        do { return try delivery(name) }
        catch { refusal = refusal ?? error.localizedDescription; return nil }
    }

    /// GPU textures are consumed once. Fonts, images, models and shaders remain
    /// reusable cache entries; the renderer re-requests textures after device loss.
    func delivery(_ name: String) throws -> Data? {
        guard Self.validAssetName(name.hasPrefix("assets/") ? String(name.dropFirst(7)) : name) else {
            throw NSError(domain: "ExactAssets", code: 1, userInfo: [NSLocalizedDescriptionKey: "asset `\(name)`: invalid name"])
        }
        if let bytes = cache[name] { return bytes }
        if let url = files[name] { return try Data(contentsOf: url) }
        if let read {
            guard names?.contains(name) == true else { return nil }
            let bytes = try read(name)
            if let bytes, !name.hasSuffix(".tex") { cache[name] = bytes }
            return bytes
        }
        guard !isComplete, let url = embeddedURL(name) else { return nil }
        do { return try Data(contentsOf: url) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile { return nil }
    }

    func url(_ name: String) -> URL? {
        guard isComplete else { return embeddedURL(name) }
        if let url = files[name] { return url }
        guard let bytes = bytes(name) else { return nil }
        do { return try materialize(name, bytes)
        } catch { refusal = refusal ?? "verified asset materialization failed: \(error)"; return nil }
    }

    /// The complete shader names, without loading the optional GPU module.
    func shaderSources() -> [String: Data] {
        let names: [String]
        if let completeNames = self.names { names = completeNames.filter { $0.hasPrefix("shaders/") && $0.hasSuffix(".wgsl") } }
        else {
            let fm = FileManager.default
            let prefix = fm.fileExists(atPath: root.appendingPathComponent("shaders").path) ? "shaders" : "gpu/shaders"
            names = ((try? fm.contentsOfDirectory(atPath: root.appendingPathComponent(prefix).path)) ?? []).filter { $0.hasSuffix(".wgsl") }.map { prefix + "/" + $0 }
        }
        var sources: [String: Data] = [:]
        for name in names { if let data = bytes(name) { sources[URL(fileURLWithPath: name).deletingPathExtension().lastPathComponent] = data } }
        return sources
    }

    /// The same cases and byte limit as gpu::asset_name.
    static func validAssetName(_ name: String) -> Bool {
        let bytes = Array(name.utf8)
        return !bytes.isEmpty && bytes.count <= 128
            && bytes.allSatisfy { $0 >= 32 && $0 < 127 && $0 != 92 }
            && name.split(separator: "/", omittingEmptySubsequences: false).allSatisfy { !$0.isEmpty && $0 != "." && $0 != ".." }
    }

    private func embeddedURL(_ name: String) -> URL? {
        guard Self.validAssetName(name.hasPrefix("assets/") ? String(name.dropFirst(7)) : name) else { return nil }
        let base = root.standardizedFileURL.resolvingSymlinksInPath()
        let url = base.appendingPathComponent(name).standardizedFileURL.resolvingSymlinksInPath()
        return url.path.hasPrefix(base.path + "/") ? url : nil
    }
}
