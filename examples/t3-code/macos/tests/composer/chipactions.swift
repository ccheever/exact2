import AppKit
import XCTest

// The composer chips' other presses (composerContextPresentation.tsx, contextChipParts.tsx, ThreadContextChip.tsx and
// AssistantCitationChip.tsx in T3 Code): a review comment and a preview annotation open their details as a skill does; a
// thread, a pull request, an image, a video, a mention and a citation's source link are actions the app performs
// (T3ComposerChipPress `act`), and so is a file (its AttachmentFilePreview); an expired terminal excerpt has no press and
// is drawn in the destructive colour.

private let thread = "[Plan the release](t3-context://v1/thread/thread_t2)"
private let review = "[app.ts L3](t3-context://v1/review-comment/review_x)"
private let pull = "[#12](t3-context://v1/review-comment/pr12)"
private let annotation = "[Fix the button](t3-context://v1/preview-annotation/preview_a)"
private let image = "![shot.png](t3-context://v1/image/image_11111111-2222-3333-4444-555555555555)"
private let file = "[notes.txt](t3-context://v1/file/file_x)"
private let citation = "[Assistant quote](t3-citation://v1/env/thread/message%3Aa1?text=Fixture+answer)"

private final class ActionFixture {
    let base = EditorFixture()
    var styler: T3ComposerStyler { base.composer.editor.styler }
    var press: T3ComposerChipPress { styler.press }
    var view: NSTextView { base.editor }

    init(_ text: String) {
        styler.contexts = ["thread/thread_t2": "thread", "review-comment/review_x": "review-comment", "review-comment/pr12": "pr-open\t\tFix tabs",
                           "preview-annotation/preview_a": "preview-annotation", "image/image_11111111-2222-3333-4444-555555555555": "image\t12 KB\t11111111-2222-3333-4444-555555555555",
                           "file/file_x": "file\t1 KB", "terminal/terminal_old": "terminal\t\t"]
        base.type(text)
        base.window.display()
        base.tick()
    }

    func chip(_ kind: String) -> T3ComposerChip { styler.chips.first { $0.kind == kind || styler.chipKind($0) == kind }! }

    /// A primary press at `point` (text view coordinates), as the window's event monitor sees it.
    @discardableResult func mouseDown(_ point: NSPoint) -> Bool {
        press.mouseDown(NSEvent.mouseEvent(with: .leftMouseDown, location: view.convert(point, to: nil), modifierFlags: [], timestamp: 0,
            windowNumber: base.window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!)
    }
    func center(_ chip: T3ComposerChip) -> NSPoint { let rect = styler.chipRect(chip)!; return NSPoint(x: rect.midX, y: rect.midY) }
}

final class ComposerChipActionTests: XCTestCase {
    func testAReviewCommentAndAnAnnotationOpenTheirDetails() {
        let fixture = ActionFixture("\(review) and \(annotation) now ")
        for (kind, context) in [("review-comment", "review_x"), ("preview-annotation", "preview_a")] {
            let chip = fixture.chip(kind)
            XCTAssertTrue(fixture.styler.isPopoverTrigger(chip))
            XCTAssertTrue(fixture.mouseDown(fixture.center(chip)), "the press is the chip's")
            XCTAssertTrue(fixture.press.isOpen)
            XCTAssertEqual(fixture.press.state["kind"] as? String, kind)
            XCTAssertEqual(fixture.press.state["context"] as? String, context)
            XCTAssertEqual(fixture.view.selectedRange(), chip.range, "selected, as the skill chip's press selects it")
            // A second press closes them (PopoverTrigger toggles).
            fixture.mouseDown(fixture.center(chip))
            XCTAssertFalse(fixture.press.isOpen)
        }
        // VoiceOver's press does the same.
        let elements = fixture.press.accessibilityElements
        XCTAssertEqual(elements.map { $0.accessibilityLabel() ?? "" }, ["Review comment, app.ts L3. Show details", "Preview annotation, Fix the button. Show details"])
        XCTAssertTrue(elements.allSatisfy { $0 is T3ComposerChipPopupElement })
        XCTAssertTrue(axPress(elements[1]))
        XCTAssertEqual(fixture.press.state["kind"] as? String, "preview-annotation")
    }

    func testActionChipsReportWhatTheyNameOnceAndTheAppTakesIt() {
        let fixture = ActionFixture("\(thread) \(pull) \(image) \(citation) now ")
        let cases: [(T3ComposerChip, String, String, String)] = [
            (fixture.chip("thread"), "thread", "context", "thread_t2"), (fixture.chip("pr-open"), "pr-open", "context", "pr12"),
            (fixture.chip("image"), "image", "context", "image_11111111-2222-3333-4444-555555555555"), (fixture.chip("citation"), "citation", "link", citation)]
        for (chip, act, key, value) in cases {
            XCTAssertTrue(fixture.styler.isActionTrigger(chip), act)
            // The citation's label, left of its comment button, is its source link.
            let rect = fixture.styler.chipRect(chip)!
            let point = chip.kind == "citation" ? NSPoint(x: rect.minX + 8, y: rect.midY) : fixture.center(chip)
            XCTAssertTrue(fixture.mouseDown(point), act)
            XCTAssertFalse(fixture.press.isOpen, "an action opens no details: \(act)")
            let state = T3ComposerChipPress.latest([fixture.press])
            XCTAssertEqual(state["open"] as? Bool, false)
            XCTAssertEqual(state["act"] as? String, act)
            XCTAssertEqual(state[key] as? String, value)
            XCTAssertEqual(state["surface"] as? String, "composer")
            XCTAssertEqual(state["seq"] as? Int, fixture.press.seq)
            // The app takes it by its sequence; an older one leaves it.
            fixture.press.close(seq: fixture.press.seq - 1)
            XCTAssertNotNil(T3ComposerChipPress.latest([fixture.press])["act"])
            fixture.press.close(seq: fixture.press.seq)
            XCTAssertNil(T3ComposerChipPress.latest([fixture.press])["act"])
        }
        // The citation's comment button still opens its comment.
        let comment = fixture.styler.citationActionRect(fixture.chip("citation"))!
        fixture.mouseDown(NSPoint(x: comment.midX, y: comment.midY))
        XCTAssertTrue(fixture.press.isOpen)
        XCTAssertEqual(fixture.press.state["kind"] as? String, "citation")
        XCTAssertNil(fixture.press.state["act"], "opening details drops an action the app has not taken")
    }

    func testAnActionClosesOpenDetailsAndVoiceOverPressesIt() {
        let fixture = ActionFixture("\(review) \(thread) now ")
        fixture.mouseDown(fixture.center(fixture.chip("review-comment")))
        XCTAssertTrue(fixture.press.isOpen)
        fixture.mouseDown(fixture.center(fixture.chip("thread")))
        XCTAssertFalse(fixture.press.isOpen, "a press on another chip is a press outside")
        XCTAssertEqual(fixture.press.state["act"] as? String, "thread")
        fixture.press.close(seq: fixture.press.seq)
        let link = fixture.press.accessibilityElements[1]
        XCTAssertEqual(link.accessibilityLabel(), "Thread, Plan the release")
        XCTAssertEqual(link.accessibilityRole(), .link)
        XCTAssertTrue(axPress(link))
        XCTAssertEqual(fixture.press.state["act"] as? String, "thread")
    }

    /// The thread chip is a link, as the citation's source link is (citationpress.swift): it leads to itself, so VoiceOver,
    /// which follows a link it pressed to the link's linked elements, does not take an element its press made appear.
    /// A chip of another kind names none.
    func testAThreadChipLeadsToItselfAndOtherChipsNameNoLinkedElements() {
        let fixture = ActionFixture("\(thread) \(pull) \(file) now ")
        let elements = fixture.press.accessibilityElements
        XCTAssertEqual(elements.map { $0.accessibilityRole() }, [.link, .button, .button])
        XCTAssertTrue((elements[0].accessibilityLinkedUIElements() ?? []).elementsEqual([elements[0]]) { $0 as AnyObject === $1 })
        XCTAssertTrue(elements.dropFirst().allSatisfy { ($0.accessibilityLinkedUIElements() ?? []).isEmpty })
        XCTAssertTrue(axPress(elements[0]))
        fixture.base.tick()
        XCTAssertEqual(fixture.press.state["act"] as? String, "thread")
        let after = fixture.press.accessibilityElements
        XCTAssertTrue(after[0] === elements[0], "the pressed link stays")
        XCTAssertTrue((after[0].accessibilityLinkedUIElements() ?? []).elementsEqual([after[0]]) { $0 as AnyObject === $1 })
    }

    /// FileContextChip: a button "File attachment, <name>, <size>" whose press is `openFile`, by pointer and by VoiceOver.
    func testAFileChipOpensItsPreviewByPointerAndVoiceOver() {
        let fixture = ActionFixture("\(file) now ")
        let chip = fixture.chip("file")
        XCTAssertTrue(fixture.styler.isActionTrigger(chip))
        XCTAssertTrue(fixture.mouseDown(fixture.center(chip)), "the press is the chip's")
        XCTAssertEqual(fixture.press.state["act"] as? String, "file")
        XCTAssertEqual(fixture.press.state["context"] as? String, "file_x")
        fixture.press.close(seq: fixture.press.seq)
        XCTAssertNil(fixture.press.state["act"])
        let element = fixture.press.accessibilityElements[0]
        XCTAssertEqual(element.accessibilityLabel(), "File attachment, notes.txt, 1 KB")
        XCTAssertEqual(element.accessibilityRole(), .button)
        XCTAssertTrue(element is T3ComposerChipPopupElement)
        // One press does it, with the focus elsewhere too: the prompt takes the focus with the chip selected, as after a
        // click, so the preview it opens gives the focus back there.
        fixture.view.window?.makeFirstResponder(nil)
        let before = fixture.press.seq
        XCTAssertTrue(axPress(element))
        XCTAssertEqual(fixture.press.state["act"] as? String, "file")
        XCTAssertEqual(fixture.press.seq, before + 1)
        XCTAssertTrue(fixture.view.window?.firstResponder === fixture.view)
        XCTAssertEqual(fixture.view.selectedRange(), chip.range)
    }

    /// A preview's close gives the prompt the focus back with the pressed chip still selected, so a typed letter replaces
    /// the chip. In T3 Code the press makes ProseMirror's NodeSelection of the chip and the dialog's `finalFocus`
    /// refocuses the editor, which keeps it: its Tiptap editor and Base UI Dialog in Chrome for Testing 154, "fix
    /// [sample.py] and [square.png] now", a click on either chip, then Escape, the Close button or the backdrop, then "z",
    /// leave "fix z and [square.png] now" or "fix [sample.py] and z now".
    func testAPreviewClosingLeavesThePressedChipSelectedAsTheReferenceDoes() {
        let text = "fix \(file) and \(image) now "
        for (kind, source) in [("file", file), ("image", image)] {
            let fixture = ActionFixture(text)
            let chip = fixture.chip(kind)
            XCTAssertTrue(fixture.mouseDown(fixture.center(chip)), kind)
            fixture.press.close(seq: fixture.press.seq)
            // The preview holds the focus while open and gives it back to the prompt as it closes.
            fixture.base.window.makeFirstResponder(nil)
            fixture.base.window.makeFirstResponder(fixture.view)
            XCTAssertEqual(fixture.view.selectedRange(), chip.range, kind)
            fixture.view.insertText("z", replacementRange: fixture.view.selectedRange())
            fixture.base.tick()
            XCTAssertEqual(fixture.view.string, text.replacingOccurrences(of: source, with: "z"), kind)
        }
    }

    func testAnExpiredExcerptHasNoPress() {
        let fixture = ActionFixture("[Terminal 1 line 2](t3-context://v1/terminal/terminal_old) now ")
        let chip = fixture.chip("terminal")
        XCTAssertFalse(fixture.styler.isActionTrigger(chip) || fixture.styler.isPopoverTrigger(chip))
        XCTAssertFalse(fixture.mouseDown(fixture.center(chip)), "the text view's press")
        XCTAssertNil(fixture.press.state["act"])
        XCTAssertTrue(fixture.press.accessibilityElements.allSatisfy { !($0 is T3ComposerChipPopupElement) })
    }

    /// TerminalContextInlineChip's expired shell is ContextChip `state="invalid"`: `border-destructive/35 bg-destructive/8
    /// text-destructive`, a solid border (only "unresolved" is dashed).
    func testAnExpiredExcerptIsDrawnInTheDestructiveColour() throws {
        let fixture = ActionFixture("[Terminal 1 line 2](t3-context://v1/terminal/terminal_old) now ")
        let chip = fixture.chip("terminal")
        XCTAssertTrue(fixture.styler.chipExpired(chip))
        XCTAssertFalse(fixture.styler.chipMissing(chip))
        let content = try XCTUnwrap(fixture.base.window.contentView)
        let underlay = try XCTUnwrap(content.subviews.compactMap { $0 as? T3ComposerUnderlay }.first { !$0.selectedOnly })
        underlay.appearance = NSAppearance(named: .aqua)
        let pill = fixture.view.convert(try XCTUnwrap(fixture.styler.chipRect(chip)), to: underlay)
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(ceil(underlay.bounds.width)), pixelsHigh: Int(ceil(underlay.bounds.height)),
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
        underlay.draw(underlay.bounds)
        NSGraphicsContext.restoreGraphicsState()
        // The fill inside the border, above the glyph: red at 8%.
        let fill = try XCTUnwrap(bitmap.colorAt(x: Int(pill.midX), y: bitmap.pixelsHigh - Int(pill.minY + 3) - 1))
        XCTAssertEqual(fill.alphaComponent, 0x14 / 255.0, accuracy: 0.02)
        XCTAssertGreaterThan(fill.redComponent, fill.greenComponent)
        // The border along the top edge is solid: every pixel across the middle of the pill's top row is drawn.
        let row = bitmap.pixelsHigh - Int(pill.minY + 0.5) - 1
        let gaps = stride(from: Int(pill.minX + pill.height / 2), to: Int(pill.maxX - pill.height / 2), by: 1).filter { (bitmap.colorAt(x: $0, y: row)?.alphaComponent ?? 0) < 0.1 }
        XCTAssertEqual(gaps, [], "no dashes")
    }
}
