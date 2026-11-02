#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class InlineFocusMacTests: XCTestCase {
    private final class Links: ExactSessionDelegate {
        var opened: [String] = []
        func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
            if name == "openURL", let url = args.first as? String { opened.append(url) }
        }
    }

    private func fixture(dialog: Bool = false, painted: Bool = true) -> (ExactSession, NSWindow) {
        _ = NSApplication.shared
        keyboardNavigation(true)
        let session = ExactApp.shared.makeSession(label: "inline-keys")
        let p = session.presenter
        p.dialogs.painted = painted
        p.buttonFace = { id in
            var face = ButtonFace(); face.title = id == 2 ? "OK" : "Cancel"
            if id == 2 { face.style = "filled"; face.macos = "push-accent" }
            return face
        }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        func run(_ id: Int, _ props: [String: String], _ handlers: [String]) -> [String: Any] {
            ["id": id, "parent": 3, "paint": true, "props": props.merging(["text": "run "]) { $1 },
             "handlers": handlers, "style": ["font_size": 18.0]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": dialog ? ["semanticTag": "dialog", "id": "modal"] : [:]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button"], "handlers": ["press"], "style": ["appearance": "auto"]],
            ["op": "create", "id": 3, "kind": "text"],
            ["op": "paragraph", "id": 3, "runs": [
                run(5, ["href": "https://example.com/both"], ["press"]),
                run(6, [:], ["press"]), run(7, ["href": "https://example.com/link"], []),
                run(9, ["href": "https://example.com/disabled", "disabled": "true", "tabIndex": "0"], ["press"])]],
            ["op": "create", "id": 8, "kind": "control", "props": ["type": "button"], "handlers": ["press"], "style": ["appearance": "auto"]],
            ["op": "children", "id": 1, "ids": [2, 3, 8]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 100, "h": 30],
            ["op": "frame", "id": 3, "x": 10, "y": 50, "w": 280, "h": 100],
            ["op": "frame", "id": 8, "x": 10, "y": 210, "w": 100, "h": 30],
        ]))
        if dialog { p.dialogs.show(p.views[1]!) }
        return (session, window)
    }

    func testEnterAndSpaceOnLinkPressAndCombinedRuns() throws {
        let (session, window) = fixture()
        defer { window.makeFirstResponder(nil); window.close(); session.destroy() }
        let links = Links(); session.delegate = links
        var pressed: [UInt32] = []
        session.presenter.onPress = { pressed.append($0) }
        let agent = Agent(session: session)
        for id in [5, 6, 7] {
            pressed.removeAll(); links.opened.removeAll()
            _ = agent.type(["id": id, "key": "Enter"])
            XCTAssertEqual(pressed, id == 7 ? [] : [UInt32(id)], "Enter on run \(id)")
            XCTAssertEqual(links.opened, id == 7 ? ["https://example.com/link"] : [])
            pressed.removeAll(); links.opened.removeAll()
            _ = agent.type(["id": id, "key": "Space"])
            XCTAssertEqual(pressed, id == 6 ? [6] : [], "Space only presses the run without href")
            XCTAssertTrue(links.opened.isEmpty, "Space never follows a link")
        }
    }

    func testDisabledLinkRemainsATabStop() throws {
        let (session, window) = fixture()
        defer { window.makeFirstResponder(nil); window.close(); session.destroy() }
        let agent = Agent(session: session)
        _ = agent.type(["id": 7, "key": "Tab"])
        XCTAssertEqual((window.firstResponder as? InlineKeyView)?.id, 9)
        _ = agent.type(["id": 8, "key": "Shift+Tab"])
        XCTAssertEqual((window.firstResponder as? InlineKeyView)?.id, 9)
        var pressed: [UInt32] = []
        session.presenter.onPress = { pressed.append($0) }
        _ = agent.type(["id": 9, "key": "Enter"])
        XCTAssertTrue(pressed.isEmpty, "focus eligibility does not grant activation")
    }

    func testDialogTabAndReturnUseTheFocusedRunInPaintedAndSheetPresentations() throws {
        for painted in [true, false] {
            let (session, window) = fixture(dialog: true, painted: painted)
            let p = session.presenter
            defer { p.dialogs.reset(); window.makeFirstResponder(nil); window.close(); session.destroy() }
            let front = try XCTUnwrap(p.dialogs.window)
            XCTAssertNotNil(p.dialogs.defaultButton(of: try XCTUnwrap(p.views[1])))
            let agent = Agent(session: session)
            _ = agent.type(["id": 2, "key": "Tab"])
            XCTAssertEqual((front.firstResponder as? InlineKeyView)?.id, 5)
            for (id, next) in [(5, 6), (6, 7), (7, 9)] {
                _ = agent.type(["id": id, "key": "Tab"])
                XCTAssertEqual((front.firstResponder as? InlineKeyView)?.id, UInt32(next))
            }
            _ = agent.type(["id": 8, "key": "Shift+Tab"])
            XCTAssertEqual((front.firstResponder as? InlineKeyView)?.id, 9)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            _ = agent.type(["id": 5, "key": "Enter"])
            XCTAssertEqual(pressed, [5], "the focused link precedes the dialog default")
            let links = Links(); session.delegate = links
            _ = agent.type(["id": 7, "key": "Enter"])
            XCTAssertEqual(links.opened, ["https://example.com/link"])
            XCTAssertEqual(pressed, [5])
        }
    }

    func testTabReachesWrappedInlineLinksAndPressRunsInBothDirections() throws {
        _ = NSApplication.shared
        keyboardNavigation(true)
        let session = ExactApp.shared.makeSession(label: "inline-focus")
        defer { session.destroy() }
        let p = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.makeFirstResponder(nil); window.close() }
        func run(_ id: Int, _ text: String, _ props: [String: String] = [:], _ handlers: [String] = []) -> [String: Any] {
            ["id": id, "parent": 3, "paint": true, "props": props.merging(["text": text]) { $1 },
             "handlers": handlers, "style": ["font_size": 18.0]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 3, "kind": "text", "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 3, "runs": [run(4, "Read "),
                run(5, "a link that wraps across lines", ["href": "https://example.com"], ["press", "focus", "blur", "contextmenu", "key"]),
                run(6, " then "), run(7, "press here", [:], ["press", "focus", "blur"])]],
            ["op": "create", "id": 8, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2, 3, 8]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 100, "h": 30],
            ["op": "frame", "id": 3, "x": 10, "y": 50, "w": 160, "h": 140],
            ["op": "frame", "id": 8, "x": 10, "y": 210, "w": 100, "h": 30],
        ]))
        let first = try XCTUnwrap(p.views[2]), paragraph = try XCTUnwrap(p.views[3]), last = try XCTUnwrap(p.views[8])
        XCTAssertGreaterThan(paragraph.inlineRects(try XCTUnwrap(p.inlineText(5))).count, 1)
        var focused: [UInt32] = [], blurred: [UInt32] = [], pressed: [UInt32] = [], menus: [UInt32] = []
        p.onFocus = { focused.append($0) }; p.onBlur = { blurred.append($0) }
        p.onPress = { pressed.append($0) }; p.onClipboard = { id, kind, _ in if kind == 10 { menus.append(id) } }
        _ = Agent(session: session).type(["id": 5, "key": "Enter"])
        XCTAssertEqual(pressed, [5], "the driver can address a run before the first Tab")
        pressed.removeAll(); focused.removeAll(); blurred.removeAll()
        p.syncKeyViewLoop()
        XCTAssertTrue(window.makeFirstResponder(first))
        blurred.removeAll()
        window.selectNextKeyView(nil)
        XCTAssertEqual(focused.last, 5, "Tab reaches the first run, not the following button")
        let link = try XCTUnwrap(window.firstResponder as? NSView)
        XCTAssertEqual((Agent(session: session).stateSections()["focus"] as? [String: Any])?["logical"] as? Int, 5)
        func key(_ code: UInt16, _ text: String) -> NSEvent {
            NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                            windowNumber: window.windowNumber, context: nil, characters: text,
                            charactersIgnoringModifiers: text, isARepeat: false, keyCode: code)!
        }
        var keys: [UInt32] = []
        p.onKey = { id, _ in keys.append(id) }
        _ = p.routeKey(key(36, "\r"), focused: true)
        XCTAssertEqual(keys, [5])
        link.keyDown(with: key(36, "\r"))
        XCTAssertEqual(pressed, [5])
        XCTAssertTrue(p.routeKey(key(0x6E, "\u{F735}"), focused: true))
        XCTAssertEqual(menus, [5])
        window.selectNextKeyView(nil)
        XCTAssertEqual(focused.last, 7, "a second interactive run has its own stop")
        window.selectNextKeyView(nil)
        XCTAssertTrue(window.firstResponder === last)
        window.selectPreviousKeyView(nil)
        XCTAssertEqual(focused.last, 7)
        window.selectPreviousKeyView(nil)
        XCTAssertTrue(window.firstResponder === link)
        XCTAssertEqual(blurred, [5, 7, 7])
        XCTAssertFalse(link.focusRingMaskBounds.isEmpty)
        XCTAssertTrue(p.selection.paragraphs.contains { $0 === paragraph }, "focus responders do not replace selectable text")
        keyboardNavigation(false)
        p.syncKeyViewLoop()
        XCTAssertFalse(link.canBecomeKeyView)
        XCTAssertNil(p.viewport.nextValidKeyView)
    }
}
#endif
