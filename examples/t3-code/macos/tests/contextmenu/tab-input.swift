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
        field = NSTextField(frame: NSRect(x: 0, y: 0, width: 96, height: 20))
        window.contentView!.addSubview(field)
        install("tab-editor:device:test", node: 3, view: field)
        install("tab-cancel:device:test", node: 4, view: nil)
    }
    private var field: NSTextField!
    private func install(_ name: String, node: UInt32, view: NSView?) {
        let element = ExactElement(hatch: .t3Anchor, id: name, node: node, hatches: hooks)
        element.data = ExactData(["anchor": name]); element.view = view; element.platform = view
        elements.append(element); input.install(element)
    }
    override func tearDown() {
        input.destroy(); elements.removeAll(); window.close(); hooks = nil; table.deallocate()
    }
    private func key(_ code: UInt16, _ flags: NSEvent.ModifierFlags = [], _ characters: String = "") -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
                         characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
    }
    private func mouse(_ type: NSEvent.EventType) -> NSEvent {
        NSEvent.mouseEvent(with: type, location: NSPoint(x: 60, y: 12), modifierFlags: [], timestamp: 0,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
    func testEscapeInTheEditorPressesCancel() {
        window.makeFirstResponder(field)
        XCTAssertNil(input.handle(key(53, [], "\u{1B}")))
        XCTAssertEqual(tabActions.map { $0.0 }, [4])
    }
    func testAChordInTheEditorStaysInTheEditor() {
        // ⌘K is a window shortcut elsewhere; while renaming it goes to the field editor and no further.
        window.makeFirstResponder(field)
        XCTAssertNil(input.handle(key(40, .command, "k")))
        XCTAssertTrue(tabActions.isEmpty)
        XCTAssertNotNil(input.handle(key(0, [], "a")), "Typing reaches the field.")
    }
    func testKeysOutsideTheEditorAndMiceGoOn() {
        // The middle click is the tab's own pointerdown/pointerup in Contract (panel.contract PanelTabChip).
        XCTAssertNotNil(input.handle(key(53, [], "\u{1B}")))
        XCTAssertNotNil(input.handle(mouse(.leftMouseDown)))
        XCTAssertTrue(tabActions.isEmpty)
    }
}
