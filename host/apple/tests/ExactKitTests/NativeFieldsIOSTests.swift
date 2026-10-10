// @ref LLP 1104 D3–D6: configured UIKit geometry and the first-frame seam.
#if os(iOS)
import UIKit
import CExact
import CoreText
import XCTest
@testable import ExactKit

final class NativeFieldsIOSTests: XCTestCase {
    // component Fieldless / view / box testId="fieldless" width=100 height=100
    // Compiled fixture has no data/module dependencies: preparation must still
    // supply a valid control environment even when no control is in the plan.
    private var fieldlessPlan: Data { Data(base64Encoded: "RVhQTAUAAABnZf+ljAD4K8Zoyd7dlwPjGA1E2ggCJPISAAAAY29tLmV4YWN0LmNhbHRyYWlu//////////8BAAAACQAAAGZpZWxkbGVzcxIAAAAEKAIAAAAAKAAAAAAAAABZQCgAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAAAAAAAAQAAAAEAAAABAAAAAgAAAAEAAAADAAAAAQAAAAQAAAABAAAABQAAAAEAAAAGAAAAAQAAAAcAAAABAAAACAAAAAH/////Av////8D/////wT/////Bf////8G/////wf/////CP////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAD//////////wAAAAAAAAAAAwAAAAAAAAAAAAAA/////wMAAAAACwACAAAABgAAAAEAAAgAAAAKAAAAAQEACAAAAAoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==")! }

    func testInitialSelectedPlanLaunchAndReplacementOfARunningFieldlessPlan() throws {
        let app = ExactApp.shared
        let previous = app.lastPlan.map { ExactGeneration(plan: $0, assets: app.resolver, token: app.selectedToken, module: app.lastModule) }
        app.installInitial(ExactGeneration(plan: fieldlessPlan, assets: AssetResolver(root: app.assetRoot)))
        let session = app.makeSession(label: "selected-fieldless")
        session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        defer {
            session.destroy()
            if let previous { app.installInitial(previous) }
            else {
                app.installInitial(ExactGeneration(plan: fieldlessPlan, assets: app.resolver, token: 1104))
                _ = app.fallBackFromInitial(reason: "test cleanup")
            }
        }
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 800)).error)
        XCTAssertTrue(session.presenter.views.values.contains { $0.props["testId"] == "fieldless" })
        XCTAssertTrue(session.apply(fieldlessPlan))
        XCTAssertTrue(session.presenter.views.values.contains { $0.props["testId"] == "fieldless" })
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
    }

    func testProvisionalBatchKeepsUpdatesWithholdsGeometryAndIsCounted() throws {
        let session = ExactApp.shared.makeSession(label: "provisional-publication")
        defer { session.destroy() }
        session.presenter.apply(Batch.decode(try JSONSerialization.data(withJSONObject: ["ops": [
            ["op": "create", "id": 1103, "kind": "view"],
            ["op": "frame", "id": 1103, "x": 1, "y": 2, "w": 3, "h": 4],
            ["op": "roots", "ids": [1103]]
        ]])))
        // Rust withholds frame/content ops at publication after three failed passes.
        let bytes = try JSONSerialization.data(withJSONObject: ["layoutProvisional": true, "error": "chrome did not settle", "ops": [
            ["op": "create", "id": 1104, "kind": "view", "props": ["testId": "provisional"]],
            ["op": "props", "id": 1103, "set": ["testId": "kept"], "clear": []],
            ["op": "roots", "ids": [1103, 1104]]
        ]])
        let batch = Batch.decode(bytes)
        XCTAssertTrue(batch.layoutProvisional)
        session.presenter.apply(batch)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 1)
        XCTAssertEqual(session.presenter.views[1104]?.frame, .zero)
        XCTAssertEqual(session.presenter.views[1103]?.frame, CGRect(x: 1, y: 2, width: 3, height: 4))
        XCTAssertEqual(session.presenter.views[1103]?.props["testId"], "kept")
    }

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
                        // A search field is UIKit's search field, its clear
                        // button's room kept (LLP 1115).
                        let control: UITextField
                        if kind == 2 {
                            let search = UISearchTextField()
                            search.clearButtonMode = .always
                            control = search
                        } else {
                            control = UITextField()
                            control.borderStyle = .roundedRect
                        }
                        control.font = font
                        control.isSecureTextEntry = kind == 1
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
        let first = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque, 0) }
        let drawn = engine.font(size: CGFloat(first.size), weight: Int(first.weight), family: Int(first.family_id), italic: first.italic != 0)
        XCTAssertEqual(drawn, cache.body)
        XCTAssertEqual(drawn.fontName, cache.body.fontName)
        cache.configure(.init(preferredContentSizeCategory: .accessibilityExtraExtraExtraLarge))
        let large = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque, 0) }
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
    /// LLP 1115 wave 2: an in-content search field is `UISearchTextField`'s
    /// look — its fill and magnifier behind the editing field, the text
    /// clear of the magnifier, the clear button while editing where UIKit's
    /// sits — and stops being one when its type or appearance changes.
    func testSearchFieldIsUIKitsSearchField() throws {
        let session = ExactApp.shared.makeSession(label: "native-search-field")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.bounds = CGRect(x: 0, y: 0, width: 300, height: 36)
        node.applyProps(set: ["type": "search", "placeholder": "Search"], clear: [])
        node.applyStyle(["font_size": .number(17)])
        let probe = UISearchTextField(frame: node.bounds)
        probe.font = .preferredFont(forTextStyle: .body)
        probe.clearButtonMode = .always
        probe.text = "Hg"
        let content = probe.textRect(forBounds: node.bounds)
        XCTAssertGreaterThan(content.minX, 20, "UIKit's text clears its magnifier")
        node.applyFieldContent(["rect": [content.minX, content.minY, content.width, content.height].map(Double.init)])
        let field = try XCTUnwrap(node.field)
        let chrome = try XCTUnwrap(node.searchChrome)
        XCTAssertEqual(field.borderStyle, .none, "the search chrome draws the field, not a rounded rect")
        XCTAssertEqual(field.clearButtonMode, .whileEditing)
        XCTAssertEqual(chrome.frame, field.frame)
        XCTAssertFalse(chrome.isUserInteractionEnabled)
        XCTAssertTrue(chrome.accessibilityElementsHidden)
        XCTAssertNotNil(chrome.leftView, "the magnifier")
        XCTAssertEqual(node.subviews.firstIndex(of: chrome).map { $0 < node.subviews.firstIndex(of: field)! }, true, "behind the field")
        XCTAssertGreaterThan(field.textRect(forBounds: field.bounds).minX, chrome.leftViewRect(forBounds: field.bounds).maxX - 1, "the text clears the magnifier")
        XCTAssertEqual(field.clearButtonRect(forBounds: field.bounds), chrome.clearButtonRect(forBounds: field.bounds))
        XCTAssertLessThanOrEqual(field.textRect(forBounds: field.bounds).maxX, field.clearButtonRect(forBounds: field.bounds).minX + 1, "the text never runs under the clear button")
        node.applyProps(set: ["type": "text"], clear: [])
        XCTAssertNil(node.searchChrome)
        XCTAssertEqual(field.borderStyle, .roundedRect)
        XCTAssertEqual(field.clearButtonMode, .never)
        node.applyProps(set: ["type": "search"], clear: [])
        XCTAssertNotNil(node.searchChrome)
        node.applyStyle(["appearance": .string("none")])
        XCTAssertNil(node.searchChrome, "appearance: none is the author's field")
        XCTAssertEqual(field.clearButtonMode, .never)
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
