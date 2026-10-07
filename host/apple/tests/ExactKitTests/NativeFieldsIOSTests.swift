// @ref LLP 1104 D3–D6: configured UIKit geometry and the first-frame seam.
#if os(iOS)
import UIKit
import CExact
import CoreText
import XCTest
@testable import ExactKit

final class NativeFieldsIOSTests: XCTestCase {
    func testChromeCacheCoversKindsFontsAndTraits() {
        let cache = FieldChromeCache()
        let traits = UITraitCollection(traitsFrom: [.init(preferredContentSizeCategory: .large), .init(displayScale: 3)])
        cache.configure(traits)
        let engine = TextEngine(resolve: { _ in nil })
        let id = engine.registerControlFont(cache.body)
        cache.prefill(family: id, font: cache.body, weight: UInt16(TextEngine.controlWeight(cache.body)), italic: false)
        for size in [11, 17, 26, 34] {
            for family in [Int(id), 4] {
                for kind in UInt8(0)...UInt8(3) {
                    let request = ExactFieldChromeRequest(kind: kind, family_id: UInt16(family), size: Float(size), weight: 400, italic: 0)
                    let font = engine.font(size: CGFloat(size), weight: 400, family: family, italic: false)
                    _ = cache.answer(request, font: font)
                    let exact = cache.answer(request, font: font)
                    XCTAssertEqual(exact.provisional, 0)
                    if kind != 3 {
                        let control = UITextField()
                        control.borderStyle = .roundedRect
                        control.font = font
                        control.isSecureTextEntry = kind == 1
                        if kind == 2 { control.keyboardType = .webSearch }
                        control.text = "Hg"
                        XCTAssertGreaterThanOrEqual(exact.minimum_height, Float(font.lineHeight))
                        // Probe constrained frames independently of the cache,
                        // from the platform floor through an authored tall box.
                        for height in [CGFloat(exact.minimum_height), 60, 90] {
                            let bounds = CGRect(x: 0, y: 0, width: 240, height: height)
                            let rect = control.textRect(forBounds: bounds)
                            // The ABI is f32; UIKit's frame arithmetic is f64.
                            XCTAssertEqual(exact.left, Float(rect.minX), accuracy: 0.00001)
                            XCTAssertEqual(exact.right, Float(bounds.maxX - rect.maxX), accuracy: 0.00001)
                            XCTAssertEqual(exact.top, Float(rect.minY), accuracy: 0.00001)
                            XCTAssertEqual(exact.bottom, Float(bounds.maxY - rect.maxY), accuracy: 0.00001)
                        }
                    } else {
                        let view = TextArea()
                        view.configureNativeChrome(true)
                        XCTAssertEqual(exact.left, Float(view.textContainerInset.left))
                        XCTAssertEqual(exact.top, Float(view.textContainerInset.top))
                        XCTAssertEqual(exact.minimum_height, 0)
                    }
                }
            }
        }
        let request = ExactFieldChromeRequest(kind: 0, family_id: id, size: 17, weight: 400, italic: 0)
        let font = engine.font(size: 17, weight: 400, family: Int(id), italic: false)
        cache.configure(.init(traitsFrom: [traits, .init(preferredContentSizeCategory: .accessibilityExtraExtraExtraLarge), .init(legibilityWeight: .bold)]))
        XCTAssertEqual(cache.answer(request, font: font).provisional, 1, "traits form part of the key")
        XCTAssertEqual(cache.answer(request, font: font).provisional, 0)
        cache.presented(false)
        XCTAssertEqual(cache.presentedProvisional, 0)
        cache.presented(true)
        XCTAssertEqual(cache.presentedProvisional, 1, "count presented batches, independent of kernel relayouts")
    }
    func testRegisteredFontIsThePreferredBodyFontAndChangesWithDynamicType() {
        let cache = FieldChromeCache()
        let engine = TextEngine.pair(resolve: { _ in nil })
        engine.fieldChrome = cache
        Owner.shared.sync { engine.measuring.fieldChrome = cache }
        cache.configure(.init(preferredContentSizeCategory: .large))
        let first = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque) }
        let drawn = engine.font(size: CGFloat(first.size), weight: Int(first.weight), family: Int(first.family_id), italic: first.italic != 0)
        XCTAssertEqual(drawn, cache.body)
        XCTAssertEqual(drawn.fontName, cache.body.fontName)
        cache.configure(.init(preferredContentSizeCategory: .accessibilityExtraExtraExtraLarge))
        let large = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque) }
        XCTAssertGreaterThan(large.size, first.size)
        XCTAssertEqual(engine.font(size: CGFloat(large.size), weight: Int(large.weight), family: Int(large.family_id), italic: false), cache.body)
    }
    func testD3MappingsAndPublishedEditorRectLeaveTheBareFieldAlone() throws {
        let session = ExactApp.shared.makeSession(label: "native-field-mappings")
        defer { session.destroy() }
        let presenter = session.presenter
        let field = NodeView(id: 1, kind: "input", presenter: presenter)
        field.bounds = CGRect(x: 0, y: 0, width: 240, height: 80)
        field.applyProps(set: ["value": "hello", "placeholder": "Placeholder", "disabled": "true", "type": "password"], clear: [])
        field.applyStyle(["font_size": .number(26), "font_style": .string("italic"), "text_align": .string("center"), "text_color": .array([.number(208), .number(32), .number(48), .number(255)]), "letter_spacing": .number(2), "accent_color": .array([.number(0), .number(180), .number(0), .number(255)])])
        field.applyFieldContent(["rect": [15.0, 10.0, 210.0, 60.0]])
        let native = try XCTUnwrap(field.field)
        XCTAssertEqual(native.text, "hello")
        XCTAssertEqual(native.borderStyle, .roundedRect)
        XCTAssertFalse(native.isEnabled)
        XCTAssertTrue(native.isSecureTextEntry)
        XCTAssertEqual(native.font?.pointSize, 26)
        XCTAssertTrue(native.font?.fontDescriptor.symbolicTraits.contains(.traitItalic) == true)
        XCTAssertEqual(native.textAlignment, .center)
        XCTAssertEqual(native.defaultTextAttributes[.kern] as? CGFloat, 2)
        XCTAssertEqual(native.frame, field.bounds)
        XCTAssertEqual(native.textRect(forBounds: native.bounds).minX, 15)
        XCTAssertEqual(native.textRect(forBounds: native.bounds).midY, 40)
        XCTAssertEqual(native.textRect(forBounds: CGRect(x: 0, y: 0, width: 100, height: 100)), CGRect(x: 15, y: 10, width: 70, height: 80), "UIKit's inset probe uses synthetic bounds")
        XCTAssertEqual(native.attributedPlaceholder?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor, .placeholderText)
        field.showFocusRing(true)
        XCTAssertFalse(field.layer.sublayers?.contains { $0 is CAShapeLayer && ($0 as? CAShapeLayer)?.lineWidth == 3 } == true)
        field.applyStyle(["appearance": .string("none"), "padding_left": .number(10)])
        XCTAssertEqual(native.borderStyle, .none)
        XCTAssertEqual(native.frame, field.contentBox())
    }
    func testNativeTextareaExtendsTheFieldLookAndHonoursContentRect() throws {
        let session = ExactApp.shared.makeSession(label: "native-textarea-mappings")
        defer { session.destroy() }
        let presenter = session.presenter
        let node = NodeView(id: 1, kind: "textarea", presenter: presenter)
        node.bounds = CGRect(x: 0, y: 0, width: 240, height: 100)
        node.applyStyle([:])
        node.applyFieldContent(["rect": [12.0, 18.0, 212.0, 62.0]])
        let view = try XCTUnwrap(node.textArea)
        XCTAssertEqual(view.layer.cornerRadius, FieldChromeCache.textareaRadius)
        XCTAssertEqual(view.textContainerInset, UIEdgeInsets(top: 18, left: 12, bottom: 20, right: 16))
        XCTAssertTrue(view.adjustsFontForContentSizeCategory)
        node.traitOverrides.userInterfaceStyle = .dark
        node.updateTraitsIfNeeded(); view.updateTraitsIfNeeded()
        XCTAssertEqual(view.layer.borderColor, UIColor.separator.resolvedColor(with: view.traitCollection).cgColor)
    }
    func testNativeFieldValuePaintsInsideThePublishedRect() throws {
        let session = ExactApp.shared.makeSession(label: "native-field-pixels")
        defer { session.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 150))
        window.overrideUserInterfaceStyle = .light
        window.backgroundColor = .white
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = CGRect(x: 10, y: 10, width: 240, height: 34)
        window.addSubview(node)
        window.makeKeyAndVisible()
        node.applyProps(set: ["value": "Visible value"], clear: [])
        node.applyStyle(["font_size": .number(17), "text_color": .array([.number(0), .number(0), .number(0), .number(255)])])
        node.applyFieldContent(["rect": [7.0, 0.0, 226.0, 34.0]])
        window.layoutIfNeeded()
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        let image = UIGraphicsImageRenderer(bounds: node.bounds).image { ctx in
            UIColor.white.setFill(); ctx.fill(node.bounds)
            node.layer.render(in: ctx.cgContext)
        }
        let cg = try XCTUnwrap(image.cgImage)
        let bytes = try XCTUnwrap(cg.dataProvider?.data) as Data
        let ink = stride(from: 0, to: bytes.count, by: 4).filter { bytes[$0] < 100 && bytes[$0 + 1] < 100 && bytes[$0 + 2] < 100 }.count
        XCTAssertGreaterThan(ink, 100, "native value draws; checking text alone misses a misplaced UIKit editor")
    }
}
#endif
