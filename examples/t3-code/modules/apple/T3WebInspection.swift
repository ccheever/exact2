#if os(macOS)
import Foundation
import WebKit

/// Safari's Web Inspector on the clone's own web views (exact2 #101, EXACT2-GAPS X2; Charlie's
/// decision of 2026-10-08). A development build marks every WKWebView this module creates
/// `isInspectable` (the terminal, the rendered-HTML preview and the offscreen Mermaid renderer),
/// so Safari's Develop menu lists them; a release build never does. Exact's own inspector stays
/// deferred (DEFERRED "no devtools UI"), so the reference's View › Toggle Developer Tools item
/// stays absent, a declared difference.
///
/// Development or release is the clone's one flavor marker, the one the embedded server already
/// reads (T3LocalPolicy.packaged): the packaged build carries `Contents/Resources/distribution.json`
/// `{"flavor":"packaged"}` and is the release; every other build is a development build. No
/// environment variable changes the answer in either.
enum T3WebInspection {
    /// This build's answer, read once from the bundle.
    static let enabled = permits(resources: Bundle.main.resourceURL)

    static func permits(resources: URL?) -> Bool { !T3LocalPolicy.packaged(resources: resources) }

    /// Each web view passes here once, before its first load. The stderr line (`t3.inspection: …`,
    /// the agent's `logs` host lines) reads the flag back from the view itself.
    static func mark(_ web: WKWebView, _ kind: String, enabled: Bool = enabled) {
        web.isInspectable = enabled
        FileHandle.standardError.write(Data("t3.inspection: \(kind) web view isInspectable=\(web.isInspectable) (\(enabled ? "development" : "packaged") build)\n".utf8))
    }
}
#endif
