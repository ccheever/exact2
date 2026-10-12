import AppKit
import XCTest

// Lane r4-timeline: the composer chips' hover details (T3ComposerChipTips.swift)
// on a real text view: one AppKit tool-tip rect per chip, saying what the
// reference's chip tooltip says.
final class ComposerChipTipTests: XCTestCase {
    func testChipsCarryTheReferenceTooltips() {
        let fixture = EditorFixture()
        let styler = fixture.composer.editor.styler
        styler.contexts = ["file/file_abc": "file\t1 KB", "thread/thread_t": "thread", "image/image_i": "image\t2 KB\tdraft"]
        fixture.type("see [a.md](src/a.md) [notes.md](t3-context://v1/file/file_abc) [Old](t3-context://v1/thread/thread_t) ![shot.png](t3-context://v1/image/image_i) [gone](t3-context://v1/file/file_gone) end")
        fixture.window.display()
        fixture.tick()
        let tips = styler.tips
        XCTAssertEqual(styler.chips.count, 5)
        var seen: [String] = []
        for chip in styler.chips {
            guard let rect = styler.segments(chip.range).first else { return XCTFail("chip \(chip.label) has no frame") }
            seen.append(tips.registered(at: NSPoint(x: rect.midX, y: rect.midY)) ?? "-")
        }
        XCTAssertEqual(seen, ["src/a.md", "notes.md\n1 KB", "Open thread", "shot.png\n2 KB", "This context is no longer available. Remove it or attach it again."])
        XCTAssertGreaterThanOrEqual(tips.count, 5, "one tool-tip rect per chip segment")
        // AppKit asks the owner by tag; an edit that removes the chips removes their rects.
        fixture.editor.selectAll(nil)
        fixture.editor.insertText("plain", replacementRange: fixture.editor.selectedRange())
        fixture.tick(); fixture.tick()
        styler.restyle()
        XCTAssertEqual(tips.count, 0)
    }
}
