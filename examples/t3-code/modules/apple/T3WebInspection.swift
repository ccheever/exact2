#if os(macOS)
import Foundation
import WebKit

/// Safari's Web Inspector on the clone's own web views (exact2 #101, EXACT2-GAPS X2; Charlie's
/// decision of 2026-10-08). A development build marks every WKWebView this module creates
/// `isInspectable` (the terminal, the rendered-HTML preview and the offscreen Mermaid renderer),
/// so Safari's Develop menu lists them; a release build never does. Exact's own inspector stays
/// deferred (DEFERRED "no devtools UI"), so the reference's View › Toggle Developer Tools item
/// stays absent, a declared difference. Exact's `iframe` web views get the same opt-in from the
/// host itself (main #309, the development web arm); these are the module's own.
///
/// A release build is any of: the clone's packaged build (`Contents/Resources/distribution.json`
/// `{"flavor":"packaged"}`, the marker the embedded server reads, T3LocalPolicy.packaged); a
/// production-trust bake (the bundle receipt's `build.trust`); and a distributed bundle (what
/// `--distribution` assembles and `exact release` signs, or an IPA), whose receipt is the shipped
/// one, its binary reduced to a digest. The last two are where #309 leaves its opt-in out. Every
/// other build is a development build. No environment variable changes the answer.
enum T3WebInspection {
    /// This build's answer, read once from the bundle.
    static let enabled = permits(resources: Bundle.main.resourceURL)

    static func permits(resources: URL?) -> Bool {
        if T3LocalPolicy.packaged(resources: resources) { return false }
        guard let resources, let data = try? Data(contentsOf: resources.appendingPathComponent("receipt.json")),
              let receipt = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let build = receipt["build"] as? [String: Any] else { return true }
        if build["trust"] as? String == "production" { return false }
        if let binary = build["binary"] as? [String: Any], Set(binary.keys) == ["sha256"] { return false }
        return true
    }

    /// Each web view passes here once, before its first load. The stderr line (`t3.inspection: …`,
    /// the agent's `logs` host lines) reads the flag back from the view itself.
    static func mark(_ web: WKWebView, _ kind: String, enabled: Bool = enabled) {
        web.isInspectable = enabled
        FileHandle.standardError.write(Data("t3.inspection: \(kind) web view isInspectable=\(web.isInspectable) (\(enabled ? "development" : "release") build)\n".utf8))
    }
}
#endif
