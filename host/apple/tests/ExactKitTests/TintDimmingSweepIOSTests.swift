#if os(iOS) || os(tvOS)
import UIKit
import XCTest
@testable import ExactKit

/// Exact's grey for a colour on a dimmed control face is UIKit's dimmed tint for it (TintDimmingIOS.swift
/// `TintDimming.grey`), computed by Core Graphics because the view UIKit would compute it on can't be touched
/// where Exact greys. This sweep is the guard: if a later iOS or tvOS changes how UIKit greys a dimmed tint,
/// it fails. Each expected grey is read from a new view UIKit dims, here where making views is safe.
@MainActor
final class TintDimmingSweepIOSTests: XCTestCase {
    /// Opaque and translucent sRGB colours across the hues and lightnesses, the clone's inks among them
    private let sweep: [[Double]] = [
        [171, 184, 201], [28, 39, 54], [77, 151, 255], [111, 131, 159], [44, 58, 78], [72, 91, 117], [15, 115, 255],
        [255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255], [255, 0, 255], [0, 106, 255],
        [255, 59, 48], [52, 199, 89], [0, 122, 255], [230, 0, 0], [128, 64, 200], [200, 150, 50], [10, 200, 120],
        [250, 128, 114], [64, 81, 104], [1, 2, 200], [100, 30, 30], [26, 26, 60], [240, 240, 200],
        // Neutral: channels within 24 of each other
        [128, 128, 128], [20, 30, 40], [250, 240, 230],
    ].flatMap { (rgb: [Double]) -> [[Double]] in [255.0, 204, 153, 26].map { (alpha: Double) -> [Double] in rgb + [alpha] } }

    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    /// UIKit's dimmed tint for `colour` on a new view: detached, or with `traits` on it (`traitOverrides`), in a
    /// window so they reach it. Its grey level (0...255) and alpha (0...1)
    private func uikit(_ colour: UIColor, _ traits: [(inout UITraitOverrides) -> Void] = []) -> (level: Double, alpha: Double) {
        let probe = UIView()
        if !traits.isEmpty {
            if window == nil {
                let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
                window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
                window?.rootViewController = UIViewController()
                window?.makeKeyAndVisible()
            }
            window?.rootViewController?.view.addSubview(probe)
            for set in traits { set(&probe.traitOverrides) }
            probe.updateTraitsIfNeeded()
        }
        defer { probe.removeFromSuperview() }
        probe.tintColor = colour
        probe.tintAdjustmentMode = .dimmed
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        XCTAssertTrue(probe.tintColor.resolvedColor(with: probe.traitCollection).getRed(&r, green: &g, blue: &b, alpha: &a))
        XCTAssertEqual(r, g, accuracy: 0.0001); XCTAssertEqual(g, b, accuracy: 0.0001)
        return (Double(r) * 255, Double(a))
    }

    private func srgb(_ c: [Double]) -> UIColor {
        UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }

    /// Every hued colour of the sweep greys to UIKit's level, keeping its own alpha, in each trait UIKit could read
    /// (light, dark, elevated, increased contrast); a neutral one stays as it came
    func testEveryColourGreysToUIKitsOwnDimmedTint() {
        var traits: [(String, [(inout UITraitOverrides) -> Void])] = [
            ("light", [{ $0.userInterfaceStyle = .light }]), ("dark", [{ $0.userInterfaceStyle = .dark }]),
            ("increased contrast", [{ $0.accessibilityContrast = .high }]),
            ("dark, increased contrast", [{ $0.userInterfaceStyle = .dark }, { $0.accessibilityContrast = .high }])]
        #if os(iOS)
        traits.append(("dark, elevated", [{ $0.userInterfaceStyle = .dark }, { $0.userInterfaceLevel = .elevated }]))
        #endif
        // The traits reach a probe so placed
        _ = uikit(.red, [{ $0.userInterfaceStyle = .dark }])
        let check = UIView()
        window?.rootViewController?.view.addSubview(check)
        check.traitOverrides.userInterfaceStyle = .dark; check.traitOverrides.accessibilityContrast = .high
        check.updateTraitsIfNeeded()
        XCTAssertEqual(check.traitCollection.userInterfaceStyle, .dark)
        XCTAssertEqual(check.traitCollection.accessibilityContrast, .high)
        check.removeFromSuperview()
        for c in sweep {
            let grey = TintDimming.grey(c)
            if (c[0...2].max()! - c[0...2].min()!) <= 24 { XCTAssertEqual(grey, c, "a neutral colour stays: \(c)"); continue }
            XCTAssertEqual(grey[3], c[3], "the colour's own alpha: \(c)")
            XCTAssertEqual(grey[0], grey[1]); XCTAssertEqual(grey[1], grey[2])
            for (name, t) in traits {
                let want = uikit(srgb(c), t)
                XCTAssertEqual(grey[0], want.level, accuracy: 0.01, "\(c) in \(name)")
                XCTAssertEqual(want.alpha, c[3] / 255 * TintDimming.uikitDimmedAlpha, accuracy: 0.001, "UIKit's dimmed alpha share for \(c)")
            }
        }
    }

    /// Greyed in either order, each colour keeps its own grey: nothing one colour leaves answers for the next
    func testColoursGreyedInEitherOrderEachTakeTheirOwn() {
        let a = [171.0, 184, 201, 255], b = [28.0, 39, 54, 255]
        for (first, second) in [(a, b), (b, a)] {
            _ = TintDimming.grey(first)
            XCTAssertEqual(TintDimming.grey(second)[0], uikit(srgb(second)).level, accuracy: 0.01)
        }
        // And past the cache's bound (512): 600 distinct hued colours, each its own UIKit grey, then the first again
        let many: [[Double]] = (0..<600).map { (i: Int) -> [Double] in
            let r = Double(i % 200), g = Double(30 + (i * 7) % 200), b = Double(255 - (i / 3) % 25)
            return [r, g, b, 255]
        }
        XCTAssertEqual(Set(many).count, 600)
        XCTAssertTrue(many.allSatisfy { $0[0...2].max()! - $0[0...2].min()! > 24 }, "every one hued, so every one is kept")
        for c in many { XCTAssertEqual(TintDimming.grey(c)[0], uikit(srgb(c)).level, accuracy: 0.01, "\(c)") }
        XCTAssertEqual(TintDimming.grey(many[0])[0], uikit(srgb(many[0])).level, accuracy: 0.01, "the first again, after the bound")
        // A CGColor greys as its channels do, in either order, and again from what is kept
        for (first, second) in [(a, b), (b, a)] {
            _ = TintDimming.grey(srgb(first).cgColor)
            for _ in 0..<2 {
                let grey = try? XCTUnwrap(TintDimming.grey(srgb(second).cgColor).components)
                XCTAssertEqual(Double(grey?[0] ?? -1) * 255, uikit(srgb(second)).level, accuracy: 0.01)
            }
        }
    }

    /// A view in a window that UIKit dims greys its tint as a new detached view does: the sweep's views stand for
    /// the ones on screen
    func testAWindowsDimmedViewGreysAsADetachedOne() throws {
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        defer { window.isHidden = true }
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        let view = UIView()
        window.rootViewController?.view.addSubview(view)
        for c in [[171.0, 184, 201, 255], [255, 59, 48, 255], [0, 122, 255, 153]] {
            view.tintColor = srgb(c)
            view.tintAdjustmentMode = .dimmed
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            XCTAssertTrue(view.tintColor.resolvedColor(with: view.traitCollection).getRed(&r, green: &g, blue: &b, alpha: &a))
            XCTAssertEqual(Double(r) * 255, uikit(srgb(c)).level, accuracy: 0.01, "\(c)")
            view.tintAdjustmentMode = .automatic
        }
    }

    /// A colour outside sRGB (display-p3, translucent too) greys from its own space, as UIKit greys it: as a CGColor,
    /// and as `textChannels`' nine numbers (its sRGB channels, then its space's code and its own channels)
    func testAWideGamutColourGreysFromItsOwnSpace() throws {
        for p3 in [[1.0, 0, 0, 1], [0, 1, 0, 1], [0.2, 0.4, 1, 0.6], [1.5, 0.2, 0, 1], [-0.1, 0.9, 0.3, 0.8]] {
            // In extended display-p3, as TextEngine.color draws it (UIColor(displayP3Red:) clamps to 0...1)
            let wide = UIColor(cgColor: try XCTUnwrap(CGColor(colorSpace: try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedDisplayP3)), components: p3.map { CGFloat($0) })))
            let want = uikit(wide).level
            let viaCG = try XCTUnwrap(TintDimming.grey(wide.cgColor).converted(to: CGColorSpace(name: CGColorSpace.extendedSRGB)!, intent: .defaultIntent, options: nil)?.components)
            XCTAssertEqual(Double(viaCG[0]) * 255, want, accuracy: 0.01, "\(p3) as a CGColor")
            XCTAssertEqual(Double(viaCG[3]), p3[3], accuracy: 0.0001, "its own alpha")
            let rgb = try XCTUnwrap(wide.cgColor.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil)?.components)
            let nine = rgb.map { Double($0) * 255 } + [1] + p3
            let grey = TintDimming.grey(nine)
            XCTAssertEqual(grey.count, 4)
            XCTAssertEqual(grey[0], want, accuracy: 0.01, "\(p3) as textChannels' nine numbers")
            XCTAssertEqual(grey[3], nine[3], "its own alpha")
        }
        // A grey past sRGB's white (an untagged extended sRGB red at 2, as a CGColor) comes back unclipped
        let hdr = UIColor(cgColor: CGColor(colorSpace: CGColorSpace(name: CGColorSpace.extendedSRGB)!, components: [2, 0.5, 0.2, 1])!)
        let want = uikit(hdr).level
        XCTAssertGreaterThan(want, 255, "UIKit's grey of it is past white")
        let viaCG = try XCTUnwrap(TintDimming.grey(hdr.cgColor).converted(to: CGColorSpace(name: CGColorSpace.extendedSRGB)!, intent: .defaultIntent, options: nil)?.components)
        XCTAssertEqual(Double(viaCG[0]) * 255, want, accuracy: 0.01)
        // As nine numbers too: past white, nine in extended sRGB, unclipped
        let grey = TintDimming.grey([255.0, 128, 51, 255, 0, 2, 0.5, 0.2, 1])
        XCTAssertEqual(grey.count, 9, "in extended sRGB: \(grey)")
        XCTAssertEqual(grey[5] * 255, want, accuracy: 0.01)
    }

    /// An HDR colour as the painters tag it (`ColorRange.tagged`, its content headroom) greys as UIKit greys it, which
    /// ignores the tag (Core Graphics' grey of the tagged colour would tone-map it): tagged and untagged give the same
    /// grey, in either order, and a grey past white comes back tagged for the layer that shows it
    func testATaggedHDRColourGreysAsUIKitGreysIt() throws {
        guard #available(iOS 26, tvOS 26, *) else { throw XCTSkip("content headroom tags are iOS 26's") }
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedSRGB))
        // Each order on a colour of its own, so the first grey of each is a cold one (nothing kept answers it)
        for (red, taggedFirst) in [(2.0, false), (2.1, true)] {
            let plain = try XCTUnwrap(CGColor(colorSpace: space, components: [red, 0.5, 0.2, 1])), tagged = ColorRange.tagged(plain)
            XCTAssertGreaterThan(tagged.contentHeadroom, 1, "the painters' tag")
            let want = uikit(UIColor(cgColor: plain)).level
            XCTAssertEqual(uikit(UIColor(cgColor: tagged)).level, want, accuracy: 0.01, "UIKit ignores the tag")
            for c in taggedFirst ? [tagged, plain] : [plain, tagged] {
                let grey = TintDimming.grey(c)
                XCTAssertEqual(Double(try XCTUnwrap(grey.converted(to: space, intent: .defaultIntent, options: nil)?.components?.first)) * 255, want, accuracy: 0.01,
                               "\(red), \(c === tagged ? "tagged" : "untagged"), \(taggedFirst ? "tagged first" : "untagged first")")
                XCTAssertGreaterThan(grey.contentHeadroom, 1, "past white, tagged for its layer")
            }
        }
        // A near-neutral HDR colour stays as it came, tagged or not, judged on its components (the tag's tone-mapping
        // would make it look hued), and as nine numbers
        let bright = try XCTUnwrap(CGColor(colorSpace: space, components: [1, 1, 1.25, 1]))
        for c in [bright, ColorRange.tagged(bright)] { XCTAssertTrue(TintDimming.grey(c) === c, "\(c)") }
        let nine: [Double] = [255, 255, 255, 255, 0, 1, 1, 1.25, 1]
        XCTAssertEqual(TintDimming.grey(nine), nine)
    }
}
#endif
