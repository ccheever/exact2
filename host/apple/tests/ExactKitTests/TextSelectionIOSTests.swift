#if os(iOS)
import CoreText
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1115.000 D11.6: `user-select="text"` on UIKit. Text its author made
/// selectable takes UIKit's own text interaction when a finger first lands
/// on it and answers it from the paragraph's own lines; text nobody made
/// selectable takes nothing. UIKit synthesizes no touch for a unit test, so
/// a gesture's result is asked for as the driver asks (`selectWord`).
/// Real long presses, double taps and handles need simulator coordinate
/// touches; the agent's contextmenu and dblclick deliver recognized events.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class TextSelectionIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func mounted(_ ops: [[String: Any]]) -> Presenter {
        let session = ExactApp.shared.makeSession(label: "text-selection")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        p.viewport.layoutIfNeeded()
        return p
    }
    private func text(_ id: Int, _ value: String, style: [String: Any] = [:], handlers: [String] = []) -> [String: Any] {
        ["op": "create", "id": id, "kind": "text", "props": ["text": value, "testId": "t\(id)"],
         "style": ["font_size": 17.0, "line_height": "22px"].merging(style) { _, new in new }, "handlers": handlers]
    }
    private func box(_ id: Int, style: [String: Any] = [:], props: [String: Any] = [:], handlers: [String] = [], kind: String = "view") -> [String: Any] {
        ["op": "create", "id": id, "kind": kind, "style": style, "props": props, "handlers": handlers]
    }
    private func frame(_ id: Int, _ y: Double, h: Double = 22, x: Double = 0, w: Double = 360) -> [String: Any] {
        ["op": "frame", "id": id, "x": x, "y": y, "w": w, "h": h]
    }
    private func node(_ p: Presenter, _ id: UInt32) throws -> NodeView { try XCTUnwrap(p.views[id], "node \(id)") }
    /// A point inside `word` of a one-line paragraph, in the node's space.
    private func point(in node: NodeView, of word: String) throws -> CGPoint {
        let shaped = SelectableText.shaped(node)
        let range = shaped.range(of: word)
        XCTAssertNotEqual(range.location, NSNotFound, word)
        let held = try XCTUnwrap(node.presenter?.textSelection.install(node))
        let rect = held.firstRect(for: try XCTUnwrap(held.textRange(from: try XCTUnwrap(held.position(from: held.beginningOfDocument, offset: range.location)),
                                                                   to: try XCTUnwrap(held.position(from: held.beginningOfDocument, offset: NSMaxRange(range))))))
        return CGPoint(x: rect.midX, y: rect.midY)
    }

    /// A page: `prose` holds two paragraphs its author made selectable, a
    /// pressable row holds one it made selectable and one it did not, and a
    /// label, a button's label and an opted-out paragraph stand beside them.
    private func page() -> Presenter {
        mounted([
            box(1),
            text(2, "A label nobody made selectable."),
            box(3, style: ["user_select": "text"]),
            text(4, "The quick brown fox jumps over the lazy dog.", handlers: ["selectionchange"]),
            text(5, "Opted out inside the prose.", style: ["user_select": "none"]),
            box(6, handlers: ["press", "contextmenu"]),
            text(7, "Selectable words in a pressable row.", style: ["user_select": "text"]),
            text(8, "The row's own label."),
            box(9, handlers: ["press"], kind: "button"), text(10, "A button's label"),
            ["op": "children", "id": 1, "ids": [2, 3, 6]], ["op": "children", "id": 3, "ids": [4, 5, 9]],
            ["op": "children", "id": 6, "ids": [7, 8]], ["op": "children", "id": 9, "ids": [10]],
            ["op": "roots", "ids": [1]],
            frame(1, 0, h: 1600, w: 400), frame(2, 80, x: 20), frame(3, 120, h: 110, x: 20), frame(4, 0), frame(5, 30), frame(9, 60, h: 40), frame(10, 9),
            frame(6, 250, h: 60, x: 20), frame(7, 4), frame(8, 32),
        ])
    }

    func testOnlyTextItsAuthorMadeSelectableIs() throws {
        let p = page()
        XCTAssertFalse(try node(p, 2).textSelectable, "nothing said: a UILabel's text selects nothing")
        XCTAssertTrue(try node(p, 4).textSelectable, "the nearest authored user-select is text")
        XCTAssertFalse(try node(p, 5).textSelectable, "the nearest authored user-select is none")
        XCTAssertTrue(try node(p, 7).textSelectable, "said on the text itself, inside a pressable row")
        XCTAssertFalse(try node(p, 8).textSelectable, "the row's own label")
        XCTAssertFalse(try node(p, 10).textSelectable, "a control's label, though the prose above says text")
        // A finger on text that is not selectable adds nothing to it.
        for id in [2, 5, 8, 10] as [UInt32] {
            let unsaid = try node(p, id)
            p.textSelection.touched(unsaid)
            XCTAssertTrue(unsaid.interactions.isEmpty, "node \(id)")
            XCTAssertTrue((unsaid.gestureRecognizers ?? []).isEmpty, "node \(id)")
            XCTAssertNil(p.textSelection.selectWord(in: unsaid, at: CGPoint(x: 30, y: 10)), "node \(id)")
        }
        XCTAssertTrue(p.textSelection.texts.isEmpty)
    }

    func testAWordIsSelectedFromTheParagraphsOwnLinesAndCopied() throws {
        let p = page()
        let prose = try node(p, 4)
        var heard: [String] = []
        p.onSelectionChange = { id, text, start, end, _ in heard.append("\(id) \(text) \(start) \(end)") }
        XCTAssertTrue(prose.interactions.isEmpty, "nothing until a finger lands")
        // A finger lands: UIKit's interaction, in its read-only mode, on the paragraph's own view.
        p.textSelection.touched(prose)
        let held = try XCTUnwrap(p.textSelection.text(for: prose))
        XCTAssertTrue(prose.interactions.contains { $0 === held.interaction })
        XCTAssertEqual(held.interaction.textInteractionMode, .nonEditable)
        XCTAssertTrue(held.textInputView === prose)
        XCTAssertFalse(held.isEditable, "no keyboard, and a long press selects a word")
        XCTAssertFalse((held as AnyObject) is UIView, "the text input is no view")
        XCTAssertFalse(prose.canPerformAction(#selector(UIResponderStandardEditActions.copy(_:)), withSender: nil), "nothing selected, nothing to copy")
        // Its single taps are off, so the node's own press hears a tap.
        let taps = (prose.gestureRecognizers ?? []).compactMap { $0 as? UITapGestureRecognizer }.filter { $0.numberOfTapsRequired == 1 }
        XCTAssertFalse(taps.isEmpty)
        XCTAssertTrue(taps.allSatisfy { !$0.isEnabled })
        XCTAssertTrue((prose.gestureRecognizers ?? []).allSatisfy { !$0.delaysTouchesEnded }, "the node's touch lifts when the finger does")

        let found = try XCTUnwrap(p.textSelection.selectWord(in: prose, at: try point(in: prose, of: "brown")))
        XCTAssertEqual(found.text, "brown")
        XCTAssertEqual(found.range, NSRange(location: 10, length: 5))
        XCTAssertEqual(heard, ["4 brown 10 15"], "`selectionchange`, in the node's own text")
        XCTAssertTrue(held.isFirstResponder)
        XCTAssertTrue(taps.allSatisfy(\.isEnabled), "with a selection up a tap in the text clears it")
        // Where UIKit draws it is where the paragraph's line has the word.
        let range = try XCTUnwrap(held.selectedTextRange)
        let rects = held.selectionRects(for: range)
        XCTAssertEqual(rects.count, 1)
        let paragraph = try XCTUnwrap(prose.paragraphLayout()), boxed = prose.paragraphBox()
        let x0 = boxed.minX + paragraph.origin(0, align: 0, width: boxed.width) + CTLineGetOffsetForStringIndex(paragraph.lines[0], 10, nil)
        let x1 = boxed.minX + paragraph.origin(0, align: 0, width: boxed.width) + CTLineGetOffsetForStringIndex(paragraph.lines[0], 15, nil)
        XCTAssertEqual(rects[0].rect.minX, x0, accuracy: 0.01)
        XCTAssertEqual(rects[0].rect.maxX, x1, accuracy: 0.01)
        XCTAssertTrue(prose.bounds.insetBy(dx: -1, dy: -4).contains(rects[0].rect), "\(rects[0].rect) in \(prose.bounds)")
        XCTAssertTrue(rects[0].containsStart && rects[0].containsEnd)
        XCTAssertEqual(rects[0].writingDirection, .leftToRight)
        XCTAssertEqual(held.text(in: range), "brown")
        XCTAssertEqual(held.caretRect(for: range.start).midX, x0, accuracy: 0.01)

        // The edit menu asks the node for Copy; Select All is not offered, as a text view's menu has none.
        XCTAssertTrue(prose.canPerformAction(#selector(UIResponderStandardEditActions.copy(_:)), withSender: nil))
        XCTAssertFalse(prose.canPerformAction(#selector(UIResponderStandardEditActions.selectAll(_:)), withSender: nil))
        XCTAssertFalse(prose.canPerformAction(#selector(UIResponderStandardEditActions.cut(_:)), withSender: nil))
        XCTAssertFalse(prose.canPerformAction(#selector(UIResponderStandardEditActions.paste(_:)), withSender: nil))
        // (A unit test's process waits forever on the system pasteboard's contents, so Copy
        // writes to one of the test's own here; `simctl pbpaste` read the system's after a real Copy.)
        let pasteboard = UIPasteboard.withUniqueName()
        defer { UIPasteboard.remove(withName: pasteboard.name) }
        p.textSelection.pasteboard = pasteboard
        pasteboard.string = "something else"
        prose.copy(nil)
        XCTAssertEqual(pasteboard.string, "brown")

        // UIKit's tap in the text clears it (it sets no range).
        held.selectedTextRange = nil
        XCTAssertNil(p.textSelection.selection)
        XCTAssertEqual(heard.last, "4  0 0")
        XCTAssertTrue(taps.allSatisfy { !$0.isEnabled })
        XCTAssertFalse(prose.canPerformAction(#selector(UIResponderStandardEditActions.copy(_:)), withSender: nil))
    }

    func testTheSelectionSurvivesAScrollAndIsDroppedWhenItsTextChanges() throws {
        let p = page()
        let prose = try node(p, 4)
        _ = try XCTUnwrap(p.textSelection.selectWord(in: prose, at: try point(in: prose, of: "quick")))
        let held = try XCTUnwrap(p.textSelection.text(for: prose))
        let before = try XCTUnwrap(held.selectionRects(for: try XCTUnwrap(held.selectedTextRange)).first?.rect)
        let onScreen = prose.convert(before, to: nil)
        // The page scrolls: the selection is the paragraph's, and moves with it.
        p.viewport.contentSize = CGSize(width: 400, height: 1600)
        p.viewport.contentOffset = CGPoint(x: 0, y: 60)
        p.viewport.layoutIfNeeded()
        XCTAssertEqual(p.textSelection.selection?.text, "quick")
        let after = try XCTUnwrap(held.selectionRects(for: try XCTUnwrap(held.selectedTextRange)).first?.rect)
        XCTAssertEqual(after, before)
        XCTAssertEqual(prose.convert(after, to: nil).minY, onScreen.minY - 60, accuracy: 0.01)
        // A restyle that leaves the words alone leaves the selection.
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["font_size": 17.0, "line_height": "22px", "text_color": [200, 0, 0, 255]]]]))
        XCTAssertEqual(p.textSelection.selection?.text, "quick")
        // Other text changing leaves it too.
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": "Another label."]]]))
        XCTAssertEqual(p.textSelection.selection?.text, "quick")
        // Its own text changing drops it.
        var heard: [String] = []
        p.onSelectionChange = { id, text, start, end, _ in heard.append("\(id) \(text) \(start) \(end)") }
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["text": "Other words stand here now."]]]))
        XCTAssertNil(p.textSelection.selection)
        XCTAssertNil(held.selectedTextRange)
        XCTAssertEqual(heard, ["4  0 0"])
        // And the new words select.
        XCTAssertEqual(p.textSelection.selectWord(in: prose, at: try point(in: prose, of: "stand"))?.text, "stand")
        // Text that stops being selectable gives its interaction back.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["user_select": "none"]]]))
        XCTAssertNil(p.textSelection.selection)
        XCTAssertTrue(p.textSelection.texts.isEmpty)
        XCTAssertTrue(prose.interactions.isEmpty)
        XCTAssertTrue((prose.gestureRecognizers ?? []).isEmpty)
    }

    func testOneSelectionAndItGoesWithItsNode() throws {
        let p = page()
        let prose = try node(p, 4), rowText = try node(p, 7)
        _ = try XCTUnwrap(p.textSelection.selectWord(in: prose, at: try point(in: prose, of: "lazy")))
        XCTAssertEqual(p.textSelection.selectWord(in: rowText, at: try point(in: rowText, of: "pressable"))?.text, "pressable")
        XCTAssertNil(p.textSelection.text(for: prose)?.selectedTextRange, "one selection on the page")
        XCTAssertTrue(p.textSelection.selection?.node === rowText)
        p.apply(wireBatch([["op": "destroy", "id": 7], ["op": "children", "id": 6, "ids": [8]]]))
        XCTAssertNil(p.textSelection.selection)
        XCTAssertNil(p.textSelection.text(for: rowText))
    }

    /// A touch whose hit view the test names (UIKit makes none for a test).
    private final class Touch: UITouch {
        let hit: UIView
        init(_ hit: UIView) { self.hit = hit; super.init() }
        override var view: UIView? { hit }
        override var window: UIWindow? { hit.window }
        override func location(in view: UIView?) -> CGPoint { hit.convert(CGPoint(x: hit.bounds.midX, y: hit.bounds.midY), to: view) }
    }

    private final class Hold: UILongPressGestureRecognizer {
        var phase = UIGestureRecognizer.State.possible
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    }

    func testARowsContextMenuWinsALongPressButDoubleTapStillSelectsText() throws {
        let p = page()
        let row = try node(p, 6), selectable = try node(p, 7), label = try node(p, 8)
        let longPress = try XCTUnwrap(row.contextRecognizer, "the row's `contextmenu` is a long press")
        XCTAssertTrue(row.gestureRecognizer(longPress, shouldReceive: Touch(selectable)))
        XCTAssertTrue(row.gestureRecognizer(longPress, shouldReceive: Touch(label)))
        XCTAssertTrue(row.gestureRecognizer(longPress, shouldReceive: Touch(row)))
        let at = selectable.convert(try point(in: selectable, of: "words"), to: nil)
        let held = try XCTUnwrap(p.textSelection.text(for: selectable))
        XCTAssertFalse(held.interaction.gesturesForFailureRequirements.isEmpty)
        for gesture in held.interaction.gesturesForFailureRequirements {
            XCTAssertTrue(row.gestureRecognizer(longPress, shouldBeRequiredToFailBy: gesture), "the row is tried before selection")
        }
        XCTAssertNil(p.agentSelect(selectable, at: at, event: "contextmenu"))
        XCTAssertNil(p.textSelection.selection)
        var menus: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { menus.append(id) } }
        let hold = Hold()
        for phase in [UIGestureRecognizer.State.began, .changed, .ended] { hold.phase = phase; row.openContext(hold) }
        XCTAssertEqual(menus, [6], "one action for the recognized long press")
        XCTAssertNil(p.textSelection.selection)
        XCTAssertEqual(p.agentSelect(selectable, at: at, event: "dblclick")?["text"] as? String, "words")
        p.textSelection.clear()
        let prose = try node(p, 4)
        let word = prose.convert(try point(in: prose, of: "brown"), to: nil)
        XCTAssertEqual(p.agentSelect(prose, at: word, event: "contextmenu")?["text"] as? String, "brown", "without a row menu a long press selects")
        XCTAssertFalse(row.gestureRecognizer(longPress, shouldReceive: Touch(UITextView())))
        XCTAssertFalse(row.gestureRecognizer(longPress, shouldReceive: Touch(UITextField())))
    }

    func testMenuPrecedenceIgnoresDisabledContainersButHonorsInertAncestry() throws {
        let p = page(), row = try node(p, 6), paragraph = try node(p, 7)
        let at = paragraph.convert(try point(in: paragraph, of: "words"), to: nil)
        let held = try XCTUnwrap(p.textSelection.text(for: paragraph))
        let selectionGesture = try XCTUnwrap(held.interaction.gesturesForFailureRequirements.first)
        let rowPress = try XCTUnwrap(row.contextRecognizer)
        paragraph.handlers = ["contextmenu"]; paragraph.updateContextGestures()
        let ownPress = try XCTUnwrap(paragraph.contextRecognizer)
        XCTAssertTrue(paragraph.contextMenuSource === paragraph)
        XCTAssertFalse(row.gestureRecognizer(rowPress, shouldReceive: Touch(paragraph)))
        XCTAssertTrue(paragraph.gestureRecognizer(ownPress, shouldBeRequiredToFailBy: selectionGesture))
        XCTAssertFalse(row.gestureRecognizer(rowPress, shouldBeRequiredToFailBy: selectionGesture))
        paragraph.handlers = []; paragraph.updateContextGestures()
        XCTAssertTrue(paragraph.contextMenuSource === row)
        XCTAssertTrue(row.gestureRecognizer(rowPress, shouldBeRequiredToFailBy: selectionGesture))
        row.handlers = ["press"]; row.updateContextGestures()
        XCTAssertNil(paragraph.contextMenuSource)
        XCTAssertEqual(p.agentSelect(paragraph, at: at, event: "contextmenu")?["text"] as? String, "words")
        p.textSelection.clear()
        row.handlers.insert("contextmenu"); row.updateContextGestures()
        let replaced = try XCTUnwrap(row.contextRecognizer)
        XCTAssertTrue(p.textSelection.text(for: paragraph) === held, "the same text interaction follows the new menu")
        XCTAssertTrue(row.gestureRecognizer(replaced, shouldBeRequiredToFailBy: selectionGesture))
        for id in [6, 1] as [UInt32] {
            let ancestor = try node(p, id)
            ancestor.props["disabled"] = "true"
            XCTAssertTrue(paragraph.contextMenuSource === row)
            XCTAssertTrue(row.gestureRecognizer(replaced, shouldReceive: Touch(paragraph)))
            XCTAssertTrue(row.gestureRecognizer(replaced, shouldBeRequiredToFailBy: selectionGesture))
            ancestor.props["disabled"] = nil
            ancestor.props["inert"] = "true"
            XCTAssertNil(paragraph.contextMenuSource)
            XCTAssertNil(p.agentSelect(paragraph, at: at, event: "contextmenu"))
            XCTAssertFalse(row.gestureRecognizer(replaced, shouldReceive: Touch(paragraph)))
            ancestor.props["inert"] = nil
        }
    }

    func testARowsNativeContextPopoverAdmitsSelectableTextAndFiresOnce() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = page(), row = try node(p, 6), paragraph = try node(p, 7)
        p.apply(wireBatch([
            box(20, props: ["id": "row-menu", "popover": "auto"]),
            box(21, props: ["accessibilityLabel": "Pin"], handlers: ["press"], kind: "button"),
            text(22, "Pin"), ["op": "children", "id": 21, "ids": [22]],
            ["op": "children", "id": 20, "ids": [21]], ["op": "children", "id": 1, "ids": [2, 3, 6, 20]],
            ["op": "props", "id": 6, "set": ["contextPopover": "row-menu"]], frame(20, 0, h: 44, w: 220), frame(21, 0, h: 44, w: 220),
        ]))
        let host = p.menus.context
        let interaction = try XCTUnwrap(row.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let local = try point(in: paragraph, of: "words"), at = paragraph.convert(local, to: row)
        XCTAssertNil(p.agentSelect(paragraph, at: paragraph.convert(local, to: nil), event: "contextmenu"))
        var menus: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { menus.append(id) } }
        let configuration = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: at))
        let owner = try XCTUnwrap(host.open)
        XCTAssertEqual(try XCTUnwrap(host.menu(owner)).children.first?.title, "Pin")
        _ = host.menu(owner); _ = host.lift(owner)
        XCTAssertEqual(menus, [6])
        XCTAssertNil(p.textSelection.selection)
        host.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        let readOnly = UITextView(); readOnly.isEditable = false
        for editor in [UITextField(), UITextView(), readOnly] as [UIView] {
            editor.frame = row.bounds; row.container.addSubview(editor)
            XCTAssertTrue(row.hitsNativeEditor(row.hitTest(at, with: nil)))
            XCTAssertNil(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: at), "a native editor keeps its long press")
            editor.removeFromSuperview()
        }
        paragraph.handlers = ["contextmenu"]; paragraph.updateContextGestures()
        XCTAssertNil(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: at), "the paragraph's own menu stays nearest")
        paragraph.handlers = []; paragraph.updateContextGestures()
        row.handlers = ["press"]; row.updateContextGestures(); p.menus.sync()
        XCTAssertTrue(paragraph.contextMenuSource === row, "a popover needs no contextmenu action")
        XCTAssertNil(p.agentSelect(paragraph, at: paragraph.convert(local, to: nil), event: "contextmenu"))
        let popoverOnly = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: at))
        _ = host.menu(try XCTUnwrap(host.open))
        XCTAssertEqual(menus, [6], "opening the popover alone invents no action")
        host.contextMenuInteraction(interaction, willEndFor: popoverOnly, animator: nil)
    }

    func testRightToLeftTextAndLargerTypeSelectTheirOwnLines() throws {
        let arabic = "هذا نص عربي قصير للتجربة with English words داخل السطر نفسه حتى نرى التحديد."
        let long = "Dynamic Type makes every line taller and breaks this sentence over more of them than before."
        let p = mounted([
            box(1, style: ["user_select": "text"]),
            text(2, arabic, style: ["direction": "rtl"]),
            text(3, long),
            text(4, long, style: ["font_size": 34.0, "line_height": "44px"]),
            ["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]],
            frame(1, 0, h: 800, w: 400), frame(2, 80, h: 44, x: 20), frame(3, 140, h: 66, x: 20), frame(4, 220, h: 400, x: 20),
        ])
        // Right to left: a word of the Arabic starts at its right edge, an English one at its left.
        let rtl = try node(p, 2)
        XCTAssertEqual(p.textSelection.selectWord(in: rtl, at: try point(in: rtl, of: "عربي"))?.text, "عربي")
        let held = try XCTUnwrap(p.textSelection.text(for: rtl))
        var rect = try XCTUnwrap(held.selectionRects(for: try XCTUnwrap(held.selectedTextRange)).first)
        XCTAssertEqual(rect.writingDirection, .rightToLeft)
        XCTAssertGreaterThan(rect.rect.midX, rtl.bounds.midX, "the paragraph starts at the right")
        XCTAssertEqual(p.textSelection.selectWord(in: rtl, at: try point(in: rtl, of: "English"))?.text, "English")
        rect = try XCTUnwrap(held.selectionRects(for: try XCTUnwrap(held.selectedTextRange)).first)
        XCTAssertEqual(rect.writingDirection, .leftToRight)
        XCTAssertEqual(held.baseWritingDirection(for: held.beginningOfDocument, in: .forward), .rightToLeft)

        // Larger type: all of the paragraph selected is one rect a line, on that line, at that line's height.
        for (id, size) in [(3, 17.0), (4, 34.0)] as [(UInt32, Double)] {
            let node = try node(p, id)
            let input = p.textSelection.install(node)
            input.selectedTextRange = input.textRange(from: input.beginningOfDocument, to: input.endOfDocument)
            XCTAssertEqual(p.textSelection.selection?.text, long, "font-size \(size)")
            let rects = input.selectionRects(for: try XCTUnwrap(input.selectedTextRange)).map(\.rect)
            let paragraph = try XCTUnwrap(node.paragraphLayout())
            XCTAssertEqual(rects.count, paragraph.lines.count, "font-size \(size)")
            XCTAssertGreaterThan(rects.count, size > 20 ? 3 : 1, "font-size \(size)")
            for (i, rect) in rects.enumerated() {
                XCTAssertEqual(rect.maxY - rect.height / 2, paragraph.baselines[i] - CGFloat(size) * 0.3, accuracy: CGFloat(size) * 0.5, "line \(i) at \(size)")
                XCTAssertGreaterThan(rect.height, CGFloat(size), "line \(i) at \(size)")
                if i > 0 { XCTAssertGreaterThan(rect.minY, rects[i - 1].minY) }
            }
            // A point on the last line is a caret on the last line.
            let last = rects[rects.count - 1]
            let caret = try XCTUnwrap(input.closestPosition(to: CGPoint(x: last.midX, y: last.midY)))
            XCTAssertEqual(input.caretRect(for: caret).midY, last.midY, accuracy: 1, "font-size \(size)")
        }
    }

    func testVoiceOverStillReadsSelectableTextAsText() throws {
        let p = page()
        let prose = try node(p, 4), plain = try node(p, 2)
        let label = prose.accessibilityLabel, traits = prose.accessibilityTraits
        XCTAssertTrue(prose.isAccessibilityElement)
        XCTAssertTrue(traits.contains(.staticText))
        XCTAssertEqual(label, "The quick brown fox jumps over the lazy dog.")
        _ = try XCTUnwrap(p.textSelection.selectWord(in: prose, at: try point(in: prose, of: "fox")))
        XCTAssertTrue(prose.isAccessibilityElement)
        XCTAssertEqual(prose.accessibilityLabel, label)
        XCTAssertEqual(prose.accessibilityTraits, traits)
        XCTAssertNil(prose.accessibilityElements, "the paragraph is the element; the text input adds none")
        XCTAssertEqual(plain.accessibilityTraits, traits, "selectable or not, the same text to VoiceOver")
    }

    /// What it costs (LLP 1115.000 D11.6's as-built row): a page of 400
    /// paragraphs holds no interaction, recognizer or view for selection,
    /// whether or not its author made it selectable, until a finger lands;
    /// then the paragraphs touched hold UIKit's, at most `kept` of them.
    func testAPageOfFourHundredParagraphsPaysOnlyForWhatAFingerTouches() throws {
        func build(selectable: Bool) -> Presenter {
            var ops: [[String: Any]] = [box(1), box(2, style: selectable ? ["user_select": "text"] : [:])]
            for i in 0..<400 { ops.append(text(100 + i, "Paragraph \(i) of the page, which is a line of ordinary words.")) }
            ops += [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": (0..<400).map { 100 + $0 }], ["op": "roots", "ids": [1]],
                    frame(1, 0, h: 9000, w: 400), frame(2, 0, h: 9000, x: 20)]
            for i in 0..<400 { ops.append(frame(100 + i, Double(i) * 22)) }
            return mounted(ops)
        }
        func count(_ p: Presenter) -> (views: Int, interactions: Int, recognizers: Int) {
            let nodes = (0..<400).compactMap { p.views[UInt32(100 + $0)] }
            var views = 0
            func walk(_ view: UIView) { views += 1; view.subviews.forEach(walk) }
            walk(p.root)
            return (views, nodes.reduce(0) { $0 + $1.interactions.count }, nodes.reduce(0) { $0 + ($1.gestureRecognizers?.count ?? 0) })
        }
        // Nothing said.
        var p = build(selectable: false)
        let unsaid = count(p)
        let clock = ContinuousClock()
        var spent = clock.measure { for _ in 0..<5 { for i in 0..<400 { p.views[UInt32(100 + i)]?.touchedText() } } }
        let untouched = count(p)
        XCTAssertEqual(unsaid.interactions, 0); XCTAssertEqual(unsaid.recognizers, 0)
        XCTAssertEqual(untouched.views, unsaid.views); XCTAssertEqual(untouched.interactions, 0); XCTAssertEqual(untouched.recognizers, 0)
        print("text selection cost, 400 paragraphs nobody made selectable: \(unsaid.views) views, \(unsaid.interactions) interactions, \(unsaid.recognizers) recognizers; after a touch on each: \(untouched.views) views, \(untouched.interactions) interactions, \(untouched.recognizers) recognizers; the hit test's question \(spent / 2000) a touch")
        session?.destroy(); window?.isHidden = true

        // Made selectable.
        p = build(selectable: true)
        let said = count(p)
        XCTAssertEqual(said.views, unsaid.views, "saying it adds no view")
        XCTAssertEqual(said.interactions, 0); XCTAssertEqual(said.recognizers, 0)
        spent = clock.measure { p.views[100]?.touchedText() }
        let one = count(p)
        XCTAssertEqual(one.views, said.views, "UIKit's interaction is on the paragraph's own view")
        XCTAssertGreaterThan(one.interactions, 0); XCTAssertGreaterThan(one.recognizers, 0)
        for i in 1..<400 { p.views[UInt32(100 + i)]?.touchedText() }
        let all = count(p)
        XCTAssertEqual(p.textSelection.texts.count, TextSelectionHost.kept)
        XCTAssertEqual(all.interactions, one.interactions * TextSelectionHost.kept)
        XCTAssertEqual(all.recognizers, one.recognizers * TextSelectionHost.kept)
        XCTAssertEqual(all.views, said.views)
        print("text selection cost, 400 paragraphs made selectable: \(said.views) views, \(said.interactions) interactions, \(said.recognizers) recognizers; after one touch: \(one.interactions) interactions, \(one.recognizers) recognizers (\(spent) to add them); after a touch on each of the 400: \(all.interactions) interactions, \(all.recognizers) recognizers on \(p.textSelection.texts.count) paragraphs, \(all.views) views")
        // A selection is not taken away to make room.
        let chosen = try node(p, 399)
        _ = try XCTUnwrap(p.textSelection.selectWord(in: chosen, at: CGPoint(x: 30, y: 11)))
        for i in 0..<20 { p.views[UInt32(100 + i)]?.touchedText() }
        XCTAssertNotNil(p.textSelection.text(for: chosen)?.selectedTextRange)
        XCTAssertEqual(p.textSelection.texts.count, TextSelectionHost.kept)
    }
}
#endif
