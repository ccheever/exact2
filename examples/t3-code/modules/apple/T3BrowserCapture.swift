#if os(macOS)
import AppKit
import WebKit

/// A Browser tab's artifacts (T3BrowserPage.swift). MIT reference, see LICENSE-T3, T3 Code 2359643cff:
/// apps/desktop/src/preview/Manager.ts (`artifactSiteSlug`, `resolveArtifactPath`) and
/// apps/desktop/src/app/DesktopEnvironment.ts (`browserArtifactsDir`).
///
/// - Artifacts live where the reference keeps them, `<T3 home>/userdata/browser-artifacts`, the embedded server's own
///   state directory, so its Storage cleanup ("Browser artifacts", `browserArtifactsAfterDays`) prunes them. A build that
///   runs no local server keeps them in its data root.
enum T3BrowserArtifacts {
    static let maxSlugLength = 80

    /// DesktopEnvironment `browserArtifactsDir`: the local server's `<T3 home>/userdata/browser-artifacts`, else the data root's.
    static func directory(dataRoot: URL?, environment: [String: String] = ProcessInfo.processInfo.environment, bundleInfo: [String: Any] = Bundle.main.infoDictionary ?? [:]) -> URL {
        let policy = T3LocalPolicy.resolve(env: environment, packaged: T3LocalPolicy.packaged(info: bundleInfo), home: NSHomeDirectory(),
                                           accountHome: String(cString: getpwuid(getuid()).pointee.pw_dir))
        if case let .allowed(home, _, _, _) = policy { return home.appendingPathComponent("userdata", isDirectory: true).appendingPathComponent("browser-artifacts", isDirectory: true) }
        return (dataRoot ?? FileManager.default.temporaryDirectory).appendingPathComponent("browser-artifacts", isDirectory: true)
    }

    /// artifactSiteSlug: the page's host, lowercased, runs of other characters as one dash, at most 80 characters.
    static func slug(_ raw: String?) -> String {
        guard let raw, let host = URL(string: raw)?.host?.lowercased(), !host.isEmpty else { return "site" }
        var slug = host.replacingOccurrences(of: "[^a-z0-9]+", with: "-", options: .regularExpression).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        slug = String(slug.prefix(maxSlugLength))
        while slug.hasSuffix("-") { slug.removeLast() }
        return slug.isEmpty ? "site" : slug
    }
    static func millis36(_ date: Date) -> String { String(Int64(date.timeIntervalSince1970 * 1000), radix: 36) }
    static func iso(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
    }
    /// recordingFileExtension: the subtype without `x-` and punctuation, else "video".
    static func fileExtension(_ mimeType: String) -> String {
        let subtype = mimeType.split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces).lowercased() } ?? ""
        let name = subtype.split(separator: "/").dropFirst().first.map(String.init) ?? ""
        let cleaned = (name.hasPrefix("x-") ? String(name.dropFirst(2)) : name).replacingOccurrences(of: "[^a-z0-9]", with: "", options: .regularExpression)
        return cleaned.isEmpty ? "video" : cleaned
    }
    /// resolveArtifactPath: a path inside the artifact directory (symbolic links resolved), else nil.
    static func resolve(_ path: String, in directory: URL) -> URL? {
        guard path.hasPrefix("/") else { return nil }
        let target = URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
        let root = directory.standardizedFileURL.resolvingSymlinksInPath().path
        return target.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") ? target : nil
    }

    /// The pasteboard the artifact actions write: an agent run never touches the person's clipboard.
    static func pasteboard(agent: Bool) -> NSPasteboard { agent ? NSPasteboard(name: NSPasteboard.Name("com.exact.t3code.browser-agent")) : .general }
}
#endif
