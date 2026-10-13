// @ref LLP 1115.002 D7.3 — a popover that is not a menu is an `NSPopover`.
// In a whole session (`SheetFixture`'s page): the popover's own subtree is in
// AppKit's popover, laid out there by the kernel at its content's size;
// `auto` is transient and `manual` application-defined; its edge is its
// `position-area`; the page's rules still close it (Escape, a click
// elsewhere, its invoker's toggle, its own hide button) and so does AppKit;
// it is a Tab loop of its own. `PopoverMacTests` holds the painted popover,
// which the driver's own chrome keeps, and the menu.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PopoverNativeMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }
    private func make() throws -> SheetFixture {
        let f = try SheetFixture()
        fixture = f
        // On the screen (AppKit shows no popover in a window that is not), with room on every
        // side of it to put a popover where it was asked.
        f.window.setFrameOrigin(NSPoint(x: 200, y: 120))
        f.window.orderFrontRegardless()
        f.settle()
        return f
    }
    private func escape(in window: NSWindow) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
    }
    private func release(in window: NSWindow) -> NSEvent {
        NSEvent.keyEvent(with: .keyUp, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
    }
    private func click(_ p: Presenter, on node: NodeView) {
        let point = node.convert(NSPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            p.menus.pointer(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0, windowNumber: node.window!.windowNumber,
                                               context: nil, eventNumber: 5, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!)
        }
    }
    /// AppKit takes a popover away in its own time; its subtree goes home when it has.
    private func gone(_ native: NativePopover) {
        let limit = Date().addingTimeInterval(3)
        while native.popover.isShown, Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertFalse(native.popover.isShown)
    }
    private func onScreen(_ view: NSView) -> NSRect { view.window!.convertToScreen(view.convert(view.bounds, to: nil)) }

    func testDynamicFallbacksChangePresentationWithoutClosingOrLosingFocus() throws {
        let f = try SheetFixture(page: """
        component Proof
          state flips = "unset"
          action enable
            flips = "flip-block, flip-inline, flip-block flip-inline"
          action reset
            flips = "unset"
          view
            column width="100%" height="100%" box-sizing="border-box" padding=8 justify-content="flex-end" align-items="flex-end"
              button testId="open" popovertarget="pop" width=80 height=32 appearance="none"
                text "Open"
              column id="pop" testId="pop" popover="auto" position="absolute" position-area="bottom span-right" position-try-fallbacks=flips width=322 height=122 box-sizing="border-box" margin=0 padding=0
                input testId="field" value="draft text" width=200 height=24
                button testId="enable" press=enable
                  text "Use flips"
                button testId="reset" press=reset
                  text "Use platform"
        """, size: CGSize(width: 420, height: 600))
        fixture = f
        f.window.setFrameOrigin(NSPoint(x: 200, y: 120))
        f.window.makeKeyAndOrderFront(nil)
        f.settle()
        let p = f.presenter, pop = try f.node("pop"), home = pop.superview
        try f.press("open")
        let entry = try XCTUnwrap(p.menus.entries.first)
        let native = try XCTUnwrap(entry.native)
        XCTAssertNil(pop.style["position_try_fallbacks"])
        let field = try XCTUnwrap(try f.node("field").field)
        XCTAssertTrue(try XCTUnwrap(native.window).makeFirstResponder(field))
        let selection = NSRange(location: 2, length: 4)
        try XCTUnwrap(field.currentEditor() as? NSTextView).setSelectedRange(selection)
        var events: [String] = []
        pop.handlers.insert("toggle")
        p.onClipboard = { _, kind, payload in if kind == 50 { events.append(payload) } }

        try f.press("enable")
        XCTAssertTrue(p.menus.isOpen(pop) && p.menus.entries.first === entry)
        XCTAssertNil(entry.native, "an authored fallback row must take effect while open")
        XCTAssertTrue(pop.window === f.window && pop.superview === entry.layer)
        XCTAssertEqual(pop.frame, NSRect(x: 90, y: 438, width: 322, height: 122))
        XCTAssertFalse(pop.isHidden)
        XCTAssertEqual(try XCTUnwrap(field.currentEditor() as? NSTextView).selectedRange(), selection)
        XCTAssertTrue((f.window.firstResponder as? NSTextView)?.delegate === field)
        XCTAssertEqual(field.stringValue, "draft text")
        XCTAssertNil(p.blocks.used[pop.id])

        try f.press("reset")
        let restored = try XCTUnwrap(entry.native, "removing the row restores platform presentation while open")
        XCTAssertTrue(restored !== native && restored.popover.isShown)
        XCTAssertTrue(pop.superview === restored.content && p.menus.isOpen(pop))
        XCTAssertEqual(try XCTUnwrap(field.currentEditor() as? NSTextView).selectedRange(), selection)
        XCTAssertTrue((restored.window?.firstResponder as? NSTextView)?.delegate === field)
        XCTAssertEqual(p.menus.presentation(of: pop), 1)
        XCTAssertTrue(events.isEmpty, "changing presentation emits no close/open toggle")
        restored.popover.animates = false
        try f.press("open")
        gone(restored)
        XCTAssertTrue(pop.isHidden && pop.superview === home)
        XCTAssertTrue(p.menus.windowed.isEmpty && p.menus.leaving.isEmpty)
    }

    /// The subtree in AppKit's popover, at the size layout returns, below
    /// its invoker; the page laid out as it was; and each way it closes.
    func testAPopoverThatIsNoMenuIsAppKitsPopoverAtItsContentsSize() throws {
        let f = try make(), p = f.presenter
        let pop = try f.node("tags-pop"), source = try f.node("tags"), home = pop.superview
        let behind = try ["page", "page-field", "tags", "status"].map(f.node)
        let pageFrames = behind.map(\.frame)
        XCTAssertFalse(p.menus.isMenuShaped(pop), "a field is no menu row")

        try f.press("tags")
        let native = try XCTUnwrap(p.menus.windowed.first)
        XCTAssertTrue(p.menus.isOpen(pop) && native.popover.isShown)
        XCTAssertEqual(native.popover.behavior, .transient, "popover=auto")
        XCTAssertEqual(p.menus.observation?["presentation"] as? String, "NSPopover")
        XCTAssertTrue(pop.superview === native.content && !pop.isHidden)
        // 260 as written and its padding beside it; its height its content's.
        let size = try XCTUnwrap(p.blocks.used[pop.id])
        XCTAssertEqual(size.width, 292)
        XCTAssertEqual(native.popover.contentSize, size, "the popover's content is the size layout returned")
        XCTAssertEqual(pop.frame, NSRect(origin: .zero, size: size), "its frame is in the popover's own space")
        let window = try XCTUnwrap(native.window)
        XCTAssertTrue(p.hosts(window) && window !== f.window)
        XCTAssertEqual(window.frame.size, CGSize(width: size.width + 26, height: size.height + 26), "AppKit's own 13 points a side")
        let anchor = onScreen(source)
        XCTAssertEqual(window.frame.maxY, anchor.minY, accuracy: 2, "below its invoker, where no position-area says otherwise")
        XCTAssertEqual(window.frame.midX, anchor.midX, accuracy: 2)
        XCTAssertEqual(behind.map(\.frame), pageFrames, "the page is laid out as it was")
        // Its field is the authored input.
        let field = try XCTUnwrap(try f.node("tag").field)
        XCTAssertTrue(field.window === window)
        XCTAssertFalse(try f.node("tag").inert)

        // Escape, as the session routes a key from the popover's own window.
        XCTAssertTrue(p.routeKey(escape(in: window), focused: true))
        _ = p.menus.key(release(in: window))
        XCTAssertFalse(p.menus.isOpen(pop))
        XCTAssertTrue(pop.isHidden)
        gone(native)
        XCTAssertTrue(pop.superview === home, "back under its authored parent")
        XCTAssertNil(p.blocks.used[pop.id])
        XCTAssertFalse(p.hosts(window))

        // Its invoker toggles it; a click elsewhere on the page closes it.
        try f.press("tags")
        let second = try XCTUnwrap(p.menus.windowed.first)
        try f.press("tags")
        XCTAssertFalse(p.menus.isOpen(pop), "the invoker's second press closes it")
        gone(second)
        try f.press("tags")
        let third = try XCTUnwrap(p.menus.windowed.first)
        click(p, on: try f.node("tag"))
        XCTAssertTrue(p.menus.isOpen(pop), "a click inside it keeps it")
        click(p, on: try f.node("status"))
        XCTAssertFalse(p.menus.isOpen(pop), "a click on the page closes it")
        gone(third)
        // Its own hide button.
        try f.press("tags")
        let fourth = try XCTUnwrap(p.menus.windowed.first)
        try f.press("done")
        XCTAssertFalse(p.menus.isOpen(pop))
        gone(fourth)
        // AppKit closes it itself (a click in another app, the app left): the page's popover closes with it.
        try f.press("tags")
        let fifth = try XCTUnwrap(p.menus.windowed.first)
        fifth.popover.close()
        gone(fifth)
        f.settle()
        XCTAssertFalse(p.menus.isOpen(pop), "AppKit's close is the page's")
        XCTAssertTrue(pop.superview === home && pop.isHidden)
        XCTAssertTrue(p.menus.windowed.isEmpty && p.menus.leaving.isEmpty)
    }

    /// `popover="manual"` is application-defined: Escape and a click
    /// elsewhere leave it, and its own button closes it. `position-area="top
    /// …"` puts it above its invoker, its margin between them.
    func testAManualPopoverStaysAndItsEdgeIsItsPositionArea() throws {
        let f = try make(), p = f.presenter
        let pop = try f.node("pinned-pop"), source = try f.node("pinned")
        try f.press("pinned")
        let native = try XCTUnwrap(p.menus.windowed.first)
        XCTAssertEqual(native.popover.behavior, .applicationDefined, "popover=manual")
        let window = try XCTUnwrap(native.window), anchor = onScreen(source)
        XCTAssertEqual(window.frame.minY, anchor.maxY + 12, accuracy: 2, "above its invoker, its margin-bottom between them")
        XCTAssertEqual(window.frame.midX, anchor.midX, accuracy: 2)
        XCTAssertEqual(native.popover.contentSize, pop.frame.size, "the popover is its border box: the margin is outside it")
        XCTAssertEqual(pop.frame.origin, .zero)
        XCTAssertFalse(p.menus.key(escape(in: window)), "Escape is not a manual popover's")
        click(p, on: try f.node("status"))
        XCTAssertTrue(p.menus.isOpen(pop), "nor is a click elsewhere")
        // An auto popover opens beside it and closes without it.
        try f.press("tags")
        XCTAssertEqual(p.menus.windowed.count, 2)
        click(p, on: try f.node("status"))
        let tags = try f.node("tags-pop")
        XCTAssertTrue(p.menus.isOpen(pop) && !p.menus.isOpen(tags))
        try f.press("unpin")
        XCTAssertFalse(p.menus.isOpen(pop))
        gone(native)
        // The edges a position-area names, in a node's flipped space.
        XCTAssertEqual(MenuHost.edge("none"), .maxY)
        XCTAssertEqual(MenuHost.edge("bottom span-right"), .maxY)
        XCTAssertEqual(MenuHost.edge("top span-all"), .minY)
        XCTAssertEqual(MenuHost.edge("right span-bottom"), .maxX)
    }

    /// Each popover in a window of its own is a Tab loop of its own, as an
    /// `NSPopover`'s is, and no stop of the page's; the driver's own chrome
    /// paints the same popover in the page's top layer.
    func testThePopoverIsATabLoopOfItsOwnAndTheDriversChromePaintsIt() throws {
        KeyboardNavigation.agent = false
        addTeardownBlock { KeyboardNavigation.agent = nil }
        let f = try make(), p = f.presenter
        let pop = try f.node("tags-pop")
        try f.press("tags")
        let native = try XCTUnwrap(p.menus.windowed.first)
        p.flushKeyViewLoop()
        let field = try XCTUnwrap(try f.node("tag").field), pageField = try XCTUnwrap(try f.node("page-field").field)
        XCTAssertTrue(native.content.nextKeyView === field, "its loop starts at its first stop")
        XCTAssertTrue(field.nextKeyView === field, "and goes round inside it")
        XCTAssertTrue(pageField.nextKeyView === pageField, "the page's loop does not go through it")
        XCTAssertTrue(p.menus.following(try f.node("tags")).isEmpty)
        try f.press("tags")
        gone(native)
        // Painted, as the driver shows it by default: in the viewport, no window of its own.
        p.menus.painted = true
        try f.press("tags")
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(p.menus.windowed.isEmpty)
        XCTAssertTrue(pop.window === f.window && pop.isDescendant(of: p.viewport))
        XCTAssertNil(p.menus.observation?["presentation"])
        XCTAssertEqual(p.menus.following(try f.node("tags")).map(\.id), [pop.id])
        try f.press("tags")
        XCTAssertFalse(p.menus.isOpen(pop))
        // AppKit shows no popover against a window that is not on screen: painted there too.
        p.menus.painted = false
        f.window.orderOut(nil)
        try f.press("tags")
        XCTAssertTrue(p.menus.isOpen(pop) && p.menus.windowed.isEmpty)
        XCTAssertTrue(pop.isDescendant(of: p.viewport))
        try f.press("tags")
    }
}
#endif
