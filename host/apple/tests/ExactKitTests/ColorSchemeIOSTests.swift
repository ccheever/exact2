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
        ColorSchemes.begin()
        views[0].applyStyle(["color_scheme": "light"])
        ColorSchemes.end()
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
        ColorSchemes.begin()
        for v in views { v.applyStyle(["color_scheme": "light"]) }
        ColorSchemes.end()
        XCTAssertEqual(seen(), Array(repeating: [.light], count: 8))
        XCTAssertEqual(views.map(\.overrideUserInterfaceStyle), [.light] + Array(repeating: .unspecified, count: 7))
        withExtendedLifetime(window) {}
    }
}
#endif
