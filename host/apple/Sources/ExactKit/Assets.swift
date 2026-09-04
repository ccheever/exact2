import Foundation

/// One complete generation's resolver. Stored bytes are verified once and
/// retained; an absent name never falls through to another generation.
public final class AssetResolver {
    let root: URL
    private let names: [String]?
    private let read: ((String) throws -> Data?)?
    var isComplete: Bool { names != nil }
    let overrides: [String: URL]
    private var cache: [String: Data] = [:]
    private var files: [String: URL] = [:]
    private var directory: URL?
    private(set) var refusal: String?

    /// A complete provider never falls through to the embedded directory.
    public init(root: URL, names: [String], read: @escaping (String) throws -> Data?) {
        self.root = root; self.names = names; self.read = read; overrides = [:]
    }

    init(root: URL, overrides: [String: URL] = [:]) {
        self.root = root; names = nil; read = nil; self.overrides = overrides
    }

    deinit { if let directory { try? FileManager.default.removeItem(at: directory) } }

    func bytes(_ name: String) -> Data? {
        if let bytes = cache[name] { return bytes }
        if let read {
            guard names?.contains(name) == true else { return nil }
            do { let bytes = try read(name); if let bytes { cache[name] = bytes }; return bytes }
            catch { refusal = refusal ?? error.localizedDescription; return nil }
        }
        guard let url = embeddedURL(name) else { return nil }
        return try? Data(contentsOf: url)
    }

    func url(_ name: String) -> URL? {
        guard isComplete else { return embeddedURL(name) }
        if let url = files[name] { return url }
        guard let bytes = bytes(name) else { return nil }
        do {
            if directory == nil {
                let dir = URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true).appendingPathComponent("exact-generation-\(UUID().uuidString)", isDirectory: true)
                try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
                directory = dir
            }
            let url = directory!.appendingPathComponent(name)
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try bytes.write(to: url, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o400], ofItemAtPath: url.path)
            files[name] = url
            return url
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

    private func embeddedURL(_ name: String) -> URL? {
        guard !name.isEmpty, !name.hasPrefix("/"), !name.contains(":"), !name.contains("\\"),
              name.split(separator: "/", omittingEmptySubsequences: false).allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else { return nil }
        if let override = overrides[name] { return override }
        let base = root.standardizedFileURL.resolvingSymlinksInPath()
        let url = base.appendingPathComponent(name).standardizedFileURL.resolvingSymlinksInPath()
        return url.path.hasPrefix(base.path + "/") ? url : nil
    }
}
