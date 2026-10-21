#if os(iOS)
import UIKit
import XCTest
import ExactCore
@testable import ExactKit
@testable import ExactGroupedLists

/// Stage 9 A2 on UIKit (LLP 1121 D10): an svg-only icon on a control's face
/// is drawn as the control draws an SF Symbol. Mounted: the native fixture's
/// bar item and root tab with the icon's path written on their image (as the
/// Rust side writes `symbolPath`), a tab bar and segments of native tabs, a
/// button's pixels beside an SF Symbol's in the same configuration, and a
/// grouped list's standard cell beside an SF Symbol's row.
///   bun host/apple/build.mjs --test --ios
final class FaceSymbolIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []
    override func tearDown() {
        MainActor.assumeIsolated {
            for session in sessions { session.destroy() }
            sessions = []
            window?.isHidden = true; window = nil
        }
        super.tearDown()
    }
    private var token: String { FaceSymbolTests.token }
    /// A window in the host's scene, white, light or dark: its pixels are drawn only there.
    private var scene: UIWindowScene? { MainActor.assumeIsolated { UIApplication.shared.connectedScenes.first as? UIWindowScene } }
    @MainActor private func sceneWindow(_ scene: UIWindowScene, dark: Bool = false) -> UIWindow {
        let w = UIWindow(windowScene: scene)
        w.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        w.overrideUserInterfaceStyle = dark ? .dark : .light
        w.backgroundColor = dark ? .black : .white
        w.rootViewController = UIViewController()
        w.rootViewController?.view.backgroundColor = w.backgroundColor
        w.makeKeyAndVisible()
        window = w
        return w
    }
    private func isTemplate(_ image: UIImage?) -> Bool { image?.renderingMode == .alwaysTemplate }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The native fixture booted in a window.
    @MainActor private func fixture(_ label: String) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let w = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        w.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        w.rootViewController = host
        w.makeKeyAndVisible()
        host.view.addSubview(view)
        window = w
        XCTAssertNil(session.boot(plan: plan, size: w.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return session
    }

    /// Writes the icon's path on the first image under each node with this test id prefix.
    @MainActor private func drawPath(_ session: ExactSession, under prefix: String) throws {
        let owners = session.presenter.views.values.filter { $0.props["testId"]?.hasPrefix(prefix) == true }
        XCTAssertFalse(owners.isEmpty, prefix)
        var ops: [[String: Any]] = []
        for owner in owners {
            let image = try XCTUnwrap(owner.container.subviews.compactMap { $0 as? NodeView }.first { $0.kind == "image" }, prefix)
            ops.append(["op": "props", "id": Int(image.id), "set": ["symbolName": "", "symbolPath": FaceSymbolTests.square, "symbolViewBox": "0 0 24 24"], "clear": []])
        }
        session.presenter.apply(wireBatch(ops))
    }

    @MainActor func testABarItemsPathIsItsInstalledTemplateImage() throws {
        let session = try fixture("face-symbol-bar")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        func compose() -> UIBarButtonItem? {
            let item = nav.topViewController?.navigationItem
            return ((item?.rightBarButtonItems ?? []) + (item?.leftBarButtonItems ?? [])).first { $0.accessibilityLabel == "Compose" }
        }
        XCTAssertNotNil(compose()?.image, "the fixture's compose item, an SF Symbol")
        try drawPath(session, under: "compose-")
        until("the item is drawn again from the path") { compose()?.image?.isSymbolImage == false }
        let image = try XCTUnwrap(compose()?.image)
        XCTAssertTrue(isTemplate(image), "the bar tints it")
        XCTAssertEqual(image.size.width, FaceSymbol.barItemPoints)
    }

    /// An iPad's traits at `size`, where a tab bar's glyph depends on the width (`FaceSymbol.tabBarPoints`)
    private func pad(_ size: UIUserInterfaceSizeClass) -> UITraitCollection {
        UITraitCollection(traitsFrom: [UITraitCollection(userInterfaceIdiom: .pad), UITraitCollection(horizontalSizeClass: size)])
    }

    @MainActor func testARootTabsPathIsItsInstalledTabBarItemImage() throws {
        let session = try fixture("face-symbol-tab")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController, "the fixture's root tabs")
        func home() -> UITabBarItem? { tabs.viewControllers?.first?.tabBarItem }
        XCTAssertTrue(home()?.image?.isSymbolImage == true)
        try drawPath(session, under: "tab-home")
        until("the tab bar item is drawn again from the path") { home()?.image?.isSymbolImage == false }
        XCTAssertTrue(isTemplate(home()?.image), "the tab bar tints it")
        XCTAssertEqual(home()?.image?.size.width, FaceSymbol.tabBarPoints(tabs.traitCollection))
        XCTAssertTrue(home()?.selectedImage?.isSymbolImage == false, "no `.fill` variant: the path in both states")
        XCTAssertEqual(home()?.selectedImage?.size, home()?.image?.size)
        XCTAssertEqual(tabs.viewControllers?.first?.tabBarItem.accessibilityIdentifier, "tab-home", "its test id, not the token")
        // An iPhone's glyph is 23 points at both widths
        tabs.tabBar.traitOverrides.userInterfaceIdiom = .phone
        for size in [UIUserInterfaceSizeClass.compact, .regular] {
            tabs.tabBar.traitOverrides.horizontalSizeClass = size
            tabs.tabBar.updateTraitsIfNeeded()
            until("the root tab's path at 23 for an iPhone at \(size.rawValue)") { home()?.image?.size.width == 23 }
        }
        // The idiom alone, at a regular width, sizes it again: an iPad's is 17 points
        tabs.tabBar.traitOverrides.userInterfaceIdiom = .pad
        tabs.tabBar.updateTraitsIfNeeded()
        until("the root tab's path drawn again at 17 for an iPad") { home()?.image?.size.width == 17 }
        // An iPad's size class sizes the glyph: compact, regular, compact again, the faces unchanged.
        for size in [UIUserInterfaceSizeClass.compact, .regular, .compact] {
            tabs.tabBar.traitOverrides.horizontalSizeClass = size
            tabs.tabBar.updateTraitsIfNeeded()
            let points = FaceSymbol.tabBarPoints(pad(size))
            until("the root tab's path drawn again at \(points) for \(size.rawValue)") { home()?.image?.size.width == points }
        }
    }

    /// A presenter of a tablist of native tabs with these faces.
    @MainActor private func tablist(_ faces: [UInt32: ButtonFace], selected: Bool) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = w.bounds
        w.addSubview(p.viewport); w.makeKeyAndVisible()
        window = w
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "props": ["accessibilityRole": "tablist"], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
                                    ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0]]
        for (i, id) in [2, 3].enumerated() {
            var props = ["type": "button", "accessibilityRole": "tab", "accessibilityLabel": ["Reply", "Map"][i]]
            if selected { props["accessibilitySelected"] = i == 0 ? "true" : "false" }
            ops += [["op": "create", "id": id, "kind": "control", "handlers": ["press"], "props": props,
                     "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                    ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 40.0]]
        }
        p.apply(wireBatch(ops + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]]))
        return p
    }
    private func face(_ symbol: String?, title: String? = nil, label: String? = nil) -> ButtonFace {
        var f = ButtonFace(); f.symbol = symbol; f.title = title; f.label = label
        return f
    }

    @MainActor func testNativeTabsPathIsTheirTabBarItemsAndSegmentsImage() throws {
        var p = tablist([2: face(token, title: "Reply"), 3: face("tray", title: "Inbox")], selected: true)
        let tabBar = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UITabBar }.first)
        let items = try XCTUnwrap(tabBar.items)
        XCTAssertTrue(isTemplate(items[0].image), "the tab bar tints it")
        // Sized for the bar's own traits: a phone's at either width, an iPad's at its size class
        XCTAssertEqual(items[0].image?.size.width, FaceSymbol.tabBarPoints(tabBar.traitCollection))
        XCTAssertTrue(items[1].image?.isSymbolImage == true, "an SF Symbol as before")
        // An iPhone's glyph is 23 points at both widths
        tabBar.traitOverrides.userInterfaceIdiom = .phone
        for size in [UIUserInterfaceSizeClass.compact, .regular] {
            tabBar.traitOverrides.horizontalSizeClass = size
            tabBar.updateTraitsIfNeeded()
            XCTAssertEqual(tabBar.items?.first?.image?.size.width, 23, "an iPhone at \(size.rawValue)")
        }
        // The idiom alone, at a regular width, sizes it again: an iPad's is 17 points
        tabBar.traitOverrides.userInterfaceIdiom = .pad
        tabBar.updateTraitsIfNeeded()
        XCTAssertEqual(tabBar.items?.first?.image?.size.width, 17, "an iPad at a regular width")
        // An iPad's size class sizes the glyph: compact, regular, compact again, the faces unchanged.
        for size in [UIUserInterfaceSizeClass.compact, .regular, .compact] {
            tabBar.traitOverrides.horizontalSizeClass = size
            tabBar.updateTraitsIfNeeded()
            XCTAssertEqual(tabBar.items?.first?.image?.size.width, FaceSymbol.tabBarPoints(pad(size)), "\(size.rawValue)")
        }
        p = tablist([2: face(token, label: "Reply"), 3: face("map", label: "Map")], selected: false)
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? UISegmentedControl }.first)
        let image = try XCTUnwrap(segments.imageForSegment(at: 0), "the path, not an empty image")
        XCTAssertTrue(isTemplate(image), "the control tints it")
        XCTAssertEqual(image.size.width, FaceSymbol.defaultPoints.rounded(.up))
        XCTAssertEqual(image.accessibilityLabel, "Reply")
        XCTAssertTrue(segments.imageForSegment(at: 1)?.isSymbolImage == true)
        // Drawn again at the body size as the text size changes, the faces unchanged.
        segments.traitOverrides.preferredContentSizeCategory = .accessibilityExtraLarge
        segments.updateTraitsIfNeeded()
        let grown = UIFont.preferredFont(forTextStyle: .body, compatibleWith: segments.traitCollection).pointSize.rounded(.up)
        XCTAssertGreaterThan(grown, FaceSymbol.defaultPoints + 10)
        XCTAssertEqual(segments.imageForSegment(at: 0)?.size.width, grown, "the path at the control's body size")
        // The agent reads the platform name; for a path, the icon token's name.
        let native = Presenter()
        native.buttonFace = { _ in var f = self.face(self.token, title: "Reply"); f.symbolIcon = "icon.reply"; return f }
        native.apply(wireBatch([["op": "create", "id": 1, "kind": "control", "handlers": ["press"], "props": ["type": "button", "accessibilityRole": "button"],
                                 "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0], ["op": "roots", "ids": [1]]]))
        let button = try XCTUnwrap(native.controls.controls[1] as? NativeButtonIOS)
        XCTAssertEqual(native.controls.nativeObservation(button)["symbol"] as? String, "icon.reply", "the icon's name, not the token")
    }

    // MARK: - The sites' sizes, measured

    /// The inked box (dark pixels on the white window) of each region of `w`'s drawing, in points.
    @MainActor private func inked(_ w: UIWindow, in regions: [CGRect]) throws -> [CGRect] {
        let format = UIGraphicsImageRendererFormat(); format.scale = 2; format.opaque = true
        let shot = try XCTUnwrap(UIGraphicsImageRenderer(bounds: w.bounds, format: format).image { _ in w.drawHierarchy(in: w.bounds, afterScreenUpdates: true) }.cgImage)
        return try regions.map { try inked(shot, in: CGRect(x: $0.minX * 2, y: $0.minY * 2, width: $0.width * 2, height: $0.height * 2), scale: 2) }
    }
    private func inked(_ image: CGImage, in region: CGRect, scale: CGFloat) throws -> CGRect {
        let wide = Int(region.width), high = Int(region.height)
        var px = [UInt8](repeating: 0, count: wide * high * 4)
        let ctx = try XCTUnwrap(CGContext(data: &px, width: wide, height: high, bitsPerComponent: 8, bytesPerRow: wide * 4, space: CGColorSpaceCreateDeviceRGB(),
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(UIColor.white.cgColor); ctx.fill(CGRect(x: 0, y: 0, width: wide, height: high))
        ctx.draw(image, in: CGRect(x: -region.minX, y: region.maxY - CGFloat(image.height), width: CGFloat(image.width), height: CGFloat(image.height)))
        var box = CGRect.null
        for y in 0..<high { for x in 0..<wide {
            let i = (y * wide + x) * 4
            guard Int(px[i]) + Int(px[i + 1]) + Int(px[i + 2]) < 300 else { continue } // the black glyph, not a grey platter
            box = box.union(CGRect(x: x, y: y, width: 1, height: 1))
        } }
        return CGRect(x: box.minX / scale, y: box.minY / scale, width: box.width / scale, height: box.height / scale)
    }

    /// Where a site gives a symbol no configuration, UIKit sizes it: an SF Symbol ("square") and the path (a
    /// square 20/24 of its em) side by side in a tab bar, a navigation bar and a segmented control, each glyph's
    /// inked box measured; the symbol's point size is its box against the same symbol drawn at 100 points,
    /// which the path's em square must be.
    @MainActor func testTheSitesPathSizesAreTheSymbolSizesUIKitShows() throws {
        let square = try XCTUnwrap(UIImage(systemName: "square"))
        let big = try XCTUnwrap(UIImage(systemName: "square", withConfiguration: UIImage.SymbolConfiguration(pointSize: 100)))
        let reference = try XCTUnwrap(UIGraphicsImageRenderer(size: big.size, format: { let f = UIGraphicsImageRendererFormat(); f.scale = 1; f.opaque = true; return f }())
            .image { c in UIColor.white.setFill(); c.fill(CGRect(origin: .zero, size: big.size)); big.withTintColor(.black).draw(at: .zero) }.cgImage)
        let perPoint = try inked(reference, in: CGRect(origin: .zero, size: big.size), scale: 1).height / 100
        guard let scene else { throw missing("scene", "a window's pixels are drawn only in a scene") }
        let w = sceneWindow(scene)
        w.tintColor = .black
        let host = try XCTUnwrap(w.rootViewController?.view)
        func views(_ v: UIView) -> [UIImageView] { ((v as? UIImageView).flatMap { $0.image == nil ? nil : [$0] } ?? []) + v.subviews.flatMap(views) }
        func measure(_ site: String, _ root: UIView, expected: CGFloat) throws {
            w.layoutIfNeeded(); root.layoutIfNeeded()
            let found = views(root)
            let symbol = try XCTUnwrap(found.first { $0.image?.isSymbolImage == true }, "\(site): the symbol's view")
            let path = try XCTUnwrap(found.first { $0.image?.isSymbolImage == false && $0.image?.renderingMode == .alwaysTemplate }, "\(site): the path's view")
            let boxes = try inked(w, in: [symbol, path].map { $0.convert($0.bounds, to: w).insetBy(dx: -2, dy: -2) })
            let points = boxes[0].height / perPoint
            print("MEASURE ios \(site): symbol view \(symbol.bounds.size) ink \(boxes[0].size) -> \(points) pt; path view \(path.bounds.size) ink \(boxes[1].size) (em \(expected), square 20/24 of it)")
            XCTAssertEqual(expected, points.rounded(), accuracy: 1, "\(site): symbol \(points) pt, path em \(expected)")
            XCTAssertEqual(boxes[1].height, expected * 20 / 24, accuracy: 1.5, "\(site): the path drawn at its em, not scaled")
        }
        let tabs = UITabBar(frame: CGRect(x: 0, y: 700, width: 402, height: 83))
        tabs.tintColor = .black; tabs.unselectedItemTintColor = .black
        tabs.traitOverrides.horizontalSizeClass = .compact
        let compact = FaceSymbol.tabBarPoints(UITraitCollection(horizontalSizeClass: .compact))
        tabs.items = [UITabBarItem(title: "A", image: square, tag: 0), UITabBarItem(title: "B", image: FaceSymbol.image(token, points: compact), tag: 1)]
        host.addSubview(tabs)
        try measure("tab bar", tabs, expected: compact)
        // A standalone tab bar given an iPad's idiom and a regular width lays its items out inline, its symbols smaller, as
        // an iPad's inline items would. Where the system places a vertical bar (the iPhone Duo), UIKit keeps an iPhone's
        // symbols under those traits, so there is none to measure
        if !host.traitCollection.placesVerticalBar {
            let wide = UITabBar(frame: CGRect(x: 0, y: 600, width: 402, height: 50))
            wide.traitOverrides.userInterfaceIdiom = .pad
            wide.traitOverrides.horizontalSizeClass = .regular
            wide.tintColor = .black; wide.unselectedItemTintColor = .black
            let regular = FaceSymbol.tabBarPoints(pad(.regular))
            wide.items = [UITabBarItem(title: "A", image: square, tag: 0), UITabBarItem(title: "B", image: FaceSymbol.image(token, points: regular), tag: 1)]
            host.addSubview(wide)
            try measure("tab bar (an iPad's, regular width)", wide, expected: regular)
            wide.removeFromSuperview()
        }
        let bar = UINavigationBar(frame: CGRect(x: 0, y: 50, width: 402, height: 44))
        let item = UINavigationItem(title: "")
        item.rightBarButtonItems = [UIBarButtonItem(image: square, style: .plain, target: nil, action: nil),
                                    UIBarButtonItem(image: FaceSymbol.image(token, points: FaceSymbol.barItemPoints), style: .plain, target: nil, action: nil)]
        bar.items = [item]
        host.addSubview(bar)
        try measure("bar item", bar, expected: FaceSymbol.barItemPoints)
        let segments = UISegmentedControl(items: [square, try XCTUnwrap(FaceSymbol.image(token))])
        segments.frame = CGRect(x: 20, y: 200, width: 200, height: 32)
        host.addSubview(segments)
        try measure("segment", segments, expected: FaceSymbol.defaultPoints)
    }

    // MARK: - A button's pixels

    /// The colour at the middle of the button's image as drawn in `w`.
    @MainActor private func ink(_ button: UIButton, in w: UIWindow) throws -> [CGFloat] {
        w.layoutIfNeeded(); button.layoutIfNeeded()
        let view = try XCTUnwrap(button.imageView)
        let format = UIGraphicsImageRendererFormat(); format.scale = 1; format.opaque = true
        let shot = UIGraphicsImageRenderer(bounds: w.bounds, format: format).image { _ in w.drawHierarchy(in: w.bounds, afterScreenUpdates: true) }
        let at = view.convert(CGPoint(x: view.bounds.midX, y: view.bounds.midY), to: w)
        // The ground (the window's corner) is the window's colour: a drawing, not a blank capture.
        var ground = [UInt8](repeating: 0, count: 4)
        let groundContext = try XCTUnwrap(CGContext(data: &ground, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: CGColorSpaceCreateDeviceRGB(),
                                                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        groundContext.draw(try XCTUnwrap(shot.cgImage), in: CGRect(x: -2, y: 3 - w.bounds.height, width: w.bounds.width, height: w.bounds.height))
        let expected: UInt8 = w.overrideUserInterfaceStyle == .dark ? 0 : 255
        XCTAssertEqual(ground.prefix(3).map { abs(Int($0) - Int(expected)) < 8 }, [true, true, true], "a drawn window: \(ground)")
        var px = [UInt8](repeating: 0, count: 4)
        let ctx = try XCTUnwrap(CGContext(data: &px, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: CGColorSpaceCreateDeviceRGB(),
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(try XCTUnwrap(shot.cgImage), in: CGRect(x: -floor(at.x), y: floor(at.y) - w.bounds.height + 1, width: w.bounds.width, height: w.bounds.height))
        return px.prefix(3).map { CGFloat($0) / 255 }
    }

    /// An unsaid path is drawn in the colour UIKit draws an SF Symbol in the same configuration: plain, filled
    /// (white), gray, light and dark, disabled, and glass (its ink) where the design draws glass.
    @MainActor func testAnUnsaidPathsPixelsAreAnSFSymbolsInTheSameButton() throws {
        guard let scene else { throw missing("scene", "a button's pixels are drawn only in a scene's window") }
        var styles = ["plain", "filled", "gray", "tinted"]
        if #available(iOS 26.0, *), LinkedDesign.liquidGlass { styles += ["glass", "prominentGlass"] }
        for style in styles {
            for dark in [false, true] {
                for enabled in [true, false] {
                    let w = sceneWindow(scene, dark: dark)
                    let host = try XCTUnwrap(w.rootViewController?.view)
                    let traits = UITraitCollection(userInterfaceStyle: dark ? .dark : .light)
                    var made: [UIButton] = []
                    for (i, symbol) in [token, "square.fill"].enumerated() {
                        var f = face(symbol, title: "Go"); f.ios = style; f.iosBefore26 = style
                        let b = NativeButtonIOS(configuration: ControlHost.configuration(f).0)
                        b.isEnabled = enabled
                        b.frame = CGRect(x: 20, y: 20 + CGFloat(i) * 80, width: 160, height: 60)
                        host.addSubview(b)
                        ButtonConfigurationIOS.apply(f, to: b, traits: traits, accent: nil)
                        made.append(b)
                    }
                    let path = try ink(made[0], in: w), symbol = try ink(made[1], in: w)
                    let label = "\(style) \(dark ? "dark" : "light") \(enabled ? "enabled" : "disabled"): \(path) vs \(symbol)"
                    for (a, b) in zip(path, symbol) { XCTAssertEqual(a, b, accuracy: 0.08, label) }
                    w.isHidden = true
                }
            }
        }
    }

    /// A symbol alone is sized by UIKit, `buttonSize` too: the path takes the size UIKit resolves for a symbol.
    @MainActor func testASymbolAlonePathFollowsTheButtonSizeAsASymbolDoes() throws {
        let known = try XCTUnwrap(UIImage(systemName: "square", withConfiguration: UIImage.SymbolConfiguration(pointSize: 100))).size.height
        var sides: [String: CGFloat] = [:]
        for size in ["regular", "large", "small"] {
            var f = face(token); f.ios = "bordered"
            if size != "regular" { f.rows.button["control_size"] = .string(size) }
            let b = UIButton(configuration: ControlHost.configuration(f).0)
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            let side = try XCTUnwrap(b.configuration?.image?.size.width)
            // The reference: UIKit's own symbol in that configuration, its point size by the same measure.
            var symbol = f; symbol.symbol = "square"
            let r = UIButton(configuration: ControlHost.configuration(symbol).0)
            ButtonConfigurationIOS.apply(symbol, to: r, traits: r.traitCollection, accent: nil)
            r.sizeToFit(); r.layoutIfNeeded()
            let shown = try XCTUnwrap(r.imageView?.bounds.height)
            print("MEASURE ios button \(size): symbol box \(shown) -> \(shown / known * 100) pt; path side \(side)")
            XCTAssertEqual(side, (shown / known * 100).rounded(.up), accuracy: 1, size)
            sides[size] = side
        }
        XCTAssertGreaterThanOrEqual(sides["large"]!, sides["regular"]!, "as UIKit sizes the symbol")
        XCTAssertLessThanOrEqual(sides["small"]!, sides["regular"]!)
    }
}

/// A grouped list's standard cell led by an svg-only icon, beside an SF Symbol's row.
final class FaceSymbolGroupedTests: GroupedListFixture {
    private func cells(_ m: @escaping () -> GroupedListModel) throws -> (Presenter, UICollectionViewListCell, UICollectionViewListCell) {
        let p = presenter(m)
        return (p, try cell(p, 10), try cell(p, 11))
    }
    private func pathModel(destructive: Bool = false, disabled: Bool = false) -> GroupedListModel {
        var m = model()
        m.sections[0].rows[0] = Row(view: 10, symbol: FaceSymbolTests.token, title: "Reply", pressable: true, destructive: destructive, disabled: disabled)
        m.sections[0].rows[1] = Row(view: 11, symbol: "bell", title: "Notifications", pressable: true)
        return m
    }

    func testTheRowsJSONsPathIsItsSymbolsToken() throws {
        let json: [String: Any] = ["style": "inset-grouped", "sections": [["view": 2, "rows": [
            ["view": 10, "custom": false, "symbol": "", "symbolPath": FaceSymbolTests.square, "symbolViewBox": "0 0 24 24", "title": "Reply"],
            ["view": 11, "custom": false, "symbol": "bell", "title": "Notifications"]]]]]
        let decoded = try XCTUnwrap(GroupedListModel(json: try JSONSerialization.data(withJSONObject: json)))
        XCTAssertEqual(decoded.sections[0].rows[0].symbol, FaceSymbolTests.token)
        XCTAssertEqual(decoded.sections[0].rows[1].symbol, "bell")
    }

    /// The symbol's standard space (titles aligned with an SF Symbol's row) and the label colour untinted.
    func testAPathRowIsLaidOutAndTintedAsASymbolsRow() throws {
        let (_, reply, bell) = try cells { self.pathModel() }
        let c = try XCTUnwrap(reply.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(c.image?.renderingMode, .alwaysTemplate)
        XCTAssertEqual(c.imageProperties.reservedLayoutSize.width, UIListContentConfiguration.ImageProperties.standardDimension)
        let label = try XCTUnwrap(c.imageProperties.tintColor, "untinted, the label colour, as a symbol takes it")
        XCTAssertEqual(label.resolvedColor(with: reply.traitCollection), UIColor.label.resolvedColor(with: reply.traitCollection))
        func titleX(_ cell: UICollectionViewListCell, _ text: String) throws -> CGFloat {
            cell.layoutIfNeeded()
            func labels(_ v: UIView) -> [UILabel] { (v as? UILabel).map { [$0] } ?? v.subviews.flatMap(labels) }
            let found = try XCTUnwrap(labels(cell.contentView).first { $0.text == text }, text)
            return found.convert(found.bounds, to: cell.contentView).minX
        }
        XCTAssertEqual(try titleX(reply, "Reply"), try titleX(bell, "Notifications"), accuracy: 0.5, "the titles align as beside a symbol")
        // A destructive row's path is red, a disabled one's grey, as a symbol's.
        let red = try XCTUnwrap((try cells { self.pathModel(destructive: true) }.1.contentConfiguration as? UIListContentConfiguration)?.imageProperties.tintColor)
        XCTAssertEqual(red.resolvedColor(with: reply.traitCollection), UIColor.systemRed.resolvedColor(with: reply.traitCollection))
        let grey = try XCTUnwrap((try cells { self.pathModel(disabled: true) }.1.contentConfiguration as? UIListContentConfiguration)?.imageProperties.tintColor)
        XCTAssertEqual(grey.resolvedColor(with: reply.traitCollection), UIColor.tertiaryLabel.resolvedColor(with: reply.traitCollection))
    }

    /// A path in the accent (`AccentColor`, as GroupedListIOSTests' testASymbolInTheAccentTakesTheCellsTint has
    /// a symbol, its file at its cap): no colour of its own, so the cell's tint, not the label, as it changes.
    /// The same for an untinted path whose written `color` is `AccentColor`
    /// (LLP 1121 stage 9 B: the colour it takes is the accent's).
    func testAPathInTheAccentTakesTheCellsTint() throws {
        let accent: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        try assertTheCellsTint(props: [:], style: ["tint_color": accent])
        try assertTheCellsTint(props: ["symbolColorWritten": "1"], style: ["text_color": accent])
    }
    private func assertTheCellsTint(props: [String: Any], style: [String: Any], file: StaticString = #filePath, line: UInt = #line) throws {
        let p = presenter { self.pathModel() }
        let source: [String: Any] = ["imageSource": "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg"]
        p.apply(wireBatch([
            ["op": "create", "id": 31, "kind": "image", "props": source.merging(props) { $1 }, "style": style],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Reply"], "style": [:]],
            ["op": "children", "id": 10, "ids": [31, 33]],
        ]))
        let row = try cell(p, 10)
        XCTAssertNil((row.contentConfiguration as? UIListContentConfiguration)?.imageProperties.tintColor, "the accent is the cell's tint", file: file, line: line)
        let light = UITraitCollection(userInterfaceStyle: .light)
        func shown() throws -> UIColor {
            row.layoutIfNeeded()
            func images(_ v: UIView) -> [UIImageView] { ((v as? UIImageView).map { [$0] } ?? []) + v.subviews.flatMap(images) }
            return try XCTUnwrap(images(row.contentView).first { $0.image?.renderingMode == .alwaysTemplate }, "the path's view", file: file, line: line).tintColor
        }
        for accent in [UIColor.systemTeal, .systemPink] {
            window.tintColor = accent
            XCTAssertEqual(try shown().resolvedColor(with: light), accent.resolvedColor(with: light), "drawn in the accent as it changes", file: file, line: line)
        }
    }

    /// Its size is the cell's Dynamic Type size, drawn again as that changes with no change to the row.
    func testAPathRowFollowsDynamicType() throws {
        let (_, reply, _) = try cells { self.pathModel() }
        let before = try XCTUnwrap((reply.contentConfiguration as? UIListContentConfiguration)?.image?.size.width)
        XCTAssertEqual(before, GroupedCell.pathPoints(reply.traitCollection).rounded(.up))
        reply.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
        reply.updateTraitsIfNeeded()
        let after = try XCTUnwrap((reply.contentConfiguration as? UIListContentConfiguration)?.image?.size.width)
        XCTAssertGreaterThan(after, before + 10, "grown with the text")
        XCTAssertEqual(after, GroupedCell.pathPoints(reply.traitCollection).rounded(.up))
    }
}
#endif
