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
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 850))
        window.rootViewController = controller; window.makeKeyAndVisible()
        session.presenter.viewport.frame = window.bounds
        controller.view.addSubview(session.presenter.viewport)
        let batch = session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size)
        XCTAssertNil(batch.error)
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
            let b = UIButton(configuration: factory)
            ButtonConfigurationIOS.apply(face, to: b, traits: b.traitCollection, accent: nil)
            let c = try XCTUnwrap(b.configuration)
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
    // Independent use of UIKit's public system-spacing API for the reference column.
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
        XCTAssertEqual(try button(session, "nowrap").titleLabel?.numberOfLines, 1)
        XCTAssertEqual(try button(session, "clamped").titleLabel?.numberOfLines, 2)
        XCTAssertEqual(try button(session, "get-app").titleLabel?.numberOfLines, 0)
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
        let cache = ButtonMeasureCache()
        let large = UITraitCollection(traitsFrom: [.init(preferredContentSizeCategory: .large), .init(displayScale: 3)])
        cache.configure(large)
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
            XCTAssertTrue(cache.configure(traits)); XCTAssertEqual(cache.answer(face: data, widthKind: 0, width: 100).provisional, 1)
        }
        XCTAssertEqual(cache.answer(face: try face(["title": ["font_weight": 600]]), widthKind: 0, width: 100).provisional, 1)
        XCTAssertEqual(cache.answer(face: try face(title: "Other"), widthKind: 0, width: 100).provisional, 1)
        XCTAssertEqual(cache.answer(face: data, widthKind: 1, width: 0).provisional, 1)
        XCTAssertEqual(cache.answer(face: data, widthKind: 2, width: 0).provisional, 1)
    }
    func testColdLaunchAndDynamicTypeRemeasurePublishNoProvisionalGeometry() throws {
        let session = try fixture(); defer { session.destroy() }
        XCTAssertGreaterThan(session.buttonMeasurements.misses, 0)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
        let old = try button(session, "sign-in").owner!.bounds.height
        let misses = session.buttonMeasurements.misses
        session.presenter.viewport.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
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
        let glass = id.hasPrefix("glass-")
        let name = glass ? String(id.dropFirst(6)) : id
        var c: UIButton.Configuration
        if glass {
            guard #available(iOS 26, *) else { throw XCTSkip("Glass is iOS 26") }
            c = .glass()
        } else {
            switch name {
            case "defrost-front", "subtitle", "method-email", "method-passkey", "clamped", "open-popover": c = .tinted()
            case "defrost-rear", "resend-code", "check-update", "disabled-authored": c = .gray()
            case "get-app", "try-demo", "subscriptions", "sign-out", "clear": c = .plain()
            case "pointer-override":
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
            "pointer-none": "Passes touch through", "open-dialog": "Open confirmation", "open-popover": "Open popover"]
        let symbols: [String: String] = ["defrost-front": "windshield.front.and.wiper", "defrost-rear": "windshield.rear.and.wiper",
            "control-tile": "lock.fill", "subtitle": "location.fill", "check-update": "arrow.clockwise", "method-email": "envelope.fill",
            "method-passkey": "key.fill", "clear": "xmark.circle.fill", "disabled-authored": "lock.fill"]
        c.title = titles[name]; c.image = symbols[name].flatMap { UIImage(systemName: $0) }
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
        if name == "nowrap" || name == "clamped" { c.titleLineBreakMode = .byTruncatingTail }
        let button = UIButton(configuration: c)
        if name == "check-update" { button.contentHorizontalAlignment = .leading }
        if name == "nowrap" { button.titleLabel?.numberOfLines = 1 }
        if name == "clamped" { button.titleLabel?.numberOfLines = 2 }
        if name != "nowrap" && name != "clamped" { button.titleLabel?.numberOfLines = 0 }
        let titleFont = button.titleLabel?.font ?? .preferredFont(forTextStyle: .body)
        c.imagePadding = standardSpacing()
        c.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: titleFont)
        if name == "control-tile" {
            c.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: 28, weight: .semibold)))
        }
        if name == "control-tile" || name == "disabled-authored" { c.image = c.image?.withTintColor(UIColor(red: 0, green: 128.0 / 255, blue: 0, alpha: 1), renderingMode: .alwaysOriginal) }
        button.configuration = c
        if name == "nowrap" { button.titleLabel?.numberOfLines = 1 }
        if name == "clamped" { button.titleLabel?.numberOfLines = 2 }
        if name != "nowrap" && name != "clamped" { button.titleLabel?.numberOfLines = 0 }
        button.isEnabled = name != "disabled-authored"
        return button
    }
    private func pixels(_ button: UIButton, in host: UIView) throws -> (UIImage, [UInt8]) {
        host.addSubview(button)
        button.layoutIfNeeded()
        let format = UIGraphicsImageRendererFormat(); format.scale = 3; format.opaque = true
        let image = UIGraphicsImageRenderer(size: button.bounds.size, format: format).image { context in
            UIColor.white.setFill(); context.fill(button.bounds)
            button.drawHierarchy(in: button.bounds, afterScreenUpdates: true)
        }
        let cg = try XCTUnwrap(image.cgImage)
        var bytes = [UInt8](repeating: 0, count: cg.width * cg.height * 4)
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let context = try XCTUnwrap(CGContext(data: &bytes, width: cg.width, height: cg.height, bitsPerComponent: 8,
            bytesPerRow: cg.width * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        button.removeFromSuperview()
        return (image, bytes)
    }
    func testFixtureRowsPixelDiffAgainstHandConfiguredUIKit() throws {
        let session = try fixture(); defer { session.destroy() }
        let host = try XCTUnwrap(window.rootViewController?.view)
        let nine = ["sign-in", "resend-code", "method-email", "method-passkey", "get-app", "try-demo", "subscriptions", "sign-out", "clear"]
        let names = ["defrost-front", "defrost-rear", "control-tile", "check-update", "subtitle", "radius-padding", "disabled-authored",
            "nowrap", "clamped", "pointer-override", "pointer-none", "open-dialog", "open-popover"] + nine + nine.map { "glass-" + $0 }
        for name in names {
            let native = try button(session, name), hand = try reference(name)
            let size = native.bounds.size
            XCTAssertGreaterThan(size.width, 0, name); XCTAssertGreaterThan(size.height, 0, name)
            let fit = hand.systemLayoutSizeFitting(CGSize(width: size.width, height: 0), withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
            XCTAssertEqual(size.height, ceil(fit.height * 3) / 3, accuracy: 1, name + " fitting height")
            native.frame = CGRect(origin: CGPoint(x: 20, y: 100), size: size)
            hand.frame = native.frame
            let actual = try pixels(native, in: host), expected = try pixels(hand, in: host)
            XCTAssertEqual(actual.1.count, expected.1.count, name)
            guard actual.1.count == expected.1.count else { continue }
            var different = 0, maximum = 0
            for i in stride(from: 0, to: actual.1.count, by: 4) {
                let delta = (0..<3).map { abs(Int(actual.1[i + $0]) - Int(expected.1[i + $0])) }.max()!
                maximum = max(maximum, delta)
                if delta > 8 { different += 1 }
            }
            let fraction = Double(different) / Double(actual.1.count / 4)
            print("button-pixel-diff \(name): \(different)/\(actual.1.count / 4) pixels >8, \(String(format: "%.4f", fraction * 100))%, max \(maximum)")
            let attachment = XCTAttachment(image: actual.0); attachment.name = name + " native"; attachment.lifetime = .keepAlways; add(attachment)
            let reference = XCTAttachment(image: expected.0); reference.name = name + " hand UIKit"; reference.lifetime = .keepAlways; add(reference)
            XCTAssertLessThanOrEqual(fraction, 0.02, name + " native and reference pixels")
        }
    }

}
#endif
