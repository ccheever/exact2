#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// What a status screen's views draw behind an alert, read from their pixels: the compiled fixture
/// `dimmed-pixels.contract` beside hand-built UIKit twins 200 points to its right, drawn before,
/// behind and after a `UIAlertController`. A colour of its own keeps it, the tint greys as UIKit
/// greys it, and a filled button greys whole, as one whose symbol's colour is its configuration's
/// (`imageColorTransformer`) does (HIG, Color; `UIView.tintAdjustmentMode`). The class stages the
/// screen once, as its tests only read the three pictures. UIKit in a scene's window, so the hosted
/// bundle runs it: bun host/apple/build.mjs --test --ios
final class DimmedPixelsIOSTests: XCTestCase {
    nonisolated(unsafe) private static var staged: Stage?
    nonisolated(unsafe) private static var window: UIWindow?
    nonisolated(unsafe) private static var session: ExactSession?
    private let light = UITraitCollection(userInterfaceStyle: .light)

    override class func tearDown() {
        unstage()
        super.tearDown()
    }

    private static func unstage() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        staged = nil
    }

    /// A mark, a status, a badge's wash and symbol and a switch whose accent is UIKit's (its green) keep
    /// their colours behind the alert; the switch is its twin's, pixel for pixel
    func testBehindAnAlertAColourOfItsOwnKeepsIt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        let indigo = rgb(.systemIndigo), green = rgb(.systemGreen), blue = rgb(.systemBlue)
        let badge = try s.frame("badge"), track = try s.frame("switch"), twinTrack = try s.frame("switch", twin: true)
        for (picture, when) in [(s.before, "before the alert"), (s.behind, "behind it"), (s.after, "after it")] {
            assertPixel(picture.at(try s.centre("own-symbol")), indigo, "the mark \(when)")
            assertPixel(picture.ink(in: try s.frame("own-text")), green, "the status \(when)")
            assertPixel(picture.at(CGPoint(x: badge.minX + 4, y: badge.midY)), [217, 235, 255], "the badge's wash \(when)")
            assertPixel(picture.at(CGPoint(x: badge.midX, y: badge.midY)), blue, "the badge's symbol \(when)")
            let on = picture.at(CGPoint(x: track.minX + track.width * 0.3, y: track.midY))
            assertPixel(on, picture.at(CGPoint(x: twinTrack.minX + twinTrack.width * 0.3, y: twinTrack.midY)), "the switch's track \(when) is its twin's")
            assertPixel(on, green, "the switch's track \(when)")
        }
    }

    /// A mark, words and a slider's track in the tint are the red tint, and behind the alert UIKit's
    /// grey, their twins' pixel for pixel: a template symbol, a label in `UIColor.tintColor` and a slider
    func testBehindAnAlertTheTintGreysAsUIKitGreysIt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        let red = rgb(.systemRed)
        let sites: [(String, (Picture, Bool) throws -> [Int])] = [
            ("the mark", { p, twin in try p.at(s.centre("tint-symbol", twin: twin)) }),
            ("the words", { p, twin in try p.ink(in: s.frame("tint-text", twin: twin)) }),
            ("the slider's track", { p, twin in let f = try s.frame("slider", twin: twin); return p.at(CGPoint(x: f.minX + 8, y: f.midY)) }),
        ]
        for (site, read) in sites {
            assertPixel(try read(s.before, false), red, "\(site) before the alert")
            assertPixel(try read(s.before, true), red, "\(site)'s twin before the alert")
            let behind = try read(s.behind, false), twin = try read(s.behind, true)
            assertPixel(behind, twin, "\(site) behind the alert is its twin's")
            XCTAssertLessThanOrEqual(spread(twin), 3, "\(site)'s twin is grey behind the alert: \(twin)")
            assertPixel(try read(s.after, false), red, "\(site) after the alert")
        }
    }

    /// A list row's symbol in a colour of its own keeps it behind the alert, and one in the tint greys
    /// as the twin template symbol does, as UIKit draws a cell's image in the cell's tint
    func testBehindAnAlertAListRowsSymbolKeepsItsOwnColourAndTheTintGreys() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        let own = try s.rowSymbol("own-row"), tint = try s.rowSymbol("tint-row")
        for (picture, when) in [(s.before, "before the alert"), (s.behind, "behind it"), (s.after, "after it")] {
            assertPixel(picture.at(own), rgb(.systemIndigo), "the row's own colour \(when)")
        }
        assertPixel(s.before.at(tint), rgb(.systemRed), "the row's tint before the alert")
        assertPixel(s.behind.at(tint), s.behind.at(try s.centre("tint-symbol", twin: true)), "the row's tint behind the alert is the twin symbol's grey")
        assertPixel(s.after.at(tint), rgb(.systemRed), "the row's tint after the alert")
    }

    /// An `accent-color` written below the roots' greys on a control behind the alert as UIKit greys a
    /// tint, its twin's under a teal container pixel for pixel: a switch, a slider, and a list row's
    /// switch, which UIKit draws in a cell under the list's view and so in the list's tint
    func testBehindAnAlertAnAuthoredAccentOnAControlGreysAsUIKitGreysIt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        func track(_ f: CGRect) -> CGPoint { CGPoint(x: f.minX + f.width * 0.3, y: f.midY) }
        func start(_ f: CGRect) -> CGPoint { CGPoint(x: f.minX + 8, y: f.midY) }
        let twinSwitch = track(try s.frame("teal-switch", twin: true))
        let sites = [("the switch", track(try s.frame("teal-switch")), twinSwitch),
                     ("the slider's track", start(try s.frame("teal-slider")), start(try s.frame("teal-slider", twin: true))),
                     ("the list row's switch", track(try s.rowSwitch("teal-row")), twinSwitch)]
        for (site, point, twin) in sites {
            assertPixel(s.before.at(point), rgb(.systemTeal), "\(site) before the alert")
            assertPixel(s.behind.at(point), s.behind.at(twin), "\(site) behind the alert is its twin's")
            XCTAssertLessThanOrEqual(spread(s.behind.at(twin)), 3, "\(site)'s twin is grey behind the alert: \(s.behind.at(twin))")
            assertPixel(s.after.at(point), rgb(.systemTeal), "\(site) after the alert")
        }
    }

    /// `-exact-tint-adjustment="normal"` keeps the tint as it is behind the alert, as UIKit's `.normal`
    /// keeps its twin's: a mark in the tint and a list row's symbol in it, the column's teal throughout
    func testBehindAnAlertATintKeptNormalStaysAsItIs() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        let mark = try s.centre("kept-symbol"), twin = try s.centre("kept-symbol", twin: true), row = try s.rowSymbol("kept-row")
        for (picture, when) in [(s.before, "before the alert"), (s.behind, "behind it"), (s.after, "after it")] {
            assertPixel(picture.at(mark), rgb(.systemTeal), "the mark \(when)")
            assertPixel(picture.at(twin), rgb(.systemTeal), "its twin \(when)")
            assertPixel(picture.at(row), rgb(.systemTeal), "the row's symbol \(when)")
        }
    }

    /// A filled button's face, label and symbol in a colour of its own (a remote control's) are a
    /// hand-built `UIButton.Configuration.filled()`'s before, behind and after the alert: UIKit greys
    /// the whole button, its symbol's green too
    func testBehindAnAlertANativeButtonGreysAsAHandBuiltOneDoes() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "an alert presents only in a scene's window") }
        let s = try stage()
        func face(_ twin: Bool) throws -> CGPoint { let f = try s.frame("native", twin: twin); return CGPoint(x: f.midX, y: f.minY + 3) }
        for (picture, when) in [(s.before, "before the alert"), (s.behind, "behind it"), (s.after, "after it")] {
            assertPixel(picture.at(try face(false)), picture.at(try face(true)), "the face \(when) is the twin's")
            for part in ["imageView", "titleLabel"] {
                assertPixel(picture.ink(in: try s.part(part, "native")), picture.ink(in: try s.part(part, "native", twin: true)), "the button's \(part) \(when) is the twin's")
            }
        }
        let symbol = try s.part("imageView", "native")
        assertPixel(s.before.ink(in: symbol), rgb(.systemGreen), "the symbol before the alert")
        XCTAssertGreaterThan(distance(s.behind.ink(in: try s.part("imageView", "native", twin: true)), rgb(.systemGreen)), 60, "UIKit greys the twin's green symbol behind the alert")
        assertPixel(s.after.ink(in: symbol), rgb(.systemGreen), "the symbol after the alert")
    }

    // MARK: - The stage

    /// The fixture and its twins (by the test id of the node each stands beside) drawn before an alert,
    /// behind it and after it
    private struct Stage {
        let before: Picture, behind: Picture, after: Picture
        let views: [String: UIView], twins: [String: UIView], cells: [String: GroupedCell]
        let root: UIView
        func frame(_ testId: String, twin: Bool = false) throws -> CGRect {
            let view = try XCTUnwrap((twin ? twins : views)[testId], "the stage has \(testId)\(twin ? "'s twin" : "")")
            return view.convert(view.bounds, to: root)
        }
        func centre(_ testId: String, twin: Bool = false) throws -> CGPoint {
            let f = try frame(testId, twin: twin)
            return CGPoint(x: f.midX, y: f.midY)
        }
        /// The centre of the symbol a list row's cell draws (`UIListContentView.imageLayoutGuide`)
        func rowSymbol(_ testId: String) throws -> CGPoint {
            let cell = try XCTUnwrap(cells[testId], "the list shows \(testId)")
            let content = try XCTUnwrap(cell.contentView as? UIListContentView, "\(testId) is a standard row")
            let guide = try XCTUnwrap(content.imageLayoutGuide, "\(testId) shows a symbol")
            let f = content.convert(guide.layoutFrame, to: root)
            return CGPoint(x: f.midX, y: f.midY)
        }
        /// The switch UIKit shows as a list row's accessory
        func rowSwitch(_ testId: String) throws -> CGRect {
            func first(_ view: UIView) -> UISwitch? { view as? UISwitch ?? view.subviews.lazy.compactMap(first).first }
            let toggle = try XCTUnwrap(cells[testId].flatMap(first), "\(testId) shows a switch")
            return toggle.convert(toggle.bounds, to: root)
        }
        /// A button's image view or title label
        func part(_ part: String, _ testId: String, twin: Bool = false) throws -> CGRect {
            let button = try XCTUnwrap((twin ? twins : views)[testId] as? UIButton, "\(testId) is a button")
            let view = try XCTUnwrap(part == "imageView" ? button.imageView : button.titleLabel, "\(testId) shows its \(part)")
            return view.convert(view.bounds, to: root)
        }
    }

    private func stage() throws -> Stage {
        if let staged = Self.staged { return staged }
        // A stage an earlier test failed to finish goes before this one starts
        Self.unstage()
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_DIMMED_PIXELS_PLAN"], "the hosted run compiles the fixture")
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first,
                                  "an alert presents only in a scene's window: run the hosted bundle")
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 660)
        window.overrideUserInterfaceStyle = .light
        let controller = UIViewController()
        controller.view.backgroundColor = .white
        window.rootViewController = controller
        window.makeKeyAndVisible()
        Self.window = window
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "dimmed-pixels")
        Self.session = session
        session.presenter.viewport.frame = controller.view.bounds
        controller.view.addSubview(session.presenter.viewport)
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: controller.view.bounds.size).error)
        window.layoutIfNeeded()
        var views: [String: UIView] = [:]
        for node in session.presenter.views.values {
            guard let id = node.props["testId"] else { continue }
            views[id] = session.presenter.controls.controls[node.id] ?? node
        }
        let twins = twins(in: controller.view, beside: views)
        let before = settled(controller.view)
        var cells: [String: GroupedCell] = [:]
        func collect(_ view: UIView) {
            if let cell = view as? GroupedCell, let id = cell.row.flatMap({ session.presenter.views[$0]?.props["testId"] }) { cells[id] = cell }
            view.subviews.forEach(collect)
        }
        collect(controller.view)
        controller.present(UIAlertController(title: "Lock?", message: nil, preferredStyle: .alert), animated: false)
        until("UIKit dims the screen under the alert") { controller.view.tintAdjustmentMode == .dimmed }
        let behind = settled(controller.view)
        var dismissed = false
        controller.dismiss(animated: false) { dismissed = true }
        until("UIKit dismisses the alert and restores the screen's tint") { dismissed && controller.view.tintAdjustmentMode != .dimmed }
        let staged = Stage(before: before, behind: behind, after: settled(controller.view), views: views, twins: twins, cells: cells, root: controller.view)
        Self.staged = staged
        return staged
    }

    /// `view`'s picture once two in a row, 50 ms apart, are the same: a control takes its natural size a
    /// pass after the batch that makes it
    private func settled(_ view: UIView) -> Picture {
        var last = Picture(view), next = last
        until("\(type(of: view)) holds still") {
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            (last, next) = (next, Picture(view))
            return last == next
        }
        return next
    }

    /// Hand-built UIKit 200 points right of the fixture's nodes, under the fixture's red tint or the
    /// teal accent its `teal` column writes, by the test id of the node each stands beside
    private func twins(in root: UIView, beside views: [String: UIView]) -> [String: UIView] {
        let red = UIView(frame: root.bounds), teal = UIView(frame: root.bounds)
        red.tintColor = .systemRed
        teal.tintColor = .systemTeal
        root.addSubview(red)
        root.addSubview(teal)
        var twins: [String: UIView] = [:]
        func place(_ view: UIView, _ testId: String, in container: UIView? = nil) {
            guard let node = views[testId] else { return XCTFail("the fixture has \(testId)") }
            view.frame = node.convert(node.bounds, to: root).offsetBy(dx: 200, dy: 0)
            (container ?? red).addSubview(view)
            twins[testId] = view
        }
        let symbol = UIImageView(image: UIImage(systemName: "square.fill", withConfiguration: UIImage.SymbolConfiguration(pointSize: 24)))
        symbol.contentMode = .center
        place(symbol, "tint-symbol")
        let words = UILabel()
        words.text = "Locked"
        words.font = .systemFont(ofSize: 28, weight: .black)
        words.textColor = .tintColor
        place(words, "tint-text")
        let toggle = UISwitch()
        toggle.isOn = true
        place(toggle, "switch")
        let slider = UISlider()
        slider.value = 0.5
        place(slider, "slider")
        var configuration = UIButton.Configuration.filled()
        configuration.baseBackgroundColor = .quaternarySystemFill
        configuration.baseForegroundColor = .label
        configuration.image = UIImage(systemName: "lock.fill")
        configuration.imageColorTransformer = UIConfigurationColorTransformer { _ in .systemGreen }
        configuration.title = "Lock"
        place(UIButton(configuration: configuration), "native")
        // A hand-built switch takes an accent as its on colour; UIKit's own is green
        let accented = UISwitch()
        accented.isOn = true
        accented.onTintColor = .tintColor
        place(accented, "teal-switch", in: teal)
        let tealSlider = UISlider()
        tealSlider.value = 0.5
        place(tealSlider, "teal-slider", in: teal)
        let kept = UIImageView(image: UIImage(systemName: "square.fill", withConfiguration: UIImage.SymbolConfiguration(pointSize: 24)))
        kept.contentMode = .center
        kept.tintAdjustmentMode = .normal
        place(kept, "kept-symbol", in: teal)
        root.layoutIfNeeded()
        return twins
    }

    /// Runs the main run loop until `done`, for at most `seconds` (docs/testing.md)
    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertTrue(done(), what)
    }

    /// `c` in the light appearance, as 8-bit sRGB channels
    private func rgb(_ c: UIColor) -> [Int] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        c.resolvedColor(with: light).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b].map { Int(($0 * 255).rounded()) }
    }

    private func distance(_ a: [Int], _ b: [Int]) -> Int { zip(a, b).map { abs($0 - $1) }.max() ?? 0 }

    /// How far a pixel is from grey: its channels' range
    private func spread(_ p: [Int]) -> Int { (p.max() ?? 0) - (p.min() ?? 0) }

    private func assertPixel(_ actual: [Int], _ expected: [Int], _ what: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertLessThanOrEqual(distance(actual, expected), 2, "\(what): \(actual), not \(expected)", file: file, line: line)
    }
}

/// A view's pixels as UIKit draws its hierarchy, in 8-bit sRGB at twice its points
private struct Picture: Equatable {
    private let width: Int, bytes: [UInt8]
    private static let scale: CGFloat = 2

    init(_ view: UIView) {
        let format = UIGraphicsImageRendererFormat()
        format.scale = Self.scale
        format.preferredRange = .standard
        let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { _ in
            _ = view.drawHierarchy(in: view.bounds, afterScreenUpdates: true)
        }
        let cg = image.cgImage!
        var bytes = [UInt8](repeating: 0, count: cg.width * cg.height * 4)
        let context = CGContext(data: &bytes, width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: cg.width * 4,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        width = cg.width
        self.bytes = bytes
    }

    func at(_ point: CGPoint) -> [Int] {
        let i = (Int(point.y * Self.scale) * width + Int(point.x * Self.scale)) * 4
        XCTAssertTrue(CapturePixels.painted(bytes, at: i), "the sampled colour is painted")
        return [Int(bytes[i]), Int(bytes[i + 1]), Int(bytes[i + 2])]
    }

    /// The commonest pixel in `rect` unlike its top-left corner, the background: the solid core of a
    /// glyph or a symbol as large as the fixture's, which outnumbers its antialiased edges (more
    /// saturated than it, in sRGB)
    func ink(in rect: CGRect) -> [Int] {
        let background = at(rect.origin)
        var counts: [[Int]: Int] = [:]
        for y in Int(rect.minY * Self.scale)..<Int(rect.maxY * Self.scale) {
            for x in Int(rect.minX * Self.scale)..<Int(rect.maxX * Self.scale) {
                let p = at(CGPoint(x: CGFloat(x) / Self.scale, y: CGFloat(y) / Self.scale))
                if zip(p, background).contains(where: { abs($0 - $1) > 24 }) { counts[p, default: 0] += 1 }
            }
        }
        return counts.max { $0.value < $1.value }?.key ?? background
    }
}

#endif
