import AppKit
import XCTest

// Settings › Appearance's editable prompt sample (settings-prompt-preview.contract): its
// `t3-prompt-preview` textarea gets its own T3ComposerEditor (T3Module promptPreview), beside
// the composer's, on a real AppKit text view driven by real key events.

private let sample = "Use $frontend-design to fix the flaky test in [surface.test.ts](apps/web/src/terminal/ghostty/surface.test.ts) and align the header with [SettingsPanels.tsx](apps/web/src/components/settings/SettingsPanels.tsx) before shipping."

private final class PreviewFixture {
    let base = EditorFixture()
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 130, width: 400, height: 60))
    let view = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 60))
    let preview = T3ComposerEditor(changed: { _ in }, hatch: .t3PromptPreview)
    let element: ExactElement

    init() {
        view.isEditable = true
        view.isRichText = false
        view.allowsUndo = true
        view.font = NSFont.systemFont(ofSize: 14)
        preview.styler.richText = false // as T3Module configures it
        scroller.documentView = view
        base.window.contentView?.addSubview(scroller)
        element = ExactElement(hatch: .t3PromptPreview, id: "prompt-font-preview", node: 40, hatches: base.hooks)
        element.view = scroller
        element.platform = view
        // T3Module hands every element to both: each editor takes only its own hatch.
        base.composer.install(element)
        preview.install(element)
        view.string = sample
        view.didChangeText()
        base.window.makeFirstResponder(view)
        view.setSelectedRange(NSRange(location: (sample as NSString).length, length: 0))
        base.tick()
    }

    deinit { preview.destroy() }
}

final class PromptPreviewEditorTests: XCTestCase {
    func testThePreviewDrawsTheSamplesChipsWithItsOwnEditor() {
        let fixture = PreviewFixture()
        XCTAssertTrue(fixture.preview.textView === fixture.view)
        XCTAssertTrue(fixture.base.composer.editor.textView === fixture.base.editor, "the composer keeps its own text view")
        XCTAssertEqual(fixture.preview.styler.chips.map(\.kind), ["skill", "mention", "mention"])
        XCTAssertEqual(fixture.preview.styler.chips.map(fixture.preview.styler.displayLabel), ["Frontend Design", "surface.test.ts", "SettingsPanels.tsx"])
        // The composer's provider skills are the composer's: the sample keeps the title-cased name
        // (SettingsFontPreviews passes EMPTY_SKILLS).
        _ = fixture.base.composer.perform(["op": "editorSync", "generation": 1, "skills": ["frontend-design": "Composer Only"]])
        XCTAssertEqual(fixture.base.composer.editor.styler.skills, ["frontend-design": "Composer Only"])
        XCTAssertEqual(fixture.preview.styler.chips.map(fixture.preview.styler.displayLabel).first, "Frontend Design")
        XCTAssertEqual(fixture.base.composer.editor.styler.chips.count, 0, "the composer's prompt is untouched")
        // A chip's label is the prompt's own family (New York here) at weight 500, matched as CSS
        // does: the family's medium face, else its regular one (Menlo has no medium), never bold.
        let weight = { (font: NSFont) in (font.fontDescriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any])?[.weight] as? CGFloat ?? -1 }
        let serif = NSFont(descriptor: NSFont.systemFont(ofSize: 18).fontDescriptor.withDesign(.serif)!, size: 18)!
        let chip = fixture.preview.styler.chipFont(serif)
        XCTAssertEqual(chip.familyName, serif.familyName)
        XCTAssertEqual(chip.pointSize, 18 * 0.86, accuracy: 0.01)
        XCTAssertEqual(weight(chip), NSFont.Weight.medium.rawValue, accuracy: 0.05)
        let system = fixture.preview.styler.chipFont(NSFont.systemFont(ofSize: 14))
        XCTAssertEqual(system.familyName, NSFont.systemFont(ofSize: 14, weight: .medium).familyName)
        XCTAssertEqual(weight(system), NSFont.Weight.medium.rawValue, accuracy: 0.05)
        if let menlo = NSFont(name: "Menlo-Regular", size: 14) {
            let mono = fixture.preview.styler.chipFont(menlo)
            XCTAssertEqual(mono.familyName, "Menlo")
            XCTAssertLessThan(weight(mono), NSFont.Weight.semibold.rawValue, "no bold for a family without a medium face")
        }
    }

    func testMarkdownMarkersStayTextAndChipEditsUndo() {
        let fixture = PreviewFixture()
        fixture.view.setSelectedRange(NSRange(location: 0, length: 3))
        fixture.view.insertText("**Try**", replacementRange: fixture.view.selectedRange()); fixture.base.tick()
        XCTAssertTrue(fixture.preview.styler.marks.isEmpty, "richTextEnabled is off in the sample: markers stay literal")
        fixture.view.undoManager?.undo()
        XCTAssertEqual(fixture.view.string, sample, "the replaced selection comes back")
        let chip = fixture.preview.styler.chips[1]
        fixture.view.setSelectedRange(NSRange(location: chip.end, length: 0))
        fixture.base.backspace()
        XCTAssertFalse(fixture.view.string.contains("surface.test.ts"))
        fixture.view.undoManager?.undo()
        XCTAssertEqual(fixture.view.string, sample, "the deleted chip comes back")
        XCTAssertEqual(fixture.preview.styler.chips.count, 3)
    }

    func testTypingAndUndoStayInThePreview() {
        let fixture = PreviewFixture()
        var clock: TimeInterval = 100
        fixture.preview.now = { clock }
        for character in " AUDIT_FONT_PROBE" { fixture.base.send(String(character), 0); clock += 0.05 }
        XCTAssertEqual(fixture.view.string, sample + " AUDIT_FONT_PROBE")
        XCTAssertEqual(fixture.base.editor.string, "", "typing never reaches the composer")
        fixture.view.undoManager?.undo()
        XCTAssertEqual(fixture.view.string, sample, "⌘Z restores the sample")
        XCTAssertEqual(fixture.preview.styler.chips.count, 3)
    }

    func testChipsAreAtomicAndTheComposersKeysAreNotThePreviews() {
        let fixture = PreviewFixture()
        fixture.base.key("Enter", node: 13)
        fixture.preview.install(fixture.base.keys["Enter"]!)
        // ← from the end of "SettingsPanels.tsx)" steps over the whole chip.
        let text = sample as NSString
        let chip = fixture.preview.styler.chips[2]
        fixture.view.setSelectedRange(NSRange(location: chip.end, length: 0))
        fixture.base.left()
        XCTAssertEqual(fixture.view.selectedRange().location, chip.start)
        fixture.view.setSelectedRange(NSRange(location: chip.end, length: 0))
        fixture.base.backspace()
        XCTAssertFalse(fixture.view.string.contains("SettingsPanels.tsx"), "⌫ removes the whole chip")
        XCTAssertEqual((fixture.view.string as NSString).length, text.length - (chip.end - chip.start))
        // A command trigger in the preview opens no menu: Return is a newline, never the composer's Enter key.
        fixture.view.setSelectedRange(NSRange(location: (fixture.view.string as NSString).length, length: 0))
        fixture.view.insertText(" /m", replacementRange: fixture.view.selectedRange()); fixture.base.tick()
        fixture.base.returnKey()
        XCTAssertFalse(pressedNodes.contains(13))
        XCTAssertTrue(fixture.view.string.hasSuffix(" /m\n"))
        XCTAssertEqual(fixture.base.editor.string, "")
        // The composer's key monitor (an NSEvent local monitor, which window.sendEvent skips) passes
        // the preview's keys through: Return never sends, a letter is never redirected to the composer.
        let send = ExactElement(hatch: .t3Send, id: "send-message", node: 2, hatches: fixture.base.hooks)
        let sendView = NSView(frame: NSRect(x: 0, y: 0, width: 10, height: 10))
        fixture.base.window.contentView?.addSubview(sendView)
        send.view = sendView
        fixture.base.composer.install(send)
        for (characters, code) in [("\r", UInt16(36)), ("x", UInt16(7))] {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: fixture.base.window.windowNumber,
                context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
            XCTAssertTrue(fixture.base.composer.handle(event) === event, "\(characters.debugDescription) passes through")
        }
        XCTAssertFalse(pressedNodes.contains(2), "no send")
        XCTAssertEqual(fixture.base.editor.string, "")
    }

    func testTabAndShiftTabLeaveThePreviewWithoutTyping() {
        let fixture = PreviewFixture()
        let before = NSTextField(frame: NSRect(x: 0, y: 0, width: 40, height: 20)), after = NSTextField(frame: NSRect(x: 50, y: 0, width: 40, height: 20))
        fixture.base.window.contentView?.addSubview(before); fixture.base.window.contentView?.addSubview(after)
        before.nextKeyView = fixture.view; fixture.view.nextKeyView = after; after.nextKeyView = before
        fixture.base.tab()
        XCTAssertTrue(fixture.base.window.firstResponder === after.currentEditor() || fixture.base.window.firstResponder === after, "Tab moves on")
        XCTAssertEqual(fixture.view.string, sample, "and types no tab")
        fixture.base.window.makeFirstResponder(fixture.view)
        fixture.base.backtab()
        XCTAssertTrue(fixture.base.window.firstResponder === before.currentEditor() || fixture.base.window.firstResponder === before, "⇧Tab moves back")
        XCTAssertEqual(fixture.view.string, sample)
    }
}
