// @ref LLP 1115.000 D7 — a key a `keycapture` handler prevented has no
// default on the Mac: the focus stays where it is, as on the web.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class KeyCaptureEscapeMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    /// A shortcut recorder: an idle field whose focus starts recording, which
    /// mounts an `aria-modal` row around an `autofocus` field that takes every
    /// key first (`keycapture`). Escape ends recording, which changes only the
    /// field's placeholder; the row stays up until the field's `blur`.
    static let page = """
    component Probe
      state recording = false
      state taking = false
      state heard = ""
      action took(key: string)
        taking = key != "blur"
        recording = false
        heard = `${heard}${key};`
      action record
        recording = true
      view
        column testId="page" width="100%" height="100%" padding=20 gap=12
          if recording or taking
            row aria-modal=true testId="modal"
              Recorder(prompt=(recording ? "Press shortcut" : "Unassigned"), capture=took, cancel=took("blur"))
          else
            row focusin=record testId="idle"
              input id="field" testId="field" value="" placeholder="Unassigned" width=200 height=24
          text heard testId="status"

    component Recorder
      props
        prompt: string
        capture: action
        cancel: action
      action keys(k: string, e: KeyboardEvent)
        preventDefault()
        stopPropagation()
        if k == "Escape"
          capture("escape")
        else
          capture(k)
      view
        input id="recorder" testId="recorder" autofocus=true keycapture=keys blur=cancel aria-label="Keybinding" value="" placeholder=prompt width=200 height=24

    """

    private func key(_ code: UInt16, _ characters: String, _ f: SheetFixture) throws {
        let p = f.presenter
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = try XCTUnwrap(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                       windowNumber: f.window.windowNumber, context: nil, characters: characters,
                                                       charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code))
            // As the session's monitor routes it, then AppKit when it was not taken.
            if type == .keyDown { p.flushKeyViewLoop() }
            if !p.routeKey(event, focused: true, in: f.window) { f.window.sendEvent(event) }
            f.settle()
        }
    }

    /// Escape in the recorder: its handler prevented it and only the
    /// placeholder changed, so the field keeps the focus and hears no blur.
    func testAPreventedEscapeThatChangesAPlaceholderKeepsTheFocus() throws {
        let f = try SheetFixture(page: Self.page)
        fixture = f
        let idle = try XCTUnwrap(try f.node("field").field)
        XCTAssertTrue(f.window.makeFirstResponder(idle))
        f.settle()
        let recorder = try f.node("recorder")
        let field = try XCTUnwrap(recorder.field)
        XCTAssertNotNil(field.currentEditor(), "the recorder mounted and took the focus")
        XCTAssertEqual(field.placeholderAttributedString?.string, "Press shortcut")
        try key(53, "\u{1b}", f)
        XCTAssertEqual(f.status, "escape;", "heard the Escape and no blur")
        XCTAssertTrue(try f.node("recorder") === recorder, "the row stays up")
        XCTAssertEqual(field.placeholderAttributedString?.string, "Unassigned")
        XCTAssertNotNil(field.currentEditor(), "the field keeps the focus")
        XCTAssertTrue(f.window.firstResponder === field.currentEditor())
        // The next key is still the recorder's.
        try key(40, "k", f)
        XCTAssertEqual(f.status, "escape;k;")
        XCTAssertNotNil(field.currentEditor())
    }

    /// A batch that changes the placeholder of a focused, empty field: AppKit
    /// restarts the field editor's editing to show it (with text in the
    /// field it does not), and the field keeps the focus and its caret, with
    /// no blur, focus or `change` heard.
    func testAPlaceholderChangeKeepsAFocusedFieldsEditing() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        addTeardownBlock { window.close() }
        window.contentView = p.viewport
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 200],
                           ["op": "create", "id": 2, "kind": "input", "props": ["value": "", "placeholder": "Press shortcut"], "handlers": ["focus", "blur", "change"]],
                           ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 200, "h": 24],
                           ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]]))
        var heard: [String] = []
        p.onFocus = { _ in heard.append("focus") }
        p.onBlur = { _ in heard.append("blur") }
        p.onChange = { _, _ in heard.append("change") }
        let field = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        heard = []
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["placeholder": "Unassigned"]]]))
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        XCTAssertEqual(field.placeholderAttributedString?.string, "Unassigned")
        XCTAssertTrue(window.firstResponder === field.currentEditor(), "the field keeps the field editor")
        XCTAssertEqual((field.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 0, length: 0), "and its caret")
        XCTAssertEqual(heard, [], "no blur, focus or change")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["placeholder"]]]))
        XCTAssertTrue(window.firstResponder === field.currentEditor())
        XCTAssertEqual(heard, [])
        // Leaving it is still heard.
        XCTAssertTrue(window.makeFirstResponder(nil))
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        XCTAssertEqual(heard.filter { $0 == "blur" }, ["blur"])
    }
}
#endif
