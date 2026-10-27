#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A hardware keyboard reaches and operates controls, as on macOS: Tab and
/// Shift-Tab walk the sequential order — inputs and pressables in tree
/// order, never plain text — through a field and around; a pressable Tab
/// reached shows its ring, one a touch focused does not. UIKit, so a
/// simulator runs it (the command in `FieldCompositionIOSTests`).
final class KeyboardFocusIOSTests: XCTestCase {
    private var window: UIWindow!

    /// The journal's focus holder is found in one pass over the views,
    /// counted by the responder queries it makes rather than timed.
    func testTheFocusHolderIsFoundInOnePassOverTheViews() {
        final class Counted: UIView {
            static var asked = 0
            var held = false
            override var isFirstResponder: Bool { Counted.asked += 1; return held }
        }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        var parent: UIView = window, views = 0
        for _ in 0..<8 {
            let child = Counted()
            parent.addSubview(Counted())
            parent.addSubview(child)
            parent = child
            views += 2
        }
        let held = Counted()
        held.held = true
        parent.addSubview(held)
        Counted.asked = 0
        XCTAssertEqual(Presenter.focusHolder(in: window), "Counted")
        XCTAssertEqual(Counted.asked, views + 1, "each view asked once")
    }

    func testPassiveAncestorHearsFocusMoveBetweenChildren() {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let wrapper = NodeView(id: 10, kind: "view", presenter: p)
        let a = NodeView(id: 11, kind: "button", presenter: p)
        let b = NodeView(id: 12, kind: "button", presenter: p)
        wrapper.frame = window.bounds
        a.frame = CGRect(x: 0, y: 0, width: 100, height: 40)
        b.frame = CGRect(x: 0, y: 50, width: 100, height: 40)
        wrapper.handlers = ["focusin", "focusout"]
        a.handlers = ["press"]; b.handlers = ["press"]
        a.applyProps(set: ["id": "a"], clear: [])
        b.applyProps(set: ["id": "b"], clear: [])
        p.root.addSubview(wrapper); p.views[10] = wrapper
        wrapper.addSubview(a); wrapper.addSubview(b)
        p.views[11] = a; p.views[12] = b
        window.makeKeyAndVisible()
        var heard: [String] = []
        p.onFocusEvent = { at, kind, target, related in heard.append("\(at):\(kind):\(target):\(related ?? "-")") }
        XCTAssertFalse(wrapper.canBecomeFirstResponder)
        XCTAssertTrue(a.becomeFirstResponder())
        tab(a)
        XCTAssertTrue(b.isFirstResponder)
        XCTAssertEqual(heard, ["10:focusin:a:-", "10:focusout:a:b", "10:focusin:b:a"])
        heard.removeAll()
        _ = p.release(12) { $0.removeFromSuperview() }
        RunLoop.main.run(until: Date().addingTimeInterval(0.02))
        XCTAssertEqual(heard, ["10:focusout:b:-"])
    }
    private func focusFields(_ kind: String = "input") -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "handlers": ["focusin", "focusout"]],
            ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 400, "h": 400],
            ["op": "create", "id": 11, "kind": kind, "props": ["id": "a", "value": "hello", "editable": kind == "textarea" ? "false" : "true"], "handlers": ["focus", "blur", "input"]],
            ["op": "frame", "id": 11, "x": 0, "y": 0, "w": 200, "h": 80],
            ["op": "create", "id": 12, "kind": "input", "props": ["id": "b"]],
            ["op": "frame", "id": 12, "x": 0, "y": 100, "w": 200, "h": 40],
            ["op": "children", "id": 10, "ids": [11, 12]], ["op": "roots", "ids": [10]]
        ]))
        window.makeKeyAndVisible()
        return p
    }

    func testPendingNodeFocusKeepsTheResolvedFieldWithoutAUniqueHTMLId() throws {
        for name: String? in [nil, "", "a"] {
            let p = focusFields()
            defer { window.endEditing(true); window.isHidden = true }
            let node = try XCTUnwrap(p.views[12]), field = try XCTUnwrap(node.field)
            node.applyProps(set: name.map { ["id": $0] } ?? [:], clear: ["id"])
            // A sheet's field leaves the window while its controller changes.
            p.viewport.removeFromSuperview()
            p.focusNode(node)
            XCTAssertTrue(p.pendingFocusNode === node, "an unnamed field must wait for its window too")
            XCTAssertEqual(p.pendingFocusObservation?["target"], name ?? "#12")
            XCTAssertFalse(field.isFirstResponder)
            window.addSubview(p.viewport)
            XCTAssertTrue(field.isFirstResponder)
            XCTAssertFalse(try XCTUnwrap(p.views[11]?.field).isFirstResponder, "a duplicate id must not redirect focus")
            XCTAssertNil(p.pendingFocusNode)
        }
    }

    func testReadOnlySelectionSendsFocusAndBlur() throws {
        let p = focusFields("textarea")
        defer { window.endEditing(true); window.isHidden = true }
        let area = try XCTUnwrap(p.views[11]?.textArea)
        var heard: [String] = []
        p.onFocus = { heard.append("focus:\($0)") }
        p.onBlur = { heard.append("blur:\($0)") }
        p.onFocusEvent = { _, kind, target, related in heard.append("\(kind):\(target):\(related ?? "-")") }
        XCTAssertFalse(area.isEditable)
        XCTAssertTrue(area.becomeFirstResponder())
        area.selectedRange = NSRange(location: 0, length: 3)
        XCTAssertTrue(try XCTUnwrap(p.views[12]?.field).becomeFirstResponder())
        XCTAssertEqual(heard, ["focus:11", "focusin:a:-", "blur:11", "focusout:a:b", "focusin:b:a"])
    }


    func testBlurRemovesWrapperBeforeFocusout() throws {
        for moving in [true, false] {
            let p = focusFields()
            defer { window.endEditing(true); window.isHidden = true }
            p.apply(wireBatch([
                ["op": "create", "id": 20, "kind": "view", "handlers": ["focusout"]],
                ["op": "children", "id": 20, "ids": [11]],
                ["op": "children", "id": 10, "ids": [20, 12]]
            ]))
            let field = try XCTUnwrap(p.views[11]?.field)
            XCTAssertTrue(field.becomeFirstResponder())
            let outgoing = expectation(description: "detached focusout")
            var heard: [String] = []
            p.onBlur = { _ in
                heard.append("blur")
                p.apply(wireBatch([["op": "destroy", "id": 20], ["op": "destroy", "id": 11]]))
            }
            p.onFocusEvent = { at, kind, target, related in
                heard.append("\(at):\(kind):\(target):\(related ?? "-")")
                if at == 20 { outgoing.fulfill() }
            }
            if moving { tab(field) } else { XCTAssertTrue(field.resignFirstResponder()) }
            wait(for: [outgoing], timeout: 5)
            XCTAssertEqual(try XCTUnwrap(p.views[12]?.field).isFirstResponder, moving)
            XCTAssertEqual(heard, moving ? ["blur", "20:focusout:a:b", "10:focusin:b:a"] : ["blur", "20:focusout:a:-"])
            XCTAssertNil(p.views[20])
        }
    }

    func testFocusTransitionUsesTheRenamedId() throws {
        let p = focusFields()
        defer { window.endEditing(true); window.isHidden = true }
        let field = try XCTUnwrap(p.views[11]?.field)
        var heard: [String] = []
        p.onFocusEvent = { _, kind, target, related in heard.append("\(kind):\(target):\(related ?? "-")") }
        XCTAssertTrue(field.becomeFirstResponder())
        p.onInput = { _, _ in p.apply(wireBatch([["op": "props", "id": 11, "set": ["id": "renamed"]]])) }
        field.insertText("!")
        // Drive the editing callback, as FieldCompositionIOSTests does: the
        // unhosted UIKit test does not deliver sendActions(.editingChanged).
        try XCTUnwrap(p.views[11]).fieldChanged()
        XCTAssertEqual(p.views[11]?.props["id"], "renamed")
        tab(field)
        XCTAssertTrue(try XCTUnwrap(p.views[12]?.field).isFirstResponder)
        XCTAssertEqual(heard, ["focusin:a:-", "focusout:renamed:b", "focusin:b:renamed"])
    }

    private func fixture() -> (Presenter, NodeView, NodeView, NodeView, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let first = NodeView(id: 1, kind: "button", presenter: p)
        let text = NodeView(id: 2, kind: "text", presenter: p)
        let field = NodeView(id: 3, kind: "input", presenter: p)
        let last = NodeView(id: 4, kind: "button", presenter: p)
        text.applyProps(set: ["text": "Northbound"], clear: [])
        first.handlers = ["press"]; field.handlers = ["change"]; last.handlers = ["press"]
        for (i, node) in [first, text, field, last].enumerated() {
            node.frame = CGRect(x: 0, y: CGFloat(i) * 50, width: 200, height: 40)
            p.root.addSubview(node); p.views[node.id] = node
        }
        window.makeKeyAndVisible()
        return (p, first, text, field, last)
    }
    /// What UIKit sends for Tab (or Shift-Tab) from this node's chain.
    private func tab(_ node: UIResponder, shift: Bool = false, file: StaticString = #filePath, line: UInt = #line) {
        var responder: UIResponder? = node, command: UIKeyCommand?
        while let r = responder, command == nil {
            command = r.keyCommands?.first { $0.input == "\t" && $0.modifierFlags == (shift ? .shift : []) }
            responder = r.next
        }
        guard let command, let action = command.action else { return XCTFail("no Tab command on the chain", file: file, line: line) }
        XCTAssertTrue(command.wantsPriorityOverSystemBehavior, "a text input's own Tab must not win", file: file, line: line)
        let target = node.target(forAction: action, withSender: command) as? NSObject
        XCTAssertNotNil(target, file: file, line: line)
        _ = target?.perform(action, with: command)
    }

    func testBlockLinksOpenWithoutAHandlerAndRespectAuthoredPressAndDisabled() {
        final class Delegate: ExactSessionDelegate {
            var urls: [String] = []
            func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
                if name == "openURL", let url = args.first as? String { urls.append(url) }
            }
        }
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "block-link")
        defer { session.destroy() }
        let p = session.presenter
        let link = NodeView(id: 9001, kind: "button", presenter: p)
        link.frame = CGRect(x: 0, y: 0, width: 200, height: 40)
        p.root.addSubview(link); p.views[link.id] = link
        link.applyProps(set: ["href": "https://example.test/article"], clear: [])
        XCTAssertTrue(link.activate(at: link.convert(CGPoint(x: 10, y: 10), to: nil)) === link)
        XCTAssertEqual(delegate.urls, ["https://example.test/article"])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        link.handlers = ["press"]
        p.press(link.id)
        XCTAssertEqual(presses, [link.id])
        XCTAssertEqual(delegate.urls.count, 1)
        link.handlers = []
        link.applyProps(set: ["disabled": "true"], clear: [])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["inert": "true"], clear: ["disabled"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.count, 1)
        link.applyProps(set: ["href": "//example.test/relative"], clear: ["inert"])
        p.press(link.id)
        XCTAssertEqual(delegate.urls.last, "//example.test/relative")
        XCTAssertTrue(link.accessibilityActivate())
        XCTAssertEqual(delegate.urls.count, 3)
    }

    // These unhosted UIKit tests exercise the responder chain, not the system
    // keyboard window. Verify keyboard visibility in a launched app.
    func testNativeSelectMenuPreservesShortcutFocusAndAllowsTextEntry() throws {
        guard #available(iOS 17.4, *) else { throw XCTSkip("UIKit menu activation requires iOS 17.4") }
        let session = ExactApp.shared.makeSession(label: "select-keyboard")
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) }
            ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        defer { window.endEditing(true); session.destroy(); window.isHidden = true }
        let p = session.presenter
        let button = try XCTUnwrap(p.controls.makeValueControl("select", 900) as? UIButton)
        button.frame = CGRect(x: 32, y: 200, width: 250, height: 44)
        button.menu = UIMenu(children: [UIAction(title: "Chocolate Glaze") { _ in }, UIAction(title: "Blueberry Spread") { _ in }])
        view.addSubview(button)
        let shortcut = NodeView(id: 901, kind: "button", presenter: p)
        p.views[shortcut.id] = shortcut
        p.root.addSubview(shortcut)
        shortcut.frame = CGRect(x: 32, y: 350, width: 100, height: 44)
        shortcut.handlers = ["press"]
        shortcut.applyProps(set: ["accessibilityKeyShortcuts": "Meta+g"], clear: [])
        func until(_ message: String, _ condition: () -> Bool) {
            let end = Date(timeIntervalSinceNow: 10)
            while !condition(), Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
            XCTAssertTrue(condition(), message)
        }
        until("the window is key") { window.isKeyWindow && view.window === window }
        XCTAssertTrue(view.becomeFirstResponder())
        button.performPrimaryAction()
        func containsOption(_ node: UIView) -> Bool {
            (node as? UILabel)?.text == "Blueberry Spread" || node.subviews.contains(where: containsOption)
        }
        until("the real UIKit menu opened") { containsOption(window) && view.isFirstResponder }
        XCTAssertTrue(view.isFirstResponder, "shortcut focus survives opening the menu")
        // nil lets UIKit request its default keyboard for this responder.
        // A nontext responder must instead supply input with no height.
        XCTAssertEqual(view.inputView?.bounds.height, 0, "shortcut focus must not request a system keyboard")
        button.contextMenuInteraction?.dismissMenu()
        until("the menu dismissed and shortcut focus remains") { !containsOption(window) && view.isFirstResponder }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let command = try XCTUnwrap(view.keyCommands?.first { $0.input == "g" && $0.modifierFlags == .command })
        let action = try XCTUnwrap(command.action)
        let target = try XCTUnwrap(view.target(forAction: action, withSender: command) as? NSObject)
        _ = target.perform(action, with: command)
        XCTAssertEqual(pressed, [shortcut.id], "hardware shortcuts still reach the app after a menu")
        view.endEditing(true)
        let field = UITextField(frame: CGRect(x: 32, y: 300, width: 250, height: 44))
        view.addSubview(field)
        XCTAssertTrue(field.becomeFirstResponder())
        until("the text field owns input") { field.isFirstResponder && view.inputView == nil }
        XCTAssertNil(view.inputView, "text entry must not inherit the shortcut responder’s empty input view")
        field.insertText("Glaze")
        XCTAssertEqual(field.text, "Glaze")
        let editor = UITextView(frame: CGRect(x: 32, y: 300, width: 250, height: 100))
        view.addSubview(editor)
        XCTAssertTrue(editor.becomeFirstResponder())
        XCTAssertNil(view.inputView, "multiline editing must also inherit the system keyboard")
        editor.insertText("Chocolate\nGlaze")
        XCTAssertEqual(editor.text, "Chocolate\nGlaze")
        XCTAssertTrue(view.becomeFirstResponder())
        XCTAssertEqual(view.inputView?.bounds.height, 0, "shortcut focus requests no keyboard again after editing")
    }

    /// A press leaves its button focused (a vote, then the `select` below
    /// it): the select's menu keeps that focus and raises no keyboard over
    /// its options, as with the view's shortcut focus (polls, LLP 1116 D2).
    func testAButtonAPressLeftFocusedRequestsNoKeyboardForASelectsMenu() throws {
        guard #available(iOS 17.4, *) else { throw XCTSkip("UIKit menu activation requires iOS 17.4") }
        let session = ExactApp.shared.makeSession(label: "select-after-press")
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        defer { window.endEditing(true); session.destroy(); window.isHidden = true }
        let p = session.presenter
        let vote = NodeView(id: 902, kind: "button", presenter: p)
        p.views[vote.id] = vote
        p.root.addSubview(vote)
        vote.frame = CGRect(x: 32, y: 120, width: 100, height: 44)
        vote.handlers = ["press"]
        let button = try XCTUnwrap(p.controls.makeValueControl("select", 903) as? UIButton)
        button.frame = CGRect(x: 32, y: 200, width: 250, height: 44)
        button.menu = UIMenu(children: [UIAction(title: "Alice") { _ in }, UIAction(title: "Bob") { _ in }])
        view.addSubview(button)
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue(vote.becomeFirstResponder(), "the press focuses its button")
        button.performPrimaryAction()
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        func containsOption(_ node: UIView) -> Bool { (node as? UILabel)?.text == "Bob" || node.subviews.contains(where: containsOption) }
        XCTAssertTrue(containsOption(window), "the real UIKit menu opened")
        XCTAssertTrue(vote.isFirstResponder, "the button's focus survives the menu")
        XCTAssertEqual(vote.inputView?.bounds.height, 0, "a focused button requests no system keyboard")
        button.contextMenuInteraction?.dismissMenu()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertTrue(vote.resignFirstResponder())
        XCTAssertNil(vote.inputView, "unfocused, it supplies none")
    }

    func testTabWalksControlsInTreeOrderAndSkipsText() throws {
        let (p, first, text, field, last) = fixture()
        defer { withExtendedLifetime(p) {} }
        let editor = try XCTUnwrap(field.field)
        XCTAssertTrue(first.becomeFirstResponder(), "a pressable takes the focus")
        tab(first)
        XCTAssertTrue(editor.isFirstResponder, "Tab passes over the paragraph to the field")
        XCTAssertFalse(text.isFirstResponder)
        tab(editor)
        XCTAssertTrue(last.isFirstResponder, "Tab leaves the field for the next control")
        tab(last)
        XCTAssertTrue(first.isFirstResponder, "the order wraps")
        tab(first, shift: true)
        XCTAssertTrue(last.isFirstResponder, "Shift-Tab walks back")
    }

    /// The driver's `type <field> key "Tab"` does what a hardware Tab does
    /// (bench t9-profile, 2026-10-08: it used to leave the focus in the field,
    /// so no blur or change ran): the next stop takes the focus, the field's
    /// editing ends, Shift goes back, and a handler-free Tab never types.
    func testAgentTabMovesTheFocusAsAKeyboardTabDoes() throws {
        let session = ExactApp.shared.makeSession(label: "agent-tab")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let name = NodeView(id: 7001, kind: "input", presenter: p)
        let email = NodeView(id: 7002, kind: "input", presenter: p)
        let save = NodeView(id: 7003, kind: "button", presenter: p)
        name.handlers = ["change", "blur"]; email.handlers = ["change", "keyup"]; save.handlers = ["press", "keyup"]
        for (i, node) in [name, email, save].enumerated() {
            node.frame = CGRect(x: 0, y: CGFloat(i) * 50, width: 200, height: 40)
            p.root.addSubview(node); p.views[node.id] = node
        }
        window.makeKeyAndVisible()
        defer { window.endEditing(true) }
        let nameField = try XCTUnwrap(name.field), emailField = try XCTUnwrap(email.field)
        var blurred: [UInt32] = [], changed: [(UInt32, String)] = []
        p.onBlur = { blurred.append($0) }
        p.onChange = { changed.append(($0, $1)) }
        var reply = session.agentInstance.type(["id": 7001, "text": "Ada"])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(nameField.isFirstResponder)
        // An unhosted window's field sends no `.editingChanged` for an inserted
        // text; a hosted one does, and that is what records the edit `change`
        // commits on end-editing. Send it, as the keyboard would.
        name.fieldChanged()
        reply = session.agentInstance.type(["id": 7001, "key": "Tab"])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertFalse(nameField.isFirstResponder, "Tab ends the field's editing")
        XCTAssertTrue(emailField.isFirstResponder, "and the next field takes the focus")
        XCTAssertEqual(nameField.text, "Ada", "Tab types nothing")
        XCTAssertTrue(blurred.contains(7001), "the field it left hears blur")
        XCTAssertTrue(changed.contains { $0.0 == 7001 && $0.1 == "Ada" }, "and change, with what was typed")
        // A held Tab, its down then its up: the release keeps the new focus.
        reply = session.agentInstance.type(["id": 7002, "key": "Tab", "phase": "down", "releaseKey": "t1"])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(save.isFirstResponder, "the down moves the focus")
        _ = session.agentInstance.type(["id": 7002, "key": "Tab", "phase": "up"])
        XCTAssertTrue(save.isFirstResponder, "and the up does not take it back")
        var keyups: [(UInt32, String)] = []
        p.onKey = { id, press in if press.up { keyups.append((id, press.chord)) } }
        _ = session.agentInstance.type(["id": 7003, "key": "Shift+Tab"])
        XCTAssertTrue(emailField.isFirstResponder, "Shift-Tab goes back")
        // Its release, Tab's and Shift's, reaches where it left the focus.
        XCTAssertEqual(keyups.map(\.0), [7002, 7002], "both keyups at the field it moved to: \(keyups)")
        XCTAssertEqual(keyups.map(\.1).last, "Shift", "Shift comes up last, without its own bit")
        XCTAssertEqual(emailField.text, "", "nor does Shift-Tab type")
    }

    func testFirstTabTakesTheFirstControlAndShowsItsRing() {
        let (p, first, _, _, last) = fixture()
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder)
        XCTAssertNotNil(first.focusRing, "a control Tab reached shows its ring")
        p.moveFocus(backward: true)
        XCTAssertTrue(last.isFirstResponder)
        XCTAssertNil(first.focusRing, "the ring leaves with the focus")
        XCTAssertTrue(first.becomeFirstResponder())
        XCTAssertNil(first.focusRing, "a touch's focus shows no ring")
    }

    /// HTML's `tabindex` (LLP 1088 D7.3): an explicit value makes a plain box
    /// focusable and, ≥ 0, a Tab stop, positive first; a negative one takes
    /// the focus but not Tab; absent is never `0`; a disabled button stays
    /// out, and a disabled box is a stop (Chrome's `<div disabled>`).
    func testTabindexMakesABoxFocusableAndOrdersTab() {
        let (p, first, _, field, last) = fixture()
        defer { withExtendedLifetime(p) {} }
        func box(_ id: UInt32, _ props: [String: String], kind: String = "view") -> NodeView {
            let n = NodeView(id: id, kind: kind, presenter: p)
            n.applyProps(set: props, clear: [])
            n.frame = CGRect(x: 220, y: CGFloat(id) * 20, width: 100, height: 10)
            p.root.addSubview(n); p.views[id] = n
            return n
        }
        let stop = box(10, ["tabIndex": "0"]), plain = box(11, [:]), early = box(12, ["tabIndex": "1"])
        let skipped = box(13, ["tabIndex": "-1"]), disabled = box(14, ["tabIndex": "0", "disabled": "true"])
        let off = box(15, ["tabIndex": "0", "disabled": "true"], kind: "button")
        XCTAssertFalse(plain.canBecomeFirstResponder, "absent is not tabindex=0")
        XCTAssertTrue(disabled.canBecomeFirstResponder, "disabled means nothing on a box")
        XCTAssertFalse(off.canBecomeFirstResponder, "a disabled button is out")
        XCTAssertTrue(skipped.becomeFirstResponder(), "-1 takes the focus by tap or script")
        p.moveFocus(backward: false)
        XCTAssertTrue(early.isFirstResponder, "from a node out of the order, Tab starts at the first: a positive tabindex")
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder, "then tree order")
        p.moveFocus(backward: false)
        XCTAssertTrue(field.field?.isFirstResponder == true)
        p.moveFocus(backward: false); p.moveFocus(backward: false)
        XCTAssertTrue(stop.isFirstResponder, "tabindex=0 with no handler is a stop, in tree order")
        p.moveFocus(backward: false)
        XCTAssertTrue(disabled.isFirstResponder, "then the disabled box")
        p.moveFocus(backward: false)
        XCTAssertTrue(early.isFirstResponder, "the order wraps, skipping -1 and the disabled button")
        p.moveFocus(backward: true)
        XCTAssertTrue(disabled.isFirstResponder)
    }

    /// UIKit's focus search (`FocusSearch`) finds nothing in a tree of
    /// exact2's own views, and finds an input's field as UIKit always did.
    func testFocusSearchSeesOnlyWhatUIKitCanFocus() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        let button = NodeView(id: 1, kind: "button", presenter: p)
        button.frame = CGRect(x: 0, y: 0, width: 200, height: 40)
        p.root.addSubview(button); p.views[button.id] = button
        window.makeKeyAndVisible()
        let all = window.bounds
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "a tree of exact2's views offers UIKit nothing to search")
        let input = NodeView(id: 2, kind: "input", presenter: p)
        input.frame = CGRect(x: 0, y: 50, width: 200, height: 40)
        p.root.addSubview(input); p.views[input.id] = input
        let editor = try XCTUnwrap(input.field)
        XCTAssertTrue(p.viewport.focusItems(in: all).contains { $0 === editor }, "a field in the window is what the search is given")
        XCTAssertFalse(p.viewport.focusItems(in: all).contains { $0 === p.root || $0 === button }, "and none of exact2's own views")
        XCTAssertTrue(p.viewport.focusItems(in: CGRect(x: 0, y: 0, width: 400, height: 45)).isEmpty, "only where it is")
        input.isHidden = true
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "nor a hidden one")
        input.isHidden = false
        input.removeFromSuperview()
        XCTAssertTrue(p.viewport.focusItems(in: all).isEmpty, "and it closes when the field leaves")
    }
}
#endif
