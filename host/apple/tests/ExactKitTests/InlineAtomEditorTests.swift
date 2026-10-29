// LLP 1045.000 stage 2: inline atoms in the Apple Markdown editor and
// reader, drawn as the attachment's picture (§10 Q9). The editor's rules
// (D5) through the text view's own commands; the press, the accessibility
// element and the appearance a pill draws for (D7, D8, §5.1); the reader's
// atom run. No claim about a physical keyboard, VoiceOver itself or a phone's
// gestures: the drive and the VoiceOver acceptance are separate.
import XCTest
import CoreText
@testable import ExactKit
@testable import ExactMarkdown
import CExact

private let kinds = #"[{"name":"citation","link":"t3-citation:"},{"name":"mention","prefix":"@","role":"link"}]"#
private let faces = #"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"The quoted passage"}]"#
private let cite = "[Quote](t3-citation:1)"
private let source = "Ask " + cite + " now"
/// The atom's start and end.
private let a = 4, b = 4 + (cite as NSString).length
/// A face in Display P3's own red and green, which sRGB has neither of.
private let wide = ##"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"","color":"color(display-p3 1 0 0)","backgroundColor":"color(display-p3 0 1 0)"}]"##

/// A face colour in Display P3 as a bound text colour's channels are
/// (`textChannels`, LLP 1100 D2): the sRGB four, then the space (1) and
/// `components`, which `TextEngine.color` draws in that space.
private func assertDisplayP3(_ channels: [Double]?, _ components: [Double], file: StaticString = #filePath, line: UInt = #line) {
    guard let c = channels else { return XCTFail("no colour", file: file, line: line) }
    XCTAssertEqual(c.count, 9, "\(c)", file: file, line: line)
    XCTAssertEqual(c.dropFirst(4).first, 1, "Display P3", file: file, line: line)
    XCTAssertEqual(Array(c.dropFirst(5)), components, file: file, line: line)
    XCTAssertEqual(TextEngine.color(c).cgColor.colorSpace?.name as String?, CGColorSpace.extendedDisplayP3 as String, file: file, line: line)
}

/// A pill's picture as `draw` paints it into a 40 by 20 Display P3 bitmap:
/// a point of its fill left of the label, halfway up, is Display P3's own
/// green, where its sRGB clip would read about (117, 251, 76).
private func assertDisplayP3Green(file: StaticString = #filePath, line: UInt = #line, _ draw: (CGContext, CGRect) -> Void) throws {
    let p3 = try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3))
    let context = try XCTUnwrap(CGContext(data: nil, width: 40, height: 20, bitsPerComponent: 8, bytesPerRow: 0, space: p3, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    draw(context, CGRect(x: 0, y: 0, width: 40, height: 20))
    let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
    let at = 10 * context.bytesPerRow + 2 * 4
    let got = (0..<4).map { Int(pixels[at + $0]) }
    for (channel, want) in zip(got, [0, 255, 0, 255]) { XCTAssertEqual(channel, want, accuracy: 2, "\(got)", file: file, line: line) }
}

/// A face whose pill draws only its icon, a black disc on white, with
/// `direction`: where the picture's ink is says which end the icon is at.
private func iconFace(_ direction: String) -> String {
    ##"[{"source":"[Quote](t3-citation:1)","label":" ","icon":"symbol:sf/circle.fill","title":"","color":"#000000","backgroundColor":"#ffffff","direction":"\##(direction)"}]"##
}

/// The mean x of the dark pixels `draw` paints into a 40 by 20 picture: the
/// icon's middle, where nothing else is dark.
private func inkMiddle(_ draw: (CGContext, CGRect) -> Void) throws -> Double {
    let srgb = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
    let context = try XCTUnwrap(CGContext(data: nil, width: 40, height: 20, bitsPerComponent: 8, bytesPerRow: 0, space: srgb, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    draw(context, CGRect(x: 0, y: 0, width: 40, height: 20))
    let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
    var xs: [Int] = []
    for y in 0..<20 { for x in 0..<40 where pixels[y * context.bytesPerRow + x * 4 + 3] > 200 && pixels[y * context.bytesPerRow + x * 4] < 100 { xs.append(x) } }
    XCTAssertFalse(xs.isEmpty, "an icon is drawn")
    return Double(xs.reduce(0, +)) / Double(max(xs.count, 1))
}

#if os(macOS)
import AppKit

final class InlineAtomEditorMacTests: XCTestCase {
    private var presenter: Presenter!
    private var session: ExactSession?
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func editor(_ text: String = source, selection: NSRange = NSRange(location: 0, length: 0), faces face: String = faces) -> (NodeView, TextArea, NSWindow) {
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
        node.props["atomFaces"] = face
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

    /// A face's paint is its pill's, over the platform's token, for the appearance it draws in (D2, D4).
    func testAFacesPaintIsItsPillsInEachAppearance() throws {
        let painted = ##"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"","color":"light-dark(#d52c36, #ff6762)","backgroundColor":"#1a85d4"}]"##
        let (_, f, window) = editor(faces: painted)
        defer { window.close() }
        window.appearance = NSAppearance(named: .aqua)
        drain()
        XCTAssertEqual(drawnAtom(f)?.ink, [213, 44, 54, 255])
        XCTAssertEqual(drawnAtom(f)?.background, [26, 133, 212, 255])
        XCTAssertEqual(attachments(f).first?.atom.ink, [213, 44, 54, 255], "the picture draws it")
        window.appearance = NSAppearance(named: .darkAqua)
        drain()
        XCTAssertEqual(drawnAtom(f)?.ink, [255, 103, 98, 255], "dark, its dark half")
        XCTAssertEqual(drawnAtom(f)?.background, [26, 133, 212, 255])
    }

    /// A face's colour in a wide space keeps it, as a bound text colour
    /// does, in the pill's picture too.
    func testAWideFaceColourKeepsItsSpaceInThePicture() throws {
        let (_, f, window) = editor(faces: wide)
        defer { window.close() }
        drain()
        let atom = try XCTUnwrap(drawnAtom(f))
        assertDisplayP3(atom.ink, [1, 0, 0, 1])
        assertDisplayP3(atom.background, [0, 1, 0, 1])
        let pill = try XCTUnwrap(attachments(f).first)
        let location = try XCTUnwrap(f.textLayoutManager?.documentRange.location)
        let bounds = CGRect(x: 0, y: 0, width: 40, height: 20)
        let image = try XCTUnwrap(pill.image(for: bounds, attributes: [.font: NSFont.systemFont(ofSize: 16)], location: location, textContainer: nil))
        try assertDisplayP3Green { context, rect in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
            image.draw(in: rect)
            NSGraphicsContext.restoreGraphicsState()
        }
    }

    /// D4: in a right-to-left line the pill lays out from the right, its
    /// icon first, as the line does; a face's `direction` pins its own.
    func testAPillFollowsItsLinesDirectionUnlessItsFacePinsOne() throws {
        for (direction, right) in [("", true), ("ltr", false), ("rtl", true)] {
            let (node, f, window) = editor(faces: iconFace(direction))
            defer { window.close() }
            node.props["paragraphDirs"] = "rtl"
            node.applyTextArea()
            drain()
            let manager = try XCTUnwrap(f.textLayoutManager)
            var drawn: (AtomAttachment, [NSAttributedString.Key: Any])?
            manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
                guard let paragraph = fragment.textElement as? NSTextParagraph else { return true }
                let text = paragraph.attributedString
                text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, at, _ in
                    if drawn == nil, let pill = value as? AtomAttachment { drawn = (pill, text.attributes(at: at.location, effectiveRange: nil)) }
                }
                return drawn == nil
            }
            let (pill, attributes) = try XCTUnwrap(drawn)
            XCTAssertEqual((attributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .rightToLeft, "the line is right to left")
            let image = try XCTUnwrap(pill.image(for: CGRect(x: 0, y: 0, width: 40, height: 20), attributes: attributes, location: manager.documentRange.location, textContainer: nil))
            let middle = try inkMiddle { context, rect in
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
                image.draw(in: rect)
                NSGraphicsContext.restoreGraphicsState()
            }
            XCTAssertEqual(middle > 20, right, "direction \(direction.isEmpty ? "unset" : direction): the icon's middle at \(middle)")
        }
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

    private func field(_ text: String = source, faces face: String = faces, dirs: String? = nil) -> (Presenter, NodeView, TextArea) {
        let made = ExactApp.shared.makeSession(label: "inline-atoms")
        session = made
        let p = made.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["id": "editor", "value": text, "markup": "markdown", "atoms": kinds, "atomFaces": face].merging(dirs.map { ["paragraphDirs": $0] } ?? [:]) { a, _ in a }, "handlers": ["input", "atompress"]],
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

    /// A face's paint is its pill's, over the platform's token, for the trait collection it draws in (D2, D4).
    func testAFacesPaintIsItsPillsInEachAppearance() throws {
        let painted = ##"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"","color":"light-dark(#d52c36ff, #ff6762ff)","backgroundColor":"#1a85d4ff"}]"##
        let (_, _, f) = field(faces: painted)
        let drawn = { f.textStorage.attribute(.markupAtom, at: a, effectiveRange: nil) as? MarkupAtom }
        let settle = { self.window.layoutIfNeeded(); RunLoop.current.run(until: Date().addingTimeInterval(0.1)) }
        window.overrideUserInterfaceStyle = .light
        settle()
        XCTAssertEqual(drawn()?.ink, [213, 44, 54, 255])
        XCTAssertEqual(drawn()?.background, [26, 133, 212, 255])
        window.overrideUserInterfaceStyle = .dark
        settle()
        XCTAssertEqual(drawn()?.ink, [255, 103, 98, 255], "dark, its dark half")
    }

    /// A face's colour in a wide space keeps it on the pill, as a bound
    /// text colour does (LLP 1100 D2), in the picture the renderer makes
    /// for the text view's traits.
    func testAWideFaceColourKeepsItsSpace() throws {
        let (_, _, f) = field(faces: wide)
        window.layoutIfNeeded()
        let atom = try XCTUnwrap(f.textStorage.attribute(.markupAtom, at: a, effectiveRange: nil) as? MarkupAtom)
        assertDisplayP3(atom.ink, [1, 0, 0, 1])
        assertDisplayP3(atom.background, [0, 1, 0, 1])
        let manager = try XCTUnwrap(f.textLayoutManager)
        manager.ensureLayout(for: manager.documentRange)
        var pill: AtomAttachment?
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
            guard let paragraph = fragment.textElement as? NSTextParagraph else { return true }
            let text = paragraph.attributedString
            text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, _, _ in
                pill = pill ?? value as? AtomAttachment
            }
            return pill == nil
        }
        // TextKit asks for the picture with the text view's traits current.
        let bounds = CGRect(x: 0, y: 0, width: 40, height: 20), location = manager.documentRange.location
        var picture: UIImage?
        f.traitCollection.performAsCurrent {
            picture = pill?.image(for: bounds, attributes: [.font: UIFont.systemFont(ofSize: 16)], location: location, textContainer: nil)
        }
        let image = try XCTUnwrap(picture)
        try assertDisplayP3Green { context, rect in
            UIGraphicsPushContext(context)
            image.draw(in: rect)
            UIGraphicsPopContext()
        }
    }

    /// D4: in a right-to-left line the pill lays out from the right, its
    /// icon first, as the line does; a face's `direction` pins its own.
    func testAPillFollowsItsLinesDirectionUnlessItsFacePinsOne() throws {
        for (direction, right) in [("", true), ("ltr", false), ("rtl", true)] {
            let (_, _, f) = field(faces: iconFace(direction), dirs: "rtl")
            window.layoutIfNeeded()
            let manager = try XCTUnwrap(f.textLayoutManager)
            manager.ensureLayout(for: manager.documentRange)
            var drawn: (AtomAttachment, [NSAttributedString.Key: Any])?
            manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
                guard let paragraph = fragment.textElement as? NSTextParagraph else { return true }
                let text = paragraph.attributedString
                text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, at, _ in
                    if drawn == nil, let pill = value as? AtomAttachment { drawn = (pill, text.attributes(at: at.location, effectiveRange: nil)) }
                }
                return drawn == nil
            }
            let (pill, attributes) = try XCTUnwrap(drawn)
            XCTAssertEqual((attributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection, .rightToLeft, "the line is right to left")
            var picture: UIImage?
            let location = manager.documentRange.location
            f.traitCollection.performAsCurrent {
                picture = pill.image(for: CGRect(x: 0, y: 0, width: 40, height: 20), attributes: attributes, location: location, textContainer: nil)
            }
            let image = try XCTUnwrap(picture)
            let middle = try inkMiddle { context, rect in
                UIGraphicsPushContext(context)
                image.draw(in: rect)
                UIGraphicsPopContext()
            }
            XCTAssertEqual(middle > 20, right, "direction \(direction.isEmpty ? "unset" : direction): the icon's middle at \(middle)")
            session?.destroy(); session = nil
        }
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

    /// A face's paint is drawn over its kind's, for the appearance drawn in;
    /// an empty one keeps the kind's (D2, D4).
    func testAFacesPaintWinsOverItsKinds() throws {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        // The kind's `color` and `background-color` rows, by the kernel's style bits (75 and 47).
        let styles = ##"{"citation":[[75,"#010203"],[47,"#040506"]]}"##
        let painted = #"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"","color":"light-dark(#d52c36, #ff6762)","backgroundColor":""}]"#
        let atom = { (dark: Bool) in
            MarkupRuns.expand(source, base: base, color: [0, 0, 0, 255], atoms: MarkdownAtoms(kinds: kinds, faces: painted, styles: styles, dark: dark))
                .first { $0.href.hasPrefix(MarkupRuns.atomScheme) }
        }
        let light = try XCTUnwrap(atom(false)), dark = try XCTUnwrap(atom(true))
        XCTAssertEqual(light.color, [213, 44, 54, 255])
        XCTAssertEqual(dark.color, [255, 103, 98, 255])
        XCTAssertEqual(light.background, [4, 5, 6, 255], "an empty face colour keeps the kind's")
    }

    /// D4: a pill's label follows its line in the reader, and one whose face
    /// pins a direction is isolated in it, as the web's box takes its `dir`.
    func testAReaderPillIsIsolatedInTheDirectionItsFacePins() throws {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        let label = { (direction: String) -> String? in
            let face = ##"[{"source":"[Quote](t3-citation:1)","label":"A quote","icon":"","title":"","color":"","backgroundColor":"","direction":"\##(direction)"}]"##
            return MarkupRuns.expand(source, base: base, color: nil, atoms: MarkdownAtoms(kinds: kinds, faces: face))
                .first { $0.href.hasPrefix(MarkupRuns.atomScheme) }?.text
        }
        XCTAssertEqual(label(""), "\u{202F}A\u{00A0}quote\u{202F}", "its line's direction")
        XCTAssertEqual(label("ltr"), "\u{202F}\u{2066}A\u{00A0}quote\u{2069}\u{202F}")
        XCTAssertEqual(label("rtl"), "\u{202F}\u{2067}A\u{00A0}quote\u{2069}\u{202F}")
        // Laid out in a right-to-left line: `notes…` in the line's direction
        // draws its ellipsis at the line's start, on the left; pinned left to
        // right, after its word, on the right.
        let ellipsisAfterWord = { (direction: String) -> Bool in
            let face = ##"[{"source":"[Quote](t3-citation:1)","label":"notes…","icon":"","title":"","color":"","backgroundColor":"","direction":"\##(direction)"}]"##
            let text = MarkupRuns.expand(source, base: base, color: nil, atoms: MarkdownAtoms(kinds: kinds, faces: face))
                .first { $0.href.hasPrefix(MarkupRuns.atomScheme) }?.text ?? ""
            let paragraph = NSMutableParagraphStyle()
            paragraph.baseWritingDirection = .rightToLeft
            let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.paragraphStyle: paragraph]))
            let utf16 = Array(text.utf16), at = { (c: Character) in CTLineGetOffsetForStringIndex(line, utf16.lastIndex(of: c.utf16.first!)!, nil) }
            return at("…") > at("n")
        }
        XCTAssertFalse(ellipsisAfterWord(""), "the line's direction")
        XCTAssertTrue(ellipsisAfterWord("ltr"), "pinned left to right")
    }

    /// A face's colour in a wide space keeps it on its run and box, as a
    /// bound text colour does (LLP 1100 D2).
    func testAWideFaceColourKeepsItsSpace() throws {
        let base = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
        let runs = MarkupRuns.expand(source, base: base, color: [0, 0, 0, 255], atoms: MarkdownAtoms(kinds: kinds, faces: wide))
        let atom = try XCTUnwrap(runs.first { $0.href.hasPrefix(MarkupRuns.atomScheme) })
        assertDisplayP3(atom.color, [1, 0, 0, 1])
        assertDisplayP3(atom.background, [0, 1, 0, 1])
    }
}

/// A Markdown reader's `rangeFrame` (D7): an atom's source range answers its
/// pill, where an app anchors a popover to it.
final class InlineAtomReaderRangeFrameTests: XCTestCase {
    private var session: ExactSession?
    #if os(macOS)
    private var window: NSWindow?
    #else
    private var window: UIWindow?
    #endif
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() {
        #if os(macOS)
        window?.close()
        #endif
        session?.destroy(); session = nil; window = nil; super.tearDown()
    }

    private func reader(_ text: String = source, width: Double = 400, style: [String: Any] = [:]) -> (Presenter, NodeView) {
        let made = ExactApp.shared.makeSession(label: "inline-atom-reader")
        session = made
        let p = made.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["id": "reader", "text": text, "markup": "markdown", "atoms": kinds, "atomFaces": faces], "style": style, "handlers": ["atompress"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 600.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": width, "h": 120.0],
        ]))
        #if os(macOS)
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        #else
        let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 300))
        p.viewport.frame = w.bounds
        w.addSubview(p.viewport)
        w.makeKeyAndVisible()
        #endif
        window = w
        return (p, p.views[2]!)
    }

    func testAnAtomsSourceRangeIsItsPill() throws {
        let (p, node) = reader()
        let pill = try XCTUnwrap(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)))
        XCTAssertGreaterThan(pill.width, 20)
        XCTAssertGreaterThan(pill.height, 10)
        // Its piece is the label on its box, in the piece's font: the frame is that piece's.
        let piece = try XCTUnwrap(node.paragraphSpec().runs.first { $0.href.hasPrefix(MarkupRuns.atomScheme) })
        let font = try XCTUnwrap(node.text?.font(piece))
        XCTAssertEqual(pill.width, (piece.text as NSString).size(withAttributes: [.font: font]).width, accuracy: 1)
        // Part of the range is the whole pill; text around it adds nothing.
        XCTAssertEqual(p.rangeFrame(2, start: UInt32(a + 2), end: UInt32(a + 3)), pill)
        XCTAssertEqual(p.rangeFrame(2, start: 0, end: UInt32((source as NSString).length)), pill)
        // Collapsed at its start or end: that edge, the pill's height.
        let start = try XCTUnwrap(p.rangeFrame(2, start: UInt32(a), end: UInt32(a)))
        let end = try XCTUnwrap(p.rangeFrame(2, start: UInt32(b), end: UInt32(b)))
        XCTAssertEqual(start, CGRect(x: pill.minX, y: pill.minY, width: 0, height: pill.height))
        XCTAssertEqual(end, CGRect(x: pill.maxX, y: pill.minY, width: 0, height: pill.height))
        // Where no pill is, unavailable.
        XCTAssertNil(p.rangeFrame(2, start: 0, end: 2))
        XCTAssertNil(p.rangeFrame(2, start: 1, end: 1))
        // `visibility: hidden` answers nothing, as the web's (LLP 1114.007).
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["visibility": "hidden"]]]))
        XCTAssertTrue(node.cssVisibilityHidden)
        XCTAssertNil(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)), "a visibility: hidden text answers nothing")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["visibility": "visible"]]]))
        XCTAssertNotNil(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)))
        node.isHidden = true
        XCTAssertNil(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)), "a hidden text answers nothing")
    }

    func testInARightToLeftParagraphAnAtomsStartIsItsRightEdge() throws {
        let (p, _) = reader("שלום " + cite + " עולם", style: ["direction": "rtl"])
        let s = 5, e = 5 + (cite as NSString).length
        let pill = try XCTUnwrap(p.rangeFrame(2, start: UInt32(s), end: UInt32(e)))
        XCTAssertEqual(try XCTUnwrap(p.rangeFrame(2, start: UInt32(s), end: UInt32(s))).minX, pill.maxX, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(p.rangeFrame(2, start: UInt32(e), end: UInt32(e))).minX, pill.minX, accuracy: 0.01)
    }

    #if os(macOS)
    func testAParagraphTheRegionReaderDrawsAnswersNothing() throws {
        let (p, node) = reader()
        XCTAssertNotNil(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)))
        // As the measurer hands a paragraph of 64 KB, or 16 KB in 256 runs, to the region reader.
        var request = ExactMeasureRequest()
        request.view = node.id; request.strut.font_size = 16
        session?.text.readerParagraphs[node.id] = RegionReaderParagraph(request, bytes: 100_000)
        XCTAssertNotNil(node.readerParagraph)
        XCTAssertNil(p.rangeFrame(2, start: UInt32(a), end: UInt32(b)), "laid out off the main thread: unavailable")
    }
    #endif

    func testAPillOnALaterLineAnswersThere() throws {
        let long = "Words that fill the first line of this narrow reader before " + cite + " ends"
        let (p, _) = reader(long, width: 200)
        let at = (long as NSString).range(of: cite)
        let pill = try XCTUnwrap(p.rangeFrame(2, start: UInt32(at.location), end: UInt32(NSMaxRange(at))))
        XCTAssertGreaterThan(pill.minY, 15, "below the first line")
        XCTAssertGreaterThan(pill.width, 20)
    }
}
