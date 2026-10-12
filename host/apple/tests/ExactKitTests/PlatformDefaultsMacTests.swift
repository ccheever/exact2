// @ref LLP 1115.000 D14.3 — each host tests its own defaults. Pinning the
// root size and the colours makes a cross-host comparison a comparison of
// layout, and hides a presentation regression; so, as
// `PlatformDefaultsIOSTests` does for UIKit, this holds what the Mac shows
// with nothing pinned and nothing written.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PlatformDefaultsMacTests: XCTestCase {
    /// A page that says nothing about its look: a text, a paragraph with a
    /// link run in it, a symbol, and a box one `rem` square (the root font
    /// size as the kernel was told it, read back as a length). One more
    /// symbol names the accent, the one thing here its author wrote.
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          view
            main width="100%" height="100%"
              text "Body" testId="body"
              text testId="paragraph"
                text "Read "
                text "the guide" href="https://example.com/"
              image "symbol:star-fill" testId="symbol"
              image "symbol:star-fill" -exact-tint-color="AccentColor" testId="accent"
              column width="1rem" height="1rem" testId="rem"

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    /// AppKit's own answer for a colour under an appearance, in sRGB,
    /// 0 to 255: asked of AppKit here, not of `SystemColor`.
    private func channels(_ color: NSColor, dark: Bool) throws -> [Double] {
        let contrast = SystemColor.highContrast
        let name: NSAppearance.Name = dark ? (contrast ? .accessibilityHighContrastDarkAqua : .darkAqua) : (contrast ? .accessibilityHighContrastAqua : .aqua)
        var out: [Double]?
        try XCTUnwrap(NSAppearance(named: name)).performAsCurrentDrawingAppearance {
            guard let c = color.usingColorSpace(.sRGB) else { return }
            out = [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map { Double(min(max($0, 0), 1)) * 255 }
        }
        return try XCTUnwrap(out)
    }

    private func assertChannels(_ got: [Double]?, _ want: [Double], _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        guard let got, got.count == 4 else { return XCTFail("\(message): no colour", file: file, line: line) }
        for i in 0..<4 { XCTAssertEqual(got[i], want[i], accuracy: 0.51, "\(message): \(got) is not \(want)", file: file, line: line) }
    }

    /// With nothing written, in a light window and in a dark one: body text
    /// is the system font size, 13, in AppKit's text colour (`textColor`,
    /// what `CanvasText` names on the Mac); a link run is `linkColor`; a
    /// symbol is `secondaryLabelColor`, as AppKit draws a template image
    /// nobody tinted, and not the accent (LLP 1115.000 D3.9; it was
    /// `controlAccentColor`), which a symbol that names it still is; the
    /// page's ground is the window background.
    func testAPageThatSaysNothingShowsTheMacsOwn() throws {
        _ = NSApplication.shared
        XCTAssertFalse(PageFacts.substituted, "the machine's own readings: no agent's stand in")
        XCTAssertEqual(NSFont.systemFontSize, 13, "the Mac's body size")
        XCTAssertEqual(PageFacts.rootFontSize, Double(NSFont.systemFontSize), "the root font size is the system font size")
        // The host's own default for a symbol is the role table's: its
        // `symbol` row, the tint on iOS and this colour on the Mac.
        let schema = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../kernel/tables/schema.json").standardized
        let table = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(contentsOf: schema)) as? [String: Any])
        let symbolRole = try XCTUnwrap((table["colors"] as? [[String]])?.first { $0[0] == "symbol" })
        XCTAssertEqual(Array(symbolRole[2...3]), ["@tint", "secondaryLabelColor"])
        for dark in [false, true] {
            let name = dark ? "dark" : "light"
            let session = ExactApp.shared.makeSession(label: "platform-defaults-\(name)")
            defer { session.destroy() }
            XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 400, height: 300)).error)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
            defer { window.close() }
            window.contentView = ExactView(session: session)
            let p = session.presenter
            func node(_ testId: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == testId }, "\(name): \(testId)") }

            let body = try node("body")
            XCTAssertEqual(body.drawsDark, dark, "\(name): the window's appearance reaches the page")
            let spec = body.paragraphSpec()
            XCTAssertEqual(spec.runs.first?.size, 13, "\(name): body text is the system's 13")
            XCTAssertEqual(spec.runs.first?.weight, 400, "\(name): at the regular weight")
            XCTAssertEqual(try node("rem").frame.size, CGSize(width: 13, height: 13), "\(name): the kernel lays `rem` out against the Mac's 13")
            assertChannels(spec.runs.first?.color ?? spec.color, try channels(.textColor, dark: dark), "\(name): body text is AppKit's text colour")

            let link = try XCTUnwrap(try node("paragraph").inlineText.first(where: \.isLink), "\(name): the run with an href")
            assertChannels(link.run(dark: dark).color, try channels(.linkColor, dark: dark), "\(name): an unsaid link is linkColor")

            let symbol = try XCTUnwrap(try node("symbol").symbolView, "\(name): the symbol's image view")
            assertChannels(try channels(try XCTUnwrap(symbol.contentTintColor), dark: dark), try channels(.secondaryLabelColor, dark: dark), "\(name): an unsaid symbol is the secondary label colour")
            let accent = try XCTUnwrap(try node("accent").symbolView, "\(name): the symbol that names the accent")
            assertChannels(try channels(try XCTUnwrap(accent.contentTintColor), dark: dark), try channels(.controlAccentColor, dark: dark), "\(name): a symbol that names the accent is it")

            assertChannels(try channels(p.pageBackground, dark: dark), try channels(.windowBackgroundColor, dark: dark), "\(name): an unset page background is the window's")
        }
    }
}
#endif
