// @ref LLP 1104 D5: a single-line field's glyphs stand where CSS puts them.
#if os(macOS)
import AppKit
import CExact
import CoreText
import XCTest
@testable import ExactKit

/// Where a field's glyphs ink, drawn at 2x as the screen shows them,
/// against the same glyphs drawn by Core Text where CSS puts them: from the
/// content box's inline start, on the baseline of the CSS line box centred
/// in the content box (Chrome's inner editor).
final class FieldInkMacTests: XCTestCase {
    static let scale: CGFloat = 2
    static let red: BatchValue = [255, 0, 0, 255]

    /// The box, in points from the top left, of the red ink: a pixel whose
    /// red stands over its green by more than a half, over white or over
    /// nothing, so a bezel's grey and white are not ink.
    static func ink(_ rep: NSBitmapImageRep) -> NSRect? {
        var (minX, minY, maxX, maxY) = (Int.max, Int.max, -1, -1)
        for y in 0..<rep.pixelsHigh {
            for x in 0..<rep.pixelsWide {
                guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB),
                      c.alphaComponent * (c.redComponent - c.greenComponent) > 0.5 else { continue }
                (minX, minY, maxX, maxY) = (min(minX, x), min(minY, y), max(maxX, x), max(maxY, y))
            }
        }
        guard maxX >= 0 else { return nil }
        return NSRect(x: CGFloat(minX) / scale, y: CGFloat(minY) / scale,
                      width: CGFloat(maxX - minX + 1) / scale, height: CGFloat(maxY - minY + 1) / scale)
    }
    static func blank(_ size: NSSize) -> NSBitmapImageRep {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width * scale), pixelsHigh: Int(size.height * scale),
                                   bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                   colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        rep.size = size
        return rep
    }
    /// The view's own drawing, its field editor's too.
    static func drawn(_ view: NSView) -> NSRect? {
        let rep = blank(view.bounds.size)
        view.cacheDisplay(in: view.bounds, to: rep)
        return ink(rep)
    }
    /// `text` in `font` from `x`, its baseline at `baseline` (both from the top left).
    static func css(_ text: String, font: NSFont, size: NSSize, x: CGFloat, baseline: CGFloat) -> NSRect? {
        let rep = blank(size)
        NSGraphicsContext.saveGraphicsState()
        defer { NSGraphicsContext.restoreGraphicsState() }
        let context = NSGraphicsContext(bitmapImageRep: rep)!
        NSGraphicsContext.current = context
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)]))
        context.cgContext.textPosition = CGPoint(x: x, y: size.height - baseline)
        CTLineDraw(line, context.cgContext)
        context.flushGraphics()
        return ink(rep)
    }
    /// The CSS baseline of a line of `font` and `lineHeight` (nil: `normal`) centred in `box`.
    static func baseline(in box: NSRect, font: NSFont, lineHeight: CGFloat?) -> CGFloat {
        let (above, below) = CSSLineBox.extents(font as CTFont, height: lineHeight)
        return box.minY + (box.height - above - below) / 2 + above
    }

    struct Case {
        let name: String
        let size: NSSize
        let style: NodeStyle
        let content: NSRect
        let lineHeight: CGFloat?
        var cell = false
    }
    static func authored(_ rows: NodeStyle) -> NodeStyle {
        rows.merging(["appearance": .string("none"), "font_size": .number(14), "text_color": red, "placeholder_color": red,
                      "outline_style": .string("none")]) { _, new in new }
    }
    /// Fields the author draws, as apps write them: a bordered 28pt field
    /// (padding 9/4 inside a 1pt border), a 26pt picker search with a 26pt
    /// line, a 21pt search with a normal line, a 20pt one with a 20pt line.
    static let authoredCases: [Case] = [
        Case(name: "bordered 28", size: NSSize(width: 240, height: 28),
             style: authored(["border_width": .number(1), "padding_left": .number(9), "padding_right": .number(9), "padding_top": .number(4), "padding_bottom": .number(4)]),
             content: NSRect(x: 10, y: 5, width: 220, height: 18), lineHeight: nil),
        Case(name: "picker 26", size: NSSize(width: 240, height: 26),
             style: authored(["line_height": .string("26px"), "padding_left": .number(20), "padding_right": .number(9)]),
             content: NSRect(x: 20, y: 0, width: 211, height: 26), lineHeight: 26),
        Case(name: "sidebar 21", size: NSSize(width: 200, height: 21),
             style: authored(["font_weight": .number(500)]),
             content: NSRect(x: 0, y: 0, width: 200, height: 21), lineHeight: nil),
        Case(name: "files 20", size: NSSize(width: 200, height: 20),
             style: authored(["line_height": .string("20px")]),
             content: NSRect(x: 0, y: 0, width: 200, height: 20), lineHeight: 20),
    ]
    /// Native fields, their content box inside the chrome the kernel has
    /// from `FieldChromeCache`, as layout adds it: at their least height,
    /// 28pt tall with 8pt of the author's padding, and in a grouped row.
    static func nativeCases() -> [Case] {
        let cache = FieldChromeCache()
        cache.configure(NSAppearance(named: .aqua)!, scale: scale)
        let font = cache.body
        func chrome(_ kind: UInt8) -> ExactFieldChrome {
            cache.measure(FieldChromeCache.Key(kind: kind, family: 0, name: font.fontName, size: font.pointSize, weight: 400,
                                               italic: false, traits: .init(appearance: NSAppearance.Name.aqua.rawValue, scale: scale)), font: font)
        }
        let bezel = chrome(0), row = chrome(4)
        let (top, right, bottom, left) = (CGFloat(bezel.top), CGFloat(bezel.right), CGFloat(bezel.bottom), CGFloat(bezel.left))
        let least = CGFloat(bezel.minimum_height), rowLeast = CGFloat(row.minimum_height)
        let style: NodeStyle = ["text_color": red, "placeholder_color": red, "outline_style": .string("none"), "written": .array([.string("text_color")])]
        return [
            Case(name: "native", size: NSSize(width: 240, height: least), style: style,
                 content: NSRect(x: left, y: top, width: 240 - left - right, height: least - top - bottom), lineHeight: nil),
            Case(name: "native padded", size: NSSize(width: 240, height: 28),
                 style: style.merging(["padding_left": .number(8), "padding_right": .number(8)]) { _, new in new },
                 content: NSRect(x: left + 8, y: top, width: 240 - left - right - 16, height: 28 - top - bottom), lineHeight: nil),
            Case(name: "native in a row", size: NSSize(width: 240, height: rowLeast), style: style,
                 content: NSRect(x: CGFloat(row.left), y: CGFloat(row.top), width: 240 - CGFloat(row.left + row.right),
                                 height: rowLeast - CGFloat(row.top + row.bottom)), lineHeight: nil, cell: true),
        ]
    }

    func node(_ c: Case, session: ExactSession, value: String, placeholder: String) -> NodeView {
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = NSRect(origin: .zero, size: c.size)
        node.applyProps(set: ["value": value, "placeholder": placeholder], clear: [])
        node.applyStyle(c.style)
        node.applyFieldContent(["rect": [c.content.minX, c.content.minY, c.content.width, c.content.height].map(Double.init), "cell": c.cell])
        return node
    }
    /// The node in a window of its own, its field the first responder.
    func focus(_ node: NodeView) throws -> NSWindow {
        let window = NSWindow(contentRect: node.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = node
        XCTAssertTrue(window.makeFirstResponder(node.field))
        let editor = try XCTUnwrap(node.field?.currentEditor() as? NSTextView)
        editor.insertionPointColor = .clear
        editor.selectedRange = NSRange(location: editor.string.utf16.count, length: 0)
        return window
    }
    func assertInk(_ got: NSRect?, _ want: NSRect?, _ what: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let got = try XCTUnwrap(got, "\(what): no ink", file: file, line: line)
        let want = try XCTUnwrap(want, "\(what): no reference ink", file: file, line: line)
        // Half a point: one device pixel at 2x.
        XCTAssertEqual(got.minX, want.minX, accuracy: 0.5, "\(what): inline start \(got) against CSS's \(want)", file: file, line: line)
        XCTAssertEqual(got.minY, want.minY, accuracy: 0.5, "\(what): top \(got) against CSS's \(want)", file: file, line: line)
        XCTAssertEqual(got.maxY, want.maxY, accuracy: 0.5, "\(what): bottom \(got) against CSS's \(want)", file: file, line: line)
    }

    /// A field's value, the same value while it is edited (the field
    /// editor's), its placeholder, and its placeholder while it has the
    /// focus, each where CSS puts the same glyphs; native or the author's.
    func testFieldsInkWhereCSSPutsTheirText() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "field-ink")
        defer { session.destroy() }
        for c in Self.authoredCases + Self.nativeCases() {
            let shown = node(c, session: session, value: "Hgt3", placeholder: "")
            let font = try XCTUnwrap(shown.field?.font)
            let at = Self.baseline(in: c.content, font: font, lineHeight: c.lineHeight)
            let want = Self.css("Hgt3", font: font, size: c.size, x: c.content.minX, baseline: at)
            try assertInk(Self.drawn(shown), want, "\(c.name), value")
            let window = try focus(shown)
            defer { window.close() }
            try assertInk(Self.drawn(shown), want, "\(c.name), edited")
            // The field resized while it is edited keeps its editor on the line.
            shown.frame.size.width += 20
            shown.applyFieldContent(["rect": [c.content.minX, c.content.minY, c.content.width + 20, c.content.height].map(Double.init), "cell": c.cell])
            shown.frame.size.width -= 20
            shown.applyFieldContent(["rect": [c.content.minX, c.content.minY, c.content.width, c.content.height].map(Double.init), "cell": c.cell])
            try assertInk(Self.drawn(shown), want, "\(c.name), edited, laid out again")

            let empty = node(c, session: session, value: "", placeholder: "Hgt3")
            try assertInk(Self.drawn(empty), want, "\(c.name), placeholder")
            let other = try focus(empty)
            defer { other.close() }
            try assertInk(Self.drawn(empty), want, "\(c.name), placeholder, focused")
        }
    }
}
#endif
