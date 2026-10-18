// @ref LLP 1121 stage 9 B (D10) — on the Mac a page symbol nobody tinted
// whose `color` its author wrote, on it or on an ancestor (the kernel's
// `symbolColorWritten`), draws in that colour: Shelf's star under
// `color="AccentColor"` is the accent, not AppKit's `secondaryLabelColor`.
// A symbol under no written colour keeps that (`PlatformDefaultsMacTests`),
// and a written tint still wins.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SymbolColorMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
    private let teal: [Any] = [[48, 176, 199, 255], [64, 200, 224, 255]]
    private let pink: [Any] = [[173, 20, 87, 255], [244, 143, 177, 255]]

    private func presenter(_ images: [(UInt32, Bool, [String: Any])]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: .aqua)
        window.contentView = p.viewport
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        for (id, written, style) in images {
            var props: [String: Any] = ["imageSource": "symbol:star", "symbolName": "star"]
            if written { props["symbolColorWritten"] = "1" }
            ops.append(["op": "create", "id": id, "kind": "image", "props": props, "style": ["font_size": 17].merging(style) { $1 }])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0])
        }
        ops += [["op": "children", "id": 1, "ids": images.map { $0.0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0]]
        p.apply(wireBatch(ops))
        return p
    }
    private func channels(_ color: NSColor?) throws -> [Double] {
        let color = try XCTUnwrap(color)
        var out: [Double]?
        try XCTUnwrap(NSAppearance(named: .aqua)).performAsCurrentDrawingAppearance {
            guard let c = color.usingColorSpace(.sRGB) else { return }
            out = [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map { Double(min(max($0, 0), 1)) * 255 }
        }
        return try XCTUnwrap(out)
    }
    private func ink(_ p: Presenter, _ id: UInt32) throws -> [Double] {
        try channels(try XCTUnwrap(p.views[id]).symbolView?.contentTintColor)
    }
    private func assertChannels(_ got: [Double], _ want: [Double], _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        for i in 0..<4 { XCTAssertEqual(got[i], want[i], accuracy: 0.51, "\(message): \(got) is not \(want)", file: file, line: line) }
    }

    func testAWrittenColourIsTheSymbolsInk() throws {
        let p = presenter([(2, true, ["text_color": tint]), (3, true, ["text_color": pink]), (4, false, ["text_color": pink]),
                           (5, true, ["text_color": pink, "tint_color": teal])])
        assertChannels(try ink(p, 2), try channels(.controlAccentColor), "Shelf's star: `color=\"AccentColor\"` is the accent")
        assertChannels(try ink(p, 3), [173, 20, 87, 255], "a token's colour")
        assertChannels(try ink(p, 4), try channels(.secondaryLabelColor), "a colour nobody wrote keeps AppKit's")
        assertChannels(try ink(p, 5), [48, 176, 199, 255], "a written tint wins")
    }

    /// The colour follows a restyle and the kernel's word as each changes;
    /// a hierarchical glyph, which bakes it into its image, is made again.
    func testTheInkFollowsItsColourAndItsWord() throws {
        let p = presenter([(2, false, ["text_color": pink]), (3, true, ["text_color": pink, "symbol_rendering": "hierarchical"])])
        assertChannels(try ink(p, 2), try channels(.secondaryLabelColor), "unwritten")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["symbolColorWritten": "1"], "clear": []]]))
        assertChannels(try ink(p, 2), [173, 20, 87, 255], "written")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 17, "text_color": teal]]]))
        assertChannels(try ink(p, 2), [48, 176, 199, 255], "a new colour")
        p.apply(wireBatch([["op": "props", "id": 2, "set": [:], "clear": ["symbolColorWritten"]]]))
        assertChannels(try ink(p, 2), try channels(.secondaryLabelColor), "unwritten again")
        let hierarchical = try XCTUnwrap(p.views[3])
        let key = hierarchical.symbolKey
        assertChannels(try ink(p, 3), [173, 20, 87, 255], "hierarchical")
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["font_size": 17, "text_color": teal, "symbol_rendering": "hierarchical"]]]))
        XCTAssertNotEqual(hierarchical.symbolKey, key, "the baked colour remakes the image")
        assertChannels(try ink(p, 3), [48, 176, 199, 255], "hierarchical, a new colour")
    }

    /// In a selected table row drawn emphasized, a fixed written colour keeps
    /// its ink, as a fixed tint does; a role's colour, and a symbol nobody
    /// coloured, take the selected ink.
    func testAnEmphasizedRowKeepsAFixedWrittenColour() throws {
        let role: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let p = presenter([(2, true, ["text_color": pink]), (3, true, ["text_color": role]), (4, false, ["text_color": pink])])
        try XCTUnwrap(p.views[1]).rowEmphasisChanged(true)
        func style(_ id: UInt32) -> NSView.BackgroundStyle? { p.views[id]?.symbolView?.cell?.backgroundStyle }
        XCTAssertEqual(style(2), .normal, "a fixed written colour")
        XCTAssertEqual(style(3), .emphasized, "a role")
        XCTAssertEqual(style(4), .emphasized, "nobody's")
    }
}
#endif
