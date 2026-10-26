#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

/// #125 on UIKit: a text view's or text field's paste:, copy: and cut: (the
/// edit menu, a hardware keyboard's ⌘V, ⌘C, ⌘X) are DOM's `paste`, `copy`
/// and `cut` at the nearest handler, before the editor's own, which an
/// action's `preventDefault()` cancels. A simulator runs it:
///   bun host/apple/build.mjs --test --ios
/// The paste-variant case runs in an app host so it can read its own pasteboard.
/// Other paste cases carry the driver's text instead.
final class FieldClipboardIOSTests: XCTestCase {
    private var window: UIWindow!
    private var heard: [String] = []

    override func tearDown() { window = nil }

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // An action that prevents a paste of "BLOCK" or any cut stands in for one calling preventDefault().
        p.onClipboard = { [unowned self, unowned p] id, kind, text in
            heard.append("\(id):\(kind):\(text)")
            if text == "BLOCK" || (id == 1 && kind == 33) { p.defaultPrevented = true }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["paste", "cut"]],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": "hello"], "handlers": ["input", "copy", "cut"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "line", "value": "hello"], "handlers": ["input"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 30.0],
        ]))
        return p
    }

    func testATextViewsCutIsItsEventThenItsOwn() throws {
        let p = presenter()
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(editor.becomeFirstResponder())
        editor.selectedRange = NSRange(location: 0, length: 2)
        editor.copy(nil)
        editor.cut(nil)
        XCTAssertEqual(editor.text, "llo", "an unprevented cut is the editor's")
        XCTAssertEqual(heard, ["2:32:", "2:33:"], "a cut's own copy is no second event")
    }

    func testATextFieldHearsAnAncestorAndAPreventedEditIsSkipped() throws {
        let p = presenter()
        let node = try XCTUnwrap(p.views[3])
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        field.selectedTextRange = field.textRange(from: field.beginningOfDocument, to: field.endOfDocument)
        field.cut(nil)
        XCTAssertEqual(field.text, "hello", "the ancestor's cut prevented it")
        XCTAssertTrue(node.fieldClipboard(#selector(NodeView.paste(_:)), text: "BLOCK"))
        XCTAssertFalse(node.fieldClipboard(#selector(NodeView.paste(_:)), text: "!"))
        XCTAssertEqual(heard, ["1:33:", "1:34:BLOCK", "1:34:!"])
    }

    /// A node no one hears declines the edit menu's copy, cut and paste,
    /// and one sent anyway is dropped, never forwarded to UIView (which
    /// implements none: a field's Cut, empty, raised doesNotRecognizeSelector).
    func testAnUnheardEditIsDeclinedAndNeverForwarded() throws {
        let p = presenter()
        let line = try XCTUnwrap(p.views[3]), editor = try XCTUnwrap(p.views[2]), root = try XCTUnwrap(p.views[1])
        for action in [#selector(NodeView.copy(_:)), #selector(NodeView.cut(_:)), #selector(NodeView.paste(_:))] {
            XCTAssertEqual(root.canPerformAction(action, withSender: nil), action != #selector(NodeView.copy(_:)), "the root hears paste and cut, not copy")
            // A field's or text area's edits are its editor's, not the node's.
            XCTAssertFalse(line.canPerformAction(action, withSender: nil))
            XCTAssertFalse(editor.canPerformAction(action, withSender: nil))
        }
        line.copy(nil)
        line.cut(nil)
        root.copy(nil)
        XCTAssertEqual(heard, [], "an unheard copy is no event and no crash")
    }

    /// With nothing focused the session's view is the first responder: the
    /// edit menu's cut, copy and paste go to the first root's handlers, as
    /// the DOM's go to `<body>`, and a hardware key to its `key` handlers.
    func testWithNothingFocusedTheRootHearsTheEditMenuAndKeys() throws {
        let session = ExactApp.shared.makeSession(label: "root-edits")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let view = ExactView(session: session)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.addSubview(view)
        window.makeKeyAndVisible()
        let p = session.presenter
        session.apply(wireBatch([
            ["op": "create", "id": 9001, "kind": "view", "props": [:], "handlers": ["cut", "key"]],
            ["op": "roots", "ids": [9001]],
            ["op": "frame", "id": 9001, "x": 0.0, "y": 0.0, "w": 390.0, "h": 844.0],
        ]))
        p.onClipboard = { [unowned self] id, kind, text in heard.append("\(id):\(kind):\(text)") }
        var keys: [String] = []
        p.onKey = { id, press in keys.append("\(id) \(press.chord)") }
        XCTAssertTrue(p.keyRoot === p.views[9001])
        XCTAssertTrue(view.becomeFirstResponder())
        XCTAssertTrue(view.canPerformAction(#selector(UIResponderStandardEditActions.cut(_:)), withSender: nil))
        XCTAssertFalse(view.canPerformAction(#selector(UIResponderStandardEditActions.copy(_:)), withSender: nil), "no copy handler")
        view.cut(nil)
        XCTAssertEqual(heard, ["9001:33:"])
        XCTAssertFalse(p.keyDown(at: p.keyRoot, "a"))
        XCTAssertEqual(keys, ["9001 a"])
    }

    func testMarkdownMatchStyleDropsSourceAttributesAndDeliversOnePaste() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "pasteboard reads need an app identity") }
        ExactMarkdown.install()
        let p = presenter()
        defer { UIPasteboard.general.items = [] }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["markup": "markdown", "value": ""]]]))
        let editor = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertNil(editor.pasteDelegate)
        XCTAssertFalse(editor.allowsEditingTextAttributes)
        XCTAssertTrue(editor.becomeFirstResponder())
        let rich = NSAttributedString(string: "Bold", attributes: [.font: UIFont.boldSystemFont(ofSize: 31), .foregroundColor: UIColor.magenta])
        let data = try rich.data(from: NSRange(location: 0, length: rich.length), documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf])
        UIPasteboard.general.items = [["public.rtf": data, "public.utf8-plain-text": Data("Bold".utf8)]]
        let inserted = expectation(description: "the markdown editor inserted the paste")
        var pastedAttributes: [NSAttributedString.Key: Any] = [:]
        p.onInput = { id, _ in
            if id == 2 {
                pastedAttributes = editor.textStorage.attributes(at: 0, effectiveRange: nil)
                inserted.fulfill()
            }
        }
        editor.pasteAndMatchStyle(nil)
        wait(for: [inserted], timeout: 5)
        XCTAssertEqual(editor.text, "Bold")
        XCTAssertEqual(heard, ["1:34:Bold"])
        XCTAssertFalse((pastedAttributes[.font] as? UIFont)?.fontDescriptor.symbolicTraits.contains(.traitBold) ?? false)
        XCTAssertNotEqual(pastedAttributes[.foregroundColor] as? UIColor, .magenta)

        let richEditor = UITextView(frame: editor.frame)
        richEditor.allowsEditingTextAttributes = true
        window.addSubview(richEditor)
        XCTAssertTrue(richEditor.becomeFirstResponder())
        let richInserted = expectation(forNotification: UITextView.textDidChangeNotification, object: richEditor)
        richEditor.paste(nil)
        wait(for: [richInserted], timeout: 5)
        let font = try XCTUnwrap(richEditor.textStorage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
        XCTAssertTrue(font.fontDescriptor.symbolicTraits.contains(.traitBold), "the same payload carries rich text")
        XCTAssertEqual(richEditor.textStorage.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor, .magenta)
    }

    func testFilePasteEligibilityExcludesWebURLs() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "pasteboard reads need an app identity") }
        let p = presenter()
        defer { UIPasteboard.general.items = [] }
        let paste = #selector(UIResponderStandardEditActions.paste(_:))
        let match = #selector(UIResponderStandardEditActions.pasteAndMatchStyle(_:))
        UIPasteboard.general.items = [["public.url": Data("https://example.com/".utf8)]]
        XCTAssertTrue(UIPasteboard.general.hasURLs)
        XCTAssertNil(UIPasteboard.general.string)
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            XCTAssertFalse(node.pastesFiles(paste), "a web URL is no file attachment")
            XCTAssertFalse(node.pastesFiles(match), "Match Style has the same file eligibility")
        }
        UIPasteboard.general.urls = [URL(fileURLWithPath: "/tmp/pasted-file")]
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            XCTAssertTrue(node.pastesFiles(paste))
            XCTAssertTrue(node.pastesFiles(match))
        }
    }

    /// Both edit-menu variants deliver one paste before the editor inserts text.
    /// A listener can also receive an image or file URL the plain editor cannot insert.
    func testUIKitPasteVariantsDeliverThePasteEventAndAcceptFiles() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "pasteboard reads need an app identity") }
        let p = presenter()
        defer { UIPasteboard.general.items = [] }
        let image = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 2)).image { $0.fill(CGRect(x: 0, y: 0, width: 2, height: 2)) }
        let paste = #selector(UIResponderStandardEditActions.paste(_:))
        let match = #selector(UIResponderStandardEditActions.pasteAndMatchStyle(_:))
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            let editor = try XCTUnwrap((node.textArea as UIResponder?) ?? node.field)
            XCTAssertTrue(editor.becomeFirstResponder())
            UIPasteboard.general.items = []
            UIPasteboard.general.image = image
            XCTAssertTrue(editor.canPerformAction(paste, withSender: nil), "Paste accepts images at a listener")
            XCTAssertTrue(editor.canPerformAction(match, withSender: nil), "Match Style accepts images at a listener")
            heard = []
            editor.pasteAndMatchStyle(nil)
            XCTAssertEqual(heard, ["1:34:"], "one paste even when there is no insertable text")
            XCTAssertEqual(node.textArea?.text ?? node.field?.text, "hello")
            UIPasteboard.general.items = []
            UIPasteboard.general.urls = [URL(fileURLWithPath: "/tmp/pasted-file")]
            XCTAssertTrue(editor.canPerformAction(paste, withSender: nil), "file URL")
            XCTAssertTrue(editor.canPerformAction(match, withSender: nil), "file URL, Match Style")
            UIPasteboard.general.items = []
            UIPasteboard.general.string = "plain"
            heard = []
            let inserted = expectation(description: "the editor inserted the paste")
            p.onInput = { changed, _ in if changed == id { inserted.fulfill() } }
            editor.pasteAndMatchStyle(nil)
            wait(for: [inserted], timeout: 5)
            p.onInput = nil
            XCTAssertEqual(heard, ["1:34:plain"])
            XCTAssertTrue((node.textArea?.text ?? node.field?.text ?? "").contains("plain"))
            UIPasteboard.general.string = "BLOCK"
            heard = []
            let before = node.textArea?.text ?? node.field?.text
            editor.pasteAndMatchStyle(nil)
            XCTAssertEqual(heard, ["1:34:BLOCK"])
            XCTAssertEqual(node.textArea?.text ?? node.field?.text, before, "preventDefault cancels insertion")
        }
        p.views[1]?.handlers = []
        UIPasteboard.general.items = []
        UIPasteboard.general.image = image
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            let editor = try XCTUnwrap((node.textArea as UIResponder?) ?? node.field)
            XCTAssertFalse(editor.canPerformAction(paste, withSender: nil), "unheard image")
            XCTAssertFalse(editor.canPerformAction(match, withSender: nil), "unheard image, Match Style")
        }
        p.views[1]?.handlers = ["paste"]
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["editable": "false"]],
                           ["op": "props", "id": 3, "set": ["editable": "false"]]]))
        for id: UInt32 in [2, 3] {
            let node = try XCTUnwrap(p.views[id])
            let editor = try XCTUnwrap((node.textArea as UIResponder?) ?? node.field)
            XCTAssertFalse(editor.canPerformAction(paste, withSender: nil), "read-only")
            XCTAssertFalse(editor.canPerformAction(match, withSender: nil), "read-only, Match Style")
        }
    }
}
#endif
