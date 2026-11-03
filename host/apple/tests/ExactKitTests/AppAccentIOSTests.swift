#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1095: the root's `accent-color` is the app's accent, its window's
/// tint, and behind an alert only the accent dims.
final class AppAccentIOSTests: XCTestCase {
    private var window: UIWindow!
    private let red: [Double] = [232, 28, 36, 255]

    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    private func view(_ id: Int, style: [String: Any] = [:]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": [:], "handlers": [], "style": style],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0]]
    }
    private func rgb(_ c: UIColor?) -> [Double] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        _ = c?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b].map { (Double($0) * 255).rounded() }
    }
    /// `c` as it draws in `view`, its red, green, blue (0...255) and alpha (0...1): UIKit's tint
    /// (`UIColor.tintColor`) resolves by the traits of the view it is drawn in
    private func rgba(_ c: UIColor?, in view: UIView) -> [Double] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        _ = c?.resolvedColor(with: view.traitCollection).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b].map { (Double($0) * 255).rounded() } + [(Double(a) * 100).rounded() / 100]
    }

    func testTheRootsAccentIsTheWindowsTint() throws {
        let p = presenter(view(1, style: ["accent_color": red]) + view(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36], "the window takes the root's accent")
        XCTAssertEqual(rgb(try XCTUnwrap(p.views[1]).tintColor), [232, 28, 36], "the tree below inherits it")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": [0, 122, 255, 255]]]]))
        XCTAssertEqual(rgb(window.tintColor), [0, 122, 255], "a restyled root's accent is the window's")
        p.apply(wireBatch(view(3) + [["op": "roots", "ids": [3]]]))
        XCTAssertNotEqual(rgb(window.tintColor), [0, 122, 255], "a root with no accent gives the window its own back")
    }

    func testAccentColorAsTheTintLeavesTheWindowAlone() {
        let before = rgb(UIWindow().tintColor)
        _ = presenter(view(1, style: ["accent_color": ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]]) + [["op": "roots", "ids": [1]]])
        XCTAssertEqual(rgb(window.tintColor), before, "AccentColor is the platform's, not a colour of the app's")
    }

    func testAutoIsNoAccentAndRedToAutoGivesTheWindowItsOwnBack() {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.tintColor = .systemPurple
        let p = Presenter()
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch(view(1, style: ["accent_color": "auto"]) + [["op": "roots", "ids": [1]]]))
        XCTAssertEqual(rgb(window.tintColor), rgb(.systemPurple), "auto is the platform's accent, not system blue")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": red]]]))
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36])
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": "auto"]]]))
        XCTAssertEqual(rgb(window.tintColor), rgb(.systemPurple), "red to auto gives the window its own tint back")
    }

    func testRestylingAnotherRootKeepsTheFirstRootsAccent() {
        let p = presenter(view(1, style: ["accent_color": red]) + view(2) + [["op": "roots", "ids": [1, 2]]])
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36])
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["background_color": [0, 0, 0, 255]]]]))
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36], "a sibling root's restyle keeps the first root's accent")
        p.apply(wireBatch([["op": "style", "id": 1, "style": [:]]]))
        XCTAssertNotEqual(rgb(window.tintColor), [232, 28, 36], "the accent goes when its root drops it")
    }

    func testTheAccentHoldsNoWindowAndALeftWindowGetsItsTintBack() {
        weak var gone: UIWindow?
        let p = Presenter()
        autoreleasepool {
            let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
            p.viewport.frame = w.bounds
            w.addSubview(p.viewport)
            p.apply(wireBatch(view(1, style: ["accent_color": red]) + [["op": "roots", "ids": [1]]]))
            XCTAssertEqual(rgb(w.tintColor), [232, 28, 36])
            p.viewport.removeFromSuperview()
            p.accent.apply()
            XCTAssertNotEqual(rgb(w.tintColor), [232, 28, 36], "a window the view left has its own tint back")
            gone = w
        }
        XCTAssertNil(gone, "the accent keeps no window alive")
        let a = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300)), b = UIWindow(frame: a.frame)
        a.tintColor = .systemPurple
        a.addSubview(p.viewport); p.accent.apply()
        XCTAssertEqual(rgb(a.tintColor), [232, 28, 36])
        b.addSubview(p.viewport); p.accent.apply()
        XCTAssertEqual([rgb(a.tintColor), rgb(b.tintColor)], [rgb(.systemPurple), [232, 28, 36]], "the view's new window takes it, the old one has its own back")
        p.reset()
        XCTAssertNotEqual(rgb(b.tintColor), [232, 28, 36], "an ended session leaves no accent behind")
    }

    /// Ending a session gives the window its tint back for good, even when a
    /// root's own rows follow the tint and restyle as it changes back.
    func testAnEndedSessionDoesNotRetakeTheAccentAsItLetsGo() {
        let tinted: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        let p = presenter(view(1, style: ["accent_color": red, "text_color": tinted]) + [["op": "roots", "ids": [1]]])
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36])
        p.reset()
        XCTAssertNotEqual(rgb(window.tintColor), [232, 28, 36], "the restyle the release causes takes nothing back")
    }

    /// Two sessions in one window (a host embedding two): the latest still
    /// there tints it, and the last to leave gives the window its own tint
    /// back, whichever leaves first.
    func testTwoSessionsShareAWindowsTint() {
        for firstLeaves in [true, false] {
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
            window.tintColor = .systemPurple
            let a = Presenter(), b = Presenter()
            for (p, colour) in [(a, red), (b, [0.0, 122, 255, 255])] {
                p.viewport.frame = window.bounds
                window.addSubview(p.viewport)
                p.apply(wireBatch(view(1, style: ["accent_color": colour]) + [["op": "roots", "ids": [1]]]))
            }
            XCTAssertEqual(rgb(window.tintColor), [0, 122, 255], "the latest session's accent")
            let (first, second) = firstLeaves ? (a, b) : (b, a)
            first.reset()
            XCTAssertEqual(rgb(window.tintColor), firstLeaves ? [0, 122, 255] : [232, 28, 36], "the one still there tints it")
            second.reset()
            XCTAssertEqual(rgb(window.tintColor), rgb(.systemPurple), "the last to leave gives the window its own back")
        }
    }

    /// A role as the accent answers Increased Contrast, as UIKit's own does.
    func testARoleAccentFollowsIncreasedContrast() {
        let blue: [String: Any] = ["sys": "systemBlueColor", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        _ = presenter(view(1, style: ["accent_color": blue]) + [["op": "roots", "ids": [1]]])
        for contrast in [UIAccessibilityContrast.normal, .high] {
            let traits = UITraitCollection(traitsFrom: [.init(userInterfaceStyle: .light), .init(accessibilityContrast: contrast)])
            func channels(_ c: UIColor) -> [CGFloat] {
                var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
                _ = c.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
                return [r, g, b, a].map { ($0 * 255).rounded() }
            }
            XCTAssertEqual(channels(window.tintColor), channels(.systemBlue), "\(contrast.rawValue)")
        }
    }

    /// An unaccented root that follows the tint, joining before an accented
    /// one, restyles as its tint changes; the accent is the whole set's.
    func testARootJoiningFirstKeepsALaterRootsAccent() {
        let tinted: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        _ = presenter(view(1, style: ["text_color": tinted]) + view(2, style: ["accent_color": red]) + [["op": "roots", "ids": [1, 2]]])
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36], "the second root's accent survives the first's restyle")
    }

    /// A role as the accent answers the elevated level (a sheet), as UIKit's own does.
    func testARoleAccentFollowsTheElevatedLevel() {
        let role: [String: Any] = ["sys": "secondarySystemBackgroundColor", "c": [[242, 242, 247, 255], [28, 28, 30, 255]]]
        _ = presenter(view(1, style: ["accent_color": role]) + [["op": "roots", "ids": [1]]])
        for level in [UIUserInterfaceLevel.base, .elevated] {
            let traits = UITraitCollection(traitsFrom: [.init(userInterfaceStyle: .dark), .init(userInterfaceLevel: level)])
            func channels(_ c: UIColor) -> [CGFloat] {
                var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
                _ = c.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
                return [r, g, b, a].map { ($0 * 255).rounded() }
            }
            XCTAssertEqual(channels(window.tintColor), channels(.secondarySystemBackground), "\(level.rawValue)")
        }
    }

    func testAP3AccentKeepsItsColourSpace() throws {
        let p3: [String: Any] = ["c": [255, 0, 0, 255], "cs": [["s": "display-p3", "v": [1.0, 0.0, 0.0, 1.0]]]]
        _ = presenter(view(1, style: ["accent_color": p3]) + [["op": "roots", "ids": [1]]])
        let space = try XCTUnwrap(window.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).cgColor.colorSpace?.name)
        // Exact makes a P3 colour in the extended space, unclipped (LLP 1100 D2)
        XCTAssertEqual(space as String, CGColorSpace.extendedDisplayP3 as String, "the window's tint is the authored Display P3 red, not its clipped sRGB")
    }

    func testEveryTintDimsBehindAnAlertAsUIKitsDoes() throws {
        var face = ButtonFace(); face.title = "Go"; face.style = "filled"; face.ios = "filled"; face.iosBefore26 = "filled"
        func native(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"], "handlers": ["press"],
              "style": ["appearance": "auto"].merging(style) { $1 }],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 120.0, "h": 34.0]]
        }
        let p = presenter(view(1, style: ["accent_color": red]) + native(2, ["accent_color": [0, 122, 255, 255]]) + native(3, [:])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]], faces: [2: face, 3: face])
        // An alert dims the tint behind it (UIKit reads the mode up the tree)
        p.viewport.tintAdjustmentMode = .dimmed
        let plain = UIView()
        try XCTUnwrap(p.controls.controls[3]?.superview).addSubview(plain)
        XCTAssertEqual(plain.tintAdjustmentMode, .dimmed, "a plain view behind the alert dims")
        XCTAssertEqual(try XCTUnwrap(p.controls.controls[2]).tintAdjustmentMode, .dimmed, "a button's own accent-color is its tint, which greys")
        XCTAssertEqual(try XCTUnwrap(p.controls.controls[3]).tintAdjustmentMode, .dimmed, "the app's accent greys")
        // As a hand-built UIKit button beside it: grey while dimmed, the accent once restored
        let uikit = UIButton(type: .system)
        try XCTUnwrap(p.controls.controls[3]?.superview).addSubview(uikit)
        XCTAssertEqual(rgb(try XCTUnwrap(p.controls.controls[3]).tintColor), rgb(uikit.tintColor))
        XCTAssertNotEqual(rgb(uikit.tintColor), [232, 28, 36], "dimmed is not the accent")
        p.viewport.tintAdjustmentMode = .automatic
        XCTAssertEqual(rgb(try XCTUnwrap(p.controls.controls[3]).tintColor), [232, 28, 36], "the app's accent is back")
    }

    /// A control whose `accent-color` is `AccentColor` resolves the tint it
    /// inherits when it is configured: configured out of the window, as an
    /// unselected tab's content is, the tint it will draw in, the app's
    /// accent, which it keeps in the window and follows when the window's tint
    /// changes. Behind an alert it keeps the accent, which UIKit dims.
    func testAControlConfiguredOutOfTheWindowTakesTheAccentInIt() throws {
        let p = presenter(view(1, style: ["accent_color": red]) + [["op": "roots", "ids": [1]]], faces: [2: tintedFace])
        XCTAssertEqual(rgb(window.tintColor), [232, 28, 36])
        p.viewport.removeFromSuperview()
        p.apply(wireBatch(tintedButton(2) + [["op": "roots", "ids": [1, 2]]]))
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonIOS)
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36], "out of the window it has the app's accent")
        window.addSubview(p.viewport)
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36], "in the window it keeps the app's accent")
        window.tintColor = .systemGreen
        XCTAssertEqual(rgb(button.written?.accent), rgb(.systemGreen), "it follows the window's tint")
        window.tintColor = UIColor(red: 232 / 255, green: 28 / 255, blue: 36 / 255, alpha: 1)
        p.viewport.tintAdjustmentMode = .dimmed
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36], "behind an alert it keeps the accent")
        p.viewport.tintAdjustmentMode = .automatic
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36])
    }

    /// A bar drawn in the tint it inherits (an `accent-color` of `AccentColor`), and a bar's or a
    /// spinner's colour made from the tint (a `Highlight` accent) or naming it (a spinner's own
    /// `color` of `AccentColor`), configured out of the window: each is UIKit's tint
    /// (`SystemColor.tint`), at its alpha, so in the window it draws the app's accent and follows
    /// the window's tint when it changes. A bar's `Highlight` is its `progressTintColor`, which
    /// UIKit greys behind an alert.
    func testAProgressConfiguredOutOfTheWindowTakesTheAccentInIt() throws {
        let p = presenter(view(1, style: ["accent_color": red]) + [["op": "roots", "ids": [1]]])
        p.viewport.removeFromSuperview()
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        let highlight: [String: Any] = ["sys": "@tint/0.2", "c": [[0, 122, 255, 51], [10, 132, 255, 51]]]
        var inked = progress(4, [:], accent: highlight)
        inked[0]["style"] = ["text_color": tint, "written": ["text_color"]]
        p.apply(wireBatch(progress(2, ["value": "0.5"], accent: highlight) + progress(3, [:], accent: highlight) + inked
            + progress(5, ["value": "0.5"], accent: tint) + [["op": "roots", "ids": [1, 2, 3, 4, 5]]]))
        let bar = try XCTUnwrap(p.controls.bars[2]), spinner = try XCTUnwrap(p.controls.spinners[3]), ink = try XCTUnwrap(p.controls.spinners[4])
        let inheriting = try XCTUnwrap(p.controls.bars[5])
        XCTAssertTrue(bar.progressTintColor === SystemColor.tint(0.2), "a fifth of UIKit's tint")
        XCTAssertNil(inheriting.progressTintColor, "UIKit draws the bar in its tint")
        XCTAssertTrue(spinner.color === SystemColor.tint(0.2))
        XCTAssertEqual(ink.color, UIColor.tintColor)
        window.addSubview(p.viewport)
        let drawn = { [self.rgba(bar.progressTintColor, in: bar), self.rgba(spinner.color, in: spinner), self.rgba(ink.color, in: ink), self.rgba(inheriting.tintColor, in: inheriting)] }
        XCTAssertEqual(drawn(), [[232, 28, 36, 0.2], [232, 28, 36, 0.2], [232, 28, 36, 1], [232, 28, 36, 1]], "in the window, the app's accent")
        window.tintColor = .systemGreen
        let green = rgb(.systemGreen)
        XCTAssertEqual(drawn(), [green + [0.2], green + [0.2], green + [1], green + [1]], "a changed tint")
        p.viewport.tintAdjustmentMode = .dimmed
        XCTAssertNotEqual(Array(rgba(bar.progressTintColor, in: bar).prefix(3)), green, "behind an alert UIKit greys the bar's colour")
        p.viewport.tintAdjustmentMode = .automatic
    }

    /// A tab bar standing in for a tablist draws its selected item in that tab's colour,
    /// and a segmented control its tablist's background, here colours that are the tint:
    /// a native tab's `accent-color`, a symbol tab's symbol's own `color` and a tablist's
    /// `background-color` of `AccentColor`. Configured out of the window, the bars inherit
    /// the tint and the background is UIKit's, so in the window they draw the app's accent,
    /// and the window's tint when it changes.
    func testATablistsControlConfiguredOutOfTheWindowTakesTheAccentInIt() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        var home = ButtonFace(); home.symbol = "house"; home.title = "Home"
        var saved = ButtonFace(); saved.symbol = "star"; saved.title = "Saved"
        let p = presenter(view(1, style: ["accent_color": red]) + [["op": "roots", "ids": [1]]], faces: [11: home, 12: saved])
        p.viewport.removeFromSuperview()
        func tab(_ id: Int, kind: String, selected: Bool, style: [String: Any] = [:]) -> [String: Any] {
            ["op": "create", "id": id, "kind": kind, "handlers": ["press"], "style": style,
             "props": (kind == "control" ? ["type": "button"] : [:]).merging(["accessibilityRole": "tab", "accessibilitySelected": selected ? "true" : "false"]) { $1 }]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            tab(11, kind: "control", selected: true, style: ["appearance": "auto", "accent_color": tint]),
            tab(12, kind: "control", selected: false, style: ["appearance": "auto"]),
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            tab(21, kind: "button", selected: true),
            tab(22, kind: "button", selected: false),
            ["op": "create", "id": 23, "kind": "image", "props": ["imageSource": "symbol:sf/house", "symbolName": "house"], "style": ["text_color": tint, "written": ["text_color"]]],
            ["op": "create", "id": 24, "kind": "text", "props": ["text": "Home"]],
            ["op": "create", "id": 25, "kind": "image", "props": ["imageSource": "symbol:sf/star", "symbolName": "star"]],
            ["op": "create", "id": 26, "kind": "text", "props": ["text": "Saved"]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 21, "ids": [23, 24]],
            ["op": "children", "id": 22, "ids": [25, 26]],
            ["op": "children", "id": 20, "ids": [21, 22]],
            ["op": "create", "id": 30, "kind": "view", "props": ["accessibilityRole": "tablist"], "style": ["background_color": tint, "written": ["background_color"]]],
            tab(31, kind: "button", selected: true),
            tab(32, kind: "button", selected: false),
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Day"]],
            ["op": "create", "id": 34, "kind": "text", "props": ["text": "Week"]],
            ["op": "children", "id": 31, "ids": [33]],
            ["op": "children", "id": 32, "ids": [34]],
            ["op": "children", "id": 30, "ids": [31, 32]],
            ["op": "roots", "ids": [1, 10, 20, 30]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 50.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 150.0, "h": 50.0],
            ["op": "frame", "id": 12, "x": 150.0, "y": 0.0, "w": 150.0, "h": 50.0],
            ["op": "frame", "id": 20, "x": 0.0, "y": 60.0, "w": 300.0, "h": 50.0],
            ["op": "frame", "id": 30, "x": 0.0, "y": 120.0, "w": 300.0, "h": 32.0],
        ]))
        let bars = try [10, 20].map { id in try XCTUnwrap(p.views[UInt32(id)]?.subviews.first { $0 is UITabBar }, "tablist \(id) is a tab bar") }
        let control = try XCTUnwrap(p.segments.control(of: 30), "tablist 30 is a segmented control")
        // The tint itself is no bar's own: each inherits its tablist's, and the background is UIKit's tint
        for bar in bars { XCTAssertTrue(bar.tintColor === bar.superview?.tintColor, "the bar inherits its tint") }
        XCTAssertEqual(control.backgroundColor, UIColor.tintColor)
        let colours = { bars.map { self.rgba($0.tintColor, in: $0) } + [self.rgba(control.backgroundColor, in: control)] }
        window.addSubview(p.viewport)
        for colour in colours() { XCTAssertEqual(colour, [232, 28, 36, 1], "in the window, the app's accent") }
        window.tintColor = .systemGreen
        for colour in colours() { XCTAssertEqual(colour, rgb(.systemGreen) + [1], "a changed tint") }
    }

    /// A pushed route's bar is projected before the route is in the window. Its title's and
    /// its bar item's `color` of `AccentColor` are UIKit's tint, the window's from the push on.
    func testAPushedRoutesBarTakesTheAccentFromThePushOn() throws {
        let session = try navigation("ACCENT_BARS", tint: .systemRed)
        let p = session.presenter
        let open = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" })
        p.press(open.id)
        // Each state from the push until its transition ends, read at every turn of the run loop
        var seen: [[[Double]]] = [], pushed = false
        let end = Date().addingTimeInterval(30)
        repeat {
            if let nav = p.navigation.primaryNavigation, nav.viewControllers.count == 2, let item = nav.topViewController?.navigationItem {
                pushed = true
                let colours = [item.standardAppearance?.titleTextAttributes[.foregroundColor] as? UIColor, item.rightBarButtonItems?.first?.tintColor]
                XCTAssertTrue(colours.allSatisfy { $0 == UIColor.tintColor }, "UIKit's tint")
                let state = colours.map { rgba($0, in: nav.navigationBar) }
                if seen.last != state { seen.append(state) }
            }
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.001))
        } while !(pushed && !p.navigation.inTransition) && Date() < end
        XCTAssertTrue(pushed, "the route is pushed")
        XCTAssertEqual(seen, [[rgb(.systemRed) + [1], rgb(.systemRed) + [1]]], "the title and the item in the window's tint from the push on")
    }

    /// A session booted from fixture `key`'s plan in a window whose tint is `tint`, its first
    /// route shown.
    private func navigation(_ key: String, tint: UIColor) throws -> ExactSession {
        let session = ExactApp.shared.makeSession(label: "accent-\(key)-\(UUID().uuidString)")
        addTeardownBlock { session.destroy() }
        let view = ExactView(session: session), host = UIViewController()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.rootViewController = host
        window.tintColor = tint
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        XCTAssertNil(session.boot(plan: try fixturePlan(key), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        let end = Date().addingTimeInterval(30)
        while !(session.presenter.navigation.primaryNavigation?.topViewController is RouteController), Date() < end {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        return session
    }

    /// An embedder that sets the tint of a view it holds the session in is the tint there, in
    /// the window and out of it, whatever the roots' accent: a spinner's own `color` of
    /// `AccentColor`, UIKit's tint, draws in it, not in the accent the session's window takes.
    func testAnEmbeddersTintHoldsOutOfTheWindow() throws {
        let p = presenter(view(1, style: ["accent_color": red]) + [["op": "roots", "ids": [1]]])
        let holder = UIView(frame: window.bounds)
        holder.tintColor = .systemGreen
        p.viewport.removeFromSuperview()
        holder.addSubview(p.viewport)
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        var inked = progress(2, [:], accent: tint)
        inked[0]["style"] = ["text_color": tint, "written": ["text_color"]]
        p.apply(wireBatch(inked + [["op": "roots", "ids": [1, 2]]]))
        let spinner = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertEqual(rgba(spinner.color, in: spinner), rgb(.systemGreen) + [1], "out of the window, the embedder's tint")
        window.addSubview(holder)
        XCTAssertEqual(rgba(spinner.color, in: spinner), rgb(.systemGreen) + [1], "in the window, the embedder's tint")
    }

    /// The canvas is the first root's background (`paintCanvas`): one of `AccentColor`,
    /// painted out of the window, is the window's tint in it.
    func testACanvasPaintedOutOfTheWindowTakesTheTintInIt() throws {
        let p = presenter([])
        window.tintColor = .systemRed
        p.viewport.removeFromSuperview()
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        p.apply(wireBatch(view(1, style: ["background_color": tint, "written": ["background_color"]]) + [["op": "roots", "ids": [1]]]))
        window.addSubview(p.viewport)
        XCTAssertEqual(rgb(p.viewport.backgroundColor), rgb(.systemRed), "in the window, the window's tint")
    }

    /// A `progress`, with a `value` a bar and without one a spinner, whose `accent-color`
    /// is `accent`.
    private func progress(_ id: Int, _ props: [String: String], accent: [String: Any]) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "props": ["type": "progress", "accessibilityRole": "progressbar"].merging(props) { $1 },
          "handlers": [], "style": ["accent_color": accent]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 160.0, "h": 16.0]]
    }

    /// A control that is leaving keeps the accent it had when the tint
    /// changes: its owner left the tree, and the next sync would not
    /// configure it either.
    func testALeavingControlKeepsItsAccentWhenTheTintChanges() throws {
        let p = presenter(view(1, style: ["accent_color": red]) + tintedButton(2) + [["op": "roots", "ids": [1, 2]]], faces: [2: tintedFace])
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonIOS)
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36])
        p.apply(wireBatch([["op": "exit", "id": 2]]))
        XCTAssertTrue(p.controls.controls[2] === button, "it draws until its exit ends")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": [0, 122, 255, 255]]]]))
        XCTAssertEqual(rgb(window.tintColor), [0, 122, 255])
        XCTAssertEqual(rgb(button.written?.accent), [232, 28, 36], "the leaving control is not configured again")
    }

    private var tintedFace: ButtonFace {
        var face = ButtonFace(); face.title = "Try again"; face.style = "filled"; face.ios = "filled"; face.iosBefore26 = "filled"
        return face
    }
    /// A native button whose accent is `AccentColor`.
    private func tintedButton(_ id: Int) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"], "handlers": ["press"],
          "style": ["appearance": "auto", "accent_color": ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 120.0, "h": 34.0]]
    }

    func testASymbolsComputedColorUsesATemplateImage() throws {
        func symbol(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "image", "props": ["imageSource": "symbol:lock", "symbolName": "lock.fill"], "handlers": [],
              "style": ["font_size": 17].merging(style) { $1 }],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0]]
        }
        let p = presenter(view(1) + symbol(2, ["text_color": [52, 199, 89, 255]]) + symbol(3, ["text_color": ["sys": "@tint"]])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let own = try XCTUnwrap(p.views[2]?.image), accent = try XCTUnwrap(p.views[3]?.image)
        XCTAssertNotEqual(own.renderingMode, .alwaysOriginal, "an authored color uses a template, which UIKit greys behind an alert")
        XCTAssertNotEqual(accent.renderingMode, .alwaysOriginal, "the accent follows the tint, which UIKit greys")
    }

    /// LLP 1011 §4: a symbol in a box of another shape keeps its aspect under
    /// CSS's default `object-fit: fill`, as an SVG does; `cover` and `none` say otherwise.
    func testASymbolKeepsItsAspectUnderFill() throws {
        func symbol(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "image", "props": ["imageSource": "symbol:sf/chevron.right", "symbolName": "chevron.right"], "handlers": [],
              "style": ["font_size": 16].merging(style) { $1 }],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 16.0, "h": 16.0]]
        }
        let p = presenter(view(1) + symbol(2, [:]) + symbol(3, ["object_fit": "cover"]) + symbol(4, ["object_fit": "none"])
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]])
        XCTAssertEqual(p.views[2]?.symbolView?.contentMode, .scaleAspectFit, "unsaid `fill` draws the glyph undistorted")
        XCTAssertEqual(p.views[3]?.symbolView?.contentMode, .scaleAspectFill)
        XCTAssertEqual(p.views[4]?.symbolView?.contentMode, .center)
    }

    /// A root's accent becomes its window's tint while the batch applies;
    /// the report that tint change asks for is made once the batch is in,
    /// so the runner never re-presents in the middle of a batch.
    func testATintSetInsideABatchIsReportedAfterIt() throws {
        let session = ExactApp.shared.makeSession(label: "colours-after-batch")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let view = ExactView(session: session)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.addSubview(view)
        var reports: [Bool] = []
        session.runtime.observeBatch = { _, _ in reports.append(session.presenter.views[9001]?.layer.bounds.width == 123) }
        defer { session.runtime.observeBatch = nil }
        session.apply(wireBatch([
            ["op": "create", "id": 9001, "kind": "view", "props": [:], "handlers": [], "style": ["accent_color": [255, 59, 48, 255]]],
            ["op": "roots", "ids": [9001]],
            ["op": "frame", "id": 9001, "x": 0.0, "y": 0.0, "w": 123.0, "h": 45.0],
        ]))
        XCTAssertFalse(reports.isEmpty, "the tint change was reported")
        XCTAssertEqual(reports.filter { !$0 }.count, 0, "no report before the batch's last op applied")
    }
}
#endif
