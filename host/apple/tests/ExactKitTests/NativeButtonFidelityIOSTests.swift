// @ref LLP 1069.011.001 §4: the compiled Contract fixture beside hand-configured UIKit.
#if os(iOS)
import UIKit
import CExact
import XCTest
@testable import ExactKit

final class NativeButtonFidelityIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }
    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_BUTTONS_PLAN"])
        let session = ExactApp.shared.makeSession(label: "button-fidelity")
        let controller = UIViewController()
        if let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first {
            window = UIWindow(windowScene: scene)
        } else {
            // SwiftPM's unhosted xctest process has no UIWindowScene.
            window = UIWindow(frame: .zero)
        }
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 850)
        window.traitOverrides.preferredContentSizeCategory = .large
        window.overrideUserInterfaceStyle = .light
        window.backgroundColor = .white
        window.rootViewController = controller; window.makeKeyAndVisible()
        window.updateTraitsIfNeeded()
        session.presenter.viewport.frame = window.bounds
        controller.view.addSubview(session.presenter.viewport)
        let batch = session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size)
        XCTAssertNil(batch.error)
        window.layoutIfNeeded(); CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        return session
    }
    private func button(_ session: ExactSession, _ name: String) throws -> NativeButtonIOS {
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == name }, name)
        return try XCTUnwrap(session.presenter.controls.controls[node.id] as? NativeButtonIOS, name)
    }
    private func face(_ rows: [String: Any] = [:], title: String = "Title", symbol: String? = "lock.fill") throws -> Data {
        try JSONSerialization.data(withJSONObject: ["title": title, "symbol": symbol as Any? ?? NSNull(),
            "ios": "filled", "style": "filled", "rows": rows], options: [.sortedKeys])
    }
    func testAbsentRowsKeepTheFactoryExceptItsRuntimeGapAndSymbolFont() throws {
        for name in ["plain", "gray", "tinted", "filled", "bordered", "glass"] {
            var face = ButtonFace(); face.title = "Title"; face.symbol = "lock.fill"; face.ios = name
            let factory = ControlHost.configuration(face).0
            let untouched = UIButton(configuration: factory)
            untouched.configuration?.title = "Title"
            let b = UIButton(configuration: factory)
            ButtonConfigurationIOS.apply(face, to: b, traits: b.traitCollection, accent: nil)
            let c = try XCTUnwrap(b.configuration)
            XCTAssertEqual(b.titleLabel?.numberOfLines, untouched.titleLabel?.numberOfLines, name)
            XCTAssertEqual(c.buttonSize, factory.buttonSize, name)
            XCTAssertEqual(c.cornerStyle, factory.cornerStyle, name)
            XCTAssertEqual(c.contentInsets, factory.contentInsets, name)
            XCTAssertEqual(c.titleLineBreakMode, factory.titleLineBreakMode, name)
            XCTAssertEqual(c.titleAlignment, factory.titleAlignment, name)
            XCTAssertNil(c.baseForegroundColor, name)
            XCTAssertNil(c.titleTextAttributesTransformer, name)
            XCTAssertNil(c.subtitleTextAttributesTransformer, name)
            XCTAssertEqual(c.imagePadding, standardSpacing(), accuracy: 0.001, name)
            XCTAssertNotNil(c.preferredSymbolConfigurationForImage, name)
        }
    }
    func testClearingControlSizeRestoresTheFactorySize() throws {
        for style in ["plain", "gray", "tinted", "filled", "bordered", "glass"] {
            var f = ButtonFace(); f.title = "Size"; f.ios = style
            let factory = ControlHost.configuration(f).0
            let b = UIButton(configuration: factory)
            f.rows.button["control_size"] = .string("large")
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            XCTAssertEqual(b.configuration?.buttonSize, .large)
            f.rows.button = [:]
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            XCTAssertEqual(b.configuration?.buttonSize, factory.buttonSize, style)
        }
    }
    func testResolvedEmFontIsNotScaledTwiceAtAccessibilitySize() throws {
        let session = try fixture(); defer { session.destroy() }
        window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
        window.updateTraitsIfNeeded()
        let surface = session.presenter.viewport
        surface.updateTraitsIfNeeded()
        var config = UIButton.Configuration.filled(); config.buttonSize = .large; config.title = "Title"
        let reference = UIButton(configuration: config)
        surface.addSubview(reference); reference.updateTraitsIfNeeded()
        let expected = try XCTUnwrap(reference.titleLabel?.font)
        XCTAssertEqual(session.buttonMeasurements.font(4).pointSize, expected.pointSize, accuracy: 0.01)
        var f = ButtonFace(); f.title = "Title"; f.ios = "filled"
        f.rows.button["control_size"] = .string("large")
        // The kernel resolved 1em against the already scaled large control font.
        f.rows.title["font_size"] = .number(Double(expected.pointSize))
        f.rows.title["font_size_resolved"] = .number(1)
        let actual = UIButton(configuration: .filled()); surface.addSubview(actual)
        ButtonConfigurationIOS.apply(f, to: actual, traits: surface.traitCollection, accent: nil)
        XCTAssertEqual(try XCTUnwrap(actual.titleLabel?.font).pointSize, expected.pointSize, accuracy: 0.01)
        f.rows.title = ["font_size": .number(13)]
        ButtonConfigurationIOS.apply(f, to: actual, traits: surface.traitCollection, accent: nil)
        XCTAssertEqual(try XCTUnwrap(actual.titleLabel?.font).pointSize,
            UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: 13), compatibleWith: surface.traitCollection).pointSize, accuracy: 0.01)
    }
    func testNonuniformResolvedCornersReportAStandIn() throws {
        var f = ButtonFace(); f.title = "Corners"
        f.rows.button = ["border_radius_top_left": .number(8), "border_radius_top_right": .number(12),
            "border_radius_bottom_right": .number(16), "border_radius_bottom_left": .number(20)]
        let b = UIButton(configuration: .bordered())
        ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
        let row = try XCTUnwrap(ButtonConfigurationIOS.observation(f, button: b)["border-radius"] as? [String: Any])
        XCTAssertNotNil(row["standIn"], "UIKit cannot draw four independent configuration corners")
        XCTAssertEqual(b.configuration?.cornerStyle, UIButton.Configuration.bordered().cornerStyle)
    }
    func testCornerRadiusDoesNotChangeTheFittingAnswer() throws {
        let session = try fixture(); defer { session.destroy() }
        let traits = session.presenter.viewport.traitCollection
        for style in ["plain", "gray", "tinted", "filled", "bordered", "glass"] {
            for width in [CGFloat(100), 200] {
                var f = ButtonFace(); f.ios = style; f.title = "A long title that wraps"
                var heights: [Float] = []
                for radius in [Double(0), 6, 18, 80] {
                    for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
                        f.rows.button["border_radius_" + corner] = .number(radius)
                    }
                    heights.append(session.buttonMeasurements.measure(f, widthKind: 0, width: width, traits: traits).height)
                }
                XCTAssertTrue(heights.allSatisfy { $0 == heights[0] }, "radius changes drawing, not height-for-width: \(style) \(width)")
            }
        }
    }
    // Independent use of UIKit's public system-spacing API for the reference column.
    func testMinContentUsesUnbreakableRunsAndTheNativeChrome() throws {
        let session = try fixture(); defer { session.destroy() }
        let traits = session.presenter.viewport.traitCollection
        for placement in ["leading", "top"] {
            var f = ButtonFace(); f.ios = "filled"; f.title = "A long title that wraps"
            f.symbol = "lock.fill"; f.placement = placement
            f.rows.button = ["padding_left": .number(22), "padding_right": .number(22)]
            let maximum = session.buttonMeasurements.measure(f, widthKind: 2, width: 0, traits: traits)
            let minimum = session.buttonMeasurements.measure(f, widthKind: 1, width: 0, traits: traits)
            var run = f; run.title = "wraps"
            let expected = session.buttonMeasurements.measure(run, widthKind: 2, width: 0, traits: traits)
            XCTAssertEqual(minimum.width, expected.width, accuracy: 0.5, placement)
            XCTAssertLessThan(minimum.width, maximum.width, placement)
            let fitted = session.buttonMeasurements.measure(f, widthKind: 0, width: CGFloat(minimum.width), traits: traits)
            XCTAssertEqual(minimum.height, fitted.height, accuracy: 0.5)
            f.rows.title["line_clamp"] = .number(2)
            XCTAssertEqual(session.buttonMeasurements.measure(f, widthKind: 1, width: 0, traits: traits).width, minimum.width, accuracy: 0.5)
            f.rows.title["white_space"] = .string("nowrap")
            let nowrap = session.buttonMeasurements.measure(f, widthKind: 1, width: 0, traits: traits)
            XCTAssertEqual(nowrap.width, maximum.width, accuracy: 0.5, "CSS nowrap has no soft breaks")
        }
        var f = ButtonFace(); f.title = "One\u{00a0}unbroken run"; f.ios = "filled"
        let minimum = session.buttonMeasurements.measure(f, widthKind: 1, width: 0, traits: traits)
        f.title = "One\u{00a0}unbroken"
        XCTAssertEqual(minimum.width, session.buttonMeasurements.measure(f, widthKind: 2, width: 0, traits: traits).width, accuracy: 0.5)
        f.title = "A long title that wraps"; f.subtitle = "A secondary unbreakable phrase"
        f.rows.subtitle["font_size"] = .number(23)
        let subtitleMinimum = session.buttonMeasurements.measure(f, widthKind: 1, width: 0, traits: traits)
        f.title = "wraps"; f.subtitle = "unbreakable"
        XCTAssertEqual(subtitleMinimum.width, session.buttonMeasurements.measure(f, widthKind: 2, width: 0, traits: traits).width, accuracy: 0.5,
            "the subtitle uses its own native font and line-break runs")
    }

    private func standardSpacing() -> CGFloat {
        let parent = UIView(), a = UIView(), b = UIView()
        for v in [a, b] { v.translatesAutoresizingMaskIntoConstraints = false; parent.addSubview(v) }
        NSLayoutConstraint.activate([a.leadingAnchor.constraint(equalTo: parent.leadingAnchor), a.widthAnchor.constraint(equalToConstant: 0),
            b.leadingAnchor.constraint(equalToSystemSpacingAfter: a.trailingAnchor, multiplier: 1), b.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            b.widthAnchor.constraint(equalToConstant: 0), a.topAnchor.constraint(equalTo: parent.topAnchor), b.topAnchor.constraint(equalTo: parent.topAnchor),
            a.heightAnchor.constraint(equalToConstant: 0), b.heightAnchor.constraint(equalToConstant: 0), a.bottomAnchor.constraint(equalTo: parent.bottomAnchor)])
        return parent.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize).width
    }
    func testSemanticFieldsSizesCornersInsetsAndAlignment() throws {
        let session = try fixture(); defer { session.destroy() }
        let sign = try button(session, "sign-in")
        XCTAssertEqual(sign.configuration?.buttonSize, .large)
        XCTAssertEqual(sign.configuration?.cornerStyle, .capsule)
        XCTAssertEqual(TextEngine.controlWeight(try XCTUnwrap(sign.titleLabel?.font)), 600)
        let tile = try button(session, "control-tile")
        XCTAssertEqual(tile.configuration?.imagePlacement, .top)
        XCTAssertEqual(tile.configuration?.background.cornerRadius, 18)
        XCTAssertEqual(tile.configuration?.image?.renderingMode, .alwaysOriginal)
        let subtitle = try button(session, "subtitle")
        XCTAssertEqual(subtitle.configuration?.subtitle, "Updated just now")
        let radius = try button(session, "radius-padding")
        XCTAssertEqual(radius.configuration?.cornerStyle, .fixed, "radius wins over capsule")
        XCTAssertEqual(radius.configuration?.background.cornerRadius, 18)
        XCTAssertEqual(radius.configuration?.contentInsets, NSDirectionalEdgeInsets(top: 10, leading: 22, bottom: 10, trailing: 22))
        XCTAssertEqual(radius.frame, radius.owner?.bounds, "padding is inside the native button once")
        let start = try button(session, "check-update")
        XCTAssertEqual(start.contentHorizontalAlignment, .leading)
        XCTAssertEqual(start.configuration?.titleAlignment, .leading)
        for (align, expected, title) in [("end", UIControl.ContentHorizontalAlignment.trailing, UIButton.Configuration.TitleAlignment.trailing),
            ("left", .left, .leading), ("right", .right, .trailing), ("center", .center, .center)] {
            var f = ButtonFace(); f.title = "Aligned"; f.rows.title["text_align"] = .string(align)
            let b = UIButton(configuration: .gray())
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            XCTAssertEqual(b.contentHorizontalAlignment, expected); XCTAssertEqual(b.configuration?.titleAlignment, title)
        }
        for (size, expected) in [("mini", UIButton.Configuration.Size.mini), ("small", .small), ("medium", .medium), ("large", .large)] {
            var f = ButtonFace(); f.title = "Sized"; f.rows.button["control_size"] = .string(size)
            let b = UIButton(configuration: .gray())
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            XCTAssertEqual(b.configuration?.buttonSize, expected)
        }
        for (corner, expected) in [("dynamic", UIButton.Configuration.CornerStyle.dynamic), ("small", .small), ("medium", .medium), ("large", .large), ("capsule", .capsule)] {
            var f = ButtonFace(); f.title = "Corners"; f.rows.button["control_corner_style"] = .string(corner)
            let b = UIButton(configuration: .gray())
            ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
            XCTAssertEqual(b.configuration?.cornerStyle, expected)
        }
    }
    func testWrappingClampFontAxesAndSymbolRows() throws {
        let session = try fixture(); defer { session.destroy() }
        XCTAssertEqual(try button(session, "nowrap").configuration?.titleLineBreakMode, .byTruncatingTail)
        XCTAssertEqual(try button(session, "clamped").titleLabel?.numberOfLines, 2)
        var f = ButtonFace(json: try face(["title": ["font_size": 23], "symbol": ["font_size": 31, "font_weight": 700, "tint_color": [0, 255, 0, 255]]]))
        let b = UIButton(configuration: .filled())
        ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
        XCTAssertEqual(b.titleLabel?.font.pointSize ?? 0, 23, accuracy: 0.01)
        XCTAssertEqual(b.configuration?.image?.renderingMode, .alwaysOriginal)
        let symbol = try XCTUnwrap(b.configuration?.preferredSymbolConfigurationForImage)
        XCTAssertEqual(symbol, UIImage.SymbolConfiguration(font: UIFontMetrics(forTextStyle: .body).scaledFont(for: UIFont.systemFont(ofSize: 31, weight: .bold))))
        f.rows.title = ["font_weight": .number(700)]
        let reference = UIButton(configuration: .filled()); reference.configuration?.title = "Title"
        ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
        XCTAssertEqual(b.titleLabel?.font.pointSize, reference.titleLabel?.font.pointSize, "weight alone keeps UIKit's size")
        XCTAssertEqual(TextEngine.controlWeight(try XCTUnwrap(b.titleLabel?.font)), 700)
        f.rows.title = [:]; f.rows.symbol = [:]
        ButtonConfigurationIOS.apply(f, to: b, traits: b.traitCollection, accent: nil)
        XCTAssertEqual(b.configuration?.image?.renderingMode, .automatic, "clearing a symbol tint restores inheritance")
    }
    func testCacheIncludesFaceRowsStyleOfferAndAllTraitsAndFitsHeightForWidth() throws {
        let session = try fixture(); defer { session.destroy() }
        let cache = ButtonMeasureCache()
        let large = UITraitCollection(traitsFrom: [.init(preferredContentSizeCategory: .large), .init(displayScale: 3)])
        cache.configure(large, in: session.presenter.viewport)
        let data = try face(title: "Send this very long message to everyone in the group")
        let first = cache.answer(face: data, widthKind: 0, width: 100)
        XCTAssertEqual(first.provisional, 1)
        let exact = cache.answer(face: data, widthKind: 0, width: 100)
        XCTAssertEqual(exact.provisional, 0); XCTAssertEqual(cache.misses, 1)
        let wide = cache.answer(face: data, widthKind: 0, width: 300)
        XCTAssertEqual(wide.provisional, 1)
        XCTAssertGreaterThan(exact.height, wide.height, "real UIKit wrapping changes height with width")
        for traits in [UITraitCollection(traitsFrom: [large, .init(legibilityWeight: .bold)]),
            UITraitCollection(traitsFrom: [large, .init(displayScale: 2)]),
            UITraitCollection(traitsFrom: [large, .init(preferredContentSizeCategory: .accessibilityExtraExtraLarge)])] {
            XCTAssertTrue(cache.configure(traits, in: session.presenter.viewport)); XCTAssertEqual(cache.answer(face: data, widthKind: 0, width: 100).provisional, 1)
        }
        XCTAssertEqual(cache.answer(face: try face(["title": ["font_weight": 600]]), widthKind: 0, width: 100).provisional, 1)
        XCTAssertEqual(cache.answer(face: try face(title: "Other"), widthKind: 0, width: 100).provisional, 1)
        XCTAssertEqual(cache.answer(face: data, widthKind: 1, width: 0).provisional, 1)
        XCTAssertEqual(cache.answer(face: data, widthKind: 2, width: 0).provisional, 1)
    }
    func testHeightForWidthAgainstRequiredWidthWindowLayout() throws {
        let session = try fixture(); defer { session.destroy() }
        let surface = session.presenter.viewport
        let cache = ButtonMeasureCache()
        for category in [UIContentSizeCategory.large, .accessibilityExtraExtraLarge] {
            window.traitOverrides.preferredContentSizeCategory = category
            window.updateTraitsIfNeeded(); surface.updateTraitsIfNeeded()
            cache.configure(surface.traitCollection, in: surface)
            for style in ["plain", "gray", "tinted", "filled", "bordered", "glass"] {
                for kind in ["wrapped", "subtitle", "nowrap", "clamped"] {
                    var f = ButtonFace()
                    f.ios = style; f.title = "Send this very long message to everyone in the group"
                    if kind == "subtitle" { f.subtitle = "Updated just now with a longer subtitle that wraps too" }
                    if kind == "nowrap" { f.rows.title["white_space"] = .string("nowrap") }
                    if kind == "clamped" { f.rows.title["line_clamp"] = .number(2) }
                    var heights: [CGFloat] = []
                    for width in [CGFloat(120), 200, 320] {
                        let b = UIButton(configuration: ControlHost.configuration(f).0)
                        surface.addSubview(b)
                        b.updateTraitsIfNeeded()
                        ButtonConfigurationIOS.apply(f, to: b, traits: surface.traitCollection, accent: nil)
                        b.translatesAutoresizingMaskIntoConstraints = false
                        let constraint = b.widthAnchor.constraint(equalToConstant: width)
                        constraint.isActive = true
                        surface.layoutIfNeeded(); b.layoutIfNeeded()
                        let measured = cache.measure(f, widthKind: 0, width: width, traits: surface.traitCollection)
                        let label = "\(category.rawValue) \(style) \(kind) \(width)"
                        XCTAssertEqual(CGFloat(measured.height), b.frame.height, accuracy: 0.5, label)
                        heights.append(b.frame.height)
                        b.removeFromSuperview()
                    }
                    if kind == "nowrap" { XCTAssertEqual(heights.first!, heights.last!, accuracy: 0.5) }
                    else if kind != "clamped" { XCTAssertGreaterThan(heights.first!, heights.last!, "real height-for-width \(style) \(kind)") }
                    if kind == "clamped" {
                        var unbounded = f; unbounded.rows.title = [:]
                        let full = cache.measure(unbounded, widthKind: 0, width: 120, traits: surface.traitCollection)
                        XCTAssertLessThan(heights.first!, CGFloat(full.height), "clamp limits the required-width height")
                    }
                }
            }
        }
    }
    func testColdLaunchAndDynamicTypeRemeasurePublishNoProvisionalGeometry() throws {
        let session = try fixture(); defer { session.destroy() }
        XCTAssertGreaterThan(session.buttonMeasurements.misses, 0)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
        let old = try button(session, "sign-in").owner!.bounds.height
        let misses = session.buttonMeasurements.misses
        window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
        window.updateTraitsIfNeeded(); session.presenter.viewport.updateTraitsIfNeeded()
        session.controlTextChanged()
        XCTAssertGreaterThan(session.buttonMeasurements.misses, misses)
        XCTAssertGreaterThan(try button(session, "sign-in").owner!.bounds.height, old)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
        let before = session.presenter.views.mapValues { $0.frame }
        session.presenter.controls.sync()
        XCTAssertEqual(session.presenter.views.mapValues { $0.frame }, before, "no later intrinsic feedback moves a button")
    }
    func testDisabledColoursPointerEventsAndNativeInvokerActivateOnce() throws {
        let session = try fixture(); defer { session.destroy() }
        let disabled = try button(session, "disabled-authored")
        XCTAssertFalse(disabled.isEnabled)
        XCTAssertEqual(disabled.configuration?.image?.renderingMode, .alwaysOriginal)
        XCTAssertNotNil(disabled.configuration?.baseForegroundColor)
        XCTAssertTrue(try button(session, "pointer-override").isUserInteractionEnabled)
        XCTAssertFalse(try button(session, "pointer-none").isUserInteractionEnabled)
        XCTAssertFalse(try button(session, "open-dialog").showsMenuAsPrimaryAction)
        XCTAssertEqual(try button(session, "open-dialog").owner!.subviews.compactMap { $0 as? UIButton }.count, 1, "no invoker overlay steals activation")
        var count = 0
        session.presenter.onPress = { _ in count += 1 }
        try button(session, "open-dialog").sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(count, 1)
        XCTAssertNotNil(session.presenter.menus.observation(), "native confirmation is presented after the same activation")
    }
    /// Intents copied from the Contract fixture, configured without the production mapper.
    private func reference(_ id: String) throws -> UIButton {
        let factories: [String: UIButton.Configuration] = [
            "plain": .plain(), "gray": .gray(), "tinted": .tinted(), "filled": .filled(),
            "borderless": .borderless(), "bordered": .bordered(), "bordered-tinted": .borderedTinted(),
            "bordered-prominent": .borderedProminent()]
        var special = factories[id]
        if #available(iOS 26, *) {
            let glassFactories: [String: UIButton.Configuration] = ["glass": .glass(), "prominent-glass": .prominentGlass(),
                "clear-glass": .clearGlass(), "prominent-clear-glass": .prominentClearGlass()]
            special = special ?? glassFactories[id]
        }
        if var c = special {
            c.title = id
            c.imagePadding = standardSpacing()
            let b = UIButton(configuration: c)
            c.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: try XCTUnwrap(b.titleLabel?.font))
            b.configuration = c
            return b
        }
        let glass = id.hasPrefix("glass-")
        let name = glass ? String(id.dropFirst(6)) : id
        var c: UIButton.Configuration
        if glass {
            guard #available(iOS 26, *) else { throw XCTSkip("Glass is iOS 26") }
            c = .glass()
        } else {
            switch name {
            case "a-tinted", "title-symbol", "defrost-front", "subtitle", "method-email", "method-passkey", "clamped", "open-popover": c = .tinted()
            case "a-gray", "defrost-rear", "resend-code", "check-update", "disabled-authored": c = .gray()
            case "a-plain", "get-app", "try-demo", "subscriptions", "sign-out", "clear": c = .plain()
            case "a-bordered", "stretched": c = .bordered()
            case "wide": c = .borderedProminent()
            case "a-clear":
                guard #available(iOS 26, *) else { throw XCTSkip("Glass is iOS 26") }
                c = .clearGlass()
            case "symbol-only", "a-glass", "a-sf", "g1", "g2", "g3", "pointer-override":
                guard #available(iOS 26, *) else { throw XCTSkip("Glass is iOS 26") }
                c = .glass()
            default: c = .filled()
            }
        }
        let titles: [String: String] = ["defrost-front": "Front Defrost", "defrost-rear": "Rear Defrost", "control-tile": "Lock",
            "subtitle": "Last Parked", "check-update": "Check for update", "sign-in": "Sign In", "resend-code": "Resend Code",
            "method-email": "Email", "method-passkey": "Passkey", "get-app": "Get the Lexus app for your vehicle",
            "try-demo": "Try Demo", "subscriptions": "Manage Subscriptions", "sign-out": "Sign Out",
            "radius-padding": "Radius 18", "disabled-authored": "Disabled colours", "nowrap": "A long title truncated at the end",
            "clamped": "A long title that stops after at most two lines", "pointer-override": "Takes touch",
            "pointer-none": "Passes touch through", "open-dialog": "Open confirmation", "open-popover": "Open popover", "symbol-title": "Send",
            "title-symbol": "Forward", "wide": "Fixed 240 wide", "a-plain": "plain", "a-gray": "gray",
            "a-bordered": "bordered", "a-glass": "glass", "a-clear": "clear", "a-tinted": "tinted",
            "narrow": "Send this very long message", "stretched": "Stretched (default bordered)",
            "disabled": "Disabled", "g1": "Lock", "g2": "Unlock", "g3": "Fade"]
        let symbols: [String: String] = ["defrost-front": "windshield.front.and.wiper", "defrost-rear": "windshield.rear.and.wiper",
            "control-tile": "lock.fill", "subtitle": "location.fill", "check-update": "arrow.clockwise", "method-email": "envelope.fill",
            "method-passkey": "key.fill", "clear": "xmark.circle.fill", "disabled-authored": "lock.fill", "symbol-only": "plus", "symbol-title": "arrow.up",
            "title-symbol": "arrowshape.turn.up.right", "a-plain": "arrow.up", "a-gray": "arrow.up", "a-bordered": "arrow.up",
            "a-glass": "arrow.up", "a-clear": "arrow.up", "a-tinted": "arrow.up", "a-sf": "paperplane.fill"]
        c.title = titles[name]; c.image = symbols[name].flatMap { UIImage(systemName: $0) }
        if name == "title-symbol" { c.imagePlacement = .trailing }
        if name == "subtitle" { c.subtitle = "Updated just now" }
        if name == "control-tile" { c.imagePlacement = .top; c.cornerStyle = .fixed; c.background.cornerRadius = 18 }
        if name == "sign-in" { c.buttonSize = .large; c.cornerStyle = .capsule }
        let small = ["get-app", "try-demo", "subscriptions", "sign-out", "clear"].contains(name)
        if small || name == "sign-in" {
            c.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { input in
                var output = input
                if let font = input.uiKit.font {
                    output.uiKit.font = small
                        ? UIFontMetrics(forTextStyle: .body).scaledFont(for: UIFont.systemFont(ofSize: 13, weight: .semibold))
                        : UIFont.systemFont(ofSize: font.pointSize, weight: .semibold)
                }
                return output
            }
        }
        if name == "resend-code" || name == "disabled-authored" {
            c.baseForegroundColor = .systemBlue
            c.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { input in
                var output = input; output.uiKit.foregroundColor = .systemBlue; return output
            }
        }
        if name == "check-update" { c.titleAlignment = .leading }
        if name == "radius-padding" { c.cornerStyle = .fixed; c.background.cornerRadius = 18; c.contentInsets = .init(top: 10, leading: 22, bottom: 10, trailing: 22) }
        if name == "nowrap" { c.titleLineBreakMode = .byTruncatingTail }
        let button = UIButton(configuration: c)
        if name == "check-update" { button.contentHorizontalAlignment = .leading }
        if name == "clamped" { button.titleLabel?.numberOfLines = 2; button.titleLabel?.lineBreakMode = .byTruncatingTail }
        let titleFont = button.titleLabel?.font ?? .preferredFont(forTextStyle: .body)
        c.imagePadding = standardSpacing()
        c.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: titleFont)
        if name == "control-tile" {
            c.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: 28, weight: .semibold)))
        }
        if name == "control-tile" || name == "disabled-authored" { c.image = c.image?.withTintColor(UIColor(red: 0, green: 128.0 / 255, blue: 0, alpha: 1), renderingMode: .alwaysOriginal) }
        button.configuration = c
        if name == "clamped" { button.titleLabel?.numberOfLines = 2; button.titleLabel?.lineBreakMode = .byTruncatingTail }
        if ["wide", "a-plain", "a-gray", "a-bordered", "a-glass", "a-clear", "a-tinted", "a-sf"].contains(name) {
            button.tintColor = UIColor(red: 52.0 / 255, green: 199.0 / 255, blue: 89.0 / 255, alpha: 1)
        }
        button.isEnabled = name != "disabled-authored" && name != "disabled"
        return button
    }
    private func assertPresentedHeight(_ native: UIButton, kernel: NodeView, expected: CGFloat, name: String,
                                       file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(native.bounds.height, kernel.bounds.height, accuracy: 0.01,
            name + " presented height equals kernel frame", file: file, line: line)
        XCTAssertEqual(native.bounds.height, expected, accuracy: 0.5,
            name + " presented height equals independent UIKit height", file: file, line: line)
    }
    func testPresentedBoundsMismatchFailsEvenWhenTheProbeStillFits() throws {
        let session = try fixture(); defer { session.destroy() }
        let native = try button(session, "sign-in"), owner = try XCTUnwrap(native.owner)
        let hand = try reference("sign-in")
        window.rootViewController?.view.addSubview(hand); defer { hand.removeFromSuperview() }
        let expected = hand.sizeThatFits(CGSize(width: native.bounds.width, height: .greatestFiniteMagnitude)).height
        let f = try XCTUnwrap(native.written?.face)
        let measured = session.buttonMeasurements.measure(f, widthKind: 0, width: native.bounds.width, traits: native.traitCollection)
        XCTAssertEqual(CGFloat(measured.height), expected, accuracy: 0.5, "a fresh probe alone cannot detect a wrong presented bound")
        let original = native.bounds
        defer { native.bounds = original }
        native.bounds.size.height += 5
        // Strict expected failure also fails if the presented-bounds assertions
        // are removed: exercise the same assertions the fixture uses.
        XCTExpectFailure("deliberately corrupt the presented height while the probe remains correct") {
            assertPresentedHeight(native, kernel: owner, expected: expected, name: "corrupted sign-in")
        }
    }
    func testFixtureMeasurementsAgainstHandConfiguredUIKit() throws {
        let session = try fixture(); defer { session.destroy() }
        let host = try XCTUnwrap(window.rootViewController?.view)
        let nine = ["sign-in", "resend-code", "method-email", "method-passkey", "get-app", "try-demo", "subscriptions", "sign-out", "clear"]
        let names = ["defrost-front", "defrost-rear", "control-tile", "check-update", "subtitle", "radius-padding", "disabled-authored",
            "nowrap", "clamped", "pointer-override", "pointer-none", "open-dialog", "open-popover"] + nine + nine.map { "glass-" + $0 } + [
            "plain", "gray", "tinted", "filled", "borderless", "bordered", "bordered-tinted", "bordered-prominent",
            "glass", "prominent-glass", "clear-glass", "prominent-clear-glass", "symbol-only", "symbol-title", "title-symbol",
            "wide", "a-plain", "a-gray", "a-bordered", "a-glass", "a-clear", "a-tinted", "a-sf", "narrow", "stretched", "disabled", "g1", "g2", "g3"]
        for name in names {
            let native = try button(session, name), hand = try reference(name)
            let size = native.bounds.size
            XCTAssertGreaterThan(size.width, 0, name); XCTAssertGreaterThan(size.height, 0, name)
            host.addSubview(hand); defer { hand.removeFromSuperview() }
            hand.updateTraitsIfNeeded()
            // Entering the hierarchy may reapply configuration and reset label fields.
            if name == "clamped" { hand.titleLabel?.numberOfLines = 2; hand.titleLabel?.lineBreakMode = .byTruncatingTail }
            hand.layoutIfNeeded()
            let f = try XCTUnwrap(native.written?.face)
            let measured = session.buttonMeasurements.measure(f, widthKind: 0, width: size.width, traits: native.traitCollection)
            let expected = hand.sizeThatFits(CGSize(width: size.width, height: .greatestFiniteMagnitude))
            XCTAssertEqual(CGFloat(measured.height), expected.height, accuracy: 0.5, name + " measured height at the same width")
            let owner = try XCTUnwrap(native.owner)
            native.layoutIfNeeded()
            let stretched: Set<String> = ["defrost-front", "defrost-rear", "control-tile", "subtitle", "method-email", "method-passkey",
                "get-app", "try-demo", "subscriptions", "sign-out", "clear"]
            if stretched.contains(name) {
                let row = try XCTUnwrap(owner.superview as? NodeView)
                // A wrapping row has several flex lines; stretch uses the
                // line's cross size, not the whole multi-line container.
                let peers = row.container.subviews.compactMap { $0 as? NodeView }.filter { abs($0.frame.minY - owner.frame.minY) < 0.01 }
                var lineHeight: CGFloat = 0
                for peer in peers {
                    let peerName = try XCTUnwrap(peer.props["testId"])
                    let peerHand = try reference(peerName)
                    host.addSubview(peerHand); peerHand.updateTraitsIfNeeded()
                    lineHeight = max(lineHeight, peerHand.sizeThatFits(CGSize(width: peer.bounds.width, height: .greatestFiniteMagnitude)).height)
                    peerHand.removeFromSuperview()
                }
                assertPresentedHeight(native, kernel: owner, expected: lineHeight, name: name)
                if peers.count == row.container.subviews.count {
                    XCTAssertEqual(native.bounds.height, row.bounds.height, accuracy: 0.01, name + " presented height equals flex row")
                }
            } else {
                assertPresentedHeight(native, kernel: owner, expected: expected.height, name: name)
            }
            if name == "clamped" {
                var c = try XCTUnwrap(hand.configuration); c.title = f.title
                let full = UIButton(configuration: c); host.addSubview(full); defer { full.removeFromSuperview() }
                XCTAssertLessThan(expected.height, full.sizeThatFits(CGSize(width: size.width, height: .greatestFiniteMagnitude)).height,
                    "the independent reference really clamps")
            }
        }
    }

}
#endif
