import AppKit
import XCTest

// The prompt as an assistive client reads it
// (T3ComposerAccessibility.swift). T3 Code's composer is a contenteditable whose value holds each chip's label and whose
// children are the chips, named as their node views name them (the reference's reads: `Terminal 2 lines 1-738 ` and the
// pop-up button "Terminal excerpt, Terminal 2 lines 1-738. Show details"). Read both ways: through the typed accessors
// (what AppKit serves a client's standard attributes from: an out-of-process read listed the chip buttons
// the module sets with `setAccessibilityChildren`, which NSTextView's `accessibilityAttributeValue(.children)` does not
// return) and through the attribute names (`accessibilityAttributeValue`).

private let terminalLink = "[Terminal 2 lines 1-3](t3-context://v1/terminal/terminal_abc)"
private let citationLink = "[Assistant quote](t3-citation://v1/env/thread/message%3Aa1?text=Fixture+answer)"
private let prompt = "\(terminalLink) Use $frontend-design on [notes.md](notes.md) with **bold** \(citationLink) now "
private let display = "Terminal 2 lines 1-3 Use Frontend Design on notes.md with bold Fixture answer now "

private final class AccessibilityFixture {
    let base = EditorFixture()
    var styler: T3ComposerStyler { base.composer.editor.styler }
    var view: NSTextView { base.editor }

    init(text: String = prompt, terminal: String = "$ echo one\none\n$ ") {
        styler.contexts = ["terminal/terminal_abc": "terminal\t\t\(terminal)"]
        base.type(text)
        base.window.display()
        base.tick()
    }

    func legacy(_ name: NSAccessibility.Attribute) -> Any? { view.accessibilityAttributeValue(name) }
    func legacy(_ name: NSAccessibility.ParameterizedAttribute, _ parameter: Any) -> Any? { view.accessibilityAttributeValue(name, forParameter: parameter) }
    func range(_ text: String) -> NSRange { (display as NSString).range(of: text) }
    /// The element an assistive client reads the text area under (`unignoredAncestor(of:)` answers an element itself).
    var textParent: AnyObject? { view.accessibilityParent().flatMap { NSAccessibility.unignoredAncestor(of: $0) } as AnyObject? }
}

final class ComposerChipAccessibilityTests: XCTestCase {
    func testTheTextReadsEachChipAsItsLabelBothWays() {
        let fixture = AccessibilityFixture()
        XCTAssertEqual(fixture.view.string, prompt, "the source stays the text view's")
        XCTAssertEqual(fixture.legacy(.value) as? String, display, "by attribute name")
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, display, "the typed accessor")
        XCTAssertEqual((fixture.legacy(.numberOfCharacters) as? NSNumber)?.intValue, (display as NSString).length)
        XCTAssertEqual(fixture.view.accessibilityNumberOfCharacters(), (display as NSString).length)
        let skill = fixture.range("Frontend Design")
        XCTAssertEqual(fixture.legacy(.stringForRange, NSValue(range: skill)) as? String, "Frontend Design")
        XCTAssertEqual(fixture.view.accessibilityString(for: skill), "Frontend Design")
        let whole = NSRange(location: 0, length: (display as NSString).length)
        XCTAssertEqual((fixture.legacy(.attributedStringForRange, NSValue(range: whole)) as? NSAttributedString)?.string, display)
        XCTAssertEqual(fixture.view.accessibilityAttributedString(for: whole)?.string, display)
        // A citation's run carries its source link element, as Chromium marks a link in an editable's text.
        let quote = fixture.range("Fixture answer")
        let attributed = fixture.legacy(.attributedStringForRange, NSValue(range: quote)) as? NSAttributedString
        let link = attributed?.attribute(NSAttributedString.Key("AXLink"), at: 0, effectiveRange: nil) as? NSAccessibilityElement
        XCTAssertEqual(link?.accessibilityLabel(), "View cited assistant text: Fixture answer")
        // A chip is one unit: any index in it names its whole label.
        let terminal = fixture.range("Terminal 2 lines 1-3")
        XCTAssertEqual((fixture.legacy(.rangeForIndex, NSNumber(value: terminal.location + 4)) as? NSValue)?.rangeValue, terminal)
        XCTAssertEqual((fixture.legacy(.styleRangeForIndex, NSNumber(value: skill.location + 2)) as? NSValue)?.rangeValue, skill)
        // Lines are the source's (the projection keeps every line break): the citation's line is its source's.
        let citationStart = (prompt as NSString).range(of: citationLink).location
        let ownLine = T3ComposerAccessibleText.own(fixture.view) { fixture.view.accessibilityAttributeValue(.lineForIndex, forParameter: NSNumber(value: citationStart)) } as? NSNumber
        XCTAssertNotNil(ownLine)
        XCTAssertEqual((fixture.legacy(.lineForIndex, NSNumber(value: quote.location)) as? NSNumber)?.intValue, ownLine?.intValue)
    }

    func testRangesMapBetweenTheReadTextAndTheSource() {
        let fixture = AccessibilityFixture()
        let source = prompt as NSString
        // The caret after the skill chip reads after its label.
        let after = source.range(of: "$frontend-design").location + ("$frontend-design" as NSString).length
        fixture.view.setSelectedRange(NSRange(location: after, length: 0))
        let label = fixture.range("Frontend Design")
        XCTAssertEqual((fixture.legacy(.selectedTextRange) as? NSValue)?.rangeValue, NSRange(location: NSMaxRange(label), length: 0))
        XCTAssertEqual(fixture.view.accessibilitySelectedTextRange(), NSRange(location: NSMaxRange(label), length: 0))
        // A selection over "bold" (its markers hidden) reads "bold"; one over the citation chip reads its label.
        let bold = source.range(of: "**bold**")
        fixture.view.setSelectedRange(bold)
        XCTAssertEqual(fixture.legacy(.selectedText) as? String, "bold")
        // VoiceOver moving the caret (AXSelectedTextRange set) selects the source the read range stands for.
        fixture.view.accessibilitySetValue(NSValue(range: fixture.range("notes.md")), forAttribute: .selectedTextRange)
        XCTAssertEqual(fixture.view.selectedRange(), source.range(of: "[notes.md](notes.md)"))
        fixture.view.setAccessibilitySelectedTextRange(NSRange(location: fixture.range("now").location, length: 3))
        XCTAssertEqual(fixture.view.selectedRange(), source.range(of: "now"))
        // A chip's bounds are its pill on screen.
        let pill = fixture.styler.chipRect(fixture.styler.chips[0])!
        let screen = fixture.base.window.convertToScreen(fixture.view.convert(pill, to: nil))
        let bounds = (fixture.legacy(.boundsForRange, NSValue(range: fixture.range("Terminal 2 lines 1-3"))) as? NSValue)?.rectValue ?? .zero
        XCTAssertEqual(bounds.midX, screen.midX, accuracy: 1)
        XCTAssertTrue(bounds.contains(NSPoint(x: screen.midX, y: screen.midY)))
        XCTAssertEqual((fixture.legacy(.rangeForPosition, NSValue(point: NSPoint(x: screen.midX, y: screen.midY))) as? NSValue)?.rangeValue,
                       fixture.range("Terminal 2 lines 1-3"))
    }

    func testTheChipsAreElementsInTextOrderWithTheReferencesRolesAndNames() {
        let fixture = AccessibilityFixture()
        // VO-Right reads the text area, then each chip: they are its siblings, right after it, where its scroll view stands
        // aside (an element VoiceOver steps into only by interacting would hide them, as the text area's own children were).
        let scroller = fixture.base.scroller
        XCTAssertFalse(scroller.isAccessibilityElement())
        let siblings = NSAccessibility.unignoredChildren(from: [scroller])
        XCTAssertTrue(siblings.first as AnyObject === fixture.view, "the text area first")
        let elements = siblings.dropFirst().compactMap { $0 as? NSAccessibilityElement }
        XCTAssertEqual(elements.map { $0.accessibilityLabel() ?? "" }, [
            "Terminal excerpt, Terminal 2 lines 1-3. Show details", "Skill Frontend Design. Show details", "Preview notes.md",
            "View cited assistant text: Fixture answer", "Add comment to citation"])
        XCTAssertEqual(elements.map { $0.accessibilityRole() }, [.popUpButton, .popUpButton, .button, .link, .popUpButton])
        XCTAssertTrue(elements.allSatisfy { $0 is T3ComposerChipElement })
        let parent = NSAccessibility.unignoredAncestor(of: scroller) as AnyObject
        XCTAssertTrue(elements.allSatisfy { $0.isAccessibilityElement() && $0.isAccessibilityEnabled() && $0.accessibilityParent() as AnyObject === parent })
        XCTAssertTrue((fixture.view.accessibilityChildren() ?? []).isEmpty, "no longer the text area's children")
        // Each is on its chip.
        let pill = fixture.base.window.convertToScreen(fixture.view.convert(fixture.styler.chipRect(fixture.styler.chips[0])!, to: nil))
        XCTAssertEqual(elements[0].accessibilityFrame().midX, pill.midX, accuracy: 0.5)
        // With no chip left the text area still stands for its scroll view, and is still there: the text area keeps its parent.
        let textParent = fixture.textParent
        XCTAssertTrue(textParent === parent)
        fixture.view.string = "plain"
        fixture.view.didChangeText()
        fixture.base.tick()
        XCTAssertFalse(scroller.isAccessibilityElement())
        XCTAssertEqual(NSAccessibility.unignoredChildren(from: [scroller]).count, 1)
        XCTAssertTrue(NSAccessibility.unignoredChildren(from: [scroller]).first as AnyObject === fixture.view)
        XCTAssertTrue(fixture.textParent === textParent)
    }

    func testRemovingTheLastChipLeavesTheFocusedTextAreaWhereItWas() {
        // A confirmed removal of the last image tile takes its chip, the prompt's last, with the focus given to the prompt
        // first: VoiceOver followed the focus there, then lost its cursor when the text area's parent changed.
        let fixture = AccessibilityFixture(text: "hello [notes.md](notes.md) ")
        XCTAssertTrue(fixture.base.window.makeFirstResponder(fixture.view))
        let before = fixture.textParent
        XCTAssertNotNil(before)
        XCTAssertEqual(fixture.styler.press.accessibilityElements.count, 1)
        fixture.view.string = "hello "
        fixture.view.didChangeText()
        fixture.base.tick()
        XCTAssertTrue(fixture.styler.press.accessibilityElements.isEmpty)
        XCTAssertTrue(fixture.textParent === before)
        XCTAssertFalse(fixture.base.scroller.isAccessibilityElement())
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.view)
    }

    func testAnEmptyPromptIsTheTextAreaItselfNotAScrollArea() {
        // The reference's empty prompt is Chromium's contenteditable: VO-Right reads "Message, edit text". A scroll view
        // AppKit serves as an element read "empty, scroll area", the text area reachable only by interacting.
        let base = EditorFixture()
        base.tick()
        XCTAssertFalse(base.scroller.isAccessibilityElement())
        let children = NSAccessibility.unignoredChildren(from: [base.scroller])
        XCTAssertEqual(children.count, 1)
        XCTAssertTrue(children.first as AnyObject === base.editor)
        XCTAssertEqual(base.editor.accessibilityRole(), .textArea)
        // A chip comes and goes: the text area stays listed.
        base.type("[notes.md](notes.md) ")
        base.tick()
        XCTAssertEqual(NSAccessibility.unignoredChildren(from: [base.scroller]).count, 2)
        base.editor.selectAll(nil)
        base.editor.deleteBackward(nil)
        base.tick()
        let after = NSAccessibility.unignoredChildren(from: [base.scroller])
        XCTAssertEqual(after.count, 1)
        XCTAssertTrue(after.first as AnyObject === base.editor)
        // The editor leaving gives the scroll view back as AppKit made it: an element holding the text area.
        base.composer.remove(base.element)
        XCTAssertTrue(base.scroller.isAccessibilityElement())
        XCTAssertTrue((base.scroller.accessibilityChildren() ?? []).first as AnyObject === base.editor)
    }

    func testTheTerminalExcerptPressOpensItsDetailsByPointerAndByVoiceOver() {
        let fixture = AccessibilityFixture()
        let press = fixture.styler.press
        let terminal = fixture.styler.chips[0]
        XCTAssertEqual(fixture.styler.chipKind(terminal), "terminal")
        // VO-Space: AXPress on the pop-up button, with the focus elsewhere. It does what a click does: the prompt takes
        // the focus with the chip selected, and its details open.
        fixture.base.window.makeFirstResponder(nil)
        let element = press.accessibilityElements[0]
        XCTAssertTrue(axPress(element))
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.view)
        XCTAssertEqual(fixture.view.selectedRange(), terminal.range)
        XCTAssertTrue(press.isOpen)
        XCTAssertEqual(press.state["kind"] as? String, "terminal")
        XCTAssertEqual(press.state["context"] as? String, "terminal_abc")
        XCTAssertEqual(press.state["name"] as? String, "Terminal 2 lines 1-3")
        press.close()
        // A pointer press on the chip (as a hand's click comes, through the application) opens it too.
        let rect = fixture.styler.chipRect(terminal)!
        XCTAssertTrue(press.mouseDown(NSEvent.mouseEvent(with: .leftMouseDown, location: fixture.view.convert(NSPoint(x: rect.midX, y: rect.midY), to: nil),
            modifierFlags: [], timestamp: 0, windowNumber: fixture.base.window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!))
        XCTAssertTrue(press.isOpen)
        // No tooltip over a terminal excerpt with text: its press shows it (TerminalContextInlineChip's popover).
        XCTAssertNil(fixture.styler.tips.tip(for: terminal))
    }

    func testAnExpiredTerminalExcerptIsAShellWithNoPress() {
        let fixture = AccessibilityFixture(text: terminalLink + " now ", terminal: "")
        let element = fixture.styler.press.accessibilityElements[0]
        XCTAssertEqual(element.accessibilityLabel(), "Terminal excerpt, Terminal 2 lines 1-3, expired")
        XCTAssertEqual(element.accessibilityRole(), .group)
        XCTAssertFalse(element is T3ComposerChipPopupElement)
        XCTAssertNil(fixture.styler.popoverChip(at: { let r = fixture.styler.chipRect(fixture.styler.chips[0])!; return NSPoint(x: r.midX, y: r.midY) }()))
        XCTAssertEqual(fixture.styler.tips.tip(for: fixture.styler.chips[0]), "Terminal context expired. Remove and re-add Terminal 2 lines 1-3 to include it in your message.")
    }

    func testAReferenceWithNoRecordIsAnUnavailableContext() {
        // composerContextPresentation's fallback (UnresolvedContextChip): CircleDashedIcon, the neutral chip, its tooltip.
        let fixture = AccessibilityFixture(text: "[Terminal 1 lines 1-2](t3-context://v1/terminal/terminal_lane1) now ")
        let chip = fixture.styler.chips[0]
        XCTAssertTrue(fixture.styler.chipMissing(chip))
        let element = fixture.styler.press.accessibilityElements[0]
        XCTAssertEqual(element.accessibilityLabel(), "Unavailable context, Terminal 1 lines 1-2")
        XCTAssertEqual(element.accessibilityRole(), .group)
        XCTAssertEqual(fixture.styler.tips.tip(for: chip), "This context is no longer available. Remove it or attach it again.")
        XCTAssertNotNil(T3ComposerIcons.svg("circle-dashed", color: "#000000"))
        XCTAssertEqual(fixture.legacy(.value) as? String, "Terminal 1 lines 1-2 now ")
    }

    func testUnavailableContextFillFollowsAppearance() throws {
        let fixture = AccessibilityFixture(text: "[Missing](t3-context://v1/terminal/missing) now ")
        let content = try XCTUnwrap(fixture.base.window.contentView)
        let underlay = try XCTUnwrap(content.subviews.compactMap { $0 as? T3ComposerUnderlay }.first { !$0.selectedOnly })
        let pill = fixture.view.convert(try XCTUnwrap(fixture.styler.chipRect(fixture.styler.chips[0])), to: underlay)
        for (appearance, alpha) in [(NSAppearance.Name.aqua, 0.4), (.darkAqua, 4.0 / 255.0)] {
            underlay.appearance = NSAppearance(named: appearance)
            let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(ceil(underlay.bounds.width)),
                pixelsHigh: Int(ceil(underlay.bounds.height)), bitsPerSample: 8, samplesPerPixel: 4,
                hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
            underlay.draw(underlay.bounds)
            NSGraphicsContext.restoreGraphicsState()
            // The top of the pill, inside the border and above the glyph and label.
            let pixel = try XCTUnwrap(bitmap.colorAt(x: Int(pill.midX), y: bitmap.pixelsHigh - Int(pill.minY + 3) - 1))
            XCTAssertEqual(pixel.alphaComponent, alpha, accuracy: 0.02, "\(appearance.rawValue)")
        }
    }

    func testTheTerminalExcerptDrawsTheTerminalGlyph() {
        let fixture = AccessibilityFixture()
        let terminal = fixture.styler.chips[0]
        XCTAssertEqual(T3ComposerIcons.name(for: terminal, kind: fixture.styler.chipKind(terminal)), "terminal")
        XCTAssertNotNil(T3ComposerIcons.svg("terminal", color: "#000000"))
    }

    func testAPlainPromptKeepsTheTextViewsOwnAnswers() {
        let fixture = AccessibilityFixture(text: "plain words only ")
        XCTAssertEqual(fixture.legacy(.value) as? String, "plain words only ")
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, "plain words only ")
        // Another text view of the class (no projection) answers as before.
        let other = NSTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 20))
        other.string = "[x](t3-context://v1/terminal/terminal_abc) $skill "
        XCTAssertEqual(other.accessibilityValue() as? String, other.string)
        XCTAssertEqual(other.accessibilityAttributeValue(.value) as? String, other.string)
    }

    func testTheReadTextFollowsSkillNamesAndRichTextWithoutAnEdit() {
        let fixture = AccessibilityFixture(text: "Use $imagegen and **bold** now ")
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, "Use Imagegen and bold now ")
        fixture.styler.skills = ["imagegen": "Image Gen"]
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, "Use Image Gen and bold now ")
        XCTAssertEqual(fixture.legacy(.value) as? String, "Use Image Gen and bold now ")
        XCTAssertEqual(fixture.view.accessibilityString(for: NSRange(location: 4, length: 9)), "Image Gen")
        XCTAssertEqual(fixture.styler.press.accessibilityElements[0].accessibilityLabel(), "Skill Image Gen. Show details")
        fixture.styler.richText = false
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, "Use Image Gen and **bold** now ")
        fixture.styler.richText = true
        fixture.styler.skills = [:]
        XCTAssertEqual(fixture.view.accessibilityValue() as? String, "Use Imagegen and bold now ")
        XCTAssertEqual(fixture.view.string, "Use $imagegen and **bold** now ")
    }

    func testPullRequestNamesIncludeStateAndFullTitle() {
        let fixture = AccessibilityFixture(text: "Review [#12](t3-context://v1/review-comment/pr12) now ")
        for (kind, name) in [("pr-open", "Open"), ("pr-draft", "Draft"), ("pr-merged", "Merged"), ("pr-closed", "Closed"), ("pull-request", "")] {
            fixture.styler.contexts = ["review-comment/pr12": "\(kind)\t\tFix tabs\tand Unicode 한글"]
            let kindLabel = name.isEmpty ? "Pull request" : "\(name) pull request"
            XCTAssertEqual(fixture.styler.press.accessibilityElements[0].accessibilityLabel(),
                           "Open \(kindLabel) #12: Fix tabs\tand Unicode 한글")
            XCTAssertEqual(fixture.styler.press.accessibilityElements[0].accessibilityRole(), .button)
            XCTAssertEqual(fixture.view.accessibilityValue() as? String, "Review #12 now ")
        }
    }

    func testTypedSelectionRangesUseTheProjection() throws {
        let fixture = AccessibilityFixture()
        let shown = fixture.range("notes.md")
        fixture.view.setAccessibilitySelectedTextRanges([NSValue(range: shown)])
        XCTAssertEqual(fixture.view.selectedRange(), (prompt as NSString).range(of: "[notes.md](notes.md)"))
        XCTAssertEqual(fixture.view.accessibilitySelectedTextRanges(), [NSValue(range: shown)])
        XCTAssertEqual(fixture.legacy(.selectedTextRanges) as? [NSValue], [NSValue(range: shown)])
        XCTAssertEqual(fixture.view.accessibilitySharedCharacterRange(),
                       try XCTUnwrap(fixture.legacy(.sharedCharacterRange) as? NSValue).rangeValue)
        let whole = NSRange(location: 0, length: (display as NSString).length)
        let rtf = try XCTUnwrap(fixture.view.accessibilityRTF(for: whole))
        XCTAssertEqual(try NSAttributedString(data: rtf, options: [.documentType: NSAttributedString.DocumentType.rtf],
                                             documentAttributes: nil).string, display)
    }

    func testTheReadTextFollowsAnEdit() {
        let fixture = AccessibilityFixture()
        fixture.view.setSelectedRange(NSRange(location: (fixture.view.string as NSString).length, length: 0))
        fixture.base.type("and $review ")
        fixture.styler.restyle()
        XCTAssertEqual(fixture.legacy(.value) as? String, display + "and Review ")
        XCTAssertEqual(fixture.styler.press.accessibilityElements.map { $0.accessibilityLabel() ?? "" }.last, "Skill Review. Show details")
    }
}
