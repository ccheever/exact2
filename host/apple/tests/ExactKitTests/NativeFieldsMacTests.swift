// @ref LLP 1104 D3–D6: real AppKit geometry and the first-frame seam.
#if os(macOS)
import AppKit
import CExact
import XCTest
@testable import ExactKit

final class NativeFieldsMacTests: XCTestCase {
    func testSearchAndPlainComboboxesServeTheirAuthoredExpandedState() throws {
        _ = NSApplication.shared
        let p = Presenter()
        for (id, type) in [(UInt32(11), "text"), (UInt32(12), "search")] {
            p.apply(wireBatch([["op": "create", "id": id, "kind": "input",
                "props": ["type": type, "value": "Pira", "accessibilityRole": "combobox", "accessibilityExpanded": "false"]]]))
            let field = try XCTUnwrap(p.views[id]?.field), cell = try XCTUnwrap(field.cell)
            XCTAssertEqual(field is NSSearchField, type == "search")
            XCTAssertEqual(field.accessibilityRole(), .comboBox)
            XCTAssertEqual(cell.accessibilityAttributeValue(.role) as? String, "AXComboBox")
            XCTAssertEqual(cell.accessibilityAttributeValue(.roleDescription) as? String, NSAccessibility.Role.comboBox.description(with: nil))
            XCTAssertTrue(cell.accessibilityAttributeNames().contains(.expanded))
            XCTAssertFalse(field.isAccessibilityExpanded())
            XCTAssertEqual(cell.accessibilityAttributeValue(.expanded) as? Bool, false)
            p.apply(wireBatch([["op": "props", "id": id, "set": ["accessibilityExpanded": "true"]]]))
            XCTAssertTrue(field.isAccessibilityExpanded())
            XCTAssertEqual(cell.accessibilityAttributeValue(.expanded) as? Bool, true)
            p.apply(wireBatch([["op": "props", "id": id, "clear": ["accessibilityRole", "accessibilityExpanded"]]]))
            XCTAssertEqual(cell.accessibilityAttributeValue(.role) as? String, "AXTextField")
        }
    }

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
                        let control = kind == 1 ? NSSecureTextField() : kind == 2 ? NSSearchField() : NSTextField()
                        control.font = font; control.stringValue = "Hg"
                        XCTAssertEqual(exact.minimum_height, Float(control.fittingSize.height))
                        for height in [CGFloat(exact.minimum_height), 60, 90] {
                            let bounds = NSRect(x: 0, y: 0, width: 240, height: height)
                            let rect = control.cell!.drawingRect(forBounds: bounds)
                            XCTAssertEqual(exact.left, Float(rect.minX), accuracy: 0.00001)
                            // A search cell centres its text and grows its
                            // buttons in a taller box; the host centres the
                            // line in the kernel's rect (`nativeEditorRect`).
                            if kind == 2, height != CGFloat(exact.minimum_height) { continue }
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
        let answer = Owner.shared.sync { TextEngine.controlText(engine.measuring.opaque, 0) }
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
    /// LLP 1115 D8: a native `input type="search"` is AppKit's search field
    /// (its magnifier and clear button, Escape clearing it), its text in the
    /// published content rect, which the kernel measured from the same
    /// class; drawn by the author (`appearance: none`) it is a plain field.
    func testSearchInputIsANativeSearchField() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "native-search-field")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 40)
        node.applyProps(set: ["value": "trains", "type": "search", "placeholder": "Search"], clear: [])
        node.applyStyle([:])
        let standard = NSSearchField(), probe = NSRect(x: 0, y: 0, width: 240, height: 40)
        standard.font = NSFont.systemFont(ofSize: 13)
        let text = standard.cell!.drawingRect(forBounds: probe)
        node.applyFieldContent(["rect": [Double(text.minX), 0, Double(text.width), 40]])
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field is NSSearchField)
        XCTAssertTrue(field.cell is SearchFieldCell)
        XCTAssertEqual(field.stringValue, "trains")
        XCTAssertEqual(field.frame, node.bounds)
        XCTAssertGreaterThan(text.minX, NSTextField().cell!.drawingRect(forBounds: probe).minX, "room for the magnifier")
        let line = try XCTUnwrap(field.cell?.drawingRect(forBounds: field.bounds))
        XCTAssertEqual(line.minX, text.minX)
        XCTAssertEqual(line.midY, 20, accuracy: 0.5, "one line centred, as the kernel measures it")
        let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = node
        var typed: [String] = []
        node.handlers = ["input"]
        session.presenter.onInput = { _, value in typed.append(value) }
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        editor.doCommand(by: #selector(NSResponder.cancelOperation(_:)))
        XCTAssertEqual(field.stringValue, "", "Escape clears a search field")
        XCTAssertEqual(typed, [""], "and the clear is the input's `input`")
        node.applyStyle(["appearance": .string("none")])
        XCTAssertFalse(node.field is NSSearchField, "an author-drawn search input is a plain field")
        XCTAssertEqual(node.field?.stringValue, "")
        node.applyStyle([:])
        XCTAssertTrue(node.field is NSSearchField)
        node.applyProps(set: ["type": "text"], clear: [])
        XCTAssertFalse(node.field is NSSearchField)
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
    /// LLP 1095 D4: the native editor's two background surfaces read Field;
    /// author-drawn and Markdown editors keep their transparent native views.
    func testTextareaThemeBackgroundAndRemoval() throws {
        _ = NSApplication.shared
        defer { ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]])) }
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": ["Field": [[204, 255, 204, 255], [0, 51, 0, 255]]]]]))
        let session = ExactApp.shared.makeSession(label: "textarea-theme")
        defer { session.destroy() }
        let node = NodeView(id: 1, kind: "textarea", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 100)
        node.applyProps(set: ["value": "Keep this text"], clear: [])
        node.applyStyle([:])
        let editor = try XCTUnwrap(node.textArea)
        let scroll = try XCTUnwrap(node.textAreaScroll)
        editor.setSelectedRange(NSRange(location: 5, length: 4))
        let inset = editor.textContainerInset
        for (name, rgb) in [(NSAppearance.Name.aqua, [204.0, 255, 204]), (.darkAqua, [0.0, 51, 0])] {
            let appearance = try XCTUnwrap(NSAppearance(named: name))
            appearance.performAsCurrentDrawingAppearance {
                for color in [editor.backgroundColor, scroll.backgroundColor] {
                    let channels = color.usingColorSpace(.sRGB)!
                    for (got, want) in zip([channels.redComponent, channels.greenComponent, channels.blueComponent], rgb) {
                        XCTAssertEqual(got, want / 255, accuracy: 0.001, name.rawValue)
                    }
                }
            }
        }
        XCTAssertTrue(editor.drawsBackground)
        XCTAssertTrue(scroll.drawsBackground)
        XCTAssertEqual(scroll.borderType, .bezelBorder)
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]]))
        node.applyStyle([:])
        XCTAssertEqual(editor.backgroundColor, NSTextView().backgroundColor)
        XCTAssertEqual(scroll.backgroundColor, NSScrollView().backgroundColor)
        XCTAssertEqual(editor.string, "Keep this text")
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 5, length: 4))
        XCTAssertEqual(editor.textContainerInset, inset)
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": ["Field": [255, 0, 0, 255]]]]))
        node.applyStyle(["appearance": .string("none"), "background_color": .array([.number(0), .number(0), .number(255), .number(255)])])
        XCTAssertFalse(editor.drawsBackground)
        XCTAssertFalse(scroll.drawsBackground)
        node.applyProps(set: ["markup": "markdown"], clear: [])
        node.applyStyle([:])
        XCTAssertFalse(editor.drawsBackground)
        XCTAssertFalse(scroll.drawsBackground)
    }

    /// LLP 1104 D6: AppKit asks only the first responder for a ring, so a
    /// bare field (the field editor is first responder) and any textarea
    /// (its text view sits in a clip view) show Exact's ring; checked by
    /// screenshot on 2026-10-07, which the masks alone never drew.
    func testExactDrawsTheRingForBareFieldsAndTextareas() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "exact-field-ring")
        defer { session.destroy() }
        func ring(_ node: NodeView) -> CALayer? { node.layer?.sublayers?.first { $0.name == "exact.fieldFocus" } }
        for bare in [false, true] {
            let node = NodeView(id: 1, kind: "textarea", presenter: session.presenter)
            node.frame = NSRect(x: 0, y: 0, width: 240, height: 100)
            node.wantsLayer = true
            node.applyStyle(bare ? ["appearance": .string("none")] : [:])
            let editor = try XCTUnwrap(node.textArea)
            let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.close() }
            window.contentView = node
            XCTAssertTrue(window.makeFirstResponder(editor))
            let r = try XCTUnwrap(ring(node), "textarea bare=\(bare) shows Exact's ring")
            XCTAssertEqual(r.borderWidth, 2)
            XCTAssertGreaterThan(r.zPosition, 0, "above the scroll view that covers the node's own border")
            node.showFieldFocus(false)
            XCTAssertNil(ring(node))
            XCTAssertTrue(window.makeFirstResponder(editor))
            node.applyProps(set: ["disabled": "true"], clear: [])
            node.presenter?.propsChanged(node)
            XCTAssertNil(ring(node), "a disabled textarea shows no ring")
        }
        for bare in [false, true] {
            let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
            node.frame = NSRect(x: 0, y: 0, width: 240, height: 40)
            node.applyProps(set: ["value": "Field"], clear: [])
            node.applyStyle(bare ? ["appearance": .string("none")] : [:])
            let field = try XCTUnwrap(node.field)
            let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.close() }
            window.contentView = node
            XCTAssertTrue(window.makeFirstResponder(field))
            XCTAssertEqual(node.fieldFocused, bare, "a bare field takes Exact's ring; a native one is AppKit's")
            window.makeFirstResponder(nil)
            XCTAssertFalse(node.fieldFocused)
        }
    }
    /// LLP 1104 D6: `outline: none` takes every ring off a focused field,
    /// Exact's and AppKit's, native or bare, single-line or not, while the
    /// field still takes the focus; unset, the ring is back at once.
    func testOutlineNoneDrawsNoRingOnAFocusedField() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "exact-field-outline")
        defer { session.destroy() }
        func ring(_ node: NodeView) -> CALayer? { node.layer?.sublayers?.first { $0.name == "exact.fieldFocus" } }
        for kind in ["textarea", "input"] {
            for bare in [false, true] {
                let what = "\(bare ? "bare" : "native") \(kind)"
                let node = NodeView(id: 1, kind: kind, presenter: session.presenter)
                node.frame = NSRect(x: 0, y: 0, width: 240, height: kind == "input" ? 40 : 100)
                node.wantsLayer = true
                let kept: NodeStyle = bare ? ["appearance": .string("none")] : [:]
                var quiet = kept
                quiet["outline_style"] = .string("none")
                node.applyStyle(quiet)
                let editor: NSView = try XCTUnwrap(node.textArea ?? node.field)
                let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
                window.isReleasedWhenClosed = false
                defer { window.close() }
                window.contentView = node
                XCTAssertTrue(window.makeFirstResponder(editor), "\(what) takes the focus")
                XCTAssertNil(ring(node), "\(what): no Exact ring above the editor")
                XCTAssertFalse(node.fieldFocused, "\(what): no Exact ring on its border")
                XCTAssertEqual(editor.focusRingType, .none, "\(what): no AppKit ring")
                XCTAssertEqual(node.textAreaScroll?.focusRingType ?? .none, .none, "\(what): no ring on the scroll view")
                node.applyStyle(kept)
                XCTAssertEqual(ring(node) != nil || node.fieldFocused || editor.focusRingType == .default, true, "\(what) keeps a ring without it")
                XCTAssertNotEqual(editor.focusRingType, .none)
                node.applyStyle(quiet)
                XCTAssertNil(ring(node))
                XCTAssertFalse(node.fieldFocused)
            }
        }
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

    /// LLP 1115.000 D7.9: the focus ring leaves a border the author wrote
    /// alone, as a native control with a border of its own keeps it under
    /// AppKit's ring. A bare field's border was repainted two points wide
    /// in the focus colour, and a textarea's covered by a ring over it.
    /// Now a field with a written border takes no ring of Exact's (AppKit's
    /// exterior one is asked for around its box), and a textarea's ring is
    /// outside its box; a border nobody wrote is still the ring's to take.
    func testTheFocusRingLeavesAnAuthoredBorderAlone() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "authored-border-ring")
        defer { session.destroy() }
        let red: BatchValue = [204, 0, 0, 255]
        var bordered: NodeStyle = ["appearance": .string("none"), "border_width": .number(2), "border_radius": .number(8)]
        for side in ["top", "right", "bottom", "left"] { bordered["border_color_" + side] = red }
        var written = bordered
        written["written"] = .array(["border_width", "border_color_top"].map { .string($0) })
        func ring(_ node: NodeView) -> CALayer? { node.layer?.sublayers?.first { $0.name == "exact.fieldFocus" } }
        /// The colour on the node's left border, half way down.
        func border(_ node: NodeView) throws -> NSColor {
            let rep = try XCTUnwrap(Capture.picture(of: node))
            let scale = CGFloat(rep.pixelsWide) / rep.size.width
            return try XCTUnwrap(rep.colorAt(x: Int(1 * scale), y: Int(node.bounds.midY * scale))?.usingColorSpace(.sRGB))
        }
        func isRed(_ c: NSColor) -> Bool { c.redComponent > 0.6 && c.greenComponent < 0.25 && c.blueComponent < 0.25 }
        for (style, authored) in [(written, true), (bordered, false)] {
            let what = authored ? "a border its author wrote" : "a border nobody wrote"
            // A bare field.
            let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
            node.frame = NSRect(x: 0, y: 0, width: 240, height: 40)
            node.wantsLayer = true
            node.applyProps(set: ["value": "Field"], clear: [])
            node.applyStyle(style)
            XCTAssertEqual(node.bordersItself, authored, what)
            let field = try XCTUnwrap(node.field)
            let window = NSWindow(contentRect: node.bounds, styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.close() }
            window.contentView = node
            XCTAssertTrue(isRed(try border(node)), "\(what): red before the focus")
            XCTAssertTrue(window.makeFirstResponder(field))
            XCTAssertEqual(node.fieldFocused, !authored, "\(what): \(authored ? "no ring of Exact's on it" : "Exact's ring takes it")")
            let focused = try border(node)
            XCTAssertEqual(isRed(focused), authored, "\(what), focused: \(focused)")
            // AppKit's own ring is asked for either way, around the node's box.
            XCTAssertEqual(field.focusRingType, .exterior)
            XCTAssertEqual(field.focusRingMaskBounds, field.convert(node.bounds, from: node))
            // The author writing it, or taking it back, while the field has the focus.
            node.applyStyle(authored ? bordered : written)
            XCTAssertEqual(node.fieldFocused, authored)
            XCTAssertEqual(isRed(try border(node)), !authored)
            window.makeFirstResponder(nil)
            XCTAssertFalse(node.fieldFocused)

            // A bare textarea: Exact's ring stays, outside the box.
            let area = NodeView(id: 2, kind: "textarea", presenter: session.presenter)
            area.frame = NSRect(x: 0, y: 0, width: 240, height: 100)
            area.wantsLayer = true
            area.applyStyle(style)
            let areaWindow = NSWindow(contentRect: area.bounds, styleMask: [.titled], backing: .buffered, defer: false)
            areaWindow.isReleasedWhenClosed = false
            defer { areaWindow.close() }
            areaWindow.contentView = area
            XCTAssertTrue(areaWindow.makeFirstResponder(try XCTUnwrap(area.textArea)))
            let r = try XCTUnwrap(ring(area), "\(what): a textarea shows Exact's ring")
            if authored {
                XCTAssertEqual(r.frame, area.bounds.insetBy(dx: -3, dy: -3), "outside the box, where AppKit's ring is")
                XCTAssertEqual(r.borderWidth, 3)
                XCTAssertEqual(r.cornerRadius, 11, "around its corners")
                XCTAssertTrue(isRed(try border(area)), "and the border under it is the author's")
                // A node that clips its box cannot show a ring outside it: inside the border, then.
                area.layer?.masksToBounds = true
                area.showFieldFocus(true)
                XCTAssertEqual(ring(area)?.frame, area.bounds.insetBy(dx: 2, dy: 2))
            } else {
                XCTAssertEqual(r.frame, area.bounds, "on the box")
                XCTAssertEqual(r.borderWidth, 2)
            }
        }
        // A pressable box with a border: its ring is AppKit's exterior one, and the border was never touched.
        let button = NodeView(id: 3, kind: "view", presenter: session.presenter)
        button.frame = NSRect(x: 0, y: 0, width: 120, height: 40)
        button.wantsLayer = true
        button.handlers = ["press"]
        var look = written
        look["appearance"] = nil
        button.applyStyle(look)
        let window = NSWindow(contentRect: button.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = button
        button.focusVisible = true
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "AppKit's ring is asked for around the box")
        XCTAssertTrue(isRed(try border(button)), "and the border stays the author's")
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
              if expanded
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


    /// LLP 1104 D3: an authored `::placeholder` colour is the placeholder's
    /// ink, its own alpha kept, on a native field, a bare one and a
    /// textarea; without the row each keeps AppKit's.
    func testAnAuthoredPlaceholderColourIsThePlaceholdersInk() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "authored-placeholder")
        defer { session.destroy() }
        let green: BatchValue = .array([.number(0), .number(128), .number(0), .number(128)])
        func isGreen(_ color: NSColor?, _ why: String) throws {
            let c = try XCTUnwrap(color?.usingColorSpace(.sRGB), why)
            for (got, want) in zip([c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent], [0, 128.0 / 255, 0, 128.0 / 255]) {
                XCTAssertEqual(got, want, accuracy: 0.01, why)
            }
        }
        let node = NodeView(id: 1, kind: "input", presenter: session.presenter)
        node.frame = NSRect(x: 0, y: 0, width: 240, height: 30)
        node.applyProps(set: ["placeholder": "Search"], clear: [])
        let field = try XCTUnwrap(node.field)
        func ink() -> NSColor? { field.placeholderAttributedString?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor }
        XCTAssertEqual(ink(), .placeholderTextColor)
        node.applyStyle(["placeholder_color": green])
        try isGreen(ink(), "a native field")
        node.applyProps(set: ["disabled": "true"], clear: [])
        try isGreen(ink(), "a disabled native field")
        node.applyStyle(["appearance": .string("none"), "placeholder_color": green])
        try isGreen(ink(), "a bare field")
        node.applyStyle([:])
        XCTAssertEqual(ink(), .placeholderTextColor, "no row: AppKit's again")
        let area = NodeView(id: 2, kind: "textarea", presenter: session.presenter)
        area.frame = NSRect(x: 0, y: 0, width: 240, height: 80)
        area.makeTextArea()
        area.applyProps(set: ["placeholder": "Notes"], clear: [])
        let text = try XCTUnwrap(area.textArea as? TextArea)
        area.applyStyle(["placeholder_color": green])
        try isGreen(text.placeholderColor, "a textarea")
        area.applyStyle([:])
        XCTAssertNil(text.placeholderColor, "no row: AppKit's again")
    }
}
#endif
