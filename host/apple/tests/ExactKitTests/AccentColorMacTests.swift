// @ref LLP 1115.000 D3.4 — on the Mac an authored `accent-color` is its
// subtree's tint, as it is UIKit's inherited `tintColor` on iOS (LLP 1116.001
// D2, `SystemColorIOSTests`): it reaches every row beneath that names the
// tint, a text's ink, a box's fill, a symbol, and not only a control. AppKit
// gives a view no tint to inherit, so the presenter reads the nearest one up
// the view chain and applies what names the tint again when it changes.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class AccentColorMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// `AccentColor` as the wire carries it to the Mac: the view's tint.
    private let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
    private let teal: [Any] = [[48, 176, 199, 255], [64, 200, 224, 255]]
    private let pink: [Any] = [[173, 20, 87, 255], [244, 143, 177, 255]]

    /// A page (1) holding a box with an accent (2) and one without (6). In
    /// the first: a fill (3), a text (4) and a symbol that names the tint
    /// (5). Beside it: the same fill (7) and symbol (8).
    private func presenter(light: Bool = true) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: light ? .aqua : .darkAqua)
        window.contentView = p.viewport
        let symbol: [String: Any] = ["imageSource": "symbol:star-fill", "symbolName": "star.fill"]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["accent_color": teal]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": tint]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Accent"], "style": ["text_color": tint]],
            ["op": "create", "id": 5, "kind": "image", "props": symbol, "style": ["tint_color": tint]],
            ["op": "create", "id": 6, "kind": "view"],
            ["op": "create", "id": 7, "kind": "view", "style": ["background_color": tint]],
            ["op": "create", "id": 8, "kind": "image", "props": symbol, "style": ["tint_color": tint]],
            ["op": "children", "id": 2, "ids": [3, 4, 5]],
            ["op": "children", "id": 6, "ids": [7, 8]],
            ["op": "children", "id": 1, "ids": [2, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
        ]))
        return p
    }

    private func channels(_ color: NSColor, dark: Bool) throws -> [Double] {
        var out: [Double]?
        try XCTUnwrap(NSAppearance(named: dark ? .darkAqua : .aqua)).performAsCurrentDrawingAppearance {
            guard let c = color.usingColorSpace(.sRGB) else { return }
            out = [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map { Double(min(max($0, 0), 1)) * 255 }
        }
        return try XCTUnwrap(out)
    }
    private func assertChannels(_ got: [Double]?, _ want: [Double], _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        guard let got, got.count == 4 else { return XCTFail("\(message): no colour", file: file, line: line) }
        for i in 0..<4 { XCTAssertEqual(got[i], want[i], accuracy: 0.51, "\(message): \(got) is not \(want)", file: file, line: line) }
    }
    /// What the three under the accent show, and the two beside it.
    private func assertTinted(_ p: Presenter, under: [Double], beside: [Double], dark: Bool, _ message: String, file: StaticString = #filePath, line: UInt = #line) throws {
        func node(_ id: UInt32) throws -> NodeView { try XCTUnwrap(p.views[id], file: file, line: line) }
        assertChannels(try node(3).channels("background_color"), under, "\(message): a fill", file: file, line: line)
        assertChannels(try node(4).paragraphSpec().runs.first?.color, under, "\(message): a text's ink", file: file, line: line)
        let symbol = try XCTUnwrap(try node(5).symbolView?.contentTintColor, file: file, line: line)
        assertChannels(try channels(symbol, dark: dark), under, "\(message): a symbol", file: file, line: line)
        assertChannels(try node(7).channels("background_color"), beside, "\(message): a fill beside it", file: file, line: line)
        let other = try XCTUnwrap(try node(8).symbolView?.contentTintColor, file: file, line: line)
        assertChannels(try channels(other, dark: dark), beside, "\(message): a symbol beside it", file: file, line: line)
    }

    /// The accent reaches every `AccentColor` beneath it, in each scheme of a
    /// `light-dark()` accent, and what is beside it keeps the system's.
    func testAnAuthoredAccentIsItsSubtreesTint() throws {
        for dark in [false, true] {
            let p = presenter(light: !dark)
            let system = try channels(.controlAccentColor, dark: dark)
            try assertTinted(p, under: dark ? [64, 200, 224, 255] : [48, 176, 199, 255], beside: system, dark: dark, dark ? "dark" : "light")
            window.close()
        }
    }

    /// AppKit has no `tintColorDidChange`: a changed accent, one taken away,
    /// and a node moved out from under one are each applied again.
    func testAChangedAccentAndAMoveAreAppliedAgain() throws {
        let p = presenter()
        let system = try channels(.controlAccentColor, dark: false)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["accent_color": pink]]]))
        try assertTinted(p, under: [173, 20, 87, 255], beside: system, dark: false, "a changed accent")
        // The nearest accent wins over one above it.
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": teal]]]))
        try assertTinted(p, under: [173, 20, 87, 255], beside: [48, 176, 199, 255], dark: false, "the nearest of two")
        p.apply(wireBatch([["op": "style", "id": 1, "style": [:]], ["op": "style", "id": 2, "style": [:]]]))
        try assertTinted(p, under: system, beside: system, dark: false, "none: the system's accent")
        // The symbol beside moves under an accent, and back out.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["accent_color": teal]],
                           ["op": "children", "id": 6, "ids": [7]], ["op": "children", "id": 2, "ids": [3, 4, 5, 8]]]))
        let moved = try XCTUnwrap(p.views[8])
        assertChannels(try channels(try XCTUnwrap(moved.symbolView?.contentTintColor), dark: false), [48, 176, 199, 255], "moved under the accent")
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [3, 4, 5]], ["op": "children", "id": 6, "ids": [7, 8]]]))
        assertChannels(try channels(try XCTUnwrap(moved.symbolView?.contentTintColor), dark: false), system, "and back out")
    }

    /// The theme's `AccentColor` (LLP 1095 D4) is the tint where no authored
    /// accent is above, and a live edit of it (LLP 1121 stage 5) repaints
    /// what names the tint, though each row's style is the same `@tint`.
    func testALiveEditOfTheThemesAccentRepaintsWhatNamesTheTint() throws {
        let p = presenter()
        defer { ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]]), root: p.root) }
        let roles = { (accent: [Int]) in
            ThemeRoles.apply(wireBatch([["op": "roles", "roles": ["AccentColor": accent]]]), root: p.root)
            p.applyThemeTint()
        }
        // The fill as the layer holds it: nothing else asks AppKit to draw
        // the node again, so the edit itself must set it.
        let painted = { () throws -> [Double] in
            let fill = try XCTUnwrap(p.views[7]?.layer?.backgroundColor.flatMap { NSColor(cgColor: $0) })
            return try self.channels(fill, dark: false)
        }
        roles([255, 0, 0, 255])
        assertChannels(try painted(), [255, 0, 0, 255], "the theme's accent")
        roles([0, 0, 255, 255])
        assertChannels(try painted(), [0, 0, 255, 255], "after a live edit")
        // Under an authored accent, the author's still.
        assertChannels(try XCTUnwrap(p.views[3]).channels("background_color"), [48, 176, 199, 255], "under the author's")
    }
}
#endif
