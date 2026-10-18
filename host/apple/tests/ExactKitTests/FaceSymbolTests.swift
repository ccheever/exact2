#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit
#if os(iOS)
@testable import ExactGroupedLists
#endif

/// Stage 9 A2 (LLP 1121 D10): a native control's face draws an svg-only
/// icon (no `sf`, a template path) as a template image the control tints as
/// it tints an SF Symbol — a bar or toolbar item, a tab, a segment, a button
/// face, a grouped row — from the path its image node carries (A1's
/// `symbolPath`) or its face's JSON (`symbolPath` beside a "" `symbol`). An
/// `sf` icon draws its symbol as before.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class FaceSymbolTests: XCTestCase {
    static let square = "M 2 2 L 22 2 L 22 22 L 2 22 Z"
    static var token: String { FaceSymbol.token(path: square, viewBox: "0 0 24 24")! }
    static let svgProps = ["imageSource": "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg", "symbolName": "",
                           "symbolPath": square, "symbolViewBox": "0 0 24 24"]
    static func sfProps(_ name: String) -> [String: String] { ["imageSource": "symbol:sf/\(name)", "symbolName": name] }

    #if os(iOS)
    private var window: UIWindow?
    override func tearDown() { MainActor.assumeIsolated { window?.isHidden = true; window = nil } }
    static func isTemplate(_ image: UIImage?) -> Bool { image?.renderingMode == .alwaysTemplate }
    #else
    private var window: NSWindow?
    override func tearDown() { MainActor.assumeIsolated { window?.close(); window = nil } }
    static func isTemplate(_ image: NSImage?) -> Bool { image?.isTemplate == true }
    #endif

    private func view(_ id: Int, _ kind: String, _ props: [String: String] = [:], handlers: [String] = [], x: Double = 0, w: Double = 100) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": w, "h": 30.0]]
    }

    /// A presenter in a window, with these faces for its native buttons.
    @MainActor private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        #if os(iOS)
        let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = w.bounds
        w.addSubview(p.viewport)
        w.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 400), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        #endif
        window = w
        p.apply(wireBatch(ops))
        return p
    }

    private func face(symbol: String?, title: String? = nil, label: String? = nil) -> ButtonFace {
        var f = ButtonFace(); f.symbol = symbol; f.title = title; f.label = label
        return f
    }

    /// The face's JSON: a "" symbol with a path is the path's token; a name stays a name.
    func testAFacesPathIsItsTokenAndDrawsATemplateImage() throws {
        let json = { (o: [String: Any]) in ButtonFace(json: try! JSONSerialization.data(withJSONObject: o)) }
        let svg = json(["button": true, "symbol": "", "symbolPath": Self.square, "symbolViewBox": "0 0 24 24"])
        let token = try XCTUnwrap(svg.symbol)
        XCTAssertTrue(FaceSymbol.isPath(token))
        let image = try XCTUnwrap(FaceSymbol.image(token, points: 20))
        XCTAssertTrue(Self.isTemplate(image), "the control tints it")
        XCTAssertEqual(image.size.width, 20, "an em square at the site's point size")
        XCTAssertEqual(json(["button": true, "symbol": "heart"]).symbol, "heart")
        XCTAssertEqual(json(["button": true, "symbol": ""]).symbol, "", "a role with no name and no path still draws nothing")
        XCTAssertNil(FaceSymbol.image(""))
        XCTAssertNotNil(FaceSymbol.image("heart"))
    }

    #if os(iOS)
    @MainActor func testABarItemsPathIsItsSymbol() throws {
        let p = presenter(view(1, "view") + view(2, "button", handlers: ["press"]) + view(3, "image", Self.svgProps)
                          + view(4, "button", handlers: ["press"]) + view(5, "image", Self.sfProps("plus"))
                          + view(6, "view") + view(7, "image", Self.svgProps)
                          + [["op": "style", "id": 6, "style": ["background_color": [52, 199, 89, 255]]],
                             ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]], ["op": "children", "id": 6, "ids": [7]],
                             ["op": "children", "id": 1, "ids": [2, 4, 6]], ["op": "roots", "ids": [1]]])
        let svg = HeaderShape.Item(try XCTUnwrap(p.views[2])), sf = HeaderShape.Item(try XCTUnwrap(p.views[4]))
        XCTAssertEqual(svg.symbol, Self.token)
        XCTAssertTrue(Self.isTemplate(FaceSymbol.image(try XCTUnwrap(svg.symbol))), "the bar item's image, tinted by the bar")
        XCTAssertEqual(sf.symbol, "plus")
        // A filled box holding the icon (an avatar-like item) draws it in its ink.
        let chip = try XCTUnwrap(BadgeFace(box: try XCTUnwrap(p.views[6])))
        XCTAssertEqual(chip.symbol, Self.token)
        XCTAssertEqual(chip.image.size.width, 36)
    }

    @MainActor func testATabsPathIsItsItemsTemplateImage() throws {
        // A tablist's native tabs with a symbol and a title each: a tab bar.
        var ops = view(1, "view", ["accessibilityRole": "tablist"], w: 300)
        for (i, id) in [2, 3].enumerated() {
            ops += [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
                     "props": ["type": "button", "accessibilityRole": "tab", "accessibilitySelected": i == 0 ? "true" : "false"],
                     "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                    ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 40.0]]
        }
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face(symbol: Self.token, title: "Reply"), 3: face(symbol: "tray", title: "Inbox")])
        let bar = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UITabBar }.first)
        let items = try XCTUnwrap(bar.items)
        XCTAssertTrue(Self.isTemplate(items[0].image), "the path, tinted by the bar")
        XCTAssertEqual(items[0].image?.size.width, FaceSymbol.tabBarPoints)
        XCTAssertNotNil(items[1].image, "an SF Symbol as before")
        // A root tab's face (UIKit's tab bar controller), from its image child.
        let tab = presenter(view(1, "button", handlers: ["press"]) + view(2, "image", Self.svgProps) + view(3, "text", ["text": "Reply"])
                            + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let face = TabFace(try XCTUnwrap(tab.views[1]))
        XCTAssertEqual(face.base, Self.token, "no `.fill` variant to strip")
        XCTAssertTrue(Self.isTemplate(FaceSymbol.image(try XCTUnwrap(face.base), points: FaceSymbol.tabBarPoints)))
    }

    @MainActor func testASegmentsPathIsItsImage() throws {
        var ops = view(1, "view", ["accessibilityRole": "tablist"], w: 300)
        for (i, id) in [2, 3].enumerated() {
            ops += [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
                     "props": ["type": "button", "accessibilityRole": "tab", "accessibilityLabel": ["Reply", "Map"][i]],
                     "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                    ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 40.0]]
        }
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face(symbol: Self.token, label: "Reply"), 3: face(symbol: "map", label: "Map")])
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        let image = try XCTUnwrap(segments.imageForSegment(at: 0), "the path, not an empty image")
        XCTAssertGreaterThan(image.size.width, 1)
        XCTAssertTrue(Self.isTemplate(image), "the control tints it")
        XCTAssertEqual(image.accessibilityLabel, "Reply")
        XCTAssertNotNil(segments.imageForSegment(at: 1))
    }

    @MainActor func testAButtonFacesPathIsItsConfigurationsTemplateImage() throws {
        let button = UIButton(type: .system)
        ButtonConfigurationIOS.apply(face(symbol: Self.token, title: "Reply"), to: button, traits: button.traitCollection, accent: nil)
        let image = try XCTUnwrap(button.configuration?.image)
        XCTAssertTrue(Self.isTemplate(image), "the button tints it as a symbol")
        XCTAssertEqual(image.size.width, (button.titleLabel?.font ?? .preferredFont(forTextStyle: .body)).pointSize.rounded(.up), accuracy: 1,
                       "at the title's size, as a symbol beside a title")
        ButtonConfigurationIOS.apply(face(symbol: "heart"), to: button, traits: button.traitCollection, accent: nil)
        XCTAssertTrue(button.configuration?.image?.isSymbolImage == true, "an SF Symbol as before")
    }
    #else
    @MainActor func testAToolbarItemsPathIsItsTemplateImage() throws {
        let p = presenter(view(1, "view", ["accessibilityRole": "toolbar", "toolbarPlacement": "window"], w: 300)
                          + view(2, "button", ["accessibilityLabel": "Reply"], handlers: ["press"]) + view(3, "image", Self.svgProps)
                          + view(4, "button", ["accessibilityLabel": "Add"], handlers: ["press"]) + view(5, "image", Self.sfProps("plus"))
                          + [["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]],
                             ["op": "children", "id": 1, "ids": [2, 4]], ["op": "roots", "ids": [1]]])
        let w = try XCTUnwrap(window)
        defer { p.toolbar.detach() }
        XCTAssertTrue(p.toolbar.attach(to: w))
        let reply = try XCTUnwrap(p.toolbar.items[2]), add = try XCTUnwrap(p.toolbar.items[4])
        XCTAssertTrue(Self.isTemplate(reply.image), "the path, tinted by the toolbar")
        XCTAssertEqual(reply.image?.size.width, FaceSymbol.toolbarPoints)
        XCTAssertTrue(Self.isTemplate(add.image), "an SF Symbol as before")
    }

    @MainActor func testASegmentsAndASidebarTabsPathIsTheirImage() throws {
        var ops = view(1, "view", ["accessibilityRole": "tablist"], w: 300)
        for (i, id) in [2, 3].enumerated() {
            ops += [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
                     "props": ["type": "button", "accessibilityRole": "tab", "accessibilityLabel": ["Reply", "Map"][i]],
                     "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                    ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 30.0]]
        }
        let p = presenter(ops + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]],
                          faces: [2: face(symbol: Self.token, label: "Reply"), 3: face(symbol: "map", label: "Map")])
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? NSSegmentedControl }.first)
        let image = try XCTUnwrap(segments.image(forSegment: 0))
        XCTAssertTrue(Self.isTemplate(image), "the path, tinted by the control")
        XCTAssertGreaterThan(image.size.width, 1)
        XCTAssertEqual(image.accessibilityDescription, "Reply")
        XCTAssertNotNil(segments.image(forSegment: 1))
        // A sidebar row's tab face, from its image child.
        let tab = presenter(view(1, "button", handlers: ["press"]) + view(2, "image", Self.svgProps) + view(3, "text", ["text": "Reply"])
                            + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let face = TabFace(try XCTUnwrap(tab.views[1]))
        XCTAssertEqual(face.base, Self.token)
        XCTAssertTrue(Self.isTemplate(FaceSymbol.image(try XCTUnwrap(face.base))))
    }

    @MainActor func testAButtonFacesPathIsItsTemplateImageAndTakesItsSymbolTint() throws {
        let button = NSButton(title: "Reply", target: nil, action: nil)
        _ = ButtonConfigurationMac.apply(face(symbol: Self.token, title: "Reply"), to: button, appearance: NSAppearance(named: .aqua)!, accent: nil)
        let image = try XCTUnwrap(button.image)
        XCTAssertTrue(Self.isTemplate(image), "the button tints it as a symbol")
        XCTAssertEqual(button.imagePosition, .imageLeading)
        // A tint its author wrote on the symbol draws it in that colour, as a palette colour draws a symbol.
        var tinted = face(symbol: Self.token, title: "Reply")
        tinted.rows.symbol["tint_color"] = .array([.number(52), .number(199), .number(89), .number(255)])
        _ = ButtonConfigurationMac.apply(tinted, to: button, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertFalse(Self.isTemplate(button.image), "its own colour")
        _ = ButtonConfigurationMac.apply(face(symbol: "heart"), to: button, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertNotNil(button.image, "an SF Symbol as before")
    }
    #endif
}

#if os(iOS)
/// A grouped row led by an svg-only icon: its JSON's path is its symbol's
/// token, and its standard cell's image is the path's template image.
final class FaceSymbolGroupedTests: GroupedListFixture {
    func testAGroupedRowsPathIsItsCellsTemplateImage() throws {
        let json: [String: Any] = ["style": "inset-grouped", "sections": [["view": 2, "rows": [
            ["view": 10, "custom": false, "symbol": "", "symbolPath": FaceSymbolTests.square, "symbolViewBox": "0 0 24 24", "title": "Reply"],
            ["view": 11, "custom": false, "symbol": "bell", "title": "Notifications"]]]]]
        let decoded = try XCTUnwrap(GroupedListModel(json: try JSONSerialization.data(withJSONObject: json)))
        XCTAssertEqual(decoded.sections[0].rows[0].symbol, FaceSymbolTests.token)
        XCTAssertEqual(decoded.sections[0].rows[1].symbol, "bell")
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[0].symbol = decoded.sections[0].rows[0].symbol
            return m
        }
        let c = try XCTUnwrap(try cell(p, 10).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(c.image?.renderingMode, .alwaysTemplate, "the cell tints it as a symbol")
        XCTAssertEqual(c.image?.size.width, FaceSymbol.defaultPoints.rounded(.up))
    }
}
#endif
#endif
