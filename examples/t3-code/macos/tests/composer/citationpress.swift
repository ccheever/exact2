import AppKit
import XCTest

// An assistant citation chip in the composer (T3 Code 1e2ecbd975 AssistantCitationChip.tsx,
// ComposerPromptEditorTiptap.tsx ComposerCitationNodeView, ContextChip.tsx). Its label is the comment, else the quote,
// whitespace as one space, 64 UTF-16 units at most (`max-w-[16em]` truncates it where drawn); it ends with a comment
// button "Add comment to citation" / "Edit citation comment" (ContextChipAction) whose press opens the comment's popover
// at the button (`side="top" align="end"`). Cite opens it under the cited text (`openCitation`), and the app's Save,
// Cancel and Escape end it (`finishCitation`); while it is open a press in the text leaves it to the app (its textarea's
// blur settles the draft). The hrefs are diff-citations.ts formatAssistantCitationHref's (`URLSearchParams`: a space is "+").

private let quoteHref = "t3-citation://v1/env/thread/message%3Aa1?text=parser+keeps+parsePatch+pure%2C+so+a+quote&start=4&end=44&prefix=Cite+check%3A+the+&suffix=+across"
private let commentHref = quoteHref + "&comment=Check+the+pure+claim"
private func link(_ href: String) -> String { "[Assistant quote](\(href))" }

private final class CitationFixture {
    let base = EditorFixture()
    var announced = 0
    var press: T3ComposerChipPress { base.composer.editor.styler.press }
    var styler: T3ComposerStyler { base.composer.editor.styler }

    init(_ text: String) {
        press.onChange = { [weak self] in self?.announced += 1 }
        base.type(text)
        base.window.display()
    }

    /// A real primary click at `point` (the text view's coordinates), through the application as the agent's tap and a
    /// hand's click come, the release queued first (chippress.swift's fixture).
    func click(_ point: NSPoint) {
        let location = base.editor.convert(point, to: nil), window = base.window
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        let up = NSEvent.mouseEvent(with: .leftMouseUp, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(down)
        if let queued = NSApp.nextEvent(matching: .leftMouseUp, until: Date(), inMode: .default, dequeue: true) { NSApp.sendEvent(queued) }
        base.tick()
    }

    /// The same click at `point` in the window's content coordinates (top-left origin, as the content view is unflipped: y from the bottom).
    func clickWindow(_ point: NSPoint) {
        let location = point, window = base.window
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        let up = NSEvent.mouseEvent(with: .leftMouseUp, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(down)
        if let queued = NSApp.nextEvent(matching: .leftMouseUp, until: Date(), inMode: .default, dequeue: true) { NSApp.sendEvent(queued) }
        base.tick()
    }

    func button(_ chip: T3ComposerChip) -> NSPoint { let rect = styler.citationActionRect(chip)!; return NSPoint(x: rect.midX, y: rect.midY) }
    func label(_ chip: T3ComposerChip) -> NSPoint { let rect = styler.chipRect(chip)!; return NSPoint(x: rect.minX + rect.width * 0.3, y: rect.midY) }
}

final class ComposerCitationTests: XCTestCase {
    func testTheLabelIsTheCommentElseTheQuoteAsTheReferenceCutsIt() {
        let quote = T3ComposerText.chips(link(quoteHref) + " ")[0]
        XCTAssertEqual(quote.kind, "citation")
        XCTAssertEqual(quote.label, "parser keeps parsePatch pure, so a quote", "URLSearchParams' \"+\" is a space")
        XCTAssertEqual(quote.detail, "", "no comment: its button adds one")
        let commented = T3ComposerText.chips(link(commentHref) + " ")[0]
        XCTAssertEqual(commented.label, "Check the pure claim", "the comment is the label")
        XCTAssertEqual(commented.detail, "comment")
        XCTAssertEqual(T3ComposerText.citationLabel(text: "a\n\n  b\tc", comment: "   "), "a b c", "a blank comment leaves the quote, whitespace runs as one space")
        let long = String(repeating: "word ", count: 20)
        XCTAssertEqual(T3ComposerText.citationLabel(text: long, comment: nil), (long as NSString).substring(to: 64) + "…")
        XCTAssertEqual(T3ComposerText.citationQuery("t3-citation://v1/e/t/m?text=a%2Bb+c&text=second&comment=").mapValues { $0 }, ["text": "a+b c", "comment": ""])
    }

    func testTheChipEndsWithItsCommentButtonAndItsLabelStopsAt16em() {
        let fixture = CitationFixture("See " + link(quoteHref) + " now")
        let chip = fixture.styler.chips[0], font = NSFont.systemFont(ofSize: 14), em = 14 * 0.86
        let label = (chip.label as NSString).size(withAttributes: [.font: fixture.styler.chipFont(font)]).width
        XCTAssertEqual(fixture.styler.labelWidth(chip, font: font), min(label, em * 16), accuracy: 0.01)
        // ContextChip: border, px-0.5em, the quote icon, gap, the label, gap, ml-0.17em, the 1.17em button, px-0.5em, border.
        XCTAssertEqual(fixture.styler.chipWidth(chip, font: font), 2 + em * (0.5 + 1.17 + 0.33 + 0.33 + 0.17 + 1.17 + 0.5) + ceil(min(label, em * 16) * 10) / 10, accuracy: 0.01)
        let rect = fixture.styler.chipRect(chip)!, button = fixture.styler.citationActionRect(chip)!
        XCTAssertEqual(button.maxX, rect.maxX - 1 - em * 0.5, accuracy: 0.01, "the button ends at the chip's padding")
        XCTAssertEqual(button.width, em * 1.17, accuracy: 0.01)
        XCTAssertEqual(button.midY, rect.midY, accuracy: 0.01)
        XCTAssertNil(fixture.styler.citationActionRect(T3ComposerChip(kind: "skill", start: 0, end: 1, label: "x", detail: "")))
        if let directory = ProcessInfo.processInfo.environment["T3_COMPOSER_TEST_DIR"], let content = fixture.base.window.contentView {
            // The pills are painted under the text view, whose background the app leaves clear.
            fixture.base.scroller.drawsBackground = false; fixture.base.editor.drawsBackground = false
            fixture.base.window.appearance = NSAppearance(named: .aqua); content.layer?.backgroundColor = NSColor.white.cgColor
            fixture.base.window.backgroundColor = .white; fixture.base.window.display()
            let area = content.convert(fixture.base.scroller.frame, from: fixture.base.scroller.superview)
            let rep = content.bitmapImageRepForCachingDisplay(in: area)!
            content.cacheDisplay(in: area, to: rep)
            try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(directory)/dc1-citation-chip.png"))
        }
    }

    func testAPressOnTheButtonOpensTheCommentAtItAndTheLabelDoesNot() {
        let fixture = CitationFixture("See " + link(quoteHref) + " now")
        let chip = fixture.styler.chips[0]
        fixture.click(fixture.label(chip))
        XCTAssertFalse(fixture.press.isOpen, "the label is the chip's source link, not the comment's trigger")
        fixture.click(fixture.button(chip))
        XCTAssertTrue(fixture.press.isOpen, "a real click on the comment button opens the comment")
        XCTAssertEqual(fixture.base.editor.selectedRange(), chip.range, "the press selects the chip, as the reference's node selection")
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.base.window, "the prompt gives the focus up, so the popover's autofocus takes it (initialFocus)")
        let state = fixture.press.state
        XCTAssertEqual(state["kind"] as? String, "citation")
        XCTAssertEqual(state["link"] as? String, link(quoteHref), "the chip's source, whose href the app rewrites")
        XCTAssertEqual(state["anchor"] as? String, "chip")
        XCTAssertEqual(state["removeOnCancel"] as? Bool, false)
        let anchor = state["anchorFrame"] as! [Double], content = fixture.base.window.contentView!
        let box = fixture.base.editor.convert(fixture.styler.citationActionRect(chip)!, to: content)
        XCTAssertEqual(anchor[0], Double(box.minX), accuracy: 0.5, "anchored at the comment button")
        XCTAssertEqual(anchor[1], Double(content.bounds.height - box.maxY), accuracy: 0.5)
        XCTAssertEqual(anchor[2], Double(box.width), accuracy: 0.5)
        // While it is open, presses in the text leave it to the app (the textarea's blur settles the draft), the button's too.
        let seq = fixture.press.seq
        fixture.click(NSPoint(x: 395, y: 8))
        XCTAssertTrue(fixture.press.isOpen, "a press in the text is the app's outside press")
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.base.editor, "the text view takes the focus, so the popover's textarea blurs and the app settles it")
        fixture.base.window.makeFirstResponder(nil)
        fixture.click(fixture.button(chip))
        XCTAssertTrue(fixture.press.isOpen && fixture.press.seq == seq, "the trigger again: the app settles it")
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.base.editor, "the press takes the focus, so the popover's textarea blurs")
        // Escape (the app's) without a comment keeps the chip and puts the caret after it.
        XCTAssertTrue(fixture.press.finishCitation(seq: seq, text: nil, focus: true))
        XCTAssertFalse(fixture.press.isOpen)
        XCTAssertEqual(fixture.base.editor.string, "See " + link(quoteHref) + " now")
        XCTAssertEqual(fixture.base.editor.selectedRange(), NSRange(location: chip.end, length: 0), "onRestoreFocus: the caret after the chip")
    }

    /// Base UI's outside press anywhere in the window (the timeline's ground takes no focus and no `pointerdown`): the
    /// popover's textarea gives the focus up and its blur settles the draft; a press in the popover is its own.
    func testAPressOutsideTheCommentTakesTheFocusFromItsTextarea() {
        let fixture = CitationFixture("See " + link(quoteHref) + " now")
        let content = fixture.base.window.contentView!
        let popover = NSView(frame: NSRect(x: 0, y: 130, width: 200, height: 60)) // above the prompt (0, 0, 400, 120)
        popover.setAccessibilityIdentifier("citation-comment")
        let field = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 60))
        popover.addSubview(field); content.addSubview(popover)
        fixture.press.press(fixture.styler.chips[0])
        XCTAssertTrue(fixture.base.window.makeFirstResponder(field))
        fixture.clickWindow(NSPoint(x: 100, y: 160))
        XCTAssertTrue(fixture.base.window.firstResponder === field, "a press in the popover keeps its focus")
        fixture.clickWindow(NSPoint(x: 300, y: 160))
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.base.window, "a press on the window's ground takes it")
        XCTAssertTrue(fixture.press.isOpen, "the app closes it once it has settled the draft")
        // With nothing of a citation open, a press outside leaves the focus alone.
        fixture.press.close()
        XCTAssertTrue(fixture.base.window.makeFirstResponder(field))
        fixture.clickWindow(NSPoint(x: 300, y: 160))
        XCTAssertTrue(fixture.base.window.firstResponder === field)
        XCTAssertTrue(T3ComposerChipPress.inCitationComment(field) && !T3ComposerChipPress.inCitationComment(content))
    }

    func testCiteOpensItUnderTheCitedTextSaveRewritesTheChipAndCancelRemovesIt() {
        let fixture = CitationFixture("Before ")
        // Cite inserted the chip at the caret (editorInsert), then asks for its comment under the cited text.
        _ = fixture.base.composer.editor.insert(["text": link(quoteHref)])
        XCTAssertFalse(fixture.press.openCitation(href: quoteHref + "x", source: []), "only the chip just inserted")
        XCTAssertTrue(fixture.press.openCitation(href: quoteHref, source: [412, 180.5, 540, 22]))
        var state = fixture.press.state
        XCTAssertEqual(state["anchor"] as? String, "source")
        XCTAssertEqual(state["anchorFrame"] as? [Double], [412, 180.5, 540, 22])
        XCTAssertEqual(state["removeOnCancel"] as? Bool, true)
        // Save: the chip's source becomes the app's (a comment added), the caret after it.
        var seq = fixture.press.seq
        XCTAssertFalse(fixture.press.finishCitation(seq: seq - 1, text: link(commentHref), focus: true), "only the comment the app showed")
        XCTAssertTrue(fixture.press.finishCitation(seq: seq, text: link(commentHref), focus: true))
        XCTAssertFalse(fixture.press.isOpen)
        XCTAssertEqual(fixture.base.editor.string, "Before " + link(commentHref) + " ")
        XCTAssertEqual(fixture.base.editor.selectedRange().location, ("Before " + link(commentHref) as NSString).length)
        XCTAssertEqual(fixture.styler.chips[0].label, "Check the pure claim")
        XCTAssertEqual(fixture.press.accessibilityElements.map { $0.accessibilityLabel() ?? "" }, ["View cited assistant text: Check the pure claim", "Edit citation comment"])
        // An outside press's commit (no focus asked) in the prompt's text keeps the reader's caret, moved with the text after the chip.
        _ = fixture.press.openCitation(href: commentHref, source: [])
        let tail = (fixture.base.editor.string as NSString).length
        fixture.base.window.makeFirstResponder(fixture.base.editor) // the press in the text took the focus
        fixture.base.editor.setSelectedRange(NSRange(location: tail, length: 0))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: link(quoteHref), focus: false))
        XCTAssertEqual(fixture.base.editor.string, "Before " + link(quoteHref) + " ")
        XCTAssertEqual(fixture.base.editor.selectedRange().location, tail - (link(commentHref) as NSString).length + (link(quoteHref) as NSString).length)
        XCTAssertTrue(fixture.press.openCitation(href: quoteHref, source: []))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: link(commentHref), focus: true))
        // Cite again, then Cancel: the chip just cited goes again (removeOnCancel), the spaces stay as the reference's.
        fixture.base.editor.setSelectedRange(NSRange(location: (fixture.base.editor.string as NSString).length, length: 0))
        _ = fixture.base.composer.editor.insert(["text": link(quoteHref)])
        XCTAssertTrue(fixture.press.openCitation(href: quoteHref, source: [1, 2, 3, 4]))
        state = fixture.press.state
        XCTAssertEqual(state["link"] as? String, link(quoteHref), "the newest chip, not the commented one")
        seq = fixture.press.seq
        XCTAssertTrue(fixture.press.finishCitation(seq: seq, text: "", focus: true))
        XCTAssertEqual(fixture.base.editor.string, "Before " + link(commentHref) + "  ")
        XCTAssertTrue(fixture.base.window.firstResponder === fixture.base.editor)
        // An edit in the prompt leaves the comment open (a draft past 8,000 characters holds it open while the prompt has the
        // focus) and it follows its chip; it closes when the chip goes, as the reference's node view keeps its popover by its citeKey.
        _ = fixture.base.composer.editor.insert(["text": link(quoteHref)])
        XCTAssertTrue(fixture.press.openCitation(href: quoteHref, source: []))
        XCTAssertEqual(fixture.press.state["anchorFrame"] as? [Double], fixture.press.trigger, "no source frame: at the button")
        seq = fixture.press.seq
        fixture.base.window.makeFirstResponder(fixture.base.editor)
        fixture.base.type("x")
        XCTAssertTrue(fixture.press.isOpen && fixture.press.seq == seq, "typing after the chip leaves its comment open")
        let cited = fixture.press.chip!
        fixture.base.editor.setSelectedRange(NSRange(location: 0, length: 0))
        fixture.base.type("Now ")
        XCTAssertTrue(fixture.press.isOpen && fixture.press.seq == seq, "typing before the chip moves it; the comment follows")
        XCTAssertEqual(fixture.press.chip?.start, cited.start + 4)
        XCTAssertEqual(fixture.press.state["link"] as? String, link(quoteHref))
        XCTAssertFalse(fixture.base.composer.editor.command(#selector(NSResponder.cancelOperation(_:)), in: fixture.base.editor),
                       "the prompt's Escape leaves a citation's comment to the window's (ChipPopoverEscape → finishCitation `escape`)")
        XCTAssertTrue(fixture.press.isOpen)
        fixture.base.editor.setSelectedRange(fixture.press.chip!.range)
        fixture.base.type("y")
        XCTAssertFalse(fixture.press.isOpen, "the chip replaced: its comment closes")
    }

    /// PopoverPopup's `finalFocus` (AssistantCitationChip.tsx; Base UI 1.5.0 runs it on every close): when the comment closes
    /// and the focus is nowhere (`document.activeElement === document.body`: the outside press landed on ground that takes
    /// none) or still in the popup, the prompt takes it with the caret after the chip (onRestoreFocus); a control the press
    /// focused keeps it. The window's Escape is the editor's Cancel in the comment's textarea and Base UI's escape-key
    /// dismissal elsewhere, which keeps the chip (resolveAssistantCitationCommentDismissal: "close").
    func testAPressThatLeavesTheFocusNowhereGivesItToThePromptAfterTheChip() {
        let fixture = CitationFixture("See " + link(quoteHref) + " now")
        let window = fixture.base.window, content = window.contentView!, prompt = fixture.base.editor
        let popover = NSView(frame: NSRect(x: 0, y: 130, width: 200, height: 60)) // above the prompt (0, 0, 400, 120)
        popover.setAccessibilityIdentifier("citation-comment")
        let field = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 60))
        popover.addSubview(field); content.addSubview(popover)
        let other = NSTextView(frame: NSRect(x: 210, y: 130, width: 100, height: 60)) // a control outside the popover
        content.addSubview(other)
        let chip = fixture.styler.chips[0]
        // The pencil, the draft untouched, a press on the window's ground: the app closes it (`chip-close`).
        fixture.click(fixture.button(chip))
        XCTAssertTrue(window.makeFirstResponder(field), "the popover's autofocus")
        fixture.clickWindow(NSPoint(x: 380, y: 160))
        XCTAssertTrue(window.firstResponder === window && T3ComposerChipPress.focusIsNowhere(window))
        fixture.press.close(seq: fixture.press.seq)
        XCTAssertFalse(fixture.press.isOpen)
        XCTAssertTrue(window.firstResponder === prompt, "finalFocus: the body had it, so onRestoreFocus")
        XCTAssertEqual(prompt.selectedRange(), NSRange(location: chip.end, length: 0), "the caret after the chip")
        // A changed draft, a press on the ground: the commit (no focus asked) puts the caret after the rewritten chip too.
        fixture.click(fixture.button(chip))
        XCTAssertTrue(window.makeFirstResponder(field))
        fixture.clickWindow(NSPoint(x: 380, y: 160))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: link(commentHref), focus: false))
        XCTAssertTrue(window.firstResponder === prompt)
        XCTAssertEqual(prompt.selectedRange(), NSRange(location: ("See " + link(commentHref) as NSString).length, length: 0))
        // A press that focused another control leaves the focus there, closed or committed.
        let commented = fixture.styler.chips[0]
        fixture.click(fixture.button(commented))
        XCTAssertTrue(window.makeFirstResponder(other))
        XCTAssertFalse(T3ComposerChipPress.focusIsNowhere(window) || T3ComposerChipPress.focusInCitationComment(window))
        fixture.press.close(seq: fixture.press.seq)
        XCTAssertTrue(window.firstResponder === other, "a control the press focused keeps it")
        fixture.click(fixture.button(commented))
        XCTAssertTrue(window.makeFirstResponder(other))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: link(quoteHref), focus: false))
        XCTAssertTrue(window.firstResponder === other)
        XCTAssertEqual(prompt.string, "See " + link(quoteHref) + " now")
        // The window's Escape after Cite: with the focus in the comment's textarea it removes the chip just cited.
        prompt.setSelectedRange(NSRange(location: (prompt.string as NSString).length, length: 0))
        _ = fixture.base.composer.editor.insert(["text": link(commentHref)])
        XCTAssertTrue(fixture.press.openCitation(href: commentHref, source: []))
        XCTAssertTrue(window.makeFirstResponder(field) && T3ComposerChipPress.focusInCitationComment(window))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: "", focus: true, escape: true))
        XCTAssertFalse(prompt.string.contains(link(commentHref)), "the editor's Cancel: removeOnCancel")
        XCTAssertTrue(window.firstResponder === prompt)
        // With the focus in the prompt (a draft past the limit held it open): escape-key closes it and keeps the chip and the caret.
        _ = fixture.base.composer.editor.insert(["text": link(commentHref)])
        XCTAssertTrue(fixture.press.openCitation(href: commentHref, source: []))
        let cited = fixture.press.chip!
        window.makeFirstResponder(prompt)
        prompt.setSelectedRange(NSRange(location: 1, length: 0))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: "", focus: true, escape: true))
        XCTAssertFalse(fixture.press.isOpen)
        XCTAssertTrue(prompt.string.contains(link(commentHref)), "escape-key's close keeps the chip")
        XCTAssertEqual(prompt.selectedRange(), NSRange(location: 1, length: 0), "and leaves the caret where the reader put it")
        // With the focus nowhere (an outside press on the ground first, past the limit): the caret after the chip.
        XCTAssertTrue(fixture.press.openCitation(href: commentHref, source: []))
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertTrue(fixture.press.finishCitation(seq: fixture.press.seq, text: "", focus: true, escape: true))
        XCTAssertTrue(prompt.string.contains(link(commentHref)))
        XCTAssertTrue(window.firstResponder === prompt)
        XCTAssertEqual(prompt.selectedRange(), NSRange(location: cited.end, length: 0), "onRestoreFocus: after the chip")
    }

    func testTheCommentButtonIsAnAccessibilityButton() {
        let fixture = CitationFixture("See " + link(quoteHref) + " and " + link(commentHref) + " ")
        // Each citation is AssistantCitationChip's source link and its comment trigger.
        let all = fixture.press.accessibilityElements
        XCTAssertEqual(all.map { $0.accessibilityLabel() ?? "" }, ["View cited assistant text: parser keeps parsePatch pure, so a quote", "Add comment to citation",
                                                                   "View cited assistant text: Check the pure claim", "Edit citation comment"])
        XCTAssertEqual(all.map { $0.accessibilityRole() }, [.link, .popUpButton, .link, .popUpButton])
        let elements = all.filter { $0 is T3ComposerChipPopupElement }
        let frame = elements[0].accessibilityFrame(), window = fixture.base.window
        let expected = window.convertToScreen(fixture.base.editor.convert(fixture.styler.citationActionRect(fixture.styler.chips[0])!, to: nil))
        XCTAssertEqual(frame.midX, expected.midX, accuracy: 0.5)
        XCTAssertTrue(elements[0].accessibilityPerformPress())
        XCTAssertTrue(fixture.press.isOpen)
        XCTAssertEqual(fixture.press.state["anchor"] as? String, "chip")
    }
}
