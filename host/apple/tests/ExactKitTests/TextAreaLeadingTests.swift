import XCTest
import ExactCore
@testable import ExactKit
#if os(macOS)
import AppKit
@testable import ExactMarkdown

/// CSS 2 §10.8.1: a textarea's line box taller than its text has half its
/// leading above the glyphs and half below, as a text node's and an
/// input's line (`CSSLineBox.extents`, rounded to the window's device
/// pixels); its placeholder and an input's text share the line. TextKit by
/// itself puts all of a forced line's leading above the glyphs.
final class TextAreaLeadingMacTests: XCTestCase {
    private let lineHeight: CGFloat = 22.75
    private var window: NSWindow!
    private var session: ExactSession!
    override func setUp() {
        _ = NSApplication.shared
        ExactMarkdown.install()
        session = ExactApp.shared.makeSession(label: "textarea-leading")
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
    }
    override func tearDown() { window?.close(); session?.destroy(); window = nil; session = nil }

    /// A node of `kind` in the window, its text 14 px black on a 22.75 px line unless said.
    private func node(_ kind: String, _ props: [String: String], height: CGFloat = 200, line: String = "22.75px") -> NodeView {
        let node = NodeView(id: 1, kind: kind, presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 300, height: height)
        let host = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        host.addSubview(node)
        window.contentView = host
        node.applyProps(set: props, clear: [])
        node.applyStyle(["appearance": .string("none"), "font_size": .number(14), "line_height": .string(line),
                         "text_color": .array([.number(0), .number(0), .number(0), .number(255)])])
        node.layoutSubtreeIfNeeded()
        return node
    }
    /// Where CSS puts the baseline of each line in a field.
    private func css(_ f: NSTextView, lines: Int) throws -> [CGFloat] {
        let above = CSSLineBox.extents(try XCTUnwrap(f.font) as CTFont, height: lineHeight, scale: window.backingScaleFactor).0
        return (0..<lines).map { CGFloat($0) * lineHeight + above + f.textContainerOrigin.y }
    }
    /// Each line's baseline as TextKit lays it out, in the text view (y down).
    private func baselines(_ f: NSTextView) throws -> [CGFloat] {
        let manager = try XCTUnwrap(f.textLayoutManager)
        manager.ensureLayout(for: manager.documentRange)
        var out: [CGFloat] = []
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
            for line in fragment.textLineFragments where line.characterRange.length > 0 {
                out.append(fragment.layoutFragmentFrame.minY + line.typographicBounds.minY + line.glyphOrigin.y + f.textContainerOrigin.y)
            }
            return true
        }
        return out
    }
    /// The top of the view's ink, in points.
    private func inkTop(_ view: NSView) throws -> CGFloat {
        // Capture whole points from the view's top (both views are
        // flipped): for a height that is not whole pixels, AppKit rounds the
        // bitmap up and draws the view against the bitmap's bottom, which can
        // put the glyphs a row low
        let rect = view.bounds.integral
        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: rect))
        view.cacheDisplay(in: rect, to: rep)
        let scale = CGFloat(rep.pixelsHigh) / rect.height
        for y in 0..<rep.pixelsHigh {
            for x in 0..<rep.pixelsWide {
                guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB), (1 - c.brightnessComponent) * c.alphaComponent > 0.5 else { continue }
                return CGFloat(y) / scale
            }
        }
        XCTFail("the text draws")
        return .infinity
    }

    /// Every line, hard or wrapped, has its baseline half the leading below
    /// its line box's top, the lines 22.75 apart; marked text keeps it, and
    /// the last line's text ends inside the box the kernel measures.
    func testEachLineHasItsBaselineWhereCSSPutsIt() throws {
        let f = try XCTUnwrap(node("textarea", ["value": "Hx\nHx and a line of text long enough to wrap once in this field"]).textArea)
        let laid = try baselines(f)
        XCTAssertGreaterThanOrEqual(laid.count, 3, "a hard line and a wrapped one")
        let expected = try css(f, lines: laid.count)
        for (got, want) in zip(laid, expected) { XCTAssertEqual(got, want, accuracy: 0.01, "\(laid) against CSS's \(expected)") }
        // The caret spans the glyphs on its line.
        let face = try XCTUnwrap(f.font) as CTFont, (ascent, descent) = CSSLineBox.content(face, scale: window.backingScaleFactor)
        let manager = try XCTUnwrap(f.textLayoutManager), content = try XCTUnwrap(manager.textContentManager)
        for (line, offset) in [(0, 0), (1, 3)] {
            let at = try XCTUnwrap(content.location(content.documentRange.location, offsetBy: offset))
            var caret = CGRect.null
            manager.enumerateTextSegments(in: NSTextRange(location: at), type: .selection, options: []) { _, rect, _, _ in caret = rect; return false }
            XCTAssertLessThanOrEqual(caret.minY + f.textContainerOrigin.y, expected[line] - ascent + 0.01, "line \(line)'s caret: \(caret)")
            XCTAssertGreaterThanOrEqual(caret.maxY + f.textContainerOrigin.y, expected[line] + descent - 0.01, "line \(line)'s caret: \(caret)")
        }
        let height = manager.usageBoundsForTextContainer.height
        XCTAssertLessThanOrEqual(height, CGFloat(laid.count) * lineHeight + 0.01, "no taller than the kernel's lines")
        XCTAssertGreaterThanOrEqual(height, expected[laid.count - 1] + descent - 0.01)
        window.makeFirstResponder(f)
        f.setSelectedRange(NSRange(location: 3, length: 0))
        f.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        let marked = try baselines(f)
        for (got, want) in zip(marked, expected) { XCTAssertEqual(got, want, accuracy: 0.01, "marked text: \(marked)") }
        f.unmarkText()
    }

    /// The placeholder, typed text and an input's text of the same font and
    /// line height draw their glyphs on one line.
    func testThePlaceholderTypedTextAndAnInputShareTheLine() throws {
        let area = node("textarea", ["value": "", "placeholder": "Hx"])
        let f = try XCTUnwrap(area.textArea as? TextArea)
        f.placeholderColor = .black
        let placeholder = try inkTop(f)
        area.applyProps(set: ["value": "Hx"], clear: [])
        let typed = try inkTop(f)
        let font = try XCTUnwrap(f.font)
        let expected = try css(f, lines: 1)[0] - font.capHeight
        XCTAssertEqual(typed, expected, accuracy: 1, "typed text's cap top on CSS's baseline")
        XCTAssertEqual(placeholder, typed, accuracy: 0.5, "the placeholder on typed text's line")
        let input = try XCTUnwrap(node("input", ["value": "Hx"], height: lineHeight).field)
        XCTAssertEqual(try inkTop(input), typed, accuracy: 0.5, "an input's text on the same line")
    }

    /// An input's text rounds its line box in the window's device pixels,
    /// as a textarea's does: on a 21 px line, 14 px text has its baseline on
    /// a half point at 2x (2 of half-leading over a 13.5 ascent), where the
    /// line box rounded in points put it half a point lower.
    func testAnInputsTextIsOnATextareasDevicePixelRow() throws {
        XCTAssertEqual(window.backingScaleFactor, 2, "a 2x display's half point")
        let area = try XCTUnwrap(node("textarea", ["value": "Hx"], height: 21, line: "21px").textArea)
        let typed = try inkTop(area)
        let input = try XCTUnwrap(node("input", ["value": "Hx"], height: 21, line: "21px").field)
        XCTAssertEqual(try inkTop(input), typed, accuracy: 0.01, "the input's glyphs on the textarea's device pixel row")
    }

    /// A line height below the font's keeps its pitch: the lines are
    /// forced to it, as TextKit 2 sets no negative spacing between some
    /// paragraphs.
    func testALineShorterThanItsFontKeepsItsPitch() throws {
        let f = try XCTUnwrap(node("textarea", ["value": "Hx\nHx\nHx\nHx"], line: "14px").textArea)
        let laid = try baselines(f)
        XCTAssertEqual(laid.count, 4)
        for (index, pair) in zip(laid, laid.dropFirst()).enumerated() { XCTAssertEqual(pair.1 - pair.0, 14, accuracy: 0.01, "line \(index + 1): \(laid)") }
        XCTAssertLessThanOrEqual(try XCTUnwrap(f.textLayoutManager).usageBoundsForTextContainer.height, 4 * 14 + 0.01)
    }

    /// A Markdown editor's lines too, beside inline code and an atom's pill.
    func testAMarkdownEditorsLinesKeepTheirHalfLeading() throws {
        let area = node("textarea", ["value": "Hx `code` @alice\n`code`\nHx", "markup": "markdown", "atoms": #"[{"name":"mention","prefix":"@"}]"#])
        let f = try XCTUnwrap(area.textArea as? TextArea)
        XCTAssertNotNil(f.markup)
        let expected = try css(f, lines: 3), laid = try baselines(f)
        XCTAssertEqual(laid.count, 3)
        for (got, want) in zip(laid, expected) { XCTAssertEqual(got, want, accuracy: 0.01, "\(laid) against CSS's \(expected)") }
    }
}
#endif

#if os(iOS)
import UIKit

/// `TextAreaLeadingMacTests` on UIKit: a textarea's lines have their
/// baselines where CSS puts them, its placeholder on its first.
final class TextAreaLeadingIOSTests: XCTestCase {
    private let lineHeight: CGFloat = 22.75

    func testEachLineAndThePlaceholderSitWhereCSSPutsThem() throws {
        let session = ExactApp.shared.makeSession(label: "textarea-leading-ios")
        defer { session.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        window.overrideUserInterfaceStyle = .light
        window.backgroundColor = .white
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        let node = NodeView(id: 1, kind: "textarea", presenter: session.presenter)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 200)
        window.addSubview(node)
        node.applyProps(set: ["value": "", "placeholder": "Hx"], clear: [])
        node.applyStyle(["appearance": .string("none"), "font_size": .number(14), "line_height": .string("22.75px"),
                         "text_color": .array([.number(0), .number(0), .number(0), .number(255)]), "placeholder_color": .array([.number(0), .number(0), .number(0), .number(255)])])
        let f = try XCTUnwrap(node.textArea)
        func inkTop() throws -> CGFloat {
            window.layoutIfNeeded()
            CATransaction.flush()
            let format = UIGraphicsImageRendererFormat()
            format.scale = 2
            let image = UIGraphicsImageRenderer(bounds: f.bounds, format: format).image { ctx in
                UIColor.white.setFill(); ctx.fill(f.bounds)
                f.layer.render(in: ctx.cgContext)
            }
            let cg = try XCTUnwrap(image.cgImage), bytes = try XCTUnwrap(cg.dataProvider?.data) as Data
            for y in 0..<cg.height { for x in 0..<cg.width where CapturePixels.painted(bytes, at: y * cg.bytesPerRow + x * 4) && bytes[y * cg.bytesPerRow + x * 4] < 128 { return CGFloat(y) / 2 } }
            XCTFail("the text draws")
            return .infinity
        }
        let font = try XCTUnwrap(f.font)
        let above = CSSLineBox.extents(font as CTFont, height: lineHeight).0 + f.textContainerInset.top
        let placeholder = try inkTop()
        node.applyProps(set: ["value": "Hx\nHx"], clear: [])
        let typed = try inkTop()
        XCTAssertEqual(placeholder, typed, accuracy: 0.5, "the placeholder on typed text's line")
        let manager = try XCTUnwrap(f.textLayoutManager)
        manager.ensureLayout(for: manager.documentRange)
        var laid: [CGFloat] = []
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
            for line in fragment.textLineFragments where line.characterRange.length > 0 {
                laid.append(fragment.layoutFragmentFrame.minY + line.typographicBounds.minY + line.glyphOrigin.y + f.textContainerInset.top)
            }
            return true
        }
        XCTAssertEqual(laid.count, 2)
        for (index, got) in laid.enumerated() {
            XCTAssertEqual(got, CGFloat(index) * lineHeight + above, accuracy: 0.01, "line \(index): \(laid)")
        }
    }
}
#endif
