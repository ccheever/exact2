#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// HTML's `autocorrect="off"` on AppKit (#111): the text is kept as typed.
/// A textarea's text view and an input's field editor then make no spelling
/// correction, smart quote or dash, or text replacement; without it each
/// keeps what it had, AppKit's or the person's from the Substitutions menu.
final class AutocorrectMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": "", "autocorrect": "off"], "handlers": ["input"]],
            ["op": "create", "id": 3, "kind": "textarea", "props": ["value": ""], "handlers": ["input"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "", "autocorrect": "off"], "handlers": ["input"]],
            ["op": "create", "id": 5, "kind": "input", "props": ["value": ""], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 70.0, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 140.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 180.0, "w": 300.0, "h": 24.0],
        ]))
        return p
    }

    /// Spelling correction, smart quotes, smart dashes, text replacement.
    private func flags(_ v: NSTextView) -> [Bool] {
        [v.isAutomaticSpellingCorrectionEnabled, v.isAutomaticQuoteSubstitutionEnabled, v.isAutomaticDashSubstitutionEnabled, v.isAutomaticTextReplacementEnabled]
    }

    func testATextareaWithAutocorrectOffKeepsWhatIsTyped() throws {
        let p = presenter()
        let off = try XCTUnwrap(p.views[2]?.textArea), plain = try XCTUnwrap(p.views[3]?.textArea)
        let appKit = flags(NSTextView(frame: .zero))
        XCTAssertEqual(flags(off), [false, false, false, false])
        XCTAssertEqual(flags(plain), appKit, "AppKit's own")
        let typed = "'0' a -- b "
        for view in [off, plain] {
            window.makeFirstResponder(view)
            for ch in typed { view.insertText(String(ch), replacementRange: view.selectedRange()) }
            view.checkTextInDocument(nil) // AppKit's checking, as after a pause in typing
        }
        XCTAssertEqual(off.string, typed)
        if appKit[1] { XCTAssertNotEqual(plain.string, typed, "the check substitutes where AppKit's smart quotes are on") }
        // Taken back, AppKit's again; set, off.
        p.views[2]?.applyProps(set: [:], clear: ["autocorrect"])
        XCTAssertEqual(flags(off), appKit)
        p.views[3]?.applyProps(set: ["autocorrect": "OFF"], clear: [])
        XCTAssertEqual(flags(plain), [false, false, false, false])
    }

    func testThePersonsSubstitutionsOutlastTheAppsUpdates() throws {
        let p = presenter()
        let plain = try XCTUnwrap(p.views[3]?.textArea)
        plain.isAutomaticQuoteSubstitutionEnabled = false // Edit > Substitutions > Smart Quotes
        plain.isAutomaticTextReplacementEnabled = true
        p.views[3]?.applyProps(set: ["value": "x"], clear: [])
        XCTAssertEqual(Array(flags(plain)[1...]), [false, plain.isAutomaticDashSubstitutionEnabled, true], "a value is no reset")
        let dash = plain.isAutomaticDashSubstitutionEnabled
        p.views[3]?.applyProps(set: ["autocorrect": "off"], clear: [])
        XCTAssertEqual(flags(plain), [false, false, false, false])
        p.views[3]?.applyProps(set: ["value": "y"], clear: [])
        p.views[3]?.applyProps(set: ["autocorrect": "on"], clear: [])
        XCTAssertEqual(Array(flags(plain)[1...]), [false, dash, true], "the person's, given back")
    }

    func testAnInputsFieldEditorTakesEachFieldsAutocorrect() throws {
        let p = presenter()
        let off = try XCTUnwrap(p.views[4]?.field), plain = try XCTUnwrap(p.views[5]?.field)
        XCTAssertTrue(window.makeFirstResponder(plain))
        let editor = try XCTUnwrap(plain.currentEditor() as? NSTextView)
        let fieldEditor = flags(editor)
        XCTAssertEqual(Array(fieldEditor[..<3]), [true, false, false], "no smart quotes or dashes in a field, as AppKit makes it")
        editor.isAutomaticQuoteSubstitutionEnabled = true // the person's, from the field's context menu
        XCTAssertTrue(window.makeFirstResponder(off))
        XCTAssertTrue(off.currentEditor() === editor, "the window's one field editor")
        XCTAssertEqual(flags(editor), [false, false, false, false], "the last field's checking is not carried")
        p.views[4]?.applyProps(set: ["value": "z"], clear: [])
        XCTAssertEqual(flags(editor), [false, false, false, false])
        XCTAssertTrue(window.makeFirstResponder(plain))
        XCTAssertEqual(flags(editor), [true, true, false, fieldEditor[3]], "the person's, given back")
    }
}
#endif
