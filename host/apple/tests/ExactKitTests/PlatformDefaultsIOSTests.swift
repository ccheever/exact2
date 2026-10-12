#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1115 wave 1 on UIKit: what the author leaves unsaid is the
/// platform's. A colour a control only inherits is not its own; an unset
/// background is `systemBackground`; the keyboard's Return does what its
/// label says; the status bar scrolls one scroller. UIKit, so a simulator
/// runs it: bun host/apple/build.mjs --test --ios
final class PlatformDefaultsIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A test's window goes with it: a field left editing in one would
    /// still hold the keyboard under the next test's window.
    override func tearDown() {
        window?.endEditing(true)
        window?.isHidden = true
        window = nil
        kept = nil
        super.tearDown()
    }

    /// Each node has a prop, so it stays a view, not a flat leaf (LLP 1068 §6.1).
    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }

    /// §2 and LLP 1115.000 D2: a colour is a node's own when its author
    /// wrote it there, which the style says row by row (`written`). So a
    /// platform colour and one equal to the parent's are the node's own; a
    /// colour it only inherits, or one the compiler implied (a sheet's, a
    /// link's), is not, whatever its value.
    func testOwnColourIsWhatItsAuthorWrote() throws {
        let red: [String: Any] = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        let page = [17, 17, 17, 255], wrote = ["text_color"]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "n1"], "style": ["text_color": page, "written": wrote]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "n2"], "style": ["text_color": page]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "n3"], "style": ["text_color": [181, 86, 43, 255], "written": wrote]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "n4"], "style": ["text_color": red, "written": wrote]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "n5"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["id": "n6"], "style": ["text_color": page, "written": wrote]],
            ["op": "create", "id": 7, "kind": "view", "props": ["id": "n7"], "style": ["text_color": [0, 122, 255, 255], "background_color": page, "written": ["background_color"]]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7]],
            ["op": "roots", "ids": [1]],
        ])
        func own(_ id: UInt32) throws -> BatchValue? { try XCTUnwrap(p.views[id]).ownColor("text_color") }
        XCTAssertNotNil(try own(1), "the page's colour is the page's own")
        XCTAssertNil(try own(2), "inherited: the value, and no word that it was written")
        XCTAssertEqual(try own(3)?.numbers, [181, 86, 43, 255])
        XCTAssertNotNil(try XCTUnwrap(p.views[3]).ownUIColor("text_color"))
        XCTAssertEqual(try own(4)?.isSystemColor, true, "a platform colour its author wrote is its own")
        var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        try XCTUnwrap(try XCTUnwrap(p.views[4]).ownUIColor("text_color")).getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertTrue(r > 0.9 && g < 0.4 && b < 0.4 && a == 1, "UIKit's red, as it draws now: \([r, g, b, a])")
        XCTAssertNil(try own(5), "no row")
        XCTAssertEqual(try own(6)?.numbers, [17, 17, 17, 255], "written, though equal to its parent's")
        XCTAssertNil(try own(7), "a colour the compiler implied differs from the parent's and is not the author's")
        XCTAssertNotNil(try XCTUnwrap(p.views[7]).ownColor("background_color"), "beside a row that is")
        // A restyle replaces the word with the rows.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["text_color": [181, 86, 43, 255]]]]))
        XCTAssertNil(try own(3), "the same colour, no longer written")
    }

    /// D2: a page with no background shows the platform's, not white.
    func testAnUnsetBackgroundIsTheSystemBackground() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view"], ["op": "roots", "ids": [1]]])
        let dark = try XCTUnwrap(p.viewport.backgroundColor).resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark))
        XCTAssertEqual(dark, UIColor.systemBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": [255, 0, 0, 255]]]]))
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        try XCTUnwrap(p.viewport.backgroundColor).getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "an authored one wins")
    }

    /// Three single-line fields in a window, each hearing `change` and
    /// `blur`, named `f1` to `f3`. A node holds its presenter weakly, so the
    /// test keeps it.
    private var kept: Presenter?
    private func threeFields() throws -> (Presenter, [NodeView], [UITextField]) {
        let p = Presenter()
        kept = p
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let fields = (1...3).map { i -> NodeView in
            let node = NodeView(id: UInt32(i), kind: "input", presenter: p)
            node.handlers = ["change", "blur"]
            node.frame = CGRect(x: 0, y: CGFloat(i) * 50, width: 200, height: 40)
            p.root.addSubview(node); p.views[node.id] = node
            node.applyProps(set: ["id": "f\(i)"], clear: [])
            return node
        }
        window.makeKeyAndVisible()
        return (p, fields, try fields.map { try XCTUnwrap($0.field) })
    }

    /// LLP 1115.000 D9.2: Return puts the keyboard away from a single-line
    /// field, as a hand-built field's delegate resigns it, whether the key
    /// is plain or says Done. Next moves to the next field, and from the
    /// last field puts the keyboard away.
    func testReturnPutsTheKeyboardAway() throws {
        let (_, fields, editors) = try threeFields()
        fields[0].applyProps(set: ["enterKeyHint": "next"], clear: [])
        fields[1].applyProps(set: ["enterKeyHint": "next"], clear: [])
        fields[2].applyProps(set: ["enterKeyHint": "done"], clear: [])
        XCTAssertTrue(editors[0].becomeFirstResponder())
        XCTAssertFalse(fields[0].textFieldShouldReturn(editors[0]))
        XCTAssertTrue(editors[1].isFirstResponder, "Next moves to the next field")
        XCTAssertFalse(fields[1].textFieldShouldReturn(editors[1]))
        XCTAssertTrue(editors[2].isFirstResponder)
        XCTAssertFalse(fields[2].textFieldShouldReturn(editors[2]))
        XCTAssertFalse(editors[2].isFirstResponder, "Done puts the keyboard away")
        fields[2].applyProps(set: [:], clear: ["enterKeyHint"])
        XCTAssertTrue(editors[2].becomeFirstResponder())
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertFalse(editors[2].isFirstResponder, "a plain Return puts the keyboard away, as a hand-built field does")
        fields[2].applyProps(set: ["enterKeyHint": "next"], clear: [])
        XCTAssertTrue(editors[2].becomeFirstResponder())
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertFalse(editors[2].isFirstResponder, "Next from the last field puts the keyboard away")
    }

    /// LLP 1115.000 D9.3: the Return key's label is the system's unless
    /// `enterkeyhint` names another. A `submit` handler does not make it Go,
    /// which was Safari's rule for a form.
    func testReturnsLabelIsTheSystemsUnlessTheAuthorNamesOne() throws {
        let (_, fields, editors) = try threeFields()
        fields[0].handlers = ["change", "submit"]
        fields[0].applyProps(set: ["placeholder": "Add"], clear: [])
        XCTAssertEqual(editors[0].returnKeyType, .default, "a field that submits keeps the system's Return")
        fields[0].applyProps(set: ["enterKeyHint": "go"], clear: [])
        XCTAssertEqual(editors[0].returnKeyType, .go, "an authored Go is Go")
        fields[0].applyProps(set: ["enterKeyHint": "send"], clear: [])
        XCTAssertEqual(editors[0].returnKeyType, .send)
        fields[0].applyProps(set: [:], clear: ["enterKeyHint"])
        XCTAssertEqual(editors[0].returnKeyType, .default, "the word taken away: the system's again")
        // The system's label for a search field is Search, as UISearchBar's
        // own field has it; `enter` asks for the plain key there too.
        fields[1].applyProps(set: ["type": "search"], clear: [])
        XCTAssertEqual(editors[1].returnKeyType, .search)
        fields[1].applyProps(set: ["enterKeyHint": "enter"], clear: [])
        XCTAssertEqual(editors[1].returnKeyType, .default)
    }

    /// D9.2: a field's `submit` runs before the keyboard goes, and the field
    /// keeps the keyboard only when that action focused it again: an "add
    /// another" field stays up and never loses the focus on the way (no
    /// `blur`, so no flicker). An action that focuses another field moves the
    /// keyboard there; one that focuses nothing lets it go.
    func testASubmitThatFocusesItsFieldAgainKeepsTheKeyboard() throws {
        let (p, fields, editors) = try threeFields()
        var blurred: [UInt32] = [], submitted = 0, editingAtSubmit = false
        p.onBlur = { blurred.append($0) }
        fields[2].handlers = ["change", "blur", "submit"]
        // The action adds its row and says nothing of the focus.
        p.onSubmit = { _ in submitted += 1; editingAtSubmit = editors[2].isFirstResponder }
        XCTAssertTrue(editors[2].becomeFirstResponder())
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertEqual(submitted, 1)
        XCTAssertTrue(editingAtSubmit, "the action runs first, the field still editing")
        XCTAssertFalse(editors[2].isFirstResponder, "then the keyboard goes")
        XCTAssertEqual(blurred, [3])
        // "Add another": the action focuses the field again, as `focus("f3")` does.
        blurred = []
        p.onSubmit = { _ in p.focusElement(["f3"]) }
        XCTAssertTrue(editors[2].becomeFirstResponder())
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertTrue(editors[2].isFirstResponder, "the action asked for the keyboard back")
        XCTAssertEqual(blurred, [], "and the field never let go of it")
        // Labelled Done, the author's focus still wins.
        fields[2].applyProps(set: ["enterKeyHint": "done"], clear: [])
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertTrue(editors[2].isFirstResponder, "Done, with an action that focuses the field again")
        fields[2].applyProps(set: [:], clear: ["enterKeyHint"])
        // The action sends the focus elsewhere.
        p.onSubmit = { _ in p.focusElement(["f1"]) }
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertTrue(editors[0].isFirstResponder, "the keyboard goes where the action sent it")
        XCTAssertEqual(blurred, [3])
        // A focus an earlier action asked for does not hold a later Return.
        p.onSubmit = nil
        XCTAssertTrue(editors[2].becomeFirstResponder())
        p.focusElement(["f3"])
        _ = fields[2].textFieldShouldReturn(editors[2])
        XCTAssertFalse(editors[2].isFirstResponder)
    }

    private final class PointTouch: UITouch {
        let point: CGPoint, target: UIView
        init(_ point: CGPoint, on target: UIView) { self.point = point; self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { view.map { $0.convert(point, from: nil) } ?? point }
    }

    /// LLP 1115.000 D9.4: a tap that lands on nothing ends the editing and
    /// the keyboard goes. UIKit does not do this by itself; hand-built iOS
    /// apps add a background tap for it, which is why it stays.
    func testATapOnBlankGroundEndsTheEditing() throws {
        let (p, _, editors) = try threeFields()
        XCTAssertTrue(editors[0].becomeFirstResponder())
        let touch = PointTouch(CGPoint(x: 300, y: 320), on: p.viewport)
        p.viewport.touchesBegan([touch], with: nil)
        p.viewport.touchesEnded([touch], with: nil)
        XCTAssertFalse(editors[0].isFirstResponder, "a tap on blank ground puts the keyboard away")
    }

    /// LLP 1115.000 D9.1: with nothing written, a drag on a scroller that
    /// scrolls vertically takes the keyboard down with it once it reaches
    /// the keys, as a hand-built list or form has it; a sideways strip
    /// leaves the keyboard alone. What the author wrote wins, `none`
    /// included, and taking the word away gives the platform's answer back.
    func testAVerticalScrollersDragTakesTheKeyboardUnlessItsAuthorSaysOtherwise() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "n2"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "n3"], "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["keyboardDismissMode": "none"], "style": ["overflow_y": "auto"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["keyboardDismissMode": "on-drag"], "style": ["overflow_y": "scroll"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "roots", "ids": [1]],
        ])
        func mode(_ id: UInt32) throws -> UIScrollView.KeyboardDismissMode { try XCTUnwrap(p.views[id]?.scroll).keyboardDismissMode }
        XCTAssertEqual(try mode(2), .interactive, "unsaid, on a vertical scroller: the keyboard follows the drag")
        XCTAssertEqual(try mode(3), UIScrollView.KeyboardDismissMode.none, "a sideways strip")
        XCTAssertEqual(try mode(4), UIScrollView.KeyboardDismissMode.none, "an authored none wins")
        XCTAssertEqual(try mode(5), .onDrag, "an authored on-drag wins")
        XCTAssertEqual(NavigationHost.ownedChanges(try XCTUnwrap(p.views[2]?.scroll), of: try XCTUnwrap(p.views[2]), insetByUIKit: false), [],
                       "the unsaid mode is what Exact wrote, so the ownership check does not report it")
        p.apply(wireBatch([["op": "props", "id": 5, "set": [:], "clear": ["keyboardDismissMode"]],
                           ["op": "props", "id": 2, "set": ["keyboardDismissMode": "none"], "clear": []]]))
        XCTAssertEqual(try mode(5), .interactive, "the word taken away: the platform's again")
        XCTAssertEqual(try mode(2), UIScrollView.KeyboardDismissMode.none)
        // The axis it scrolls is part of the unsaid answer.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["overflow_x": "scroll", "overflow_y": "scroll"]]]))
        XCTAssertEqual(try mode(3), .interactive)
    }

    /// LLP 1115.000 D11.6: a button's `aria-keyshortcuts` is a key command
    /// that carries the button's name, so holding ⌘ on an iPad lists it. A
    /// command with no title is not listed, and Exact's had none.
    func testAShortcutsKeyCommandCarriesItsButtonsName() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["accessibilityKeyShortcuts": "Meta+n", "accessibilityLabel": "New Note"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["accessibilityKeyShortcuts": "Meta+Shift+f Escape"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 100.0, "h": 44.0],
        ])
        let commands = p.shortcutCommands(#selector(UIResponder.resignFirstResponder))
        let named = try XCTUnwrap(commands.first { $0.input == "n" && $0.modifierFlags == .command })
        XCTAssertEqual(named.title, "New Note", "the command's title is its button's name")
        XCTAssertEqual(named.discoverabilityTitle, "New Note", "which is what the ⌘ overlay lists")
        XCTAssertTrue(named.wantsPriorityOverSystemBehavior)
        // A button with no name has nothing to be listed by.
        let bare = try XCTUnwrap(commands.first { $0.input == "f" })
        XCTAssertEqual(bare.title, "")
        XCTAssertEqual(commands.count, 3)
    }

    /// LLP 1115.000 D12.5: the bar's title is a heading by its role. An
    /// `aria-level` with no `role="heading"` gets no heading style from the
    /// compiler (which warns, `lower-level-without-heading`), and the bar
    /// read it as the title anyway: a large title over body text. It is no
    /// title now, so the header is not the bar, as on every other host.
    func testALevelWithNoHeadingRoleIsNotTheBarsTitle() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationKey": "home"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "header"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Inbox", "accessibilityHeadingLevel": "1"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["id": "tap"]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Inbox", "accessibilityHeadingLevel": "2"]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "roots", "ids": [1]],
        ])
        let route = try XCTUnwrap(p.views[1]), text = try XCTUnwrap(p.views[3]), pressable = try XCTUnwrap(p.views[4])
        XCTAssertFalse(text.isHeading)
        XCTAssertNil(HeaderShape(route: route, back: nil), "a level alone: the header holds no heading, so it is not the bar")
        XCTAssertFalse(HeaderTitle.holdsHeading(pressable), "nor is a pressable around one the title's group")
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["accessibilityRole": "heading"], "clear": []],
                           ["op": "props", "id": 5, "set": ["accessibilityRole": "heading"], "clear": []]]))
        let shape = try XCTUnwrap(HeaderShape(route: route, back: nil), "with its role it is the bar's title")
        XCTAssertEqual(shape.title, "Inbox")
        XCTAssertEqual(shape.level, 1, "level 1: the large title")
        XCTAssertTrue(HeaderTitle.holdsHeading(pressable))
        // The role alone is ARIA's level 2, which lowering also says.
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["accessibilityHeadingLevel"]]]))
        XCTAssertEqual(HeaderShape(route: route, back: nil)?.level, 2)
    }

    /// Only one scroll view answers the status bar, or UIKit scrolls none:
    /// with no routes and a page that does not scroll, the first vertical
    /// scroller; a horizontal one and the viewport are off.
    func testOneScrollerAnswersTheStatusBar() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "n2"], "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "n3"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["id": "n4"], "style": ["overflow_y": "scroll"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 400.0, "h": 350.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 3, "w": 400.0, "h": 2000.0],
        ])
        let page = try XCTUnwrap(p.views[3]?.scroll)
        XCTAssertTrue(page.scrollsToTop, "the page's content scroller")
        XCTAssertFalse(try XCTUnwrap(p.views[2]?.scroll).scrollsToTop, "a horizontal strip")
        XCTAssertFalse(try XCTUnwrap(p.views[4]?.scroll).scrollsToTop, "a nested scroller")
        XCTAssertFalse(p.viewport.scrollsToTop, "the viewport, which does not scroll")
    }

    /// LLP 1116 D2: iOS has no square checkbox. A plain checkbox is the
    /// checklist mark, as Reminders and Notes draw it: an empty circle at the
    /// body text's size that fills with a checkmark in the accent colour.
    /// VoiceOver reads it as UIKit reads a toggle (LLP 1115.000 D13.3): the
    /// `toggleButton` trait and UIKit's own value, a `UISwitch`'s "1" and
    /// "0", which it speaks in the reader's language; it had read Safari's
    /// English "checked" and "unchecked". A box whose ARIA role is
    /// `checkbox` or `switch` reads the same, and `tree --ax` says checkbox
    /// and its state for both. A radio is its own symbols at the same size
    /// (D13.2, RadioIOSTests).
    func testACheckboxIsTheChecklistMark() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "n1"]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "checkbox", "checked": "false", "accessibilityLabel": "Milk", "testId": "milk"],
             "handlers": ["input", "change"], "style": ["accent_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "radio", "name": "r", "value": "a", "checked": "false"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"],
             "props": ["testId": "drawn", "accessibilityRole": "checkbox", "accessibilityChecked": "true", "accessibilityLabel": "Eggs"]],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["press"],
             "props": ["testId": "wifi", "accessibilityRole": "switch", "accessibilityChecked": "false", "accessibilityLabel": "Wi-Fi"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 40.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 60.0, "y": 10.0, "w": 16.0, "h": 16.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 60.0, "w": 100.0, "h": 44.0],
            ["op": "frame", "id": 5, "x": 10.0, "y": 110.0, "w": 100.0, "h": 44.0],
        ])
        let box = try XCTUnwrap(p.controls.controls[2] as? ExactCheckbox)
        XCTAssertFalse(box is ExactRadio)
        let size = box.intrinsicContentSize
        XCTAssertGreaterThan(size.width, 18, "the body text's mark, not Safari's 16-point box")
        XCTAssertEqual(size.width, size.height, accuracy: 1.5, "a circle's square")
        XCTAssertEqual(try XCTUnwrap(p.controls.controls[3]).intrinsicContentSize, size, "a radio is the same size")
        XCTAssertTrue(box.accessibilityTraits.contains(.button))
        // UIKit's own toggle is the oracle: what a `UISwitch` reports, "1"
        // and "0" (read from one in a running app, `tree --ax`; a test
        // process without the accessibility runtime derives no value).
        let uikit = UISwitch()
        XCTAssertTrue(box.accessibilityTraits.contains(.toggleButton), "a toggle to VoiceOver")
        XCTAssertEqual(box.accessibilityValue, "0", "UIKit's value for off, which VoiceOver speaks in the reader's language")
        if let value = uikit.accessibilityValue { XCTAssertEqual(box.accessibilityValue, value, "as a UISwitch's") }
        p.syncAccessibility()
        let eggs = try XCTUnwrap(p.views[4]), wifi = try XCTUnwrap(p.views[5])
        XCTAssertTrue(eggs.accessibilityTraits.contains(.toggleButton))
        XCTAssertEqual(eggs.accessibilityValue, "1", "a box with role=checkbox, checked")
        XCTAssertTrue(wifi.accessibilityTraits.contains(.toggleButton))
        XCTAssertEqual(wifi.accessibilityValue, "0", "a box with role=switch, off")
        // `tree --ax`: the role is what the author wrote, the state UIKit's value.
        window.layoutIfNeeded()
        func seen(_ testId: String) throws -> [String: Any] {
            let all = p.axElements(roots: [p.viewport])["elements"] as? [[String: Any]] ?? []
            return try XCTUnwrap(all.first { $0["testId"] as? String == testId || ($0["native"] as? [String: Any])?["identifier"] as? String == testId }, "\(testId) in \(all)")
        }
        func state(_ e: [String: Any]) -> [String] { [e["role"] as? String ?? "", e["value"] as? String ?? "", "\((e["states"] as? [String: Any])?["checked"] ?? "none")"] }
        XCTAssertEqual(state(try seen("milk")), ["checkbox", "0", "false"])
        XCTAssertEqual(state(try seen("drawn")), ["checkbox", "1", "true"])
        XCTAssertEqual(state(try seen("wifi")), ["switch", "0", "false"])
        // The mark as drawn: (alpha, red, green, blue) at a point given as a
        // fraction of its box.
        func drawn() -> (CGFloat, CGFloat) -> [Int] {
            let format = UIGraphicsImageRendererFormat(); format.scale = 1
            let image = UIGraphicsImageRenderer(bounds: box.bounds, format: format).image { _ in box.draw(box.bounds) }
            let cg = try! XCTUnwrap(image.cgImage)
            let (w, h) = (cg.width, cg.height)
            var data = [UInt8](repeating: 0, count: w * h * 4)
            let ctx = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
            return { fx, fy in
                let i = (min(h - 1, Int(fy * CGFloat(h))) * w + min(w - 1, Int(fx * CGFloat(w)))) * 4
                return [Int(data[i + 3]), Int(data[i]), Int(data[i + 1]), Int(data[i + 2])]
            }
        }
        let off = drawn()
        XCTAssertEqual(off(0.5, 0.5)[0], 0, "off: an empty circle")
        XCTAssertTrue(stride(from: 0.0, to: 0.3, by: 0.02).contains { off(0.5, $0)[0] > 20 }, "its ring (the tertiary label colour, translucent), above the middle")
        XCTAssertEqual(off(0.12, 0.12)[0], 0, "a circle: nothing in the corner a rounded square fills")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["checked": "true"]]]))
        XCTAssertTrue(box.isOn)
        uikit.isOn = true
        XCTAssertEqual(box.accessibilityValue, "1", "on")
        if let value = uikit.accessibilityValue { XCTAssertEqual(box.accessibilityValue, value, "as a UISwitch's") }
        XCTAssertEqual(state(try seen("milk")), ["checkbox", "1", "true"])
        let on = drawn()
        let fill = on(0.5, 0.22)
        XCTAssertGreaterThan(fill[0], 200, "on: filled, \(fill)")
        XCTAssertGreaterThan(fill[1], 200, "in the accent: \(fill)")
        XCTAssertLessThan(fill[2], 60, "in the accent: \(fill)")
        XCTAssertEqual(on(0.12, 0.12)[0], 0, "still a circle")
    }

    func testARIAUnavailableCheckboxKeepsItsTraitAfterRefusedChange() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "checkbox", "checked": "false", "accessibilityDisabled": "true"],
             "handlers": ["change"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
        ])
        let box = try XCTUnwrap(p.controls.controls[2] as? ExactCheckbox)
        XCTAssertTrue(box.accessibilityTraits.contains(.notEnabled))
        box.isOn = true
        p.syncAccessibility()
        box.isOn = false // The action left `checked` false, so the host restores it.
        XCTAssertTrue(box.isEnabled, "ARIA state leaves input enabled")
        XCTAssertFalse(box.isOn)
        XCTAssertTrue(box.accessibilityTraits.contains(.notEnabled))
    }
}
#endif
