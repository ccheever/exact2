// CSS Text Decoration 3 §2.1: a box's lines are drawn through its inline
// descendants' text, in the box's colour, and a descendant cannot remove
// them. Where boxes draw the same line, Chrome shows the nearest box's.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class TextDecorationMacTests: XCTestCase {
    func testABoxDrawsItsLinesThroughNestedRunsInItsOwnColour() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-decoration-mac")
        defer { session.destroy() }
        let p = session.presenter
        let red = [255.0, 0, 0, 255], green = [0.0, 255, 0, 255], blue = [0.0, 0, 255, 255]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text",
             "style": ["font_size": 16.0, "text_color": red, "text_decoration_line": "line-through"]],
            ["op": "paragraph", "id": 1, "runs": [
                ["id": 2, "parent": 1, "paint": true, "props": ["text": "plain "], "style": ["text_color": red]],
                ["id": 3, "parent": 1, "paint": false, "props": [:], "style": ["text_color": green, "text_decoration_line": "underline line-through"]],
                ["id": 4, "parent": 3, "paint": true, "props": ["text": "under"], "style": ["text_color": blue]],
                ["id": 5, "parent": 1, "paint": true, "props": ["text": " own"], "style": ["text_color": blue, "text_decoration_line": "underline line-through"]],
                ["id": 6, "parent": 1, "paint": true, "props": ["text": " link", "href": "https://example.com"], "style": ["text_color": blue]],
            ]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        let spec = node.paragraphSpec()
        XCTAssertEqual(spec.runs.map(\.text), ["plain ", "under", " own", " link"])
        // The paragraph's line-through crosses every run in its red, but for
        // the container's child, where the nearer container's green shows.
        XCTAssertEqual(spec.runs.map(\.strikeColor), [red, green, red, red])
        XCTAssertEqual(spec.runs.map(\.underlineColor), [nil, green, nil, nil])
        // Propagated, not inherited: the runs' own lines stay their own, so
        // a link keeps AppKit's underline.
        XCTAssertEqual(spec.runs.map(\.decoration), ["", "", "underline line-through", ""])
        XCTAssertTrue(spec.runs[3].underlined)

        let text = try XCTUnwrap(node.text)
        let attributed = text.attributed(spec)
        let at = { (i: Int) in attributed.attributes(at: i, effectiveRange: nil) }
        let color = { (value: Any?) in (value as? NSColor)?.usingColorSpace(.sRGB).map { [$0.redComponent, $0.greenComponent, $0.blueComponent] } }
        let under = 6, own = 11
        XCTAssertEqual(at(0)[.strikethroughStyle] as? Int, NSUnderlineStyle.single.rawValue)
        XCTAssertEqual(color(at(0)[.strikethroughColor]), [1, 0, 0])
        XCTAssertNil(at(0)[.underlineStyle])
        XCTAssertEqual(at(under)[.underlineStyle] as? Int, NSUnderlineStyle.single.rawValue)
        XCTAssertEqual(color(at(under)[.underlineColor]), [0, 1, 0])
        XCTAssertEqual(color(at(under)[.strikethroughColor]), [0, 1, 0])
        // Its own lines take its own colour: no colour of a box's.
        XCTAssertEqual(at(own)[.underlineStyle] as? Int, NSUnderlineStyle.single.rawValue)
        XCTAssertEqual(at(own)[.strikethroughStyle] as? Int, NSUnderlineStyle.single.rawValue)
        XCTAssertNil(at(own)[.underlineColor])
        XCTAssertNil(at(own)[.strikethroughColor])
    }

    /// A block's lines reach the text of its in-flow descendants, in the
    /// block's colour, and none of a button's or a box out of flow's; a
    /// block that stops drawing them takes them away (Chrome's tree).
    func testABlocksLinesReachItsDescendantsParagraphs() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-decoration-block-mac")
        defer { session.destroy() }
        let p = session.presenter
        let green = [0.0, 255, 0, 255]
        let text = { (id: Int, words: String, style: [String: Any]) -> [String: Any] in
            ["op": "create", "id": id, "kind": "text", "props": ["text": words], "style": style.merging(["font_size": 16.0]) { a, _ in a }]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["text_color": green, "text_decoration_line": "underline"]],
            ["op": "create", "id": 2, "kind": "view", "style": [:]],
            text(3, "two boxes down", [:]),
            ["op": "create", "id": 4, "kind": "button", "style": [:]],
            text(5, "a button's", [:]),
            text(6, "out of flow", ["position_type": "absolute"]),
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
        ]))
        let underline = { (id: UInt32) in try XCTUnwrap(p.views[id]).paragraphSpec().runs.map(\.underlineColor) }
        XCTAssertEqual(try underline(3), [green], "two boxes down, in the box's colour")
        XCTAssertEqual(try underline(5), [nil], "a button is atomic")
        XCTAssertEqual(try underline(6), [nil], "a box out of flow takes none")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": green]]]))
        XCTAssertEqual(try underline(3), [nil], "the block's line taken away")
    }

    /// A line holding hidden text keeps the lines of the text that shows,
    /// and the hidden text has neither glyphs nor lines, as in Chrome.
    func testHiddenTextDrawsNoLineAndItsNeighboursKeepTheirs() throws {
        let font = CTFontCreateWithName("Helvetica" as CFString, 40, nil)
        let red = NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)
        func part(_ text: String, ink: NSColor, hidden: Bool) -> NSAttributedString {
            var a: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: ink,
                                                    .strikethroughStyle: NSUnderlineStyle.single.rawValue, .strikethroughColor: red]
            if hidden { a[.exactHidden] = true }
            return NSAttributedString(string: text, attributes: a)
        }
        for shadowed in [false, true] {
            let string = NSMutableAttributedString(attributedString: part("MMMM", ink: .blue, hidden: true))
            string.append(part("WWWW", ink: .black, hidden: false))
            if shadowed { string.addAttribute(.exactShadow, value: TextRunShadow([0, 2, 1, 0, 0, 0, 128]), range: NSRange(location: 4, length: 4)) }
            let line = CTLineCreateWithAttributedString(string)
            let hiddenWidth = CTLineGetOffsetForStringIndex(line, 4, nil)
            let (w, h) = (400, 60)
            let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            ctx.translateBy(x: 0, y: 15)
            XCTAssertTrue(TextLinePaint.drawShadowed(line, in: ctx, scale: 1))
            let px = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
            var struck = 0, blue = 0, struckLeft = 0
            for y in 0..<h { for x in 0..<w {
                let i = (y * w + x) * 4
                guard px[i + 3] > 200 else { continue }
                if px[i] > 200, px[i + 1] < 80, px[i + 2] < 80 { struck += 1; if CGFloat(x) < hiddenWidth - 2 { struckLeft += 1 } }
                if px[i + 2] > 200, px[i] < 80 { blue += 1 }
            } }
            XCTAssertGreaterThan(struck, 40, "the shown text keeps its line-through")
            XCTAssertEqual(struckLeft, 0, "the hidden text has no line")
            XCTAssertEqual(blue, 0, "the hidden text has no glyphs")
        }
    }
}
#endif
