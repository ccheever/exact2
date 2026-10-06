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
}
#endif
