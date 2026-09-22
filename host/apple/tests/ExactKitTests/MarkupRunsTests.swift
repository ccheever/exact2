// Markdown source becomes the engine's runs through the archive's seam
// (LLP 1045 D3): what measure expands is what paint expands.
import XCTest
@testable import ExactKit

final class MarkupRunsTests: XCTestCase {
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
        XCTAssertEqual(runs.first { $0.text == " one" }?.color?[3], 255)
    }
}
