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
    /// The web's order (docs/contract-grammar.md#keys): a field's `key`
    /// handlers hear Escape and Tab before the dialog's defaults, and one that
    /// calls `preventDefault()` keeps the dialog open and the focus put.
    func testKeyHandlersHearEscapeAndTabBeforeTheDialogAndCanKeepThem() throws {
        let p = fixture()
        let opener = p.views[2]!, node = p.views[4]!, field = try XCTUnwrap(node.field)
        node.handlers.insert("key")
        var heard: [String] = [], prevent = true
        p.onKey = { id, name in heard.append("\(id) \(name)"); if prevent { p.defaultPrevented = true } }
        window.makeFirstResponder(opener)
        click(opener)
        XCTAssertTrue(field.currentEditor() === window.firstResponder)
        XCTAssertTrue(p.routeKey(key(53, character: "\u{1b}"), focused: true))
        XCTAssertTrue(p.routeKey(key(48, character: "\t"), focused: true))
        XCTAssertEqual(heard, ["4 Escape", "4 Tab"])
        XCTAssertNotNil(p.dialogs.active, "a prevented Escape leaves the dialog open")
        XCTAssertTrue(field.currentEditor() === window.firstResponder, "a prevented Tab leaves the focus")
        prevent = false
        XCTAssertTrue(p.routeKey(key(48, character: "\t"), focused: true))
        XCTAssertTrue(window.firstResponder === p.views[5], "an unprevented Tab is the dialog's")
        window.makeFirstResponder(field)
        XCTAssertTrue(p.routeKey(key(53, character: "\u{1b}"), focused: true))
        XCTAssertEqual(heard, ["4 Escape", "4 Tab", "4 Tab", "4 Escape"])
        XCTAssertNil(p.dialogs.active, "an unprevented Escape closes it")
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
    func testNativeButtonsKeepTheirFocusOrderAndRespectModalInputBlocking() throws {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "control", "props": ["type": "button"], "style": ["appearance": "auto"]],
            ["op": "create", "id": 8, "kind": "control", "props": ["type": "button"], "handlers": ["press"], "style": ["appearance": "auto"]],
            ["op": "children", "id": 3, "ids": [4, 5, 7]],
            ["op": "children", "id": 1, "ids": [2, 3, 8]],
            ["op": "frame", "id": 7, "x": 100, "y": 80, "w": 100, "h": 30],
            ["op": "frame", "id": 8, "x": 100, "y": 10, "w": 100, "h": 30],
        ]))
        let inside = try XCTUnwrap(p.views[7])
        let background = try XCTUnwrap(p.controls.controls[8] as? NativeButtonMac)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(2)
        XCTAssertTrue(inside.acceptsFirstResponder, "a native button without handlers remains a focus stop")
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[5])
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === inside)
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[4]?.field?.currentEditor())
        background.performClick(nil)
        XCTAssertTrue(pressed.isEmpty, "native activation behind a modal stays blocked")
        p.dialogs.close(p.views[3]!)
        background.performClick(nil)
        XCTAssertEqual(pressed, [8], "native activation resumes after the modal closes")
    }
    /// A native row that only closes its dialog (LLP 1069.011.000 D9) is its
    /// own press, as a custom command-only button is: not its ancestor's.
    func testANativeCloseRowClosesItsDialog() throws {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "control", "props": ["type": "button", "commandfor": "form", "command": "close"], "style": ["appearance": "auto"]],
            ["op": "children", "id": 3, "ids": [4, 5, 9]],
            ["op": "frame", "id": 9, "x": 100, "y": 80, "w": 100, "h": 30],
        ]))
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.press(2)
        XCTAssertTrue(p.dialogs.active === p.views[3])
        XCTAssertTrue(try XCTUnwrap(p.views[9]).pressable)
        try XCTUnwrap(p.controls.controls[9] as? NativeButtonMac).performClick(nil)
        XCTAssertNil(p.dialogs.active, "its command closed the dialog")
        XCTAssertTrue(pressed.isEmpty, "a command-only button has no runner press handler")
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

    func testNativeMenuDialogCommandsUseTheProjectedRows() throws {
        let p = fixture()
        let popover = NodeView(id: 20, kind: "view", presenter: p)
        p.views[20] = popover
        p.root.addSubview(popover)
        popover.addSubview(p.views[2]!)
        popover.isHidden = true
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        for withHandler in [false, true] {
            p.views[2]!.handlers = withHandler ? ["press"] : []
            let menu = p.menus.menu(of: popover)
            XCTAssertEqual(menu.items.count, 1, "command-only rows must be menu items")
            let item = try XCTUnwrap(menu.items.first)
            p.press(2)
            XCTAssertNil(p.dialogs.active, "ordinary hidden-node activation stays blocked")
            presses.removeAll()
            _ = p.menus.perform(try XCTUnwrap(item.action), with: item)
            XCTAssertTrue(p.dialogs.active === p.views[3])
            XCTAssertEqual(presses, withHandler ? [2] : [])
            p.dialogs.close(p.views[3]!)
            p.views[2]!.props["disabled"] = "true"
            _ = p.menus.perform(try XCTUnwrap(item.action), with: item)
            XCTAssertNil(p.dialogs.active)
            p.views[2]!.props.removeValue(forKey: "disabled")
        }
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

    func testTabUsesTheEditingCandidateBeforeItsFocusableAncestor() throws {
        let p = fixture()
        let editor = try textarea(p, id: 7, parent: 3)
        p.views[3]!.props["tabIndex"] = "1"
        p.press(2)
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertTrue(p.dialogs.key(key(48)))
        XCTAssertTrue(window.firstResponder === p.views[3], "Tab wraps from the last editor to the positive-tab-index dialog")
        XCTAssertTrue(window.makeFirstResponder(editor))
        XCTAssertTrue(p.dialogs.key(key(48, modifiers: .shift)))
        XCTAssertTrue(window.firstResponder === p.views[5], "Shift-Tab starts at the editor, not its focusable ancestor")
    }

    func testMarkedTextareaKeepsEscapeAndTabForItsInputMethod() throws {
        let p = fixture()
        let editor = try textarea(p, id: 7, parent: 3)
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["accessibilityKeyShortcuts": "Escape"], "clear": []]]))
        for code: UInt16 in [48, 53] {
            p.dialogs.show(p.views[3]!)
            XCTAssertTrue(window.makeFirstResponder(editor))
            editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            XCTAssertTrue(editor.hasMarkedText())
            XCTAssertFalse(p.dialogs.key(key(code)) || p.shortcuts.perform(key(code)), "composition keys must reach the text input client")
            XCTAssertTrue(p.dialogs.active === p.views[3])
            XCTAssertTrue(window.firstResponder === editor)
            editor.unmarkText()
        }
        XCTAssertTrue(p.shortcuts.perform(key(53)), "the declared shortcut resumes after composition")
        XCTAssertNil(p.dialogs.active)
    }

    func testCustomInputClientKeepsCompositionKeys() {
        let p = fixture()
        let editor = MarkedInputClient(frame: NSRect(x: 10, y: 45, width: 200, height: 30))
        p.views[3]!.container.addSubview(editor)
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["accessibilityKeyShortcuts": "Escape"], "clear": []]]))
        for code: UInt16 in [48, 53] {
            p.dialogs.show(p.views[3]!)
            XCTAssertTrue(window.makeFirstResponder(editor))
            editor.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
            XCTAssertTrue(editor.hasMarkedText())
            XCTAssertFalse(p.dialogs.key(key(code)) || p.shortcuts.perform(key(code)), "custom NSTextInputClient composition must own Tab/Escape")
            XCTAssertTrue(p.dialogs.active === p.views[3], "Escape must not dismiss during composition")
            XCTAssertTrue(window.firstResponder === editor, "Tab must not move focus during composition")
            editor.unmarkText()
        }
        XCTAssertTrue(p.dialogs.key(key(53)), "Escape resumes after composition")
        XCTAssertNil(p.dialogs.active)
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

    /// With nothing focused a bare-key shortcut still presses its button, as
    /// a page's do; a field's `focus` and `blur` arrive when it takes and
    /// loses the focus, not at its first edit (jukebox F11, F23).
    func testBareShortcutsWithNothingFocusedAndFieldFocusEvents() throws {
        _ = NSApplication.shared
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = """
        component App
          state heard = ""
          action note(what: string)
            heard = `${heard} ${what}`
          view
            column width="100%" height="100%"
              button "Play" testId="play" aria-keyshortcuts="Space" press=note("space")
              input testId="field" aria-label="Search" focus=note("focus") blur=note("blur")
              textarea testId="notes" aria-label="Notes" focus=note("tfocus") blur=note("tblur")

        """
        let input = directory.appendingPathComponent("app.contract")
        let output = directory.appendingPathComponent("app.plan")
        try source.write(to: input, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", input.path, "-o", output.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "bare-shortcuts")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: output), size: CGSize(width: 500, height: 400)).error)
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        let p = session.presenter
        func heard() -> String {
            let text = session.agent(#"{"op":"state"}"#)
            let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
            return (json?["slots"] as? [String: Any])?["heard"] as? String ?? ""
        }
        func node(_ testId: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == testId }) }
        let space = key(49, character: " ")
        window.makeFirstResponder(nil)
        XCTAssertTrue(window.firstResponder === window)
        XCTAssertTrue(view.ownsShortcutFocus(), "with nothing focused the window's keys are the page's")
        XCTAssertTrue(p.routeKey(space, focused: view.ownsShortcutFocus(), in: window))
        XCTAssertEqual(heard(), " space")
        let field = try XCTUnwrap(try node("field").field)
        window.makeFirstResponder(field)
        XCTAssertEqual(heard(), " space focus", "focus arrives as the field takes it, before any edit")
        XCTAssertFalse(p.routeKey(space, focused: view.ownsShortcutFocus(), in: window), "a field's Space is its text")
        let notes = try XCTUnwrap(try node("notes").textArea)
        window.makeFirstResponder(notes)
        XCTAssertEqual(heard(), " space focus blur tfocus")
        window.makeFirstResponder(nil)
        XCTAssertEqual(heard(), " space focus blur tfocus tblur")
    }
}

private final class MarkedInputClient: NSView, NSTextInputClient {
    private var marked = false
    override var acceptsFirstResponder: Bool { true }
    func insertText(_ string: Any, replacementRange: NSRange) {}
    override func doCommand(by selector: Selector) {}
    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) { marked = true }
    func unmarkText() { marked = false }
    func selectedRange() -> NSRange { NSRange(location: 1, length: 0) }
    func markedRange() -> NSRange { NSRange(location: marked ? 0 : NSNotFound, length: marked ? 1 : 0) }
    func hasMarkedText() -> Bool { marked }
    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? { nil }
    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect { .zero }
    func characterIndex(for point: NSPoint) -> Int { 0 }
}
#endif
