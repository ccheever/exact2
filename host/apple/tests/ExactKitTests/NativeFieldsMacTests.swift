// @ref LLP 1104 D3–D6: real AppKit geometry and the first-frame seam.
#if os(macOS)
import AppKit
import CExact
import XCTest
@testable import ExactKit

final class NativeFieldsMacTests: XCTestCase {
    private var platformTextareaInset: NSSize {
        let editor = NSTextView(usingTextLayoutManager: true)
        return NSSize(width: editor.textContainerInset.width + (editor.textContainer?.lineFragmentPadding ?? 0), height: editor.textContainerInset.height)
    }
    func testTextareaInsetReadsReuseTheMainThreadMeasurement() {
        let first = FieldChromeCache.textareaInset
        let measured = FieldChromeCache.textareaInsetMeasurements
        for _ in 0..<20 { XCTAssertEqual(FieldChromeCache.textareaInset, first) }
        XCTAssertEqual(FieldChromeCache.textareaInsetMeasurements, measured)
        let appearance = NSAppearance(named: .vibrantDark)!
        let other = FieldChromeCache.textareaInset(for: appearance, scale: 3)
        XCTAssertEqual(FieldChromeCache.textareaInsetMeasurements, measured + 1)
        for _ in 0..<20 { XCTAssertEqual(FieldChromeCache.textareaInset(for: appearance, scale: 3), other) }
        XCTAssertEqual(FieldChromeCache.textareaInsetMeasurements, measured + 1)
    }
    func testChromeKindsFontsScalesAndAppearances() {
        let cache = FieldChromeCache()
        cache.configure(NSAppearance(named: .aqua)!, scale: 2)
        let engine = TextEngine(resolve: { _ in nil })
        let id = engine.registerControlFont(cache.body)
        cache.prefill(family: id, font: cache.body, weight: UInt16(TextEngine.controlWeight(cache.body)), italic: false)
        for size in [11, 13, 26, 34] {
            for family in [Int(id), 4] {
                for kind in UInt8(0)...UInt8(3) {
                    let request = ExactFieldChromeRequest(kind: kind, family_id: UInt16(family), size: Float(size), weight: 400, italic: 0)
                    let font = engine.font(size: CGFloat(size), weight: 400, family: family, italic: false)
                    _ = cache.answer(request, font: font)
                    let exact = cache.answer(request, font: font)
                    XCTAssertEqual(exact.provisional, 0)
                    if kind == 3 {
                        let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 240, height: 100))
                        scroller.borderType = .bezelBorder
                        XCTAssertEqual(exact.left, Float(scroller.contentView.frame.minX + platformTextareaInset.width))
                        XCTAssertEqual(exact.top, Float(scroller.contentView.frame.minY + platformTextareaInset.height))
                        XCTAssertEqual(exact.minimum_height, 0)
                    } else {
                        let control = kind == 1 ? NSSecureTextField() : NSTextField()
                        control.font = font; control.stringValue = "Hg"
                        XCTAssertEqual(exact.minimum_height, Float(control.fittingSize.height))
                        for height in [CGFloat(exact.minimum_height), 60, 90] {
                            let bounds = NSRect(x: 0, y: 0, width: 240, height: height)
                            let rect = control.cell!.drawingRect(forBounds: bounds)
                            XCTAssertEqual(exact.left, Float(rect.minX), accuracy: 0.00001)
                            XCTAssertEqual(exact.top, Float(rect.minY), accuracy: 0.00001)
                            XCTAssertEqual(exact.right, Float(bounds.maxX - rect.maxX), accuracy: 0.00001)
                            XCTAssertEqual(exact.bottom, Float(bounds.maxY - rect.maxY), accuracy: 0.00001)
                        }
                    }
                }
            }
        }
        let request = ExactFieldChromeRequest(kind: 0, family_id: id, size: 13, weight: 400, italic: 0)
        let font = engine.font(size: 13, weight: 400, family: Int(id), italic: false)
        for (appearance, scale) in [(NSAppearance.Name.aqua, CGFloat(1)), (.darkAqua, CGFloat(1)), (.accessibilityHighContrastDarkAqua, CGFloat(2))] {
            cache.configure(NSAppearance(named: appearance)!, scale: scale)
            XCTAssertEqual(cache.answer(request, font: font).provisional, 1)
            XCTAssertEqual(cache.answer(request, font: font).provisional, 0)
        }
        cache.presented(false)
        XCTAssertEqual(cache.presentedProvisional, 0)
        cache.presented(true)
        XCTAssertEqual(cache.presentedProvisional, 1)
    }
    func testRegisteredFontMatchesTheSystemControlFont() {
        let engine = TextEngine.pair(resolve: { _ in nil })
        let cache = FieldChromeCache()
        engine.fieldChrome = cache
        Owner.shared.sync { engine.measuring.fieldChrome = cache }
        let answer = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque) }
        let standard = NSFont.systemFont(ofSize: NSFont.systemFontSize(for: .regular))
        XCTAssertEqual(answer.size, Float(standard.pointSize))
        XCTAssertEqual(engine.font(size: CGFloat(answer.size), weight: Int(answer.weight), family: Int(answer.family_id), italic: answer.italic != 0), standard)
        XCTAssertEqual(engine.measuring.platformControlID, engine.platformControlID)
        let request = ExactFieldChromeRequest(kind: 0, family_id: answer.family_id, size: answer.size, weight: answer.weight, italic: answer.italic)
        XCTAssertEqual(cache.answer(request, font: standard).provisional, 0, "default kinds prefilled before layout")
    }
    func testD3MappingsAndEditingUseThePublishedContentRect() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "native-field-mappings")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 80)
        node.applyProps(set: ["value": "hello", "placeholder": "Placeholder", "type": "password"], clear: [])
        node.applyStyle(["font_size": .number(26), "font_style": .string("italic"), "text_align": .string("center"), "text_color": .array([.number(208), .number(32), .number(48), .number(255)]), "letter_spacing": .number(2), "accent_color": .array([.number(0), .number(180), .number(0), .number(255)])])
        node.applyFieldContent(["rect": [15.0, 10.0, 210.0, 60.0]])
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field is NSSecureTextField)
        XCTAssertTrue(field.isBezeled)
        XCTAssertEqual(field.bezelStyle, NSTextField().bezelStyle)
        XCTAssertEqual(field.drawsBackground, NSTextField().drawsBackground)
        XCTAssertEqual(field.focusRingType, .default)
        XCTAssertEqual(field.font?.pointSize, 26)
        XCTAssertTrue(field.font?.fontDescriptor.symbolicTraits.contains(.italic) == true)
        XCTAssertEqual(field.alignment, .center)
        XCTAssertEqual(field.attributedStringValue.attribute(.kern, at: 0, effectiveRange: nil) as? CGFloat, 2)
        XCTAssertEqual(field.placeholderAttributedString?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor, .placeholderTextColor)
        XCTAssertEqual(field.frame, node.bounds)
        let line = try XCTUnwrap(field.cell?.drawingRect(forBounds: field.bounds))
        XCTAssertEqual(line.minX, 15)
        XCTAssertEqual(line.width, 210)
        XCTAssertEqual(line.midY, 40)
        XCTAssertLessThan(line.height, 60)
        let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = node
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        node.styleFieldEditor(editor)
        XCTAssertEqual(editor.typingAttributes[.kern] as? CGFloat, 2)
        XCTAssertEqual(editor.insertionPointColor, node.caretColor)
        editor.insertText("abc", replacementRange: editor.selectedRange())
        XCTAssertEqual(editor.textStorage?.attribute(.kern, at: 0, effectiveRange: nil) as? CGFloat, 2)
        node.applyProps(set: ["disabled": "true"], clear: [])
        XCTAssertFalse(field.isEnabled)
        XCTAssertEqual(node.alphaValue, 1)
        node.applyStyle(["appearance": .string("none"), "padding_left": .number(10)])
        XCTAssertFalse(field.isBezeled)
        XCTAssertFalse(field.drawsBackground)
        XCTAssertEqual(field.focusRingType, .exterior)
        XCTAssertEqual(field.frame, node.contentBox())
    }
    func testTextareaBorderContentRectAndFocusMask() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "native-textarea-mappings")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "textarea", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 100)
        node.applyStyle([:])
        node.applyFieldContent(["rect": [12.0, 18.0, 212.0, 62.0]])
        let scroll = try XCTUnwrap(node.textAreaScroll as? TextAreaScroll)
        let editor = try XCTUnwrap(node.textArea)
        XCTAssertEqual(editor.focusRingType, .default)
        XCTAssertEqual(editor.textContainerInset, platformTextareaInset)
        XCTAssertGreaterThan(editor.textContainerInset.width, 0)
        XCTAssertEqual(scroll.borderType, .bezelBorder)
        XCTAssertEqual(scroll.frame, node.bounds)
        XCTAssertEqual(editor.textContainerOrigin, NSPoint(x: 11, y: 17))
        XCTAssertEqual(editor.textContainer?.containerSize.width, 212)
        let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = node
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertEqual(scroll.focusRingMaskBounds, scroll.bounds)
        XCTAssertEqual(editor.focusRingMaskBounds, editor.convert(scroll.bounds, from: scroll))
        window.makeFirstResponder(nil)
        XCTAssertEqual(scroll.focusRingMaskBounds, .zero)
        node.applyStyle(["appearance": .string("none")])
        XCTAssertEqual(scroll.borderType, .noBorder)
        XCTAssertEqual(editor.focusRingType, .exterior)
        XCTAssertEqual(editor.textContainerOrigin, .zero)
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertEqual(editor.focusRingMaskBounds, editor.convert(node.bounds, from: node))
        window.makeFirstResponder(nil)
        XCTAssertTrue(editor.focusRingMaskBounds.isEmpty)
        node.applyProps(set: ["markup": "markdown"], clear: [])
        XCTAssertFalse(node.isNativeTextControl)
    }
    func testBareFieldsRingFollowsTheWholeAuthoredBoxAndClearsOnBlur() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "bare-field-ring")
        defer { session.destroy() }
        for type in ["text", "password"] {
            let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
            node.frame = NSRect(x: 0, y: 0, width: 240, height: 60)
            node.applyProps(set: ["type": type, "value": "Bare"], clear: [])
            node.applyStyle(["appearance": .string("none"), "padding_left": .number(10), "border_radius": .number(8)])
            let field = try XCTUnwrap(node.field)
            let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = node
            XCTAssertEqual(node.focusRingMaskBounds, .zero)
            XCTAssertTrue(window.makeFirstResponder(field))
            XCTAssertEqual(node.focusRingMaskBounds, node.bounds)
            XCTAssertEqual(field.focusRingType, .exterior)
            XCTAssertEqual(field.focusRingMaskBounds, field.convert(node.bounds, from: node))
            window.makeFirstResponder(nil)
            XCTAssertEqual(node.focusRingMaskBounds, .zero)
            window.close()
        }
    }

    func testFontEnvironmentAndNewFontMissNeverPresentProvisionalGeometry() throws {
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/a2-control-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          state expanded = false
          action show
            expanded = true
          view
            column font-size=30 font-style="italic" letter-spacing=3 color="red"
              input testId="native" value="System font" padding="1em"
              textarea testId="area" rows=2
              input testId="bare" appearance="none" value="Inherited"
              button testId="more" press=show
                text "More"
              when expanded
                input testId="new-font" font-size=31 value="New font"

        """
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try source.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "control-font-environment")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: CGSize(width: 400, height: 800)).error)
        func node(_ testId: String) throws -> NodeView {
            try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId })
        }
        let native = try node("native"), bare = try node("bare"), area = try node("area")
        XCTAssertEqual(native.field?.font, session.fieldChrome.body)
        XCTAssertEqual(area.textArea?.font, session.fieldChrome.body)
        XCTAssertEqual(native.number("letter_spacing"), 0)
        XCTAssertEqual(native.number("padding_left"), session.fieldChrome.body.pointSize)
        XCTAssertEqual(bare.field?.font?.pointSize, 30)
        XCTAssertTrue(bare.field?.font?.fontDescriptor.symbolicTraits.contains(.italic) == true)
        XCTAssertEqual(bare.number("letter_spacing"), 3)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
        let misses = session.fieldChrome.misses
        session.apply(session.runtime.press(try node("more").id, now: session.now()))
        XCTAssertEqual(try node("new-font").field?.font?.pointSize, 31)
        XCTAssertGreaterThan(session.fieldChrome.misses, misses)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
    }

}
#endif
