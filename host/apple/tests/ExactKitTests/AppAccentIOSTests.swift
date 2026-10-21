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

    func testASymbolsOwnTintIsItsTintAsTheAccentIs() throws {
        func symbol(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "image", "props": ["imageSource": "symbol:lock", "symbolName": "lock.fill"], "handlers": [],
              "style": ["font_size": 17].merging(style) { $1 }],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0]]
        }
        let p = presenter(view(1) + symbol(2, ["tint_color": [52, 199, 89, 255]]) + symbol(3, [:])
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let own = try XCTUnwrap(p.views[2]?.image), accent = try XCTUnwrap(p.views[3]?.image)
        XCTAssertNotEqual(own.renderingMode, .alwaysOriginal, "an authored tint-color is the symbol's tint, which UIKit greys behind an alert")
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
