import AppKit
import WebKit
import XCTest

// Task app-developer-tools (exact2 #101, EXACT2-GAPS X2): every web view the module creates is
// inspectable from Safari in a development build and never in a release build: the clone's packaged
// build (`distribution.json`, T3LocalPolicy.packaged), a production-trust bake or a distributed bundle
// (its receipt), as main #309 decides for the iframe arm. This binary has neither file beside it, so
// it is a development build, whatever the environment says.
private let ignoreEvent: ExactNativeEventFn = { _, _, _, _, _ in }

final class WebInspectionTests: XCTestCase {
    private func scratch(_ name: String) -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-inspection-\(UUID().uuidString)/\(name)", isDirectory: true)
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    func testThePackagedFlavorIsTheReleaseLine() throws {
        let packaged = scratch("packaged"), development = scratch("development"), other = scratch("other")
        try #"{"flavor":"packaged"}"#.write(to: packaged.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        try #"{"flavor":"development"}"#.write(to: other.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        XCTAssertFalse(T3WebInspection.permits(resources: packaged), "the packaged build is never inspectable")
        XCTAssertTrue(T3WebInspection.permits(resources: development), "no marker: a development build")
        XCTAssertTrue(T3WebInspection.permits(resources: other), "another flavor is not the packaged build")
        XCTAssertTrue(T3WebInspection.permits(resources: nil))
        XCTAssertEqual(T3WebInspection.permits(resources: packaged), !T3LocalPolicy.packaged(resources: packaged), "one line with the embedded server's policy")
    }

    /// The receipt's lines, as main #309 draws them for the iframe arm: production trust and a
    /// distributed bundle (the shipped receipt: its binary is a digest alone) are release builds.
    func testTheReceiptMarksProductionAndDistributedBundles() throws {
        func resources(_ receipt: String) throws -> URL {
            let url = scratch("receipt")
            try receipt.write(to: url.appendingPathComponent("receipt.json"), atomically: true, encoding: .utf8)
            return url
        }
        XCTAssertTrue(T3WebInspection.permits(resources: try resources(#"{"build":{"trust":"development","binary":{"sha256":"ab","metadata":{}}}}"#)), "a development bake")
        XCTAssertFalse(T3WebInspection.permits(resources: try resources(#"{"build":{"trust":"production","binary":{"sha256":"ab","metadata":{}}}}"#)), "production trust")
        XCTAssertFalse(T3WebInspection.permits(resources: try resources(#"{"build":{"trust":"development","binary":{"sha256":"ab"}}}"#)), "the shipped receipt of a distributed bundle")
        XCTAssertTrue(T3WebInspection.permits(resources: try resources("not json")), "an unreadable receipt decides nothing")
        let both = try resources(#"{"build":{"trust":"development","binary":{"sha256":"ab","metadata":{}}}}"#)
        try #"{"flavor":"packaged"}"#.write(to: both.appendingPathComponent("distribution.json"), atomically: true, encoding: .utf8)
        XCTAssertFalse(T3WebInspection.permits(resources: both), "the packaged marker wins over a development receipt")
    }

    func testMarkSetsAndClearsTheFlag() {
        let web = WKWebView(frame: .zero)
        T3WebInspection.mark(web, "test", enabled: true)
        XCTAssertTrue(web.isInspectable)
        T3WebInspection.mark(web, "test", enabled: false)
        XCTAssertFalse(web.isInspectable, "a release build clears it")
    }

    /// The three creation paths: the terminal, the rendered-HTML preview and the Mermaid renderer.
    /// The terminal was inspectable only with EXACT_ASSETS or T3_TERMINAL_INSPECTABLE before; now
    /// the build decides, so a development build launched without either (a lane copy opened with
    /// `open`) is inspectable too.
    func testEveryWebViewTheModuleCreatesFollowsTheBuild() throws {
        unsetenv("EXACT_ASSETS"); unsetenv("T3_TERMINAL_INSPECTABLE")
        XCTAssertTrue(T3WebInspection.enabled, "this test binary carries no distribution.json")
        let terminal = T3TerminalView(props: [:], events: ExactNativeEvents(fn: ignoreEvent, ctx: nil, nonce: 1), agent: true)
        defer { terminal.destroy() }
        XCTAssertTrue(terminal.web.isInspectable, "terminal")
        let media = R6MediaPreview(agent: true), host = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        defer { media.destroy() }
        media.mount(host: host, kind: "html", raw: "http://127.0.0.1:9/api/assets/page?sig=1", name: "page.html")
        let preview = try XCTUnwrap(host.subviews.first as? WKWebView)
        XCTAssertTrue(preview.isInspectable, "html-preview")
        XCTAssertTrue(T3TimelineMermaid.renderer().isInspectable, "mermaid")
    }
}
