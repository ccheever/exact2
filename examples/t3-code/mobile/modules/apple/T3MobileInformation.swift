#if os(iOS)
// @ref llp/1107.009-mobile-settings.decision.md#information-sources
import Foundation

/// Bundle metadata and app-owned notices only. Never substitutes preferences for caches.
final class T3MobileInformation {
    private let bundle: Bundle
    init(bundle: Bundle = .main) { self.bundle = bundle }
    func read() -> [String: Any] {
        var result: [String: Any] = ["version": bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
                                     "build": bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""]
        if let root = bundle.resourceURL,
           let data = try? Data(contentsOf: root.appendingPathComponent("assets/mobile-third-party-licenses.json")),
           let manifest = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            result["notices"] = manifest
            result["noticeCoverage"] = manifest["coverage"] as? String ?? ""
        }
        return result
    }
}
#endif
