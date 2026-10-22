#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1034 §8: `color-scheme` on a subtree is the view's own appearance,
/// which UIKit's traits carry below: a child's `light-dark()` colour, and a
/// material view's traits, resolve in it; leaving it off restores.
final class ColorSchemeIOSTests: XCTestCase {
    private let pair: BatchValue = .array([.array([255, 255, 255, 255]), .array([0, 0, 0, 255])])

    func testASubtreeResolvesInItsColorScheme() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.overrideUserInterfaceStyle = .light
        window.makeKeyAndVisible()
        let sheet = NodeView(id: 1, kind: "view", presenter: p)
        p.views[1] = sheet; window.addSubview(sheet)
        sheet.frame = window.bounds
        let child = NodeView(id: 2, kind: "view", presenter: p)
        p.views[2] = child; sheet.container.addSubview(child)
        child.frame = CGRect(x: 0, y: 0, width: 50, height: 50)
        child.applyStyle(["background_color": pair])
        let material = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        sheet.container.addSubview(material)
        XCTAssertFalse(child.drawsDark)
        XCTAssertEqual(child.channels("background_color"), [255, 255, 255, 255])
        child.applyBoxLayer()
        XCTAssertEqual(child.layer.backgroundColor?.components?.first ?? -1, 1, accuracy: 0.01)

        sheet.applyStyle(["color_scheme": "dark"])
        XCTAssertEqual(sheet.overrideUserInterfaceStyle, .dark)
        XCTAssertTrue(child.drawsDark, "the child inherits the scheme")
        XCTAssertEqual(child.channels("background_color"), [0, 0, 0, 255])
        child.applyBoxLayer()
        XCTAssertEqual(child.layer.backgroundColor?.components?.first ?? -1, 0, accuracy: 0.01, "and re-resolved its fill")
        XCTAssertEqual(material.traitCollection.userInterfaceStyle, .dark, "a material below resolves dark")

        // The property is inherited: a child carrying its parent's scheme
        // overrides nothing, and decides again when the parent's changes.
        child.applyStyle(["background_color": pair, "color_scheme": "dark"])
        XCTAssertEqual(child.overrideUserInterfaceStyle, .unspecified)
        XCTAssertTrue(child.drawsDark)
        sheet.applyStyle(["color_scheme": "light"])
        XCTAssertEqual(child.overrideUserInterfaceStyle, .dark)
        XCTAssertTrue(child.drawsDark, "its own scheme, now another than its parent's")
        sheet.applyStyle(["color_scheme": "dark"])
        XCTAssertEqual(child.overrideUserInterfaceStyle, .unspecified)
        XCTAssertTrue(child.drawsDark)
        // A view styled before it is placed decides where it lands.
        let late = NodeView(id: 3, kind: "view", presenter: p)
        late.applyStyle(["color_scheme": "light"])
        p.views[3] = late; sheet.container.addSubview(late)
        XCTAssertEqual(late.overrideUserInterfaceStyle, .light)
        XCTAssertFalse(late.drawsDark)
        late.removeFromSuperview(); p.views[3] = nil

        // A nested `light` wins below it.
        child.applyStyle(["background_color": pair, "color_scheme": "light"])
        XCTAssertFalse(child.drawsDark)
        XCTAssertEqual(child.channels("background_color"), [255, 255, 255, 255])

        // Left off again: the surrounding (light) scheme.
        child.applyStyle(["background_color": pair])
        sheet.applyStyle([:])
        XCTAssertEqual(sheet.overrideUserInterfaceStyle, .unspecified)
        XCTAssertFalse(child.drawsDark)
        XCTAssertEqual(material.traitCollection.userInterfaceStyle, .light)
    }

    /// A chain of node views carrying one scheme, in a window, each with a
    /// plain view below it that records every appearance it is given.
    @MainActor
    private func chain(_ count: Int) -> (views: [NodeView], seen: () -> [[UIUserInterfaceStyle]], window: UIWindow) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.overrideUserInterfaceStyle = .light
        window.makeKeyAndVisible()
        var views: [NodeView] = [], probes: [UIView] = [], seen: [[UIUserInterfaceStyle]] = Array(repeating: [], count: count)
        for i in 0..<count {
            let v = NodeView(id: UInt32(i + 1), kind: "view", presenter: p)
            p.views[v.id] = v
            if let parent = views.last { parent.container.addSubview(v) } else { window.addSubview(v) }
            v.frame = CGRect(x: 0, y: 0, width: 300, height: 300)
            v.applyStyle(["color_scheme": "dark"])
            let probe = UIView(frame: CGRect(x: 0, y: 0, width: 1, height: 1))
            v.container.addSubview(probe)
            views.append(v); probes.append(probe)
        }
        window.layoutIfNeeded()
        for (i, probe) in probes.enumerated() {
            probe.updateTraitsIfNeeded()
            probe.registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (view: UIView, _: UITraitCollection) in
                seen[i].append(view.traitCollection.userInterfaceStyle)
            }
        }
        return (views, { seen }, window)
    }

    /// In a batch, a child that keeps its parent's old scheme never shows
    /// the parent's new one on its way to overriding it.
    @MainActor
    func testAChildKeepingTheOldSchemeNeverShowsTheNewOne() {
        let (views, seen, window) = chain(2)
        XCTAssertEqual(views[1].overrideUserInterfaceStyle, .unspecified, "it inherits its parent's dark")
        let batch = ColorSchemes.begin()
        views[0].applyStyle(["color_scheme": "light"])
        batch.end()
        XCTAssertEqual(views[1].overrideUserInterfaceStyle, .dark)
        XCTAssertTrue(views[1].drawsDark)
        XCTAssertEqual(seen()[1], [], "never light")
        XCTAssertEqual(seen()[0], [.light])
        withExtendedLifetime(window) {}
    }

    /// A chain whose scheme changes from its root, each node's style in turn
    /// as a batch sends them: each view's appearance changes once.
    @MainActor
    func testAChainChangingItsSchemeUpdatesEachViewOnce() {
        let (views, seen, window) = chain(8)
        let batch = ColorSchemes.begin()
        for v in views { v.applyStyle(["color_scheme": "light"]) }
        batch.end()
        XCTAssertEqual(seen(), Array(repeating: [.light], count: 8))
        XCTAssertEqual(views.map(\.overrideUserInterfaceStyle), [.light] + Array(repeating: .unspecified, count: 7))
        withExtendedLifetime(window) {}
    }

    private var session: ExactSession?
    private var window: UIWindow?
    override func tearDown() {
        window?.isHidden = true; window = nil
        session?.destroy(); session = nil
        super.tearDown()
    }

    /// A dark box moved onto a light parent, with a paragraph and a button
    /// made in it by the same batch: what the batch paints after its ops (the
    /// paragraph's raster, the button's baked ink) is in the box's dark.
    @MainActor
    func testADarkBoxMovedOntoALightParentPaintsDarkInTheSameBatch() throws {
        let session = ExactApp.shared.makeSession(label: "scheme-move")
        self.session = session
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.overrideUserInterfaceStyle = .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        let ink: [Any] = [[0, 0, 0, 255], [255, 255, 255, 255]]
        var face = ButtonFace()
        face.title = "Go"; face.style = "plain"; face.ios = "plain"; face.iosBefore26 = "plain"
        face.rows.title["text_color"] = .array([.array([0, 0, 0, 255]), .array([255, 255, 255, 255])])
        p.buttonFace = { $0 == 3 ? face : ButtonFace() }
        func frame(_ id: Int, y: Double = 0, h: Double = 60) -> [String: Any] { ["op": "frame", "id": id, "x": 0.0, "y": y, "w": 300.0, "h": h] }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["color_scheme": "dark"]], frame(1, h: 150),
            ["op": "create", "id": 2, "kind": "view", "style": ["color_scheme": "light"]], frame(2, y: 160, h: 150),
            ["op": "create", "id": 5, "kind": "view", "style": ["color_scheme": "dark"]], frame(5, h: 150),
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Box"], "style": ["color_scheme": "dark"]], frame(6, y: 110, h: 30),
            ["op": "children", "id": 5, "ids": [6]], ["op": "children", "id": 1, "ids": [5]], ["op": "roots", "ids": [1, 2]],
        ]))
        XCTAssertEqual(try XCTUnwrap(p.views[5]).overrideUserInterfaceStyle, .unspecified, "it inherits its parent's dark")
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"], "handlers": ["press"],
             "style": ["appearance": "auto", "color_scheme": "dark", "text_color": ink]], frame(3, h: 40),
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Ink"], "style": ["font_size": 40, "color_scheme": "dark", "text_color": ink]],
            frame(4, y: 60),
            ["op": "children", "id": 5, "ids": [6, 3, 4]], ["op": "children", "id": 1, "ids": []], ["op": "children", "id": 2, "ids": [5]],
        ]))
        let box = try XCTUnwrap(p.views[5]), text = try XCTUnwrap(p.views[4])
        XCTAssertEqual(box.overrideUserInterfaceStyle, .dark, "dark on a light parent")
        XCTAssertTrue(text.drawsDark)
        let button = try XCTUnwrap(p.controls.controls[3] as? NativeButtonIOS)
        let baked = try XCTUnwrap(button.configuration?.baseForegroundColor?.resolvedColor(with: button.traitCollection))
        var white: CGFloat = 0
        XCTAssertTrue(baked.getWhite(&white, alpha: nil))
        XCTAssertGreaterThan(white, 0.9, "the button's ink is its dark one")
        // The paragraph's raster, painted as the batch ended: its ink is white,
        // as its layer shows it (one colour's coverage is tinted there).
        let raster = try XCTUnwrap(text.textRaster, "painted by the batch")
        let shown = try XCTUnwrap(text.textRasterLayer)
        let w = raster.width, h = raster.height
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: CGFloat(w) / shown.bounds.width, y: -CGFloat(h) / shown.bounds.height)
        shown.render(in: ctx)
        let bytes = try XCTUnwrap(ctx.data).bindMemory(to: UInt8.self, capacity: w * h * 4)
        var inked = 0, bright = 0
        for i in 0..<(w * h) where bytes[i * 4 + 3] == 255 {
            inked += 1
            if bytes[i * 4] > 200 { bright += 1 }
        }
        XCTAssertGreaterThan(inked, 0)
        XCTAssertEqual(bright, inked, "every opaque pixel of its ink is white")
    }
}
#endif
