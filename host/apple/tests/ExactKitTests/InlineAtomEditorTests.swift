// LLP 1045.000 stage 2: inline atoms in the Apple Markdown editor and
// reader, drawn as the attachment's picture (§10 Q9). The editor's rules
// (D5) through the text view's own commands; the press, the accessibility
// element and the appearance a pill draws for (D7, D8, §5.1); the reader's
// atom run. No claim about a physical keyboard, VoiceOver itself or a phone's
// gestures: the drive and the VoiceOver acceptance are separate.
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

private let kinds = #"[{"name":"citation","link":"t3-citation:"},{"name":"mention","prefix":"@","role":"link"}]"#
private let faces = #"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"The quoted passage"}]"#
private let cite = "[Quote](t3-citation:1)"
private let source = "Ask " + cite + " now"
/// The atom's start and end.
private let a = 4, b = 4 + (cite as NSString).length

#if os(macOS)
import AppKit

final class InlineAtomEditorMacTests: XCTestCase {
    private var presenter: Presenter!
    private var session: ExactSession?
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func editor(_ text: String = source, selection: NSRange = NSRange(location: 0, length: 0)) -> (NodeView, TextArea, NSWindow) {
        _ = NSApplication.shared
        presenter = Presenter()
        session = ExactApp.shared.makeSession(label: "inline-atoms")
        presenter.session = session
        let node = NodeView(id: 3, kind: "textarea", presenter: presenter)
        node.frame = NSRect(x: 0, y: 0, width: 500, height: 300)
        node.makeTextArea()
        node.props["markup"] = "markdown"
        node.props["value"] = text
        node.props["atoms"] = kinds
        node.props["atomFaces"] = faces
        node.handlers = ["input", "atompress"]
        node.applyTextArea()
        node.layoutTextArea()
        let window = NSWindow(contentRect: node.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = node
        let f = node.textArea as! TextArea
        window.makeFirstResponder(f)
        f.setSelectedRange(selection)
        node.restyleMarkup()
        f.atomsChanged()
        f.undoManager?.removeAllActions()
        return (node, f, window)
    }

    /// The pills TextKit laid out: each attachment in the paragraphs it asked for.
    private func attachments(_ f: TextArea) -> [AtomAttachment] {
        guard let manager = f.textLayoutManager else { return [] }
        manager.ensureLayout(for: manager.documentRange)
        f.layoutSubtreeIfNeeded()
        manager.textViewportLayoutController.layoutViewport()
        var out: [AtomAttachment] = []
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
            guard let paragraph = fragment.textElement as? NSTextParagraph else { return true }
            let text = paragraph.attributedString
            text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, _, _ in
                if let pill = value as? AtomAttachment { out.append(pill) }
            }
            return true
        }
        return out
    }

    func testAnAtomIsOnePillOverItsSourceWhichStays() throws {
        let (_, f, window) = editor()
        defer { window.close() }
        XCTAssertEqual(f.string, source)
        let pills = attachments(f)
        XCTAssertEqual(pills.map(\.atom.label), ["A quote"])
        XCTAssertFalse(pills[0].allowsTextAttachmentView, "the picture draws it, not a view (§10 Q9)")
        let x = { (p: Int) in f.firstRect(forCharacterRange: NSRange(location: p, length: 0), actualRange: nil).minX }
        XCTAssertGreaterThan(x(b), x(a), "the pill has its label's width")
        XCTAssertEqual(x(a + 3), x(b), accuracy: 0.5, "an offset inside draws at its end")
        XCTAssertEqual(f.accessibilityValue(), "Ask A quote now", "read as its label")
        XCTAssertEqual(f.accessibilityAttributedString(for: NSRange(location: 0, length: (source as NSString).length))?.string, "Ask A quote now", "a line read as VoiceOver reads it")
        XCTAssertEqual(f.accessibilityString(for: NSRange(location: 2, length: b - 2)), "k A quote")
        XCTAssertNil(f.typingAttributes[.markupAtom])
    }

    func testACaretInsideGoesToTheEndAndAnArrowCrossesInOneStep() {
        let (_, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.moveLeft(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: a, length: 0))
        f.doCommand(by: #selector(NSResponder.moveRight(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: b, length: 0))
        f.doCommand(by: #selector(NSResponder.moveLeftAndModifySelection(_:)))
        XCTAssertEqual(f.selectedRange(), NSRange(location: a, length: b - a), "⇧← takes it whole")
    }

    func testBackspaceAndDeleteRemoveTheWholeSourceAsOneUndoStep() throws {
        let (_, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.doCommand(by: #selector(NSResponder.deleteBackward(_:)))
        XCTAssertEqual(f.string, "Ask  now")
        XCTAssertEqual(f.selectedRange(), NSRange(location: a, length: 0))
        f.breakUndoCoalescing()
        try XCTUnwrap(f.undoManager).undo()
        XCTAssertEqual(f.string, source)
        f.setSelectedRange(NSRange(location: a, length: 0))
        f.doCommand(by: #selector(NSResponder.deleteForward(_:)))
        XCTAssertEqual(f.string, "Ask  now")
    }

    func testTypingBesideAnAtomLandsOutsideIt() {
        let (_, f, window) = editor(selection: NSRange(location: b, length: 0))
        defer { window.close() }
        f.insertText("!", replacementRange: f.selectedRange())
        XCTAssertEqual(f.string, "Ask " + cite + "! now")
        XCTAssertEqual(attachments(f).count, 1)
        // Typing over a range that ends inside it replaces it whole.
        f.setSelectedRange(NSRange(location: 2, length: 5))
        f.insertText("z", replacementRange: NSRange(location: 2, length: 5))
        XCTAssertEqual(f.string, "Asz! now")
    }

    func testAPressSelectsTheAtomAndAnAccessibilityElementNamesIt() throws {
        let (_, f, window) = editor()
        defer { window.close() }
        let hit = try XCTUnwrap(f.atomFrames().first)
        XCTAssertEqual(hit.range, NSRange(location: a, length: b - a))
        XCTAssertEqual(f.atom(at: CGPoint(x: hit.frame.midX, y: hit.frame.midY))?.range, hit.range)
        let element = try XCTUnwrap(f.accessibilityChildren()?.compactMap { $0 as? AtomElement }.first)
        XCTAssertEqual(element.accessibilityRole(), .button)
        XCTAssertEqual(element.accessibilityLabel(), "A quote")
        XCTAssertEqual(element.accessibilityHelp(), "The quoted passage")
        XCTAssertGreaterThan(element.accessibilityFrame().width, 0)
        XCTAssertTrue(element.accessibilityPerformPress())
        XCTAssertEqual(f.selectedRange(), hit.range, "a press selects it before its atompress")
        // Selected, it draws selected; the selection facts name it.
        XCTAssertTrue((f.textStorage?.attribute(.markupAtom, at: a, effectiveRange: nil) as? MarkupAtom)?.selected == true, "restyled selected")
        XCTAssertTrue(attachments(f).first?.atom.selected == true)
        let facts = try XCTUnwrap(MarkupCommands.selection(f.string, range: hit.range, atoms: AtomHandle.of(MarkdownAtoms(kinds: kinds, faces: faces))))
        XCTAssertTrue(facts.contains(#""atom":"[Quote](t3-citation:1)""#), facts)
    }

    func testAClickOnAPillSelectsTheAtom() throws {
        let (_, f, window) = editor()
        defer { window.close() }
        let hit = try XCTUnwrap(f.atomFrames().first)
        let p = f.convert(CGPoint(x: hit.frame.midX, y: hit.frame.midY), to: nil)
        // As the window routes a click: from its frame view, in window coordinates.
        let frameView = try XCTUnwrap(window.contentView?.superview)
        let hitView = frameView.hitTest(frameView.convert(p, from: nil))
        XCTAssertTrue(hitView is AtomCatcher, "a pill's click is the atom's: \(String(describing: hitView))")
        let down = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: p, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
        let up = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseUp, location: p, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0))
        NSApp.postEvent(up, atStart: true)
        hitView?.mouseDown(with: down)
        XCTAssertEqual(f.selectedRange(), hit.range)
        // Beside the pill, the text's own click.
        XCTAssertTrue(frameView.hitTest(frameView.convert(f.convert(CGPoint(x: hit.frame.maxX + 30, y: hit.frame.midY), to: nil), from: nil)) === f)
    }

    private func drawnAtom(_ f: TextArea) -> MarkupAtom? { f.textStorage?.attribute(.markupAtom, at: a, effectiveRange: nil) as? MarkupAtom }
    private func drain() { RunLoop.current.run(until: Date().addingTimeInterval(0.1)) }

    func testAPillIsDrawnAgainForAnotherAppearanceUnasked() throws {
        let (node, f, window) = editor()
        defer { window.close() }
        // The window's appearance changes; nothing calls a restyle.
        window.appearance = NSAppearance(named: .darkAqua)
        drain()
        XCTAssertTrue(node.drawsDark, "dark")
        XCTAssertTrue(drawnAtom(f)?.dark == true, "restyled dark")
        XCTAssertTrue(attachments(f).first?.atom.dark == true)
        window.appearance = NSAppearance(named: .aqua)
        drain()
        XCTAssertFalse(node.drawsDark, "light")
        XCTAssertTrue(drawnAtom(f)?.dark == false, "restyled light")
        XCTAssertTrue(attachments(f).first?.atom.dark == false)
    }

    func testASelectedPillIsEmphasizedOnlyInTheKeyWindowAndKeyChangesRestyle() throws {
        let (_, f, window) = editor(selection: NSRange(location: a, length: b - a))
        defer { window.close() }
        let before = try XCTUnwrap(drawnAtom(f))
        XCTAssertTrue(before.selected)
        XCTAssertEqual(before.emphasized, window.isKeyWindow, "emphasized as the window is key")
        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: window)
        drain()
        XCTAssertFalse(drawnAtom(f) === before, "a key change restyles the pills")
        let after = try XCTUnwrap(drawnAtom(f))
        NotificationCenter.default.post(name: NSColor.systemColorsDidChangeNotification, object: nil)
        drain()
        XCTAssertFalse(drawnAtom(f) === after, "an accent colour change restyles the pills")
    }

    func testTheTooltipIsAskedOfTheTextViewAndFollowsARewrap() throws {
        let (_, f, window) = editor("Ask about this long line of words before it " + cite + " now")
        defer { window.close() }
        let element = try XCTUnwrap(f.atomElements.first)
        let old = try XCTUnwrap(f.atomFrames().first).frame
        let tip = { (p: CGPoint) in f.view(f, stringForToolTip: 0, point: p, userData: nil) }
        XCTAssertEqual(tip(CGPoint(x: old.midX, y: old.midY)), "The quoted passage")
        XCTAssertEqual(tip(CGPoint(x: old.maxX + 40, y: old.midY)), "", "beside it, no tooltip")
        // A narrower width rewraps the line: the pill moves, the tooltip with it.
        f.setFrameSize(NSSize(width: 160, height: f.frame.height))
        let moved = try XCTUnwrap(f.atomFrames().first).frame
        XCTAssertNotEqual(moved.origin, old.origin, "rewrapped")
        XCTAssertEqual(tip(CGPoint(x: moved.midX, y: moved.midY)), "The quoted passage")
        XCTAssertTrue(f.atomElements.first === element, "its element is kept across a rewrap")
    }

    func testALineKeepsItsHeightWithAnAtom() throws {
        func lineHeight(_ text: String) throws -> CGFloat {
            let (_, f, window) = editor(text)
            defer { window.close() }
            let manager = try XCTUnwrap(f.textLayoutManager)
            manager.ensureLayout(for: manager.documentRange)
            var height: CGFloat = 0
            manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
                height = fragment.textLineFragments.first?.typographicBounds.height ?? 0
                return false
            }
            return height
        }
        let plain = try lineHeight("Ask Quote now"), atom = try lineHeight(source)
        XCTAssertGreaterThan(plain, 0)
        XCTAssertEqual(atom, plain, accuracy: 0.01, "the pill sits inside the font's ascender and descender (§5.1)")
    }

    func testMarkupTurnedOffClearsTheAtoms() throws {
        let (node, f, window) = editor()
        defer { window.close() }
        XCTAssertEqual(f.atomElements.count, 1)
        node.props["markup"] = nil
        node.applyTextArea()
        XCTAssertNil(f.markup)
        XCTAssertTrue(f.atomElements.isEmpty, "no element for an atom that is not drawn")
        XCTAssertEqual(f.view(f, stringForToolTip: 0, point: .zero, userData: nil), "")
    }

    func testCopyTakesTheSourceAndPlainCopyTheLabel() {
        let (node, f, window) = editor(selection: NSRange(location: a + 2, length: 2))
        defer { window.close() }
        XCTAssertEqual(f.markup?.copyRange(NSRange(location: a + 2, length: b - a), in: f.textStorage!), NSRange(location: a, length: b - a + 2))
        XCTAssertEqual(MarkdownLink.installed?.plain(source, range: nil, atoms: node.props["atoms"]!, faces: node.props["atomFaces"]!), "Ask A quote now")
        // ⌘⇧C of a selection of exactly a prefix atom: the token is found in
        // the whole source, where the space before it decides it.
        let mention = "see @x here"
        let faced = #"[{"source":"@x","label":"x.rs","icon":"","title":""}]"#
        XCTAssertEqual(MarkdownLink.installed?.plain(mention, range: NSRange(location: 4, length: 2), atoms: kinds, faces: faced), "x.rs")
        // The text view's command copies the selection's labels (the
        // person's clipboard is put back after).
        let board = NSPasteboard.general, kept = board.pasteboardItems?.map { item in
            item.types.reduce(into: [NSPasteboard.PasteboardType: Data]()) { $0[$1] = item.data(forType: $1) }
        } ?? []
        defer {
            board.clearContents()
            board.writeObjects(kept.map { types in
                let item = NSPasteboardItem()
                for (type, data) in types { item.setData(data, forType: type) }
                return item
            })
        }
        f.setSelectedRange(NSRange(location: a, length: b - a))
        f.copyPlainText(nil)
        XCTAssertEqual(board.string(forType: .string), "A quote")
    }
}
#else
import UIKit

final class InlineAtomEditorIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func field(_ text: String = source) -> (Presenter, NodeView, TextArea) {
        let made = ExactApp.shared.makeSession(label: "inline-atoms")
        session = made
        let p = made.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": text, "markup": "markdown", "atoms": kinds, "atomFaces": faces], "handlers": ["input", "atompress"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 600.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 560.0, "h": 120.0],
        ]))
        let node = p.views[2]!, f = node.textArea as! TextArea
        XCTAssertTrue(f.becomeFirstResponder())
        node.restyleMarkup()
        f.atomsChanged()
        return (p, node, f)
    }

    func testAnAtomIsOnePillAndItsElementSitsBesideTheTextView() throws {
        let (_, node, f) = field()
        XCTAssertEqual(f.text, source)
        let x = { (p: Int) in f.caretRect(for: f.position(from: f.beginningOfDocument, offset: p)!).minX }
        XCTAssertGreaterThan(x(b), x(a))
        XCTAssertEqual(x(a + 3), x(b), accuracy: 0.5)
        XCTAssertEqual(f.accessibilityValue, "Ask A quote now")
        let elements = try XCTUnwrap(node.accessibilityElements)
        XCTAssertTrue(elements.first as AnyObject === f, "the text view first, its atoms beside it")
        let atom = try XCTUnwrap(elements.compactMap { $0 as? AtomElement }.first)
        XCTAssertEqual(atom.accessibilityLabel, "A quote")
        XCTAssertTrue(atom.accessibilityTraits.contains(.button))
        XCTAssertTrue(atom.accessibilityActivate())
        XCTAssertEqual(f.selectedRange, NSRange(location: a, length: b - a))
    }

    func testATraitChangeRedrawsThePillsAndKeepsTheElements() throws {
        let (_, node, f) = field()
        let element = try XCTUnwrap(node.accessibilityElements?.compactMap { $0 as? AtomElement }.first)
        window.overrideUserInterfaceStyle = .dark
        window.layoutIfNeeded()
        RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue((f.textStorage.attribute(.markupAtom, at: a, effectiveRange: nil) as? MarkupAtom)?.dark == true, "restyled dark, unasked")
        XCTAssertTrue(node.accessibilityElements?.compactMap { $0 as? AtomElement }.first === element, "a restyle keeps the element VoiceOver may be on")
    }

    func testACaretInsideGoesToTheEndAndBackspaceRemovesItWhole() {
        let (_, _, f) = field()
        f.selectedRange = NSRange(location: b, length: 0)
        f.selectedRange = NSRange(location: a + 3, length: 0)
        XCTAssertEqual(f.selectedRange, NSRange(location: b, length: 0))
        f.deleteBackward()
        // UIKit's smart delete takes one of the two spaces left, as when a word goes.
        XCTAssertEqual(f.text, "Ask now")
        XCTAssertEqual(f.selectedRange, NSRange(location: a, length: 0))
        f.insertText("x")
        XCTAssertEqual(f.text, "Ask xnow")
    }
}
#endif

/// The reader: an atom is its label on a rounded inline box, and its run's
/// press is the text's `atompress`, never a link to follow.
final class InlineAtomReaderTests: XCTestCase {
    override func setUp() { super.setUp(); ExactMarkdown.install() }

    func testAnAtomPieceIsItsLabelOnABox() throws {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        let runs = MarkupRuns.expand(source, base: base, color: [0, 0, 0, 255], atoms: MarkdownAtoms(kinds: kinds, faces: faces))
        let atom = try XCTUnwrap(runs.first { $0.href.hasPrefix(MarkupRuns.atomScheme) })
        XCTAssertEqual(atom.text, "\u{202F}A\u{00A0}quote\u{202F}")
        XCTAssertNotNil(atom.background)
        XCTAssertNotEqual(atom.backgroundBox, 0)
        XCTAssertEqual(atom.backgroundRadii.count, 4)
        XCTAssertFalse(atom.decoration.contains("underline"))
        XCTAssertEqual(runs.map(\.text).joined(), "Ask \u{202F}A\u{00A0}quote\u{202F} now")
        // Without atoms the same source is its link text, as before.
        XCTAssertEqual(MarkupRuns.expand(source, base: base, color: nil).map(\.text).joined(), "Ask Quote now")
    }
}
