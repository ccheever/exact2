import AppKit
import XCTest

// Settings uses the same shared field. Swift retains only a weak mounted-view
// registry so the ordinary desktop Select All menu includes an empty preview.
final class PromptPreviewEditorTests: XCTestCase {
    func testPreviewRegistryDoesNotTakeDelegateSelectionStylesOrSource() {
        let fixture = EditorFixture(), registry = T3ComposerPreview()
        let view = NSTextView(frame: NSRect(x: 0, y: 130, width: 300, height: 60))
        let delegate = InnerDelegate()
        view.delegate = delegate
        view.string = "**sample** [file](src/file)"
        view.setSelectedRange(NSRange(location: 2, length: 3))
        let before = view.attributedString()
        fixture.window.contentView?.addSubview(view)
        let element = ExactElement(hatch: .t3PromptPreview, id: "preview", node: 40, hatches: fixture.hooks)
        element.view = view; element.platform = view
        fixture.composer.install(element); registry.install(element)
        XCTAssertTrue(registry.textView === view)
        XCTAssertTrue(view.delegate === delegate)
        XCTAssertEqual(view.selectedRange(), NSRange(location: 2, length: 3))
        XCTAssertEqual(view.attributedString(), before)
        XCTAssertTrue(fixture.composer.editor.textView === fixture.editor)
        element.ended = true
        XCTAssertNil(registry.textView)
        element.ended = false
        registry.remove(element)
        XCTAssertNil(registry.textView)
        XCTAssertTrue(view.delegate === delegate)
    }

    func testPreviewRegistryIsWeakAndIgnoresOtherFields() {
        let fixture = EditorFixture(), registry = T3ComposerPreview()
        registry.install(fixture.element)
        XCTAssertNil(registry.textView)
        weak var released: ExactElement?
        do {
            let element = ExactElement(hatch: .t3PromptPreview, id: "preview", node: 40, hatches: fixture.hooks)
            let view = NSTextView()
            element.platform = view
            released = element; registry.install(element)
            XCTAssertNotNil(registry.textView)
        }
        XCTAssertNil(released)
        XCTAssertNil(registry.textView)
        registry.destroy()
    }
}
