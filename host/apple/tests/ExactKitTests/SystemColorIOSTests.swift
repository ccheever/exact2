#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

extension UIColor {
    /// Named like a colour, returning none: what the lookup must not `perform`.
    @objc class func exactTestNumberColor() -> Int { 3 }
}

/// LLP 1095 D5: a colour row naming a platform colour shows UIKit's own
/// colour for the view's traits, and its fallback where UIKit has none.
final class SystemColorIOSTests: XCTestCase {
    private func channels(_ c: UIColor, _ traits: UITraitCollection) -> [Double] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        _ = c.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a].map { Double($0) * 255 }
    }

    func testARoleIsUIKitsColourInEachAppearanceAndLevel() throws {
        let value: BatchValue = ["sys": "secondarySystemBackgroundColor", "c": [[1, 2, 3, 255], [4, 5, 6, 255]]]
        XCTAssertTrue(value.isSchemeColor, "an appearance change is something to it")
        let dark = UITraitCollection(traitsFrom: [UITraitCollection(userInterfaceStyle: .dark)])
        let elevated = UITraitCollection(traitsFrom: [dark, UITraitCollection(userInterfaceLevel: .elevated)])
        XCTAssertEqual(try XCTUnwrap(value.channels(dark: false)), channels(.secondarySystemBackground, UITraitCollection(userInterfaceStyle: .light)))
        XCTAssertEqual(try XCTUnwrap(value.channels(dark: true)), channels(.secondarySystemBackground, dark))
        XCTAssertEqual(try XCTUnwrap(value.channels(dark: true, elevated: true)), channels(.secondarySystemBackground, elevated))
        XCTAssertNotEqual(value.channels(dark: true), value.channels(dark: true, elevated: true), "a sheet's dark grey is lighter")
    }

    func testANameUIKitLacksIsTheFallback() {
        let missing: BatchValue = ["sys": "systemNoSuchHueColor", "c": [[1, 2, 3, 255], [4, 5, 6, 255]]]
        XCTAssertEqual(missing.channels(dark: true), [4, 5, 6, 255])
    }

    func testANodesTextColourRowFollowsItsTraits() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": ["sys": "systemGroupedBackgroundColor", "c": [[0, 0, 0, 255], [0, 0, 0, 255]]]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertTrue(try XCTUnwrap(node.style["background_color"]).isSystemColor)
        window.overrideUserInterfaceStyle = .light
        window.updateTraitsIfNeeded(); window.layoutIfNeeded(); node.updateTraitsIfNeeded()
        XCTAssertEqual(node.drawsDark, window.overrideUserInterfaceStyle == .dark)
        XCTAssertEqual(node.channels("background_color"), channels(.systemGroupedBackground, UITraitCollection(userInterfaceStyle: .light)))
        window.overrideUserInterfaceStyle = .dark
        window.updateTraitsIfNeeded(); window.layoutIfNeeded(); node.updateTraitsIfNeeded()
        XCTAssertEqual(node.drawsDark, window.overrideUserInterfaceStyle == .dark)
        XCTAssertEqual(node.channels("background_color"), channels(.systemGroupedBackground, UITraitCollection(userInterfaceStyle: .dark)))
    }

    /// A subtree's own Increased Contrast, not only the system's: the box,
    /// an inline run kept by name, and the symbol's key all follow it.
    func testAViewsContrastOverrideResolvesItsColours() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.overrideUserInterfaceStyle = .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let blue: [String: Any] = ["sys": "systemBlueColor", "c": [[0, 0, 255, 255], [0, 0, 255, 255]]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["background_color": blue]],
            ["op": "paragraph", "id": 1, "runs": [["id": 2, "parent": 1, "paint": true, "props": ["text": "blue"], "style": ["text_color": blue]]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        try XCTSkipIf(SystemColor.highContrast, "the system's Increased Contrast is on")
        node.updateTraitsIfNeeded()
        let normal = UITraitCollection(traitsFrom: [UITraitCollection(userInterfaceStyle: .light), UITraitCollection(accessibilityContrast: .normal)])
        let high = UITraitCollection(traitsFrom: [UITraitCollection(userInterfaceStyle: .light), UITraitCollection(accessibilityContrast: .high)])
        XCTAssertEqual(node.channels("background_color"), channels(.systemBlue, normal))
        XCTAssertEqual(node.paragraphSpec().runs.first?.color, channels(.systemBlue, normal))
        let key = node.symbolLookKey
        node.traitOverrides.accessibilityContrast = .high
        node.updateTraitsIfNeeded()
        XCTAssertEqual(node.drawsHighContrast, true)
        XCTAssertNotEqual(channels(.systemBlue, normal), channels(.systemBlue, high))
        XCTAssertEqual(node.channels("background_color"), channels(.systemBlue, high))
        XCTAssertEqual(node.paragraphSpec().runs.first?.color, channels(.systemBlue, high), "the run resolves now, not at decoding")
        XCTAssertNotEqual(node.symbolLookKey, key, "a palette resolved for the old contrast is made again")
    }

    func testAColourNamedMethodReturningNoObjectIsTheFallback() {
        XCTAssertEqual(SystemColor.channels("exactTestNumberColor", dark: false, fallback: [1, 2, 3, 255]), [1, 2, 3, 255])
    }

    /// LLP 1095 D8: `@tint` is the requesting view's own tint, its window's,
    /// not the key window's, and follows it when it changes.
    func testATintRowIsItsWindowsTint() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.overrideUserInterfaceStyle = .light
        window.tintColor = .systemRed
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        node.updateTraitsIfNeeded()
        let traits = node.traitCollection
        XCTAssertEqual(node.channels("background_color"), channels(.systemRed, traits))
        window.tintColor = .systemGreen
        XCTAssertEqual(node.channels("background_color"), channels(.systemGreen, traits))
    }

    /// The view's own tint reaches a plain text run, its shadow, a box
    /// shadow and a palette symbol, and a tint change rebuilds the glyph.
    func testEveryResolverTakesTheViewsTint() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.overrideUserInterfaceStyle = .light
        window.tintColor = .systemBlue
        p.viewport.frame = window.bounds
        p.viewport.tintColor = .systemRed
        window.addSubview(p.viewport)
        let accent: [String: Any] = ["sys": "@tint", "c": [[0, 122, 255, 255], [10, 132, 255, 255]]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "hello"], "style": [
                "text_color": accent, "text_shadow": ["o": [1.0, 1.0], "b": 0.0, "c": accent],
                "box_shadow": [["o": [0.0, 2.0], "b": 4.0, "c": accent]],
                "symbol_rendering": "palette", "symbol_palette": [accent]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        node.updateTraitsIfNeeded()
        let red = channels(.systemRed, node.traitCollection)
        let run = try XCTUnwrap(node.paragraphSpec().runs.first)
        XCTAssertEqual(run.color, red, "a plain text run")
        XCTAssertEqual(Array((node.paragraphSpec().shadow ?? run.shadow ?? []).suffix(4)), red, "its text shadow")
        let shade = try XCTUnwrap(node.boxShadows.first?.color.components).map { Double($0) * 255 }
        XCTAssertEqual(shade.count, 4)
        for (got, want) in zip(shade, red) { XCTAssertEqual(got, want, accuracy: 0.01, "a box shadow") }
        let key = node.symbolLookKey
        p.viewport.tintColor = .systemGreen
        XCTAssertNotEqual(node.symbolLookKey, key, "a palette naming the tint is made again")
    }

    func testMarkdownAndPlainTextResolveTheSameViewTintAndTraits() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.overrideUserInterfaceStyle = .dark
        window.tintColor = .systemBlue
        p.viewport.frame = window.bounds
        p.viewport.tintColor = .systemRed
        window.addSubview(p.viewport)
        for (name, expected) in [("@tint", UIColor.systemRed), ("systemBlueColor", .systemBlue),
                                 ("secondarySystemBackgroundColor", .secondarySystemBackground)] {
            let color: [String: Any] = ["sys": name, "c": [[1, 2, 3, 255], [4, 5, 6, 255]]]
            p.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "text", "props": ["text": "hello"], "style": ["text_color": color]],
                ["op": "create", "id": 2, "kind": "text", "props": ["text": "hello", "markup": "markdown"], "style": ["text_color": color]],
                ["op": "roots", "ids": [1, 2]],
            ]))
            let plain = try XCTUnwrap(p.views[1]), markdown = try XCTUnwrap(p.views[2])
            for node in [plain, markdown] {
                node.traitOverrides.accessibilityContrast = .high
                node.traitOverrides.userInterfaceLevel = .elevated
                node.updateTraitsIfNeeded()
                node.invalidateText()
                XCTAssertTrue(node.drawsDark && node.drawsHighContrast == true && node.drawsElevated)
            }
            let expected = channels(expected, plain.traitCollection)
            XCTAssertEqual(plain.paragraphSpec().runs.first?.color, expected, name)
            let runs = markdown.paragraphSpec().runs.filter { !$0.text.isEmpty }
            XCTAssertFalse(runs.isEmpty)
            for run in runs { XCTAssertEqual(run.color, expected, name) }
            p.apply(wireBatch([["op": "destroy", "id": 1], ["op": "destroy", "id": 2]]))
        }
    }

    /// LLP 1095 D9: off a window there is no report, so the kernel's table
    /// (replaced whole by each) never loses `AccentColor`; in one, there is.
    func testNoColourReportIsMadeOffAWindow() throws {
        let session = ExactApp.shared.makeSession(label: "colours-off-window")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        var reads = 0
        session.runtime.observeBatch = { _, _ in reads += 1 }
        defer { session.runtime.observeBatch = nil }
        let view = ExactView(session: session)
        session.reportColors()
        XCTAssertEqual(reads, 0, "no window: nothing reported")
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.addSubview(view)
        reads = 0
        session.reportColors()
        XCTAssertEqual(reads, 1, "in a window: reported")
    }

    /// LLP 1095 D9: the report's `AccentColor` is the tint read on main,
    /// wherever the resolution runs.
    func testAReportedTintIsTheOneReadOnMain() throws {
        let pair = try XCTUnwrap(SystemColor.tintPair(.systemRed))
        XCTAssertEqual(pair[1], channels(.systemRed, UITraitCollection(traitsFrom: [UITraitCollection(userInterfaceStyle: .dark), UITraitCollection(accessibilityContrast: SystemColor.highContrast ? .high : .normal)])))
        var highlight = pair[0]; highlight[3] *= 0.2
        let off = DispatchQueue.global().sync { SystemColor.channels("@tint/0.2", dark: false, tint: pair[0], fallback: nil) }
        XCTAssertEqual(off, highlight)
    }
}
#endif
