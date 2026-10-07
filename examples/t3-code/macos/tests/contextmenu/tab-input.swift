import AppKit
import XCTest

private var tabActions: [(UInt32, UInt32)] = []
private let resolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let act: ExactHatches.ActFn = { _, node, action in tabActions.append((node, action)); return 1 }
private let log: ExactHatches.LogFn = { _, _, _ in }
private let delegate: ExactHatches.DelegateFn = { _, _, _ in }

final class TabInputTests: XCTestCase {
    private var table: UnsafeMutableRawPointer!
    private var hooks: ExactHatches!
    private var window: NSWindow!
    private var input: RightPanelTabsInput!
    private var elements: [ExactElement] = []
    override func setUp() {
        _ = NSApplication.shared
        tabActions = []
        table = .allocate(byteCount: 40, alignment: 8)
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(act, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHatches(host: nil, table: UnsafeRawPointer(table))!
        window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 300, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        input = RightPanelTabsInput()
        let row = NSView(frame: NSRect(x: 0, y: 0, width: 140, height: 24))
        window.contentView!.addSubview(row)
        install("r12-tab:device:test", node: 1, view: row)
        let close = NSButton(frame: NSRect(x: 0, y: 0, width: 16, height: 24))
        row.addSubview(close)
        install("tab-close:device:test", node: 2, view: close)
        install("tab-cancel:device:test", node: 4, view: nil)
    }
    private func install(_ name: String, node: UInt32, view: NSView?) {
        let element = ExactElement(hatch: .t3Anchor, id: name, node: node, hatches: hooks)
        element.data = ExactData(["anchor": name]); element.view = view
        elements.append(element); input.install(element)
    }
    override func tearDown() {
        input.destroy(); elements.removeAll(); window.close(); hooks = nil; table.deallocate()
    }
    private func mouse(_ type: NSEvent.EventType, count: Int = 1, x: CGFloat = 60) -> NSEvent {
        let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: 12), modifierFlags: [], timestamp: 0,
                          windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: count, pressure: 1)!
        guard type == .otherMouseDown, let cg = event.cgEvent else { return event }
        // NSEvent's convenience constructor always sets button zero. Round-trip
        // through CG to produce the real middle-button number, preserving location.
        cg.setIntegerValueField(.mouseEventButtonNumber, value: 2)
        let converted = NSEvent(cgEvent: cg)!
        cg.location = CGPoint(x: cg.location.x + x - converted.locationInWindow.x,
                              y: cg.location.y - (12 - converted.locationInWindow.y))
        return NSEvent(cgEvent: cg)!
    }
    func testMiddleClickClosesExactlyOneTabAndConsumesEvent() {
        XCTAssertNil(input.handle(mouse(.otherMouseDown)))
        XCTAssertEqual(tabActions.map { $0.0 }, [2])
    }
    func testMiddleClickClosesTheTabUnderThePointerAmongSeveral() {
        // A second tab beside the first: a middle click closes the one it is on, not the first
        // the monitor lists (adopt-main-fixes-r3, found by the agent's `auxclick`, exact2 #186).
        let row = NSView(frame: NSRect(x: 150, y: 0, width: 100, height: 24))
        window.contentView!.addSubview(row)
        install("r12-tab:files", node: 5, view: row)
        let close = NSButton(frame: NSRect(x: 0, y: 0, width: 16, height: 24))
        row.addSubview(close)
        install("tab-close:files", node: 6, view: close)
        XCTAssertNil(input.handle(mouse(.otherMouseDown, x: 200)))
        XCTAssertEqual(tabActions.map { $0.0 }, [6])
        tabActions = []
        XCTAssertNil(input.handle(mouse(.otherMouseDown, x: 60)))
        XCTAssertEqual(tabActions.map { $0.0 }, [2])
        tabActions = []
        XCTAssertNotNil(input.handle(mouse(.otherMouseDown, x: 145)), "Between the tabs nothing closes.")
        XCTAssertTrue(tabActions.isEmpty)
    }
    func testDoubleClickIsLeftToTheTitleButtonsOwnDblclick() {
        // r4-surfaces.contract R4TabChip: the title button's `dblclick` starts rename, so the
        // agent's `tap … dblclick` and a person's reach the same path; the monitor passes it on.
        XCTAssertNotNil(input.handle(mouse(.leftMouseDown, count: 2)))
        XCTAssertTrue(tabActions.isEmpty)
    }
    func testOrdinaryClickAndRemovedTabsDoNotDispatch() {
        XCTAssertNotNil(input.handle(mouse(.leftMouseDown)))
        input.remove(elements[0])
        XCTAssertNotNil(input.handle(mouse(.otherMouseDown)))
        XCTAssertTrue(tabActions.isEmpty)
    }
    func testShiftF10PassesToTheTabsOwnKeyHandler() {
        // The tab's buttons answer Shift+F10 in Contract (r4-surfaces.contract R4TabChip tabKey); the monitor leaves it alone.
        let button = NSButton(frame: NSRect(x: 24, y: 0, width: 90, height: 24))
        elements[0].view!.addSubview(button)
        window.makeFirstResponder(button)
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .shift, timestamp: 0,
                                    windowNumber: window.windowNumber, context: nil, characters: "\u{F70D}", charactersIgnoringModifiers: "\u{F70D}", isARepeat: false, keyCode: 109)!
        XCTAssertNotNil(input.handle(event))
        XCTAssertTrue(tabActions.isEmpty)
    }
}
