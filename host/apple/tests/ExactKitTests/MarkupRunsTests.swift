// Markdown source becomes the engine's runs through the archive's seam
// (LLP 1045 D3): what measure expands is what paint expands.
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

final class MarkupRunsTests: XCTestCase {
    override func setUp() {
        super.setUp()
        ExactMarkdown.install() // LLP 1047.001 D4: the capability this tests
    }

    func testNavigationTargetsUseSupportedParsedAbsoluteSchemes() {
        let unsafe = ["javascript:probe", "JaVaScRiPt:probe", "\u{0}\u{1f} javascript:probe",
                      "java\tscript:probe", "java\nscript:probe", "java\rscript:probe",
                      "data:text/html,probe", "vbscript:probe", "file:///tmp/probe", "custom:probe",
                      "blob:https://example.test/id", "about:blank", "ftp://example.test/file",
                      "http://[invalid", "/relative/path", "../sibling", "//example.test/path", "#local"]
        for href in unsafe { XCTAssertNil(MarkupRuns.navigationURL(href), href) }
        for href in ["https://example.test/x", "HTTP://example.test/x", "mailto:test@example.test", "tel:+15551234567"] {
            XCTAssertNotNil(MarkupRuns.navigationURL(href), href)
        }
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        for href in ["javascript:probe", "data:text/plain,probe", "file:///tmp/probe", "custom:probe", "relative", "../sibling", "//example.test/path"] {
            let runs = MarkupRuns.expand("[visible](\(href))", base: base, color: nil)
            XCTAssertEqual(runs.map(\.text).joined(), "visible", href)
            XCTAssertTrue(runs.allSatisfy { $0.href.isEmpty }, href)
            XCTAssertTrue(runs.allSatisfy { !$0.decoration.contains("underline") }, href)
        }
        // An absolute path needs no document base: a location in the app,
        // followed through the navigation root (notes diary, LLP 1045 D4).
        let path = MarkupRuns.expand("[Ideas](/note/3)", base: base, color: nil)
        XCTAssertEqual(path.first { $0.text == "Ideas" }?.href, "/note/3")
        XCTAssertEqual(path.first { $0.text == "Ideas" }?.decoration, "underline")
    }

    func testASourceExpandsIntoStyledRunsAgainstTheNodesFont() {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
        let runs = MarkupRuns.expand("# Title\n\nSome **bold** and `code` and [a link](https://e.dev).", base: base, color: [10, 20, 30, 255])
        XCTAssertEqual(runs.map(\.text).joined(), "Title\n\nSome bold and code and a link.")
        let title = runs[0]
        XCTAssertEqual(title.size, 26); XCTAssertEqual(title.weight, 700)
        XCTAssertEqual(runs.first { $0.text == "bold" }?.weight, 700)
        XCTAssertEqual(runs.first { $0.text == "code" }?.family, MarkupRuns.monospaceFamily)
        let link = runs.first { $0.text == "a link" }!
        XCTAssertEqual(link.href, "https://e.dev"); XCTAssertEqual(link.decoration, "underline")
        XCTAssertEqual(link.color, [10, 20, 30, 255])
        // The gap between blocks is a short line free of the node's line height.
        let gap = runs.first { $0.text == "\n" && $0.size < 16 }!
        XCTAssertNil(gap.lineHeight)
        XCTAssertEqual(MarkupRuns.expand("", base: base, color: nil), [])
    }

    func testMarkersTakeTheInkAtReducedOpacity() {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        let runs = MarkupRuns.expand("- one\n\n> quoted", base: base, color: [0, 0, 0, 255])
        let bullet = runs.first { $0.text.hasPrefix("•") }!
        XCTAssertEqual(bullet.color?[3], 255 * 0.62)
        XCTAssertEqual(runs.first { $0.text == "quoted" }?.color?[3], 255 * 0.62)
        // The item's text is its own run, followed by the block's newline glyph.
        XCTAssertEqual(runs.first { $0.text == "one" }?.color?[3], 255)
        // The item's runs carry its indent, its marker hung before it (LLP 1045 D4).
        XCTAssertTrue(bullet.hang); XCTAssertEqual(bullet.indent, 40)
        XCTAssertEqual(runs.first { $0.text == "one" }?.indent, 40)
        XCTAssertEqual(runs.first { $0.text == "quoted" }?.indent, 0)
    }
}
