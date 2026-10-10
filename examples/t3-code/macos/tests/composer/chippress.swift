import AppKit
import XCTest

// settings-appearance-and-skill-chip (S1-12): a skill chip is ContextChipPopover's trigger in T3 Code
// (ComposerPromptEditorTiptap.tsx ComposerSkillNodeView). A press on the chip, by a real mouse event
// through the window, reports it to the app with the chip's window frame (T3ComposerChipPress.swift),
// in the composer and in Settings › Appearance's prompt sample; the chip is an accessibility button
// "Skill <label>. Show details".

private let previewSample = "Use $frontend-design to fix the flaky test in [surface.test.ts](apps/web/src/terminal/ghostty/surface.test.ts) before shipping."

/// The test process is never the active app, so its window is never key; the app's is (agent mode
/// included). A text view that takes the first click stands in for a click into the key window.
private final class FirstMouseTextView: NSTextView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

private final class ChipPreviewFixture {
    let base = EditorFixture()
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 130, width: 400, height: 60))
    let view: NSTextView = FirstMouseTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 60))
    let preview = T3ComposerEditor(changed: { _ in }, hatch: .t3PromptPreview)
    let element: ExactElement
    var announced = 0

    init() {
        view.isEditable = true
        view.isRichText = false
        view.allowsUndo = true
        view.font = NSFont.systemFont(ofSize: 14)
        preview.styler.richText = false
        scroller.documentView = view
        base.window.contentView?.addSubview(scroller)
        element = ExactElement(hatch: .t3PromptPreview, id: "prompt-font-preview", node: 40, hatches: base.hooks)
        element.view = scroller
        element.platform = view
        base.composer.install(element)
        preview.install(element)
        preview.styler.press.onChange = { [weak self] in self?.announced += 1 }
        view.string = previewSample
        view.didChangeText()
        base.window.makeFirstResponder(view)
        base.tick()
    }

    /// A real primary click at `point` (the text view's coordinates): the release queued first, as
    /// the agent's tap does, since NSTextView tracks the mouse inside mouseDown.
    func click(_ point: NSPoint, in target: NSTextView? = nil) {
        let target = target ?? view
        let location = target.convert(point, to: nil)
        let window = base.window
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        let up = NSEvent.mouseEvent(with: .leftMouseUp, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!
        // Through the application, as the agent's tap and a hand's click come (AgentMac.swift).
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(down)
        // The release, when the text view's tracking loop left it queued.
        if let queued = NSApp.nextEvent(matching: .leftMouseUp, until: Date(), inMode: .default, dequeue: true) { NSApp.sendEvent(queued) }
        base.tick()
    }

    func center(_ chip: T3ComposerChip, in styler: T3ComposerStyler? = nil) -> NSPoint {
        let rect = (styler ?? preview.styler).chipRect(chip)!
        return NSPoint(x: rect.midX, y: rect.midY)
    }

    deinit { preview.destroy() }
}

final class ComposerChipPressTests: XCTestCase {
    func testAPressOnThePreviewsSkillChipReportsItsDetailsAtTheChip() {
        let fixture = ChipPreviewFixture()
        let press = fixture.preview.styler.press
        let chip = fixture.preview.styler.chips[0]
        XCTAssertEqual(chip.kind, "skill")
        XCTAssertFalse(press.isOpen)
        fixture.click(fixture.center(chip))
        XCTAssertTrue(press.isOpen, "a real click on the chip opens its details")
        XCTAssertEqual(fixture.announced, 1, "the press is announced (t3.chip)")
        let state = press.state
        XCTAssertEqual(state["surface"] as? String, "preview")
        XCTAssertEqual(state["name"] as? String, "frontend-design")
        XCTAssertEqual(state["label"] as? String, "Frontend Design")
        XCTAssertEqual(state["open"] as? Bool, true)
        // The frame is the chip's pill in the window content's top-left space.
        let frame = state["frame"] as! [Double]
        let rect = fixture.view.convert(fixture.preview.styler.chipRect(chip)!, to: fixture.base.window.contentView)
        XCTAssertEqual(frame[0], Double(rect.minX), accuracy: 0.5)
        XCTAssertEqual(frame[1], Double(fixture.base.window.contentView!.bounds.height - rect.maxY), accuracy: 0.5)
        XCTAssertEqual(frame[2], Double(rect.width), accuracy: 0.5)
        XCTAssertGreaterThan(frame[3], 10)
        // A second press on the chip closes it (PopoverTrigger toggles); a third opens it again.
        fixture.click(fixture.center(chip))
        XCTAssertFalse(press.isOpen)
        fixture.click(fixture.center(chip))
        XCTAssertTrue(press.isOpen)
        // While open the frame follows the chip: a view above the text moves, the press is announced again.
        let announced = fixture.announced, x = (press.state["frame"] as! [Double])[0]
        fixture.scroller.setFrameOrigin(NSPoint(x: 12, y: 130))
        XCTAssertEqual((press.state["frame"] as! [Double])[0], x + 12, accuracy: 0.5)
        XCTAssertEqual(fixture.announced, announced + 1)
        // A press elsewhere in the text closes it, as does a mention chip (only skill chips open details).
        let mention = fixture.preview.styler.chips[1]
        fixture.click(fixture.center(mention))
        XCTAssertFalse(press.isOpen, "a press on another chip is a press outside")
        fixture.click(fixture.center(chip))
        XCTAssertTrue(press.isOpen)
        fixture.click(NSPoint(x: 395, y: 50))
        XCTAssertFalse(press.isOpen, "a press on the text outside the chip")
    }

    func testEscapeAnEditTheAppsCloseAndTheEditorLeavingCloseIt() {
        let fixture = ChipPreviewFixture()
        let press = fixture.preview.styler.press
        let chip = fixture.preview.styler.chips[0]
        press.press(chip)
        XCTAssertTrue(press.isOpen)
        fixture.base.escape()
        XCTAssertFalse(press.isOpen, "Escape closes the details")
        XCTAssertEqual(fixture.view.string, previewSample)
        press.press(chip)
        fixture.view.setSelectedRange(NSRange(location: (previewSample as NSString).length, length: 0))
        fixture.base.send("x", 7)
        XCTAssertFalse(press.isOpen, "typing closes them")
        press.press(fixture.preview.styler.chips[0])
        let seq = press.seq
        press.close(seq: seq - 1)
        XCTAssertTrue(press.isOpen, "the app closes only the press it saw")
        press.close(seq: seq)
        XCTAssertFalse(press.isOpen)
        press.press(fixture.preview.styler.chips[0])
        fixture.preview.remove(fixture.element)
        XCTAssertFalse(press.isOpen, "the sample leaving the window closes them")
    }

    func testTheNewestPressWinsAcrossTheComposerAndThePreview() {
        let fixture = ChipPreviewFixture()
        let composer = fixture.base.composer.editor
        _ = fixture.base.composer.perform(["op": "editorSync", "generation": 1, "skills": ["frontend-design": "Composer Design"]])
        fixture.base.window.makeFirstResponder(fixture.base.editor)
        fixture.base.type("Try $frontend-design now")
        fixture.base.window.display()
        let chip = composer.styler.chips[0]
        XCTAssertEqual(composer.styler.press.surface, "composer")
        fixture.click(fixture.center(chip, in: composer.styler), in: fixture.base.editor)
        XCTAssertTrue(composer.styler.press.isOpen, "a real click on the composer's chip opens its details")
        XCTAssertEqual(composer.styler.press.state["label"] as? String, "Composer Design", "the provider's display name")
        XCTAssertEqual(composer.styler.press.state["owner"] as? String, "owner-a")
        let presses = [composer.styler.press, fixture.preview.styler.press]
        XCTAssertEqual(T3ComposerChipPress.latest(presses)["surface"] as? String, "composer")
        fixture.preview.styler.press.press(fixture.preview.styler.chips[0])
        XCTAssertEqual(T3ComposerChipPress.latest(presses)["surface"] as? String, "preview", "the newer press")
        fixture.preview.styler.press.close()
        XCTAssertEqual(T3ComposerChipPress.latest(presses)["surface"] as? String, "composer")
        composer.styler.press.close()
        let closed = T3ComposerChipPress.latest(presses)
        XCTAssertEqual(closed["open"] as? Bool, false)
        XCTAssertEqual(closed["seq"] as? Int, fixture.preview.styler.press.seq)
    }

    func testTheChipIsAnAccessibilityButtonThatOpensItsDetails() {
        let fixture = ChipPreviewFixture()
        let elements = fixture.preview.styler.press.accessibilityElements
        XCTAssertEqual(elements.map { $0.accessibilityLabel() ?? "" }, ["Skill Frontend Design. Show details"])
        XCTAssertEqual(elements.first?.accessibilityRole(), .button)
        // realinput-1010c-fixes RC-4: an enabled button, as the reference's PopoverTrigger (the default is disabled).
        XCTAssertEqual(elements.first?.isAccessibilityEnabled(), true)
        let children = fixture.view.accessibilityChildren() as? [NSAccessibilityElement] ?? []
        XCTAssertTrue(children.contains { $0 === elements.first }, "the text view lists the chip's button")
        // As the agent's accessibility walk reads it (AgentAccessibility.swift axFacts), without trapping.
        let ordered: [Any] = fixture.view.accessibilityChildrenInNavigationOrder() ?? []
        XCTAssertTrue(NSAccessibility.unignoredChildren(from: ordered).contains { $0 as AnyObject === elements.first }, "and navigates to it")
        XCTAssertTrue(elements.first?.accessibilityParent() as AnyObject === fixture.view)
        XCTAssertTrue(elements.first!.accessibilityPerformPress())
        XCTAssertTrue(fixture.preview.styler.press.isOpen)
        // The mention chips are not this popover's triggers.
        XCTAssertEqual(fixture.preview.styler.chips.filter { $0.kind == "skill" }.count, elements.count)
    }
}
