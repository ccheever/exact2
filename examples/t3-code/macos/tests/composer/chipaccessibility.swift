import AppKit
import XCTest

// composer-chips-accessibility (G1-8, G5-D4, DCP-Gv): the prompt as an assistive client reads it
// (T3ComposerAccessibility.swift). T3 Code's composer is a contenteditable whose value holds each chip's label and whose
// children are the chips, named as their node views name them (realinput-1011c's reads: `Terminal 2 lines 1-738 ` and the
// pop-up button "Terminal excerpt, Terminal 2 lines 1-738. Show details"). Read both ways: through the typed accessors
// (what AppKit serves a client's standard attributes from: realinput-1011c's out-of-process read listed the chip buttons
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
        let ordered = NSAccessibility.unignoredChildren(from: fixture.view.accessibilityChildrenInNavigationOrder() ?? [])
        let elements = ordered.compactMap { $0 as? NSAccessibilityElement }
        XCTAssertEqual(elements.map { $0.accessibilityLabel() ?? "" }, [
            "Terminal excerpt, Terminal 2 lines 1-3. Show details", "Skill Frontend Design. Show details", "Preview notes.md",
            "View cited assistant text: Fixture answer", "Add comment to citation"])
        XCTAssertEqual(elements.map { $0.accessibilityRole() }, [.popUpButton, .popUpButton, .button, .link, .popUpButton])
        XCTAssertTrue(elements.allSatisfy { $0.isAccessibilityElement() && $0.isAccessibilityEnabled() && $0.accessibilityParent() as AnyObject === fixture.view })
        XCTAssertEqual((fixture.view.accessibilityChildren() ?? []).count, elements.count)
        // Each is on its chip.
        let pill = fixture.base.window.convertToScreen(fixture.view.convert(fixture.styler.chipRect(fixture.styler.chips[0])!, to: nil))
        XCTAssertEqual(elements[0].accessibilityFrame().midX, pill.midX, accuracy: 0.5)
    }

    func testTheTerminalExcerptPressOpensItsDetailsByPointerAndByVoiceOver() {
        let fixture = AccessibilityFixture()
        let press = fixture.styler.press
        let terminal = fixture.styler.chips[0]
        XCTAssertEqual(fixture.styler.chipKind(terminal), "terminal")
        // VO-Space: AXPress on the pop-up button.
        let element = press.accessibilityElements[0]
        XCTAssertTrue(element.accessibilityPerformPress())
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

    func testTheReadTextFollowsAnEdit() {
        let fixture = AccessibilityFixture()
        fixture.view.setSelectedRange(NSRange(location: (fixture.view.string as NSString).length, length: 0))
        fixture.base.type("and $review ")
        fixture.styler.restyle()
        XCTAssertEqual(fixture.legacy(.value) as? String, display + "and Review ")
        XCTAssertEqual(fixture.styler.press.accessibilityElements.map { $0.accessibilityLabel() ?? "" }.last, "Skill Review. Show details")
    }
}
