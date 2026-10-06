#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1034 §8 on AppKit (`ColorSchemeIOSTests`): `color-scheme` is the
/// view's own appearance, which `effectiveAppearance` carries below.
final class ColorSchemeMacTests: XCTestCase {
    private let pair: BatchValue = .array([.array([255, 255, 255, 255]), .array([0, 0, 0, 255])])

    func testASubtreeResolvesInItsColorScheme() throws {
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 402, height: 874), styleMask: [.borderless], backing: .buffered, defer: false)
        window.appearance = NSAppearance(named: .aqua)
        let content = try XCTUnwrap(window.contentView)
        let sheet = NodeView(id: 1, kind: "view", presenter: p)
        p.views[1] = sheet; content.addSubview(sheet)
        sheet.frame = content.bounds
        let child = NodeView(id: 2, kind: "view", presenter: p)
        p.views[2] = child; sheet.container.addSubview(child)
        child.frame = NSRect(x: 0, y: 0, width: 50, height: 50)
        child.applyStyle(["background_color": pair])
        let material = NSVisualEffectView(frame: .zero)
        sheet.container.addSubview(material)
        XCTAssertFalse(child.drawsDark)
        XCTAssertEqual(child.channels("background_color"), [255, 255, 255, 255])

        sheet.applyStyle(["color_scheme": "dark"])
        XCTAssertEqual(sheet.appearance?.name, .darkAqua)
        XCTAssertTrue(child.drawsDark, "the child inherits the scheme")
        XCTAssertEqual(child.channels("background_color"), [0, 0, 0, 255])
        XCTAssertEqual(material.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]), .darkAqua, "a material below resolves dark")

        child.applyStyle(["background_color": pair, "color_scheme": "light"])
        XCTAssertFalse(child.drawsDark, "a nested light wins")

        child.applyStyle(["background_color": pair])
        sheet.applyStyle([:])
        XCTAssertNil(sheet.appearance)
        XCTAssertFalse(child.drawsDark)
    }
}
#endif
