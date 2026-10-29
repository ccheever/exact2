// LLP 1081 stage 2: page symbols take computed color; native faces keep their control tint.
#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class SymbolColorIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil }

    private let red: [Double] = [232, 28, 36, 255]
    private let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
    private let teal: [Any] = [[48, 176, 199, 255], [64, 200, 224, 255]]
    private let pink: [Any] = [[173, 20, 87, 255], [244, 143, 177, 255]]

    /// A root whose accent is red, holding star symbols.
    private func presenter(_ images: [(Int, Bool, [String: Any])]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "props": [:], "handlers": [], "style": ["accent_color": red]],
                                    ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0]]
        for (id, written, style) in images {
            var props: [String: Any] = ["imageSource": "symbol:star", "symbolName": "star"]
            if written { props["symbolColorWritten"] = "1" }
            ops.append(["op": "create", "id": id, "kind": "image", "props": props, "handlers": [], "style": ["font_size": 17].merging(style) { $1 }])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0])
        }
        ops += [["op": "children", "id": 1, "ids": images.map { $0.0 }], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return p
    }
    private func ink(_ p: Presenter, _ id: UInt32) throws -> [Double] {
        let color = try XCTUnwrap(p.views[id]?.symbolView?.tintColor)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        _ = color.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a].map { (Double($0) * 255).rounded() }
    }

    func testAWrittenColourIsTheSymbolsInk() throws {
        let p = presenter([(2, true, ["text_color": tint]), (3, true, ["text_color": pink]), (4, false, ["text_color": pink]),
                           (5, true, ["text_color": pink, "tint_color": teal])])
        XCTAssertNotNil(try XCTUnwrap(p.views[2]).symbolInk)
        XCTAssertEqual(try ink(p, 2), red, "`color=\"AccentColor\"`: the accent")
        XCTAssertEqual(try ink(p, 3), [173, 20, 87, 255], "a token's colour")
        XCTAssertEqual(try ink(p, 4), [173, 20, 87, 255], "computed colour is inherited")
        XCTAssertEqual(try ink(p, 5), [173, 20, 87, 255], "raster tint is ignored on a symbol")
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["symbolColorWritten": "1"], "clear": []]]))
        XCTAssertEqual(try ink(p, 4), [173, 20, 87, 255], "written later")
        p.apply(wireBatch([["op": "props", "id": 4, "set": [:], "clear": ["symbolColorWritten"]]]))
        XCTAssertEqual(try ink(p, 4), [173, 20, 87, 255], "provenance does not change computed colour")
    }

    func testComputedSymbolColorsInASnapshot() throws {
        let label: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let p = presenter([(2, false, ["text_color": label]), (3, false, ["text_color": pink]),
                           (4, false, ["text_color": teal, "symbol_rendering": "hierarchical"])])
        window.overrideUserInterfaceStyle = .light
        p.viewport.backgroundColor = .systemBackground
        for (index, id) in [2, 3, 4].enumerated() {
            p.apply(wireBatch([["op": "frame", "id": id, "x": 20 + index * 90, "y": 40, "w": 60, "h": 60]]))
        }
        p.viewport.layoutIfNeeded()
        let picture = UIGraphicsImageRenderer(bounds: p.viewport.bounds).image { p.viewport.layer.render(in: $0.cgContext) }
        let attachment = XCTAttachment(image: picture)
        attachment.name = "Computed symbol color"
        attachment.lifetime = .keepAlways
        add(attachment)
        XCTAssertEqual(try ink(p, 3), [173, 20, 87, 255])
    }

    func testChangingHierarchicalInkKeepsTheImageAndItsReplacementIdentity() throws {
        let p = presenter([(2, false, ["text_color": pink, "symbol_rendering": "hierarchical", "symbol_effect": "replace"])])
        let node = try XCTUnwrap(p.views[2]), leaf = try XCTUnwrap(node.symbolView)
        let key = node.symbolKey, generation = node.loadGeneration, original = node.image
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 17, "text_color": teal, "symbol_rendering": "hierarchical", "symbol_effect": "replace"]]]))
        XCTAssertEqual(try ink(p, 2), [48, 176, 199, 255])
        XCTAssertEqual(node.symbolKey, key)
        XCTAssertEqual(node.loadGeneration, generation)
        XCTAssertTrue(node.image === original, "paint changes do not replace the glyph")
        let ink = UIImage.SymbolConfiguration(hierarchicalColor: try XCTUnwrap(node.symbolInk))
        let config = try XCTUnwrap(leaf.preferredSymbolConfiguration)
        XCTAssertTrue(config.isEqual(config.applying(ink)))
    }

    func testProjectedSegmentsRedrawPaletteAndInheritedHierarchicalTint() throws {
        let p = presenter([])
        defer { p.reset() }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 100, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "frame", "id": 100, "x": 0, "y": 0, "w": 300, "h": 40]]
        for id in [102, 104] {
            ops += [
                ["op": "create", "id": id, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilityLabel": "Cloud"], "handlers": ["press"]],
                ["op": "create", "id": id + 1, "kind": "image", "props": ["imageSource": "symbol:sf/cloud.sun.fill", "symbolName": "cloud.sun.fill"],
                 "style": ["font_size": 24, "symbol_rendering": "palette", "symbol_palette": [[255, 0, 0, 255], [0, 0, 255, 255]]]],
                ["op": "frame", "id": id + 1, "x": 0, "y": 0, "w": 32, "h": 32],
                ["op": "children", "id": id, "ids": [id + 1]]]
        }
        ops += [["op": "children", "id": 100, "ids": [102, 104]], ["op": "children", "id": 1, "ids": [100]]]
        p.apply(wireBatch(ops))
        let control = try XCTUnwrap(p.segments.control(of: 100)), icon = try XCTUnwrap(p.views[103])
        let glyph = icon.image, before = try XCTUnwrap(control.imageForSegment(at: 0)?.pngData())
        p.apply(wireBatch([["op": "style", "id": 103, "style": ["font_size": 24, "symbol_rendering": "palette",
            "symbol_palette": [[0, 255, 0, 255], [255, 0, 255, 255]]]]]))
        XCTAssertTrue(icon.image === glyph, "paint preserves the page glyph")
        XCTAssertNotEqual(control.imageForSegment(at: 0)?.pngData(), before, "the projected palette changes")
        p.apply(wireBatch([["op": "style", "id": 103, "style": ["font_size": 24, "symbol_rendering": "hierarchical"]]]))
        let hierarchical = icon.image
        try XCTUnwrap(p.views[100]).tintColor = .red
        p.segments.sync()
        let red = try XCTUnwrap(control.imageForSegment(at: 0)?.pngData())
        try XCTUnwrap(p.views[100]).tintColor = .blue
        p.segments.sync()
        XCTAssertTrue(icon.image === hierarchical)
        XCTAssertNotEqual(control.imageForSegment(at: 0)?.pngData(), red, "native inherited tint repaints hierarchical segments")
    }

    /// A projected segment made again while UIKit dims tints keeps its palette as written: UIKit
    /// keeps the image as made, and nothing makes it again when the dimming ends
    func testAProjectedSegmentMadeWhileDimmedKeepsItsPalette() throws {
        let p = presenter([])
        defer { p.reset() }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 100, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "frame", "id": 100, "x": 0, "y": 0, "w": 300, "h": 40]]
        for id in [102, 104] {
            ops += [
                ["op": "create", "id": id, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilityLabel": "Cloud"], "handlers": ["press"]],
                ["op": "create", "id": id + 1, "kind": "image", "props": ["imageSource": "symbol:sf/cloud.sun.fill", "symbolName": "cloud.sun.fill"],
                 "style": ["font_size": 24, "symbol_rendering": "palette", "symbol_palette": [[255, 0, 0, 255], [0, 0, 255, 255]]]],
                ["op": "frame", "id": id + 1, "x": 0, "y": 0, "w": 32, "h": 32],
                ["op": "children", "id": id, "ids": [id + 1]]]
        }
        ops += [["op": "children", "id": 100, "ids": [102, 104]], ["op": "children", "id": 1, "ids": [100]]]
        p.apply(wireBatch(ops))
        let control = try XCTUnwrap(p.segments.control(of: 100))
        let before = try XCTUnwrap(control.imageForSegment(at: 0)?.pngData())
        p.viewport.tintAdjustmentMode = .dimmed
        p.segments.sync()
        p.viewport.tintAdjustmentMode = .automatic
        XCTAssertEqual(control.imageForSegment(at: 0)?.pngData(), before, "the palette as written, once the dimming ends")
    }

    /// LLP 1121 D9: a symbol a native control shows as its face keeps the
    /// control's tint. Under the same written green, hierarchical (which
    /// bakes its colour into the image's configuration): a tablist of two
    /// image tabs and a radio group of two image radios are segmented
    /// controls, whose images are made without it (`faceImage`); a one-tab
    /// tablist stays as authored, and its image, like a page symbol beside
    /// them, takes it.
    func testAProjectedSegmentKeepsTheControlsTint() throws {
        let p = presenter([])
        let style: [String: Any] = ["font_size": 17, "text_color": [0, 200, 0, 255], "symbol_rendering": "hierarchical"]
        let symbol: [String: Any] = ["imageSource": "symbol:sf/star.fill", "symbolName": "star.fill", "symbolColorWritten": "1"]
        var ops: [[String: Any]] = []
        func group(_ id: Int, _ role: String, _ member: String, _ count: Int) {
            ops.append(["op": "create", "id": id, "kind": "view", "props": ["accessibilityRole": role], "handlers": [], "style": [:]])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": Double(id), "w": 300.0, "h": 32.0])
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
        let green = [0.0, 200.0, 0.0, 255.0]
        for id: UInt32 in [400, 303] { XCTAssertEqual(try ink(p, id), green, "\(id): a page symbol takes its written green") }
        let written = UIImage.SymbolConfiguration(hierarchicalColor: try XCTUnwrap(p.views[400]?.symbolInk))
        XCTAssertTrue(try XCTUnwrap(p.views[400]?.symbolView?.preferredSymbolConfiguration).isEqual(try XCTUnwrap(p.views[400]?.symbolView?.preferredSymbolConfiguration).applying(written)), "the image view configures hierarchical ink")
        for owner: UInt32 in [100, 200] {
            let control = try XCTUnwrap(p.views[owner]?.subviews.first { $0 is UISegmentedControl } as? UISegmentedControl, "\(owner) projects")
            XCTAssertEqual(control.numberOfSegments, 2)
            XCTAssertNotNil(control.imageForSegment(at: 0))
            // What the installed segment was drawn from: not the page's image,
            // and made without the written green.
            let icon = try XCTUnwrap(p.views[owner + 3]), drawn = try XCTUnwrap(p.segments.drawn(of: owner, segment: 0))
            XCTAssertFalse(drawn === icon.image, "\(owner): the segment is drawn from its own image")
            let face = try XCTUnwrap(drawn.symbolConfiguration)
            XCTAssertFalse(face.isEqual(face.applying(written)), "\(owner): the segment's image is made without the written green")
            XCTAssertTrue(try XCTUnwrap(icon.faceImage?.symbolConfiguration).isEqual(face), "\(owner): it is the face's")
        }
    }
}
#endif
