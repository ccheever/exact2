#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1005's macOS pointer line: a press that moves the focus into a text
/// area or an input reports `pointerdown` first, then the old field's
/// `change` and `blur` and the new one's `focus`, then `pointerup`, as
/// Chrome orders them. NSWindow focuses the hit editor before its
/// `mouseDown`, so the window's monitor (ExactViewMac.swift) reports the
/// down before the window has the event; the editor's tracking then
/// reports only the up.
@MainActor
final class EditorPointerOrderMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    static let page = """
    component App
      state heard = ""
      state on = true
      action note(what: string)
        heard = `${heard} ${what}`
      action changed(value: string)
        heard = `${heard} change-a`
      action off
        heard = `${heard} down-d`
        on = false
      action openSheet
        heard = `${heard} down-y`
        showModal("sheet")
      action openAny
        heard = `${heard} down-w`
        showModal("anyclose")
      view
        column testId="page" width="100%" height="100%" padding=20 gap=8
          box pointerdown=note("down-a") pointerup=note("up-a")
            input testId="a" value="" width=240 height=24 focus=note("focus-a") blur=note("blur-a") change=changed
          box pointerdown=note("down-b") pointerup=note("up-b")
            input testId="b" value="bee" width=240 height=24 focus=note("focus-b") blur=note("blur-b")
          box pointerdown=note("down-t") pointerup=note("up-t")
            textarea testId="t" value="tea" width=240 height=48 focus=note("focus-t") blur=note("blur-t")
          box pointerdown=note("down-p") pointerup=note("up-p")
            input testId="p" type="password" value="pass" width=240 height=24 focus=note("focus-p") blur=note("blur-p")
          box pointerdown=off pointerup=note("up-d")
            input testId="d" disabled=(!on) width=240 height=24 focus=note("focus-d")
          box position="relative" width=240 height=24
            input testId="c" width=240 height=24 focus=note("focus-c")
            box testId="cover" position="absolute" left=0 top=0 width=240 height=24 pointerdown=note("down-cover") pointerup=note("up-cover")
          box pointerdown=note("down-zo") pointerup=note("up-zo")
            box pointerdown=note("down-z") pointerup=note("up-z")
              input testId="z" width=240 height=24 focus=note("focus-z")
          box pointerdown=note("down-yo") pointerup=note("up-yo")
            box pointerdown=openSheet pointerup=note("up-y")
              input testId="y" width=240 height=24 focus=note("focus-y")
          box pointerdown=openAny pointerup=note("up-w")
            input testId="w" width=240 height=24 focus=note("focus-w")
          row gap=8
            button testId="open" commandfor="sheet" command="show-modal"
              text "Open"
            button testId="pop" popovertarget="pop-over"
              text "Pop"
            button testId="ask-open" commandfor="ask" command="show-modal"
              text "Ask"
          dialog id="sheet" testId="sheet" padding=20 width=320
            column gap=8
              input testId="s1" width=240 height=24 focus=note("focus-s1") blur=note("blur-s1")
              box pointerdown=note("down-s2") pointerup=note("up-s2")
                input testId="s2" width=240 height=24 focus=note("focus-s2")
          dialog id="ask" role="alertdialog" aria-label="Sure?"
            text "Really?"
            button commandfor="ask" command="close" testId="ask-cancel"
              text "Cancel"
          dialog id="anyclose" testId="anyclose" closedby="any" padding=20 width=200
            box height=20
          column id="pop-over" testId="pop-over" popover="auto" padding=16 gap=8 width=280
            input testId="o1" width=240 height=24 focus=note("focus-o1") blur=note("blur-o1")
            box pointerdown=note("down-o2") pointerup=note("up-o2")
              input testId="o2" width=240 height=24 focus=note("focus-o2")
    """

    private func make() throws -> SheetFixture {
        clicksArriveAsInTheKeyWindow()
        let f = try SheetFixture(page: Self.page, size: CGSize(width: 420, height: 520))
        fixture = f
        f.window.orderFrontRegardless()
        f.settle()
        return f
    }
    /// What the handlers heard since the last `clear`.
    private func heard(_ f: SheetFixture) -> [String] { Array(everything(f).dropFirst(seen)) }
    private var seen = 0
    private func everything(_ f: SheetFixture) -> [String] {
        let text = f.session.agent(#"{"op":"state"}"#)
        let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
        return ((json?["slots"] as? [String: Any])?["heard"] as? String ?? "").split(separator: " ").map(String.init)
    }
    private func clear(_ f: SheetFixture) { f.settle(); seen = everything(f).count }
    /// A press inside node `id`'s editor as a hand's comes, through the
    /// application (EditorPointerMacTests.press): the window's monitor sees
    /// the down first, then the window. `between` runs after the down.
    private func press(_ f: SheetFixture, _ id: String, flags: NSEvent.ModifierFlags = [], clicks: Int = 1, between: () -> Void = {}) throws {
        let node = try f.node(id)
        let window = try XCTUnwrap(node.window, id)
        let at = node.convert(NSPoint(x: 4, y: 6), to: nil)
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: at, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: clicks, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let up = event(.leftMouseUp)
        NSApp.postEvent(up, atStart: true)
        NSApp.postEvent(event(.leftMouseDown), atStart: true)
        // The down goes when its turn ends, as AppKit lets a hand's go.
        autoreleasepool { if let down = NSApp.nextEvent(matching: [.leftMouseDown], until: nil, inMode: .default, dequeue: true) { NSApp.sendEvent(down) } }
        between()
        if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
        f.settle()
    }
    private func type(_ f: SheetFixture, _ id: String, _ text: String) throws {
        let editor = try XCTUnwrap(try f.node(id).field?.currentEditor() as? NSTextView, id)
        editor.insertText(text, replacementRange: editor.selectedRange())
        f.settle()
    }

    /// Tests 1 and 2: field to field, then to a text area and a password
    /// input, each in Chrome's order.
    func testAPressThatMovesTheFocusReportsItsDownFirst() throws {
        let f = try make()
        try press(f, "a")
        try type(f, "a", "x")
        clear(f)
        try press(f, "b")
        XCTAssertEqual(heard(f), ["down-b", "change-a", "blur-a", "focus-b", "up-b"])
        clear(f)
        try press(f, "t")
        XCTAssertEqual(heard(f), ["down-t", "blur-b", "focus-t", "up-t"])
        clear(f)
        try press(f, "p")
        XCTAssertEqual(heard(f), ["down-p", "blur-t", "focus-p", "up-p"])
        clear(f)
        try press(f, "b")
        XCTAssertEqual(heard(f), ["down-b", "blur-p", "focus-b", "up-b"])
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// Test 3: one down and one up a press, the field handing its down to
    /// its editor, the focused editor's own press, and a text area's.
    func testEachPressIsReportedOnce() throws {
        let f = try make()
        for id in ["b", "b", "t", "t", "p", "p"] {
            clear(f)
            try press(f, id)
            XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-\(id)", "up-\(id)"], id)
            XCTAssertNil(f.presenter.pointerHeld, id)
        }
    }

    /// Condition 1: a double and a triple click keep their selection, a down
    /// and an up each press; a control-click keeps its own way.
    func testMultipleClicksSelectAndReportEachPress() throws {
        let f = try make()
        try press(f, "t")
        let area = try XCTUnwrap(try f.node("t").textArea)
        area.string = "tea and toast"
        clear(f)
        try press(f, "t", clicks: 2)
        try press(f, "t", clicks: 3)
        XCTAssertEqual(heard(f), ["down-t", "up-t", "down-t", "up-t"])
        XCTAssertEqual(area.selectedRange(), NSRange(location: 0, length: 13), "the triple click selected the paragraph")
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// Condition 2: an editor under another view is not pressed; the view
    /// on top is, and the editor takes no focus.
    func testACoveredEditorIsNotReported() throws {
        let f = try make()
        clear(f)
        try press(f, "c")
        XCTAssertEqual(heard(f), ["down-cover", "up-cover"])
    }

    /// Test 6: a `pointerdown` that disables the editor it went down in:
    /// the up still comes at the release, and nothing stays held.
    func testADownThatDisablesItsEditorStillReleases() throws {
        let f = try make()
        clear(f)
        try press(f, "d")
        XCTAssertEqual(heard(f).first, "down-d")
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-d", "up-d"])
        XCTAssertTrue(try f.node("d").disabled)
        XCTAssertNil(f.presenter.pointerHeld)
        XCTAssertNil(f.presenter.mouseChain.early)
    }

    /// Test 4: a first click into a window that is not key, as a person's
    /// comes (no click counts there unless the view takes it; LLP 1115.000
    /// D7.9). An input's field takes the first click, so its down comes
    /// first; a text area takes none, so nothing is reported and nothing
    /// is focused, as before.
    func testAFirstClickIntoAnInactiveWindowKeepsTheOrder() throws {
        let f = try make()
        try press(f, "a")
        NodeView.everyClickCounts = false
        XCTAssertFalse(NSApp.isActive && f.window.isKeyWindow)
        for (id, expected) in [("b", ["down-b", "blur-a", "focus-b", "up-b"]), ("t", []), ("p", ["down-p", "blur-b", "focus-p", "up-p"])] {
            clear(f)
            try press(f, id)
            XCTAssertEqual(heard(f), expected, id)
            XCTAssertNil(f.presenter.pointerHeld, id)
            XCTAssertNil(f.presenter.mouseChain.early, id)
        }
    }

    /// A `pointerdown` that opens a sheet, whose animation runs a loop that
    /// can take the button's up before the window has the down, or before
    /// the down has bubbled on: one down and one up a node, every down
    /// before any up, and nothing held after.
    func testADownThatOpensASheetReportsThePressOnce() throws {
        let f = try make()
        clear(f)
        try press(f, "y")
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-y", "down-yo", "up-y", "up-yo"])
        XCTAssertEqual(heard(f).first, "down-y")
        XCTAssertNil(f.presenter.pointerHeld)
        XCTAssertNil(f.presenter.mouseChain.early)
        f.presenter.dialogs.reset()
        f.settle()
        clear(f)
        try press(f, "b")
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-b", "up-b"])
    }

    /// An up a loop takes while the early down is still bubbling (a
    /// handler's, here the test's, delivering the queued up, the only one,
    /// from the inner node's `pointerdown`): every node hears its down
    /// first, and the up comes after the focus the press moves, from the
    /// editor's tracking, which has the up then.
    func testAnUpTakenWhileTheDownBubblesWaitsForTheWindow() throws {
        let f = try make()
        try press(f, "a")
        clear(f)
        let inner = try XCTUnwrap(try f.node("z").superview as? NodeView)
        let forward = f.presenter.onPointer
        var taken = false
        f.presenter.onPointer = { id, kind, sample in
            forward?(id, kind, sample)
            guard kind == .down, id == inner.id, !taken, let up = NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) else { return }
            taken = true
            NSApp.sendEvent(up)
        }
        defer { f.presenter.onPointer = forward }
        try press(f, "z")
        XCTAssertTrue(taken)
        XCTAssertEqual(heard(f), ["down-z", "down-zo", "blur-a", "focus-z", "up-z", "up-zo"])
        XCTAssertNil(f.presenter.mouseChain.early)
        XCTAssertNil(f.presenter.pointerHeld)
        XCTAssertNil(NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true), "no up left over")
    }

    /// A click in another editor while the early down still bubbles (in its
    /// handler's loop) leaves the first press alone: it is released by its
    /// own up, after its downs and the focus it moves.
    func testAClickWhileTheDownBubblesLeavesThePressAlone() throws {
        let f = try make()
        try press(f, "a")
        clear(f)
        let inner = try XCTUnwrap(try f.node("z").superview as? NodeView)
        let other = try f.node("c")
        let forward = f.presenter.onPointer
        var clicked = false
        f.presenter.onPointer = { id, kind, sample in
            forward?(id, kind, sample)
            guard kind == .down, id == inner.id, !clicked else { return }
            clicked = true
            let at = other.convert(NSPoint(x: 4, y: 6), to: nil)
            NSApp.sendEvent(NSEvent.mouseEvent(with: .leftMouseDown, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: f.window.windowNumber, context: nil, eventNumber: 2, clickCount: 1, pressure: 1)!)
        }
        defer { f.presenter.onPointer = forward }
        try press(f, "z")
        XCTAssertTrue(clicked)
        let log = heard(f).filter { $0.hasPrefix("down-z") || $0.hasPrefix("up-z") || $0 == "focus-z" }
        XCTAssertEqual(log, ["down-z", "down-zo", "focus-z", "up-z", "up-zo"])
        XCTAssertNil(f.presenter.mouseChain.early)
    }

    /// The same, the inner `pointerdown` taking the editor out of its
    /// window first: the release still waits for the down to bubble.
    func testAnUpAfterTheEditorLeftWhileTheDownBubblesWaitsToo() throws {
        let f = try make()
        clear(f)
        let inner = try XCTUnwrap(try f.node("z").superview as? NodeView)
        let forward = f.presenter.onPointer
        var taken = false
        f.presenter.onPointer = { id, kind, sample in
            forward?(id, kind, sample)
            guard kind == .down, id == inner.id, !taken, let up = NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) else { return }
            taken = true
            inner.removeFromSuperview()
            NSApp.sendEvent(up)
        }
        defer { f.presenter.onPointer = forward }
        try press(f, "z")
        XCTAssertTrue(taken)
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-z", "down-zo", "up-z", "up-zo"])
        XCTAssertNil(f.presenter.mouseChain.early)
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// A second contact's up while the first's down still bubbles (the
    /// handler's loop open for another click) is that contact's, not held,
    /// and does not release the first, its editor gone from the window or
    /// not.
    func testOnlyThePresssOwnUpIsHeld() throws {
        for detach in [false, true] {
            let f = try make()
            clear(f)
            let inner = try XCTUnwrap(try f.node("z").superview as? NodeView)
            let forward = f.presenter.onPointer
            var held: [Bool] = []
            f.presenter.onPointer = { id, kind, sample in
                forward?(id, kind, sample)
                guard kind == .down, id == inner.id, held.isEmpty, let up = NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) else { return }
                if detach { inner.removeFromSuperview() }
                held.append(f.presenter.releaseEarlyPress(up))
                NSApp.sendEvent(up) // another contact's, through the monitor
                held.append(f.presenter.mouseChain.heldUp === up)
            }
            try press(f, "z")
            f.presenter.onPointer = forward
            XCTAssertEqual(held, [true, true], "detach \(detach): the first up is held, and stays the held one")
            XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-z", "down-zo", "up-z", "up-zo"], "detach \(detach)")
            XCTAssertNil(f.presenter.pointerHeld)
            fixture?.close(); fixture = nil
        }
    }

    /// A window under an alert's sheet takes no click: nothing is reported
    /// early for an editor behind it.
    func testNoEarlyDownBehindAnAlert() throws {
        let f = try make()
        try f.press("ask-open")
        XCTAssertNotNil(f.window.attachedSheet, "the alert's sheet")
        clear(f)
        try press(f, "b")
        XCTAssertEqual(heard(f), [])
        XCTAssertNil(f.presenter.mouseChain.early)
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// An early press whose view leaves the window before the button comes
    /// up, its monitor with it: released as it goes.
    func testAnEarlyPressIsReleasedWhenTheViewLeavesItsWindow() throws {
        let f = try make()
        clear(f)
        let node = try f.node("b")
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: node.convert(NSPoint(x: 4, y: 6), to: nil), modifierFlags: [],
                                      timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber, context: nil,
                                      eventNumber: 1, clickCount: 1, pressure: 1)!
        f.presenter.pressEditorFirst(down)
        f.settle()
        XCTAssertEqual(heard(f), ["down-b"], "the monitor's early down, no tracking yet")
        f.window.contentView = NSView()
        f.settle()
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-b", "up-b"], "the focused field's blur aside")
        XCTAssertNil(f.presenter.mouseChain.early)
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// A `pointerdown` that opens a `closedby="any"` dialog, whose sheet
    /// takes the clicks on the window behind it: the press is released at
    /// its up all the same, and the next press is reported.
    func testADownThatOpensALightDismissDialogStillReleases() throws {
        let f = try make()
        clear(f)
        try press(f, "w")
        XCTAssertEqual(heard(f).first, "down-w")
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-w", "up-w"])
        XCTAssertNil(f.presenter.pointerHeld)
        XCTAssertNil(f.presenter.mouseChain.early)
        f.presenter.dialogs.reset()
        f.settle()
        clear(f)
        try press(f, "b")
        XCTAssertEqual(heard(f).filter { $0.hasPrefix("down") || $0.hasPrefix("up") }, ["down-b", "up-b"])
    }

    /// Condition 1: a control-click, AppKit's contextual click, takes no
    /// early down, so it goes the way it went: what AppKit makes of it (a
    /// contextual menu) and the editor's own report, untouched. Asked of
    /// the monitor's step itself, as no menu is opened here: a real one
    /// would be on screen while other classes run (ContextMenuMacTests
    /// measures menu windows).
    func testAControlClickTakesNoEarlyDown() throws {
        let f = try make()
        try press(f, "a")
        for id in ["b", "t", "p"] {
            clear(f)
            let node = try f.node(id)
            for flags: NSEvent.ModifierFlags in [.control, [.control, .shift]] {
                let down = NSEvent.mouseEvent(with: .leftMouseDown, location: node.convert(NSPoint(x: 4, y: 6), to: nil), modifierFlags: flags,
                                              timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber, context: nil,
                                              eventNumber: 1, clickCount: 1, pressure: 1)!
                f.presenter.pressEditorFirst(down)
                f.settle()
                XCTAssertEqual(heard(f), [], "\(id) \(flags)")
                XCTAssertNil(f.presenter.mouseChain.early, id)
                XCTAssertNil(f.presenter.pointerHeld, id)
            }
            // The same press unmodified is early: the control key alone decides.
            let plain = NSEvent.mouseEvent(with: .leftMouseDown, location: node.convert(NSPoint(x: 4, y: 6), to: nil), modifierFlags: [],
                                           timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber, context: nil,
                                           eventNumber: 1, clickCount: 1, pressure: 1)!
            f.presenter.pressEditorFirst(plain)
            f.settle()
            XCTAssertEqual(heard(f), ["down-\(id)"], id)
            f.presenter.releaseEarlyPress(nil)
            f.settle()
            XCTAssertNil(f.presenter.pointerHeld, id)
        }
    }

    /// Condition 3: a press in a window the session does not host is not
    /// the session's.
    func testAWindowTheSessionDoesNotHostIsUntouched() throws {
        let f = try make()
        clear(f)
        let other = NSWindow(contentRect: NSRect(x: 500, y: 0, width: 300, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        defer { other.close() }
        let field = NSTextField(frame: NSRect(x: 10, y: 40, width: 200, height: 24))
        other.contentView?.addSubview(field)
        other.orderFrontRegardless()
        XCTAssertFalse(f.presenter.hosts(other))
        let at = NSPoint(x: 20, y: 50)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: other.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!)
        }
        f.settle()
        XCTAssertEqual(heard(f), [])
        XCTAssertNil(f.presenter.mouseChain.early)
        XCTAssertNil(f.presenter.pointerHeld)
    }

    /// Test 5: a sheet's input and a popover's, in their own windows.
    func testSheetsAndPopoversReportTheDownFirst() throws {
        let f = try make()
        try f.press("open")
        let s1 = try f.node("s1")
        XCTAssertTrue(try XCTUnwrap(s1.window).isSheet)
        try press(f, "s1")
        clear(f)
        try press(f, "s2")
        XCTAssertEqual(heard(f), ["down-s2", "blur-s1", "focus-s2", "up-s2"])
        f.presenter.dialogs.reset()
        f.settle()
        try f.press("pop")
        XCTAssertTrue(f.presenter.menus.hosts(try XCTUnwrap(try f.node("o1").window)))
        try press(f, "o1")
        clear(f)
        try press(f, "o2")
        XCTAssertEqual(heard(f), ["down-o2", "blur-o1", "focus-o2", "up-o2"])
    }

    /// The agent's tap comes through the application as a hand's does.
    func testAnAgentTapReportsTheDownFirst() throws {
        let f = try make()
        try press(f, "a")
        clear(f)
        let reply = Agent(session: f.session).tap(["id": Int(try f.node("b").id), "mouse": true, "at": [4.0, 6.0]])
        XCTAssertNil(reply["error"], "\(reply)")
        f.settle()
        XCTAssertEqual(heard(f), ["down-b", "blur-a", "focus-b", "up-b"])
    }
}
#endif
