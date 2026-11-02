#if os(macOS)
import Foundation

/// The app's bundled scripts (the Browser's recording, annotation and Playwright scripts): `EXACT_ASSETS`
/// in a development run, else the bundle's `assets/`.
enum T3Assets {
    static func fileURL(_ file: String, assets: String? = ProcessInfo.processInfo.environment["EXACT_ASSETS"], resources: URL? = Bundle.main.resourceURL) -> URL? {
        var candidates: [URL] = []
        if let root = assets, root.hasPrefix("/") { candidates.append(URL(fileURLWithPath: root).appendingPathComponent("assets/\(file)")) }
        if let resources { candidates.append(resources.appendingPathComponent("assets/\(file)")) }
        return candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
    }
}
#endif
