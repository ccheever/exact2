#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// Stage 9 A2 (LLP 1121 D10): a native control's face draws an svg-only
/// icon (no `sf`, a template path) as a template image the control tints as
/// it tints an SF Symbol, from the path its image node carries (A1's
/// `symbolPath`) or its face's JSON (`symbolPath` beside a "" `symbol`). An
/// `sf` icon draws its symbol as before. UIKit's mounted faces are
/// FaceSymbolIOSTests'; here the token, and AppKit's.
///   bun host/apple/build.mjs --test   (and --test --ios)
final class FaceSymbolTests: XCTestCase {
    static let square = "M 2 2 L 22 2 L 22 22 L 2 22 Z"
    static var token: String { FaceSymbol.token(path: square, viewBox: "0 0 24 24")! }
    static let svgProps = ["imageSource": "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg", "symbolName": "",
                           "symbolPath": square, "symbolViewBox": "0 0 24 24"]
    static func sfProps(_ name: String) -> [String: String] { ["imageSource": "symbol:sf/\(name)", "symbolName": name] }

    #if os(iOS)
    static func isTemplate(_ image: UIImage?) -> Bool { image?.renderingMode == .alwaysTemplate }
    #else
    static func isTemplate(_ image: NSImage?) -> Bool { image?.isTemplate == true }
    #endif

    /// The face's JSON: a "" symbol with a path is the path's token; a name stays a name; the agent reads the icon's name.
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
        XCTAssertEqual(FaceSymbol.observed(token), "", "the agent's unnamed form")
        XCTAssertEqual(FaceSymbol.observed("heart"), "heart")
        XCTAssertNil(FaceSymbol.observed(nil))
        XCTAssertEqual(FaceSymbol.observed(token, icon: "icon.reply"), "icon.reply", "the token's name as its author wrote it")
        XCTAssertEqual(FaceSymbol.observed("heart", icon: "icon.reply"), "heart")
        XCTAssertEqual(json(["button": true, "symbol": "", "symbolPath": Self.square, "symbolIcon": "icon.reply"]).symbolIcon, "icon.reply")
    }

    /// Stage 14 C2: an even-odd icon's token says its rule and draws it; a
    /// nonzero icon's token is as it was, so its equality and caches hold.
    func testAnEvenOddFacesTokenCarriesItsRule() throws {
        XCTAssertEqual(FaceSymbol.token(path: Self.square, viewBox: "0 0 24 24", rule: nil), Self.token, "nonzero: today's token")
        XCTAssertEqual(FaceSymbol.token(path: Self.square, viewBox: "0 0 24 24", rule: "nonzero"), Self.token)
        let face = ButtonFace(json: try JSONSerialization.data(withJSONObject: ["button": true, "symbol": "", "symbolPath": IconPathTests.frame,
                                                                                 "symbolViewBox": "0 0 24 24", "symbolFillRule": "evenodd"]))
        let token = try XCTUnwrap(face.symbol)
        XCTAssertEqual(token, "icon-path:evenodd|0 0 24 24|" + IconPathTests.frame)
        XCTAssertTrue(FaceSymbol.isPath(token))
        XCTAssertEqual(FaceSymbol.observed(token, icon: "icon.gallery"), "icon.gallery")
        XCTAssertEqual(try IconPathTests.inked(try XCTUnwrap(FaceSymbol.pathImage(token, points: 24)), at: [4, 12]), [true, false], "the hole clear")
        let plain = try XCTUnwrap(FaceSymbol.token(path: IconPathTests.frame, viewBox: "0 0 24 24"))
        XCTAssertEqual(try IconPathTests.inked(try XCTUnwrap(FaceSymbol.pathImage(plain, points: 24)), at: [4, 12]), [true, true], "nonzero fills it")
    }

    #if os(macOS)
    private var window: NSWindow?
    private var session: ExactSession?
    override func tearDown() {
        MainActor.assumeIsolated { window?.close(); window = nil; session?.destroy(); session = nil }
    }

    private func view(_ id: Int, _ kind: String, _ props: [String: String] = [:], handlers: [String] = [], x: Double = 0, w: Double = 100) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": w, "h": 30.0]]
    }
    @MainActor private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 400), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        window = w
        p.apply(wireBatch(ops))
        return p
    }
    private func face(symbol: String?, title: String? = nil, label: String? = nil) -> ButtonFace {
        var f = ButtonFace(); f.symbol = symbol; f.title = title; f.label = label
        return f
    }

    /// The image's colour at its middle, drawn 24 points square, unpremultiplied RGBA 0...1.
    private func middle(_ image: NSImage?) throws -> [CGFloat] { try colour(image, x: 12, y: 12) }
    /// The image's colour at (`x`, `y`) of it drawn 24 points square (y from the top).
    private func colour(_ image: NSImage?, x: Int, y: Int) throws -> [CGFloat] {
        let image = try XCTUnwrap(image)
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 24, pixelsHigh: 24, bitsPerSample: 8, samplesPerPixel: 4,
                                                 hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        image.draw(in: NSRect(x: 0, y: 0, width: 24, height: 24))
        NSGraphicsContext.restoreGraphicsState()
        let c = try XCTUnwrap(rep.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB))
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    }

    @MainActor func testAToolbarItemsPathIsItsInstalledTemplateImage() throws {
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
        XCTAssertEqual(try middle(reply.image)[3], 1, accuracy: 0.01, "the path inked")
        XCTAssertNotNil(add.image, "an SF Symbol as before")
    }

    /// Stage 14 C2, mounted: a toolbar button's even-odd icon (its image
    /// node's `symbolFillRule`) is installed with its hole clear and its band
    /// inked; the rule cleared through the batch, the item's image is filled
    /// again. (AppKit draws the item through SwiftUI, which no in-process
    /// capture reaches, so its placement on the screen is not sampled here.)
    @MainActor func testAToolbarItemsEvenOddPathKeepsItsHole() throws {
        var even = Self.svgProps
        even["symbolPath"] = IconPathTests.frame; even["symbolFillRule"] = "evenodd"
        let p = presenter(view(1, "view", ["accessibilityRole": "toolbar", "toolbarPlacement": "window"], w: 300)
                          + view(2, "button", ["accessibilityLabel": "Gallery"], handlers: ["press"]) + view(3, "image", even)
                          + [["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let w = try XCTUnwrap(window)
        defer { p.toolbar.detach() }
        XCTAssertTrue(p.toolbar.attach(to: w))
        XCTAssertEqual(try middle(try XCTUnwrap(p.toolbar.items[2]).image)[3], 0, accuracy: 0.01, "the hole clear")
        XCTAssertEqual(try colour(try XCTUnwrap(p.toolbar.items[2]).image, x: 12, y: 4)[3], 1, accuracy: 0.01, "the band inked")
        p.apply(wireBatch([["op": "props", "id": 3, "clear": ["symbolFillRule"]]]))
        XCTAssertEqual(try middle(try XCTUnwrap(p.toolbar.items[2]).image)[3], 1, accuracy: 0.01, "nonzero: filled")
    }

    /// The inked box (dark pixels: a glyph in the label colour on a light ground) of a bitmap, in points.
    private func inked(_ rep: NSBitmapImageRep, in region: CGRect? = nil, scale: CGFloat) throws -> CGRect {
        let area = region ?? CGRect(x: 0, y: 0, width: rep.pixelsWide, height: rep.pixelsHigh)
        var box = CGRect.null
        for y in Int(area.minY)..<min(rep.pixelsHigh, Int(area.maxY)) { for x in Int(area.minX)..<min(rep.pixelsWide, Int(area.maxX)) {
            guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB), c.alphaComponent > 0.5,
                  c.redComponent + c.greenComponent + c.blueComponent < 2.4 else { continue }
            box = box.union(CGRect(x: x, y: y, width: 1, height: 1))
        } }
        return CGRect(x: box.minX / scale, y: box.minY / scale, width: box.width / scale, height: box.height / scale)
    }

    /// Measured, once, by hand (EXACT_MEASURE_FACE_SYMBOLS=1; it shows a window on the screen and captures it
    /// with `screencapture`, as AppKit draws a toolbar button through SwiftUI, which no in-process capture
    /// reaches): an SF Symbol ("square") and the path (a square 20/24 of its em) side by side in a toolbar;
    /// the symbol's point size is its inked box against the same symbol drawn at 100 points, which the path's
    /// em square must be.
    @MainActor func testTheToolbarsPathSizeIsTheSymbolSizeAppKitShows() throws {
        guard ProcessInfo.processInfo.environment["EXACT_MEASURE_FACE_SYMBOLS"] == "1" else { throw XCTSkip("a hand measurement: EXACT_MEASURE_FACE_SYMBOLS=1") }
        let p = presenter(view(1, "view", ["accessibilityRole": "toolbar", "toolbarPlacement": "window"], w: 300)
                          + view(2, "button", ["accessibilityLabel": "Reply"], handlers: ["press"]) + view(3, "image", Self.svgProps)
                          + view(4, "button", ["accessibilityLabel": "Square"], handlers: ["press"]) + view(5, "image", Self.sfProps("square"))
                          + [["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]],
                             ["op": "children", "id": 1, "ids": [2, 4]], ["op": "roots", "ids": [1]]])
        let w = try XCTUnwrap(window)
        w.appearance = NSAppearance(named: .aqua)
        defer { p.toolbar.detach() }
        XCTAssertTrue(p.toolbar.attach(to: w))
        let screen = try XCTUnwrap(NSScreen.main)
        w.setFrameTopLeftPoint(NSPoint(x: screen.frame.minX + 200, y: screen.frame.maxY - 200))
        w.orderFrontRegardless(); w.displayIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.8))
        // The reference: "square" at 100 points, drawn black at 1x.
        let big = try XCTUnwrap(NSImage(systemSymbolName: "square", accessibilityDescription: nil)?
            .withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: 100, weight: .regular)))
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(big.size.width.rounded(.up)), pixelsHigh: Int(big.size.height.rounded(.up)),
                                                 bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        big.draw(in: NSRect(origin: .zero, size: big.size))
        NSGraphicsContext.restoreGraphicsState()
        let perPoint = try inked(rep, scale: 1).height / 100
        func buttons(_ v: NSView) -> [NSButton] { ((v as? NSButton).flatMap { $0.image == nil ? nil : [$0] } ?? []) + v.subviews.flatMap(buttons) }
        let all = buttons(try XCTUnwrap(w.contentView?.superview))
        let reply = try XCTUnwrap(p.toolbar.items[2]?.image), square = try XCTUnwrap(p.toolbar.items[4]?.image)
        let symbolButton = try XCTUnwrap(all.first { $0.image === square }, "the symbol's button")
        let pathButton = try XCTUnwrap(all.first { $0.image === reply }, "the path's button")
        // The window's frame on the screen, top-left origin, captured.
        let frame = w.frame, top = screen.frame.maxY - frame.maxY
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("face-symbol-\(UUID().uuidString).png")
        defer { if ProcessInfo.processInfo.environment["EXACT_MEASURE_KEEP"] == nil { try? FileManager.default.removeItem(at: file) } else { print("MEASURE kept \(file.path)") } }
        let capture = Process()
        capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        capture.arguments = ["-x", "-R", "\(Int(frame.minX)),\(Int(top)),\(Int(frame.width)),\(Int(frame.height))", file.path]
        try capture.run(); capture.waitUntilExit()
        let shot = try XCTUnwrap(NSBitmapImageRep(data: try Data(contentsOf: file)), "screencapture wrote the window")
        let scale = CGFloat(shot.pixelsWide) / frame.width
        func region(_ b: NSButton) -> CGRect {
            let r = b.convert(b.bounds, to: nil) // window coordinates, bottom-left origin
            // Inside its bezel, whose edge and shadow are no glyph.
            let inner = r.insetBy(dx: 8, dy: 6)
            return CGRect(x: inner.minX * scale, y: (frame.height - inner.maxY) * scale, width: inner.width * scale, height: inner.height * scale)
        }
        let symbolInk = try inked(shot, in: region(symbolButton), scale: scale), pathInk = try inked(shot, in: region(pathButton), scale: scale)
        let points = symbolInk.height / perPoint
        print("MEASURE mac toolbar: scale \(scale) regions \(region(symbolButton)) \(region(pathButton)); symbol button \(symbolButton.frame.size) ink \(symbolInk.size) -> \(points) pt; path ink \(pathInk.size) (em \(FaceSymbol.toolbarPoints), square 20/24 of it = \(FaceSymbol.toolbarPoints * 20 / 24)); reference \(perPoint * 100) per 100 pt")
        XCTAssertEqual(FaceSymbol.toolbarPoints, points.rounded(), accuracy: 1, "symbol \(points) pt")
        XCTAssertEqual(pathInk.height, FaceSymbol.toolbarPoints * 20 / 24, accuracy: 1.5, "the path drawn at its em, not scaled")
    }

    /// A segmented control draws its images as they are: an SF Symbol and the path side by side, each glyph's
    /// inked box measured in the control's drawing, the symbol's point size as in the toolbar's measure.
    @MainActor func testASegmentsPathSizeIsTheSymbolSizeAppKitDraws() throws {
        _ = NSApplication.shared
        let square = try XCTUnwrap(NSImage(systemSymbolName: "square", accessibilityDescription: nil))
        let big = try XCTUnwrap(square.withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: 100, weight: .regular)))
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(big.size.width.rounded(.up)), pixelsHigh: Int(big.size.height.rounded(.up)),
                                                 bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        big.draw(in: NSRect(origin: .zero, size: big.size))
        NSGraphicsContext.restoreGraphicsState()
        let perPoint = try inked(rep, scale: 1).height / 100
        let control = NSSegmentedControl(images: [square, try XCTUnwrap(FaceSymbol.image(Self.token))], trackingMode: .selectOne, target: nil, action: nil)
        control.appearance = NSAppearance(named: .aqua)
        control.frame = NSRect(x: 0, y: 0, width: 120, height: 28)
        control.layoutSubtreeIfNeeded()
        let shot = try XCTUnwrap(control.bitmapImageRepForCachingDisplay(in: control.bounds))
        control.cacheDisplay(in: control.bounds, to: shot)
        let scale = CGFloat(shot.pixelsWide) / control.bounds.width, half = CGFloat(shot.pixelsWide) / 2
        let inset = 8 * scale
        let symbolInk = try inked(shot, in: CGRect(x: inset, y: 4 * scale, width: half - 2 * inset, height: CGFloat(shot.pixelsHigh) - 8 * scale), scale: scale)
        let pathInk = try inked(shot, in: CGRect(x: half + inset, y: 4 * scale, width: half - 2 * inset, height: CGFloat(shot.pixelsHigh) - 8 * scale), scale: scale)
        let points = symbolInk.height / perPoint
        print("MEASURE mac segment: symbol ink \(symbolInk.size) -> \(points) pt; path ink \(pathInk.size) (em \(FaceSymbol.defaultPoints), square 20/24 of it = \(FaceSymbol.defaultPoints * 20 / 24))")
        XCTAssertEqual(FaceSymbol.defaultPoints, points.rounded(), accuracy: 1, "symbol \(points) pt")
        XCTAssertEqual(pathInk.height, FaceSymbol.defaultPoints * 20 / 24, accuracy: 1.5, "the path drawn at its em")
    }

    @MainActor func testNativeTabsPathIsTheirSegmentsImageAndAButtonsAgentSymbolIsItsIconName() throws {
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
        XCTAssertEqual(image.size.width, FaceSymbol.defaultPoints)
        XCTAssertEqual(image.accessibilityDescription, "Reply")
        XCTAssertNotNil(segments.image(forSegment: 1))
        // A native button's agent observation names no token.
        let b = presenter([["op": "create", "id": 1, "kind": "control", "handlers": ["press"], "props": ["type": "button", "accessibilityRole": "button"],
                            "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                           ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0], ["op": "roots", "ids": [1]]],
                          faces: [1: { var f = face(symbol: Self.token, title: "Reply"); f.symbolIcon = "icon.reply"; return f }()])
        let button = try XCTUnwrap(b.controls.controls[1] as? NativeButtonMac)
        XCTAssertEqual(b.controls.nativeObservation(button)["symbol"] as? String, "icon.reply", "the icon's name, not the token")
    }

    @MainActor func testAButtonFacesPathIsItsTemplateImageAndTakesItsSymbolTintAlphaAndAll() throws {
        let button = NSButton(title: "Reply", target: nil, action: nil)
        let aqua = try XCTUnwrap(NSAppearance(named: .aqua))
        _ = ButtonConfigurationMac.apply(face(symbol: Self.token, title: "Reply"), to: button, appearance: aqua, accent: nil)
        XCTAssertTrue(Self.isTemplate(button.image), "the button tints it as a symbol")
        XCTAssertEqual(button.imagePosition, .imageLeading)
        // A tint its author wrote on the symbol draws it in that colour, its alpha kept.
        for (alpha, label) in [(255.0, "opaque"), (128.0, "translucent"), (0.0, "transparent")] {
            var tinted = face(symbol: Self.token, title: "Reply")
            tinted.rows.symbol["tint_color"] = .array([.number(255), .number(0), .number(0), .number(alpha)])
            _ = ButtonConfigurationMac.apply(tinted, to: button, appearance: aqua, accent: nil)
            XCTAssertFalse(Self.isTemplate(button.image), label)
            let px = try middle(button.image)
            XCTAssertEqual(px[3], alpha / 255, accuracy: 0.02, "\(label): \(px)")
            if alpha > 0 { XCTAssertEqual(px[0], 1, accuracy: 0.02, "\(label): \(px)"); XCTAssertEqual(px[2], 0, accuracy: 0.02, "\(label): \(px)") }
        }
        _ = ButtonConfigurationMac.apply(face(symbol: "heart"), to: button, appearance: aqua, accent: nil)
        XCTAssertNotNil(button.image, "an SF Symbol as before")
    }

    /// The window's sidebar over a compiled app whose first tab is an svg-only icon token: the plan's path,
    /// sent by the Rust side as `symbolPath`, is the row's installed image; the agent reads the icon's name (`icon.reply`) for it.
    @MainActor func testASidebarRowsPathIsItsInstalledImageFromThePlan() throws {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/face-symbol-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("assets/icons"), withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try #"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 2 L22 2 L22 22 L2 22 Z"/></svg>"#
            .write(to: dir.appendingPathComponent("assets/icons/reply.svg"), atomically: true, encoding: .utf8)
        try "tamber t\n\ntheme\n  ink = #000000\n\nicons\n  icon.reply = icon(svg \"icons/reply.svg\")\n"
            .write(to: dir.appendingPathComponent("app.tamber"), atomically: true, encoding: .utf8)
        try """
        routes nav
          tab today "/"
          tab history "/history"

        component App
          action pick(name: string)
            nav = select(nav, name)
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%" display="flex" flex-direction="column"
              column flex=1 min-height=0 position="relative"
                for t in nav.tabs key=t.name
                  column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
                    for e in t.stack key=e.id
                      column navigationKey=`${e.id}` position="absolute" inset=0
                        text "Page"
              row role="tablist" aria-label="Places"
                button role="tab" aria-controls="panel-today" aria-selected=(nav.tab == "today") press=pick("today") flex=1
                  image "symbol:icon.reply"
                  text "Today"
                button role="tab" aria-controls="panel-history" aria-selected=(nav.tab == "history") press=pick("history") flex=1
                  image "symbol:calendar"
                  text "History"

        """.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let plan = dir.appendingPathComponent("app.plan")
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", plan.path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
        let session = ExactApp.shared.makeSession(label: "face-symbol-" + UUID().uuidString)
        self.session = session
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: CGSize(width: 700, height: 500)).error)
        // A native tab's face, from the kernel through the Rust side: the plan's path and the token's name.
        let tab = try XCTUnwrap(session.presenter.views.values.first { $0.props["accessibilityControls"] == "panel-today" })
        let face = try XCTUnwrap(session.presenter.buttonFace)(tab.id)
        XCTAssertEqual(face.symbol, Self.token, "the plan's path")
        XCTAssertEqual(face.symbolIcon, "icon.reply", "the icon's name as its author wrote it")
        let view = ExactView(session: session)
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 500), styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        window = w
        let split = ExactSidebarController(view: view, autosaveName: nil, minimumPageWidth: 320)
        split.view.frame = NSRect(x: 0, y: 0, width: 700, height: 500)
        w.contentViewController = split
        XCTAssertTrue(view.attachWindowToolbar(to: w))
        w.layoutIfNeeded()
        XCTAssertTrue(session.presenter.sidebar.installed)
        let today = try XCTUnwrap(split.table.view(atColumn: 0, row: 0, makeIfNecessary: true) as? SidebarCell)
        let image = try XCTUnwrap(today.imageView?.image, "the row shows the path")
        XCTAssertTrue(Self.isTemplate(image), "AppKit tints it as the row's symbol")
        XCTAssertEqual(try middle(image)[3], 1, accuracy: 0.01, "the path inked")
        let history = try XCTUnwrap(split.table.view(atColumn: 0, row: 1, makeIfNecessary: true) as? SidebarCell)
        XCTAssertNotNil(history.imageView?.image, "an SF Symbol as before")
        let rows = try XCTUnwrap(session.presenter.sidebar.summary["rows"] as? [[String: Any]])
        XCTAssertEqual(rows.first?["symbol"] as? String, "icon.reply", "the agent reads the icon's name, not the token")
        XCTAssertNotEqual(rows.last?["symbol"] as? String, "", "a role's SF Symbol name")
    }
    #endif
}
#endif
