// LLP 1081 stage 2: symbols take computed color without replacing their image.
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
    private func presentedImage(_ p: Presenter, _ id: UInt32) throws -> NSImage? {
        let node = try XCTUnwrap(p.views[id])
        return node.image?.withSymbolConfiguration(try XCTUnwrap(node.symbolView?.symbolConfiguration))
    }
    private func assertChannels(_ got: [Double], _ want: [Double], _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        for i in 0..<4 { XCTAssertEqual(got[i], want[i], accuracy: 0.51, "\(message): \(got) is not \(want)", file: file, line: line) }
    }

    func testAnUntintedPathAndSFSymbolDrawTheSameUnsaidInk() throws {
        let label: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let p = presenter([(2, false, ["text_color": label]), (3, false, ["text_color": label])])
        defer { p.reset() }
        p.apply(wireBatch([
            ["op": "style", "id": 1, "style": ["background_color": [[255, 255, 255, 255], [30, 30, 30, 255]]]],
            ["op": "props", "id": 2, "set": ["imageSource": "symbol:icon||0 0 24 24|M0 0 L24 0 L24 24 L0 24 Z|icons/square.svg", "symbolName": "", "symbolPath": "M 0 0 L 24 0 L 24 24 L 0 24 Z", "symbolViewBox": "0 0 24 24"]],
            ["op": "props", "id": 3, "set": ["imageSource": "symbol:sf/square.fill", "symbolName": "square.fill"]],
            ["op": "frame", "id": 2, "x": 20, "y": 20, "w": 24, "h": 24],
            ["op": "frame", "id": 3, "x": 60, "y": 20, "w": 24, "h": 24],
        ]))
        XCTAssertNotNil(p.views[2]?.image)
        XCTAssertTrue(p.views[2]?.symbolFound == true)
        let page = try XCTUnwrap(p.views[1])
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            window.appearance = NSAppearance(named: appearance)
            p.views[2]?.reapplyColors(); p.views[3]?.reapplyColors()
            let rep = try XCTUnwrap(Capture.picture(of: page))
            let scale = CGFloat(rep.pixelsWide) / page.bounds.width
            func rgb(_ x: CGFloat) throws -> [Int] {
                let c = try XCTUnwrap(rep.colorAt(x: Int(x * scale), y: Int(32 * scale))?.usingColorSpace(.sRGB))
                return [c.redComponent, c.greenComponent, c.blueComponent].map { Int(($0 * 255).rounded()) }
            }
            let path = try rgb(32), sf = try rgb(72)
            print("Unsaid icon ink \(appearance.rawValue): path \(path), SF \(sf)")
            for (a, b) in zip(path, sf) { XCTAssertEqual(a, b, accuracy: 1) }
        }
    }

    func testAWrittenColourIsTheSymbolsInk() throws {
        let p = presenter([(2, true, ["text_color": tint]), (3, true, ["text_color": pink]), (4, false, ["text_color": pink]),
                           (5, true, ["text_color": pink, "tint_color": teal])])
        assertChannels(try ink(p, 2), try channels(.controlAccentColor), "Shelf's star: `color=\"AccentColor\"` is the accent")
        assertChannels(try ink(p, 3), [173, 20, 87, 255], "a token's colour")
        assertChannels(try ink(p, 4), [173, 20, 87, 255], "computed colour is inherited")
        assertChannels(try ink(p, 5), [173, 20, 87, 255], "raster tint is ignored")
    }

    /// Computed color follows restyling while the glyph retains its identity.
    func testTheInkFollowsItsColourAndItsWord() throws {
        let p = presenter([(2, false, ["text_color": pink]), (3, true, ["text_color": pink, "symbol_rendering": "hierarchical"])])
        assertChannels(try ink(p, 2), [173, 20, 87, 255], "inherited")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["symbolColorWritten": "1"], "clear": []]]))
        assertChannels(try ink(p, 2), [173, 20, 87, 255], "written")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 17, "text_color": teal]]]))
        assertChannels(try ink(p, 2), [48, 176, 199, 255], "a new colour")
        p.apply(wireBatch([["op": "props", "id": 2, "set": [:], "clear": ["symbolColorWritten"]]]))
        assertChannels(try ink(p, 2), [48, 176, 199, 255], "still inherited")
        let hierarchical = try XCTUnwrap(p.views[3])
        let key = hierarchical.symbolKey
        assertChannels(try ink(p, 3), [173, 20, 87, 255], "hierarchical")
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["font_size": 17, "text_color": teal, "symbol_rendering": "hierarchical"]]]))
        XCTAssertEqual(hierarchical.symbolKey, key, "paint keeps the image identity")
        assertChannels(try ink(p, 3), [48, 176, 199, 255], "hierarchical, a new colour")
    }

    /// How many of the image's pixels are green, and how many drawn at all,
    /// as it is drawn.
    private func pixels(_ image: NSImage?) -> (green: Int, drawn: Int) {
        guard let image, image.size.width > 0, let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(image.size.width * 2), pixelsHigh: Int(image.size.height * 2),
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { return (0, 0) }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        image.draw(in: NSRect(x: 0, y: 0, width: rep.pixelsWide, height: rep.pixelsHigh))
        NSGraphicsContext.restoreGraphicsState()
        var count = 0, drawn = 0
        for y in 0..<rep.pixelsHigh { for x in 0..<rep.pixelsWide {
            guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
            if c.alphaComponent > 0.3 { drawn += 1 }
            if c.alphaComponent > 0.3, c.greenComponent > 0.6, c.redComponent < 0.35, c.blueComponent < 0.35 { count += 1 }
        } }
        return (count, drawn)
    }

    /// LLP 1121 D9: a symbol a native control shows as its face keeps the
    /// control's tint. Under the same written green, hierarchical (which
    /// bakes its colour into the image), a tablist of two image tabs and a
    /// radio group of two image radios are segmented controls whose images
    /// carry no green; a one-tab tablist stays as authored, and its image,
    /// like a page symbol beside them, is green.
    func testAProjectedSegmentKeepsTheControlsTint() throws {
        let p = presenter([])
        let style: [String: Any] = ["font_size": 17, "text_color": [0, 200, 0, 255], "symbol_rendering": "hierarchical"]
        let symbol: [String: Any] = ["imageSource": "symbol:sf/star.fill", "symbolName": "star.fill", "symbolColorWritten": "1"]
        var ops: [[String: Any]] = []
        func group(_ id: Int, _ role: String, _ member: String, _ count: Int) {
            ops.append(["op": "create", "id": id, "kind": "view", "props": ["accessibilityRole": role], "handlers": [], "style": [:]])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": Double(id / 4), "w": 280.0, "h": 24.0])
            var members: [Int] = []
            for k in 1...count {
                let tab = id + k * 2, image = tab + 1
                ops.append(["op": "create", "id": tab, "kind": "button", "props": ["accessibilityRole": member, "accessibilityLabel": "Star \(k)"], "handlers": ["press"], "style": [:]])
                ops.append(["op": "create", "id": image, "kind": "image", "props": symbol, "handlers": [], "style": style])
                ops.append(["op": "frame", "id": image, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0])
                ops.append(["op": "children", "id": tab, "ids": [image]])
                members.append(tab)
            }
            ops.append(["op": "children", "id": id, "ids": members])
        }
        group(100, "tablist", "tab", 2); group(200, "radiogroup", "radio", 2); group(300, "tablist", "tab", 1)
        ops.append(["op": "create", "id": 400, "kind": "image", "props": symbol, "handlers": [], "style": style])
        ops.append(["op": "frame", "id": 400, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0])
        ops.append(["op": "children", "id": 1, "ids": [100, 200, 300, 400]])
        p.apply(wireBatch(ops))
        XCTAssertGreaterThan(pixels(try presentedImage(p, 400)).green, 0, "a page symbol bakes its written green")
        XCTAssertGreaterThan(pixels(try presentedImage(p, 303)).green, 0, "a one-tab tablist stays as authored: its image is the page's")
        for owner: UInt32 in [100, 200] {
            let control = try XCTUnwrap(p.views[owner]?.subviews.first { $0 is NSSegmentedControl } as? NSSegmentedControl, "\(owner) projects")
            XCTAssertEqual(control.segmentCount, 2)
            XCTAssertNotNil(control.image(forSegment: 0))
            let face = pixels(control.image(forSegment: 0))
            XCTAssertGreaterThan(face.drawn, 0, "\(owner): the segment draws its symbol")
            XCTAssertEqual(face.green, 0, "\(owner): the segment keeps the control's tint")
        }
    }

    /// Fixed computed color keeps its ink in an emphasized row; a role follows selection.
    func testAnEmphasizedRowKeepsAFixedWrittenColour() throws {
        let role: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let p = presenter([(2, true, ["text_color": pink]), (3, true, ["text_color": role]), (4, false, ["text_color": pink])])
        try XCTUnwrap(p.views[1]).rowEmphasisChanged(true)
        func style(_ id: UInt32) -> NSView.BackgroundStyle? { p.views[id]?.symbolView?.cell?.backgroundStyle }
        XCTAssertEqual(style(2), .normal, "a fixed written colour")
        XCTAssertEqual(style(3), .emphasized, "a role")
        XCTAssertEqual(style(4), .normal, "an inherited fixed colour")
        // A fixed colour written while the row is selected takes the
        // emphasis away at once; deselected, every cell is normal.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["font_size": 17, "text_color": pink]]]))
        XCTAssertEqual(style(3), .normal, "a fixed colour that came while selected")
        try XCTUnwrap(p.views[1]).rowEmphasisChanged(false)
        XCTAssertEqual([style(2), style(3), style(4)], [.normal, .normal, .normal])
    }
}
#endif
