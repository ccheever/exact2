#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 D3 on a right press: only a menu-shaped popover is the Mac's
/// menu. One that is not opens as a button's popover opens, anchored to its
/// node, and stays open through the batches after it.
final class ContextMenuShapeMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll() }

    /// A source (1) naming popover 2: Pin (4) and Delete (5), each a
    /// button whose label is a text (13, 14).
    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func text(_ id: Int, _ value: String) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "text", "props": ["text": value]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
        }
        func row(_ id: Int, _ label: Int) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "m", "popovertargetaction": "hide"]],
             ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 40], ["op": "children", "id": id, "ids": [label]]]
        }
        let ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "handlers": ["contextmenu", "pointerdown", "pointerup"], "props": ["contextPopover": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m", "accessibilityRole": "menu"]],
            ["op": "create", "id": 9, "kind": "view"],
        ] + text(13, "Pin") + row(4, 13) + text(14, "Delete") + row(5, 14)
            + [["op": "children", "id": 2, "ids": [4, 5]], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
               ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 100]]
        p.apply(wireBatch(ops))
        return p
    }

    /// A right press whose `pointerup` brings a batch, as an app's action
    /// does. The popover had been an NSMenu which that batch's `sync` closed
    /// before it tracked, so nothing opened (the T3 Code clone's thread
    /// details card, whose rows drew a check icon beside their labels).
    func testARightPressOpensAContextPopoverThatIsNoMenuAsAPopover() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent presents painted popovers")
        for shaped in [false, true] {
            let p = presenter(), window = try XCTUnwrap(windows.last)
            let source = try XCTUnwrap(p.views[1]), pop = try XCTUnwrap(p.views[2])
            defer { p.menus.reset() }
            if !shaped {
                // Pin's row gains an icon box beside its label.
                p.apply(wireBatch([["op": "create", "id": 30, "kind": "view"], ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 16, "h": 16],
                                   ["op": "children", "id": 4, "ids": [30, 13]]]))
            }
            XCTAssertEqual(p.menus.isMenuShaped(pop), shaped)
            window.orderFrontRegardless()
            var tracked = false, released = false, finished = false
            p.onPointer = { _, phase, _ in
                guard phase == .up else { return }
                released = true
                p.apply(wireBatch([["op": "props", "id": 13, "set": ["text": "Pin chat"]]]))
            }
            let center = NotificationCenter.default
            let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
                guard let menu = note.object as? NSMenu else { return }
                tracked = true
                RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
            }
            defer { center.removeObserver(observer) }
            let down = try XCTUnwrap(NSEvent.mouseEvent(with: .rightMouseDown, location: source.convert(NSPoint(x: 10, y: 10), to: nil),
                modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil,
                eventNumber: 1, clickCount: 1, pressure: 1))
            source.rightMouseDown(with: down)
            RunLoop.main.perform(inModes: [.common]) { finished = true }
            let deadline = Date(timeIntervalSinceNow: 5)
            while !(finished && (!shaped || tracked)) && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
            XCTAssertTrue(finished)
            if shaped {
                XCTAssertTrue(tracked, "a menu-shaped popover is still the Mac's menu")
                XCTAssertTrue(released, "its tracking ends the press")
            } else {
                XCTAssertFalse(tracked, "no native menu")
                XCTAssertTrue(p.menus.isOpen(pop), "the popover opened")
                p.apply(wireBatch([["op": "props", "id": 14, "set": ["text": "Delete chat"]]]))
                XCTAssertTrue(p.menus.isOpen(pop), "a later batch leaves it open")
            }
        }
    }
}
#endif
