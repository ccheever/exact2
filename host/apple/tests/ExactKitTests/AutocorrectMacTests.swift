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
        // The person's own correction (System Settings ▸ Keyboard), which a
        // plain field no longer forces on (LLP 1115.000 D7.7).
        XCTAssertEqual(Array(fieldEditor[..<3]), [NSSpellChecker.isAutomaticSpellingCorrectionEnabled, false, false], "the person's correction; no smart quotes or dashes in a field, as AppKit makes it")
        editor.isAutomaticQuoteSubstitutionEnabled = true // the person's, from the field's context menu
        XCTAssertTrue(window.makeFirstResponder(off))
        XCTAssertTrue(off.currentEditor() === editor, "the window's one field editor")
        XCTAssertEqual(flags(editor), [false, false, false, false], "the last field's checking is not carried")
        p.views[4]?.applyProps(set: ["value": "z"], clear: [])
        XCTAssertEqual(flags(editor), [false, false, false, false])
        XCTAssertTrue(window.makeFirstResponder(plain))
        XCTAssertEqual(flags(editor), [fieldEditor[0], true, false, fieldEditor[3]], "the person's, given back")
    }

    /// LLP 1115.000 D7.7: unsaid `autocorrect` and `spellcheck` are the
    /// person's settings. The host writes only what the author wrote: a
    /// plain textarea starts as AppKit makes a text view (it was forced to
    /// correct and to check spelling), what the person then chooses in
    /// Edit ▸ Spelling and Grammar outlasts the app's updates (each update
    /// wrote both again), and a written value holds only while it is written.
    func testUnsaidCheckingIsThePersonsAndOnlyAWrittenValueIsWritten() throws {
        let p = presenter()
        let plain = try XCTUnwrap(p.views[3]?.textArea), node = try XCTUnwrap(p.views[3])
        let appKit = NSTextView(frame: .zero)
        XCTAssertEqual(plain.isContinuousSpellCheckingEnabled, appKit.isContinuousSpellCheckingEnabled, "spelling as AppKit makes a text view")
        XCTAssertEqual(plain.isAutomaticSpellingCorrectionEnabled, appKit.isAutomaticSpellingCorrectionEnabled, "correction as the person's settings have it")
        // The person's choices, the opposite of what was forced, and an update from the app.
        plain.isContinuousSpellCheckingEnabled = false
        plain.isAutomaticSpellingCorrectionEnabled = false
        node.applyProps(set: ["value": "x"], clear: [])
        XCTAssertFalse(plain.isContinuousSpellCheckingEnabled, "an update does not turn spelling back on")
        XCTAssertFalse(plain.isAutomaticSpellingCorrectionEnabled, "nor correction")
        // Written, it is the author's, whatever the person's is; taken back, the person's again.
        node.applyProps(set: ["spellcheck": "true", "autocorrect": "on"], clear: [])
        XCTAssertTrue(plain.isContinuousSpellCheckingEnabled)
        XCTAssertTrue(plain.isAutomaticSpellingCorrectionEnabled)
        node.applyProps(set: ["spellcheck": "false"], clear: [])
        XCTAssertFalse(plain.isContinuousSpellCheckingEnabled)
        plain.isAutomaticSpellingCorrectionEnabled = true // nothing the person does moves a written value's hold
        node.applyProps(set: [:], clear: ["spellcheck", "autocorrect"])
        XCTAssertFalse(plain.isContinuousSpellCheckingEnabled, "the person's, given back")
        XCTAssertFalse(plain.isAutomaticSpellingCorrectionEnabled, "the person's, given back")
        // The window's one field editor: a field that says nothing does not keep the last field's answer.
        p.views[4]?.applyProps(set: ["spellcheck": "true"], clear: [])
        let written = try XCTUnwrap(p.views[4]?.field), unsaid = try XCTUnwrap(p.views[5]?.field)
        XCTAssertTrue(window.makeFirstResponder(unsaid))
        let editor = try XCTUnwrap(unsaid.currentEditor() as? NSTextView)
        let spelling = editor.isContinuousSpellCheckingEnabled
        XCTAssertTrue(window.makeFirstResponder(written))
        XCTAssertTrue(editor.isContinuousSpellCheckingEnabled, "the author's `spellcheck`")
        XCTAssertTrue(window.makeFirstResponder(unsaid))
        XCTAssertEqual(editor.isContinuousSpellCheckingEnabled, spelling, "the editor's own again")
    }
}
#endif
