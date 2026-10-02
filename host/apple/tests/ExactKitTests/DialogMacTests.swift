#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class DialogMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                          styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["commandfor": "form", "command": "show-modal", "accessibilityKeyShortcuts": "Meta+o"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["semanticTag": "dialog", "id": "form", "closedby": "any"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "draft", "autofocus": "true"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["commandfor": "form", "command": "close"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "children", "id": 3, "ids": [4, 5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 80, "h": 30],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 150],
            ["op": "frame", "id": 4, "x": 10, "y": 10, "w": 200, "h": 30],
            ["op": "frame", "id": 5, "x": 10, "y": 80, "w": 80, "h": 30],
        ]))
        return p
    }
    private func key(_ code: UInt16, modifiers: NSEvent.ModifierFlags = [], character: String = "") -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: character, charactersIgnoringModifiers: character, isARepeat: false, keyCode: code)!
    }
    private func click(_ node: NodeView) {
        let point = node.convert(NSPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
        // Hit testing is AppKit's; no target-id activation can bypass the backdrop.
        let hit = window.contentView?.hitTest(point)
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        let up = NSEvent.mouseEvent(with: .leftMouseUp, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!
        hit?.mouseDown(with: down)
        hit?.mouseUp(with: up)
    }
    func testCommandOnlyButtonsPresentRealFormTrapFocusAndRestore() throws {
        let p = fixture()
        let opener = p.views[2]!, dialog = p.views[3]!, field = try XCTUnwrap(p.views[4]?.field), close = p.views[5]!
        XCTAssertTrue(dialog.isHidden)
        window.makeFirstResponder(opener)
        click(opener)
        XCTAssertTrue(p.dialogs.active === dialog)
        XCTAssertFalse(dialog.isHidden)
        XCTAssertTrue(field.currentEditor() === window.firstResponder)
        XCTAssertTrue(opener.inert)
        XCTAssertFalse(p.shortcuts.perform(key(31, modifiers: .command, character: "o")))
        XCTAssertFalse(close.inert)
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === close)
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(field.currentEditor() === window.firstResponder)
        XCTAssertTrue(p.dialogs.key(key(48, modifiers: .shift)))
        XCTAssertTrue(window.firstResponder === close)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.press(opener.id)
        XCTAssertTrue(presses.isEmpty)
        click(close)
        XCTAssertNil(p.dialogs.active)
        XCTAssertTrue(presses.isEmpty, "a command-only button has no runner press handler")
        XCTAssertTrue(window.firstResponder === opener)
        XCTAssertTrue(dialog.superview === p.views[1])
        XCTAssertEqual(field.stringValue, "draft")
        XCTAssertTrue(p.shortcuts.perform(key(31, modifiers: .command, character: "o")))
        XCTAssertTrue(p.dialogs.active === dialog)
        close.handlers.insert("press")
        p.press(close.id)
        XCTAssertEqual(presses, [5], "the confirmation action is dispatched once before closing")
        XCTAssertNil(p.dialogs.active)
    }
    func testDismissalPoliciesAndBackgroundPointerBlocking() {
        let p = fixture()
        let modal = p.views[3]!
        for policy in ["none", "closerequest", "any", "invalid"] {
            modal.props["closedby"] = policy
            p.press(2)
            p.dialogs.outside()
            XCTAssertEqual(p.dialogs.active == nil, policy == "any")
            if policy == "any" { p.press(2) }
            XCTAssertTrue(p.dialogs.key(key(53)))
            XCTAssertEqual(p.dialogs.active == nil, policy != "none")
            p.dialogs.close(modal)
        }
        p.press(2)
        modal.props["closedby"] = "none"
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        click(p.views[2]!)
        XCTAssertTrue(pressed.isEmpty)
        XCTAssertNotNil(p.dialogs.active)
        XCTAssertNil(p.views[2]!.hitTest(.zero))
    }
    func testBatchesKeepEditorAndRemovalAndResetRetireOwnership() throws {
        let p = fixture()
        p.press(2)
        let field = try XCTUnwrap(p.views[4]?.field), editor = window.firstResponder
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3]],
                          ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 260, "h": 160]]))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertTrue(p.views[4]?.field === field)
        XCTAssertEqual(p.dialogs.active?.frame.size, NSSize(width: 260, height: 160))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        XCTAssertNil(p.dialogs.active)
        XCTAssertNil(p.views[3]?.superview)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3]]]))
        p.press(2)
        XCTAssertNotNil(p.dialogs.active)
        p.reset()
        XCTAssertNil(p.dialogs.active)
        XCTAssertFalse(p.viewport.subviews.contains { $0 is DialogBackdrop })
    }
    func testACommandCannotTargetReplacementNodesAfterAppAction() {
        let p = fixture()
        p.views[2]!.handlers.insert("press")
        p.onPress = { _ in p.reset() }
        p.press(2)
        XCTAssertNil(p.dialogs.active)
    }
    func testNativeMenuActionsRemainAvailableWithoutAModal() {
        let p = fixture()
        let menu = NodeView(id: 20, kind: "view", presenter: p)
        let action = NodeView(id: 21, kind: "button", presenter: p)
        action.handlers = ["press"]
        p.views[20] = menu; p.views[21] = action
        p.root.addSubview(menu); menu.addSubview(action)
        menu.isHidden = true // NSMenu projects the rows of a hidden popover.
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let item = NSMenuItem(title: "Action", action: nil, keyEquivalent: "")
        item.representedObject = NSNumber(value: action.id)
        _ = p.menus.perform(NSSelectorFromString("pick:"), with: item)
        XCTAssertEqual(pressed, [21])
        p.press(2)
        _ = p.menus.perform(NSSelectorFromString("pick:"), with: item)
        XCTAssertEqual(pressed, [21], "a modal blocks a queued background menu selection")
        p.dialogs.close(p.views[3]!)
    }

    private func textarea(_ p: Presenter, id: UInt32, parent: UInt32) throws -> NSTextView {
        let node = NodeView(id: id, kind: "textarea", presenter: p)
        p.views[id] = node
        p.views[parent]!.container.addSubview(node)
        node.frame = NSRect(x: 10, y: 45, width: 200, height: 30)
        let editor = try XCTUnwrap(node.textArea)
        editor.string = "A real textarea draft"
        XCTAssertTrue(editor.delegate === node)
        XCTAssertFalse(editor.isFieldEditor)
        return editor
    }

    func testTextareaTabAdvancesFromTheActualTextView() throws {
        let p = fixture()
        let editor = try textarea(p, id: 7, parent: 3)
        p.press(2)
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertTrue(p.dialogs.key(key(48, modifiers: .shift)))
        XCTAssertTrue(window.firstResponder === p.views[5], "Shift-Tab leaves the last textarea for the preceding button")
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[4]?.field?.currentEditor(), "Tab wraps from the last textarea to the first field")
        p.dialogs.close(p.views[3]!)
    }

    func testTextareaFocusAndSelectionRestoreAfterClosingDialog() throws {
        let p = fixture()
        let editor = try textarea(p, id: 7, parent: 1)
        XCTAssertTrue(window.makeFirstResponder(editor))
        let selection = NSRange(location: 2, length: 4)
        editor.setSelectedRange(selection)
        p.press(2)
        editor.setSelectedRange(NSRange(location: 0, length: 0))
        p.dialogs.close(p.views[3]!)
        XCTAssertTrue(window.firstResponder === editor, "restore the text view, not its non-focusable delegate")
        XCTAssertEqual(editor.selectedRange(), selection)
    }

    func testMarkedTextareaKeepsEscapeAndTabForItsInputMethod() throws {
        let p = fixture()
        let editor = try textarea(p, id: 7, parent: 3)
        for code: UInt16 in [48, 53] {
            p.dialogs.show(p.views[3]!)
            XCTAssertTrue(window.makeFirstResponder(editor))
            editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            XCTAssertTrue(editor.hasMarkedText())
            XCTAssertFalse(p.dialogs.key(key(code)), "composition keys must reach the text input client")
            XCTAssertTrue(p.dialogs.active === p.views[3])
            XCTAssertTrue(window.firstResponder === editor)
            editor.unmarkText()
        }
        p.dialogs.close(p.views[3]!)
    }

    func testSelectAllAndTextPaintingFollowTheTopLayer() {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Background"]],
            ["op": "create", "id": 8, "kind": "text", "props": ["text": "Dialog text"]],
            ["op": "children", "id": 1, "ids": [2, 3, 7]],
            ["op": "children", "id": 3, "ids": [4, 5, 8]],
            ["op": "frame", "id": 7, "x": 0, "y": 100, "w": 200, "h": 20],
            ["op": "frame", "id": 8, "x": 0, "y": 120, "w": 200, "h": 20],
        ]))
        p.selection.selectAll()
        XCTAssertEqual(p.selection.selectedText(), "Background")
        p.press(2)
        XCTAssertTrue(p.selection.paragraphs.contains { $0.id == 8 })
        p.selection.selectAll()
        XCTAssertEqual(p.selection.selectedText(), "Dialog text")
        p.dialogs.close(p.views[3]!)
        p.selection.selectAll()
        XCTAssertEqual(p.selection.selectedText(), "Background")
    }

    func testDialogFocusDoesNotCrossSessionBoundary() {
        let p = fixture(), peer = Presenter()
        let mount = NSView(frame: window.contentView!.bounds)
        window.contentView = mount
        p.viewport.frame = NSRect(x: 0, y: 0, width: 250, height: 400)
        peer.viewport.frame = NSRect(x: 250, y: 0, width: 250, height: 400)
        mount.addSubview(p.viewport); mount.addSubview(peer.viewport)
        let peerButton = NodeView(id: 2, kind: "button", presenter: peer)
        peer.views[2] = peerButton
        peer.root.addSubview(peerButton)
        peerButton.frame = NSRect(x: 0, y: 0, width: 100, height: 40)
        peer.fitDocument()
        p.press(2)
        XCTAssertNotNil(p.dialogs.active)
        XCTAssertFalse(peerButton.inert)
        XCTAssertTrue(window.makeFirstResponder(peerButton))
        p.dialogs.sync()
        XCTAssertTrue(window.firstResponder === peerButton)
        XCTAssertFalse(p.dialogs.key(key(53)))
        XCTAssertNotNil(p.dialogs.active)
        p.dialogs.close(p.views[3]!)
        XCTAssertTrue(window.firstResponder === peerButton)
    }

    func testNegativeTabIndexAutofocusAndEmptyDialogStayContained() {
        let p = fixture()
        p.views[4]!.props["tabIndex"] = "-1"
        p.press(2)
        XCTAssertTrue(p.views[4]!.field?.currentEditor() === window.firstResponder)
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[5])
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[5])
        p.apply(wireBatch([["op": "children", "id": 3, "ids": []]]))
        XCTAssertTrue(window.firstResponder === p.views[3])
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[3])
        p.dialogs.close(p.views[3]!)
    }

    func testExplicitInsetsAndNestedDialogs() {
        let p = fixture()
        p.views[3]!.applyStyle(["left": 12, "bottom": ["pct": 10, "px": 8]])
        p.press(2)
        XCTAssertEqual(p.views[3]!.frame.minX, 12)
        XCTAssertEqual(p.views[3]!.frame.maxY, p.viewport.bounds.height * 0.9 - 8, accuracy: 0.01)
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "dialog", "id": "inner"]],
            ["op": "children", "id": 3, "ids": [4, 5, 6]],
            ["op": "frame", "id": 6, "x": 0, "y": 0, "w": 100, "h": 80],
        ]))
        p.dialogs.show(p.views[6]!)
        XCTAssertTrue(p.views[4]!.inert)
        XCTAssertTrue(window.firstResponder === p.views[6])
        XCTAssertTrue(p.dialogs.key(key(53)))
        XCTAssertTrue(p.dialogs.active === p.views[3])
        XCTAssertFalse(p.views[4]!.inert)
        p.reset()
    }

    func testReloadUnmountAndDestroyReleaseOnlyTheirDialog() throws {
        _ = NSApplication.shared
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = """
        component App
          view
            column width="100%" height="100%"
              button "Open" testId="open" commandfor="form" command="show-modal"
              dialog id="form" width=240 padding=16
                input testId="edit" value="draft" autofocus=true
                button "Close" commandfor="form" command="close"

        """
        let input = directory.appendingPathComponent("app.contract")
        let output = directory.appendingPathComponent("app.plan")
        try source.write(to: input, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", input.path, "-o", output.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let plan = try Data(contentsOf: output)
        let session = ExactApp.shared.makeSession(label: "dialog-lifecycle")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 500, height: 400)).error)
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let mount = NSView(frame: NSRect(x: 0, y: 0, width: 500, height: 400))
        window.contentView = mount
        view.frame = mount.bounds; mount.addSubview(view)
        let p = session.presenter
        func open() throws {
            p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" }).id)
            XCTAssertNotNil(p.dialogs.active)
        }
        try open()
        let stale = p.dialogs.command(try XCTUnwrap(p.views.values.first { $0.props["command"] == "close" }))
        view.removeFromSuperview()
        XCTAssertNil(p.dialogs.active)
        mount.addSubview(view)
        try open()
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 500, height: 400)).error)
        XCTAssertNil(p.dialogs.active)
        try open()
        stale?()
        XCTAssertNotNil(p.dialogs.active, "an old command cannot close a reused id after reload")
        session.destroy()
        XCTAssertNil(p.dialogs.active)
        XCTAssertFalse(p.viewport.subviews.contains { $0 is DialogBackdrop })
    }

}
#endif
