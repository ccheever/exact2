import AppKit
import XCTest

// Settings uses the same shared field. Swift retains only a weak mounted-view
// registry so the ordinary desktop Select All menu includes an empty preview.

private let editorResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let editorAct: ExactHatches.ActFn = { _, _, _ in 0 }
private let editorLog: ExactHatches.LogFn = { _, _, _ in }
private let editorDelegate: ExactHatches.DelegateFn = { _, _, _ in }

/// Stands in for Exact's node: the text view's real delegate.
final class InnerDelegate: NSObject, NSTextViewDelegate {}

/// A mounted composer (hatch `t3-composer`) on a real AppKit text view.
final class EditorFixture {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    let hooks: ExactHatches
    let window: NSWindow
    let scroller = NSScrollView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 120))
    let composer = T3Composer()
    let element: ExactElement

    init() {
        _ = NSApplication.shared
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(editorResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(editorDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHatches(host: nil, table: UnsafeRawPointer(table))!
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 420, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        editor.isEditable = true
        scroller.documentView = editor
        window.contentView?.addSubview(scroller)
        element = ExactElement(hatch: .t3Composer, id: "composer", node: 1, hatches: hooks)
        element.view = scroller
        element.platform = editor
        composer.install(element)
    }

    deinit { composer.destroy(); window.close(); table.deallocate() }
}

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
        XCTAssertTrue(fixture.composer.textView === fixture.editor, "the composer's own text view, which the preview leaves alone")
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
