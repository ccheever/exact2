// @ref LLP 1115.002 D5 — stage 3a, the gate's questions that need a key
// window: a row's selection across its reuse and by the keyboard, the
// selection's drawing by the window's key state, and whose a click in a row
// is. The window is really brought to the front and made key (the earlier
// probes had none), and a click or a key is an `NSEvent` sent through
// `NSApplication.sendEvent`, the path a person's takes; the pointer is not
// moved. Skipped where this process cannot have a key window.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class TableFeedKeyMacTests: XCTestCase {
    private var fixture: TableFixture?
    private var other: NSWindow?
    override func tearDown() { fixture?.close(); fixture = nil; other?.close(); other = nil; super.tearDown() }

    /// Events from the application's queue, delivered, until `done` or the queue is empty after it.
    private func pump(_ seconds: Double = 2, until done: () -> Bool = { true }) {
        let limit = Date().addingTimeInterval(seconds)
        while Date() < limit {
            if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.01), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
            else if done() { return }
        }
    }
    /// The fixture's window at the front and key, with the table first responder.
    private func keyed(count: Int = 10_000) throws -> TableFixture {
        let f = try TableFixture(count: count)
        fixture = f
        NSApp.setActivationPolicy(.accessory)
        f.window.setFrameTopLeftPoint(NSPoint(x: 80, y: (NSScreen.main?.visibleFrame.maxY ?? 800) - 40))
        NSApp.activate(ignoringOtherApps: true)
        f.window.makeKeyAndOrderFront(nil)
        pump(3) { f.window.isKeyWindow && NSApp.isActive }
        try XCTSkipUnless(f.window.isKeyWindow && NSApp.isActive, "this process cannot bring a window to the front and make it key")
        XCTAssertTrue(f.window.makeFirstResponder(f.table))
        f.settle()
        return f
    }
    private func mouse(_ type: NSEvent.EventType, _ point: NSPoint, _ window: NSWindow) -> NSEvent {
        NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                           context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
    }
    /// A click in the middle of `view`: the button down, `held` runs while
    /// it is down, then up. A view that tracks the mouse in a loop of its
    /// own takes the up from the queue; any other is sent it.
    private func click(_ view: NSView, in f: TableFixture, held: @escaping () -> Void = {}) {
        let point = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        var up = false
        let timer = Timer(timeInterval: 0.05, repeats: false) { _ in
            held()
            NSApp.postEvent(self.mouse(.leftMouseUp, point, f.window), atStart: false)
            up = true
        }
        RunLoop.main.add(timer, forMode: .common)
        NSApp.sendEvent(mouse(.leftMouseDown, point, f.window))
        pump { up }
        f.settle()
    }
    private func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], in f: TableFixture) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            NSApp.sendEvent(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber,
                                             context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!)
        }
        f.settle()
    }
    private func down(in f: TableFixture) { key(125, "\u{F701}", [.function, .numericPad], in: f) }
    private func up(in f: TableFixture) { key(126, "\u{F700}", [.function, .numericPad], in: f) }
    /// The colour the table drew at the right of `row`, in the middle of its height.
    private func fill(_ row: Int, in f: TableFixture) throws -> NSColor {
        let view = try XCTUnwrap(f.table.rowView(atRow: row, makeIfNecessary: false), "row \(row) has a view")
        view.window?.displayIfNeeded()
        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: rep)
        return try XCTUnwrap(rep.colorAt(x: Int(view.bounds.width) - 14, y: Int(view.bounds.midY))?.usingColorSpace(.sRGB))
    }
    /// How far a colour is from grey: the accent's fill is far, the unemphasized one is grey.
    private func chroma(_ c: NSColor) -> CGFloat { max(c.redComponent, c.greenComponent, c.blueComponent) - min(c.redComponent, c.greenComponent, c.blueComponent) }

    func testTheSelectionStaysWithItsItemAcrossARowsReuseAndTheKeyboard() throws {
        let f = try keyed()
        // A click on a row: nothing while the button is down, as in a
        // hand-built table on macOS 27; selected, and the row's press, when it comes up.
        var whileDown = -2
        click(try f.node("title-12"), in: f) { whileDown = f.table.selectedRow }
        XCTAssertEqual(whileDown, -1, "a stock table selects when the mouse comes up")
        XCTAssertEqual(f.table.selectedRow, 12)
        XCTAssertTrue(f.status.hasPrefix("opened 12 presses 1 "), f.status)
        XCTAssertTrue(f.window.firstResponder === f.table)
        let first = try XCTUnwrap(f.cells[12]?.row)
        XCTAssertEqual(f.feed.selectedKey, first.props["listItemKey"])

        // Scrolled away: the runner destroys the row and the table lets its
        // view go. Back: another row for the same item, selected.
        f.scroll(to: 20_000)
        XCTAssertNil(f.presenter.views[first.id], "the runner destroyed the selected row")
        XCTAssertNil(f.table.rowView(atRow: 12, makeIfNecessary: false))
        XCTAssertEqual(f.table.selectedRow, 12)
        f.scroll(to: 0)
        let again = try XCTUnwrap(f.cells[12]?.row)
        XCTAssertFalse(again === first, "a new row for the item")
        XCTAssertEqual(f.item(again), 12)
        XCTAssertEqual(f.table.rowView(atRow: 12, makeIfNecessary: false)?.isSelected, true)
        XCTAssertTrue(f.status.hasPrefix("opened 12 presses 1 "), "coming back pressed nothing: \(f.status)")

        // The keyboard, a row at a time past everything that was mounted:
        // each row it reaches is mounted, in its cell and in the port.
        let were = Set(f.mounted.values.map(\.id))
        for step in 1...90 {
            down(in: f)
            let row = 12 + step
            XCTAssertEqual(f.table.selectedRow, row)
            let node = try XCTUnwrap(f.cells[row]?.row, "row \(row) is mounted and in its cell")
            XCTAssertEqual(f.item(node), row)
            XCTAssertTrue(f.clip.bounds.insetBy(dx: 0, dy: -0.5).contains(f.inDocument(node)), "row \(row) is in the port")
        }
        XCTAssertTrue(were.allSatisfy { f.presenter.views[$0] == nil } || f.mounted[12] == nil, "the rows it started among were destroyed on the way")
        XCTAssertNil(f.mounted[12])
        XCTAssertGreaterThan(f.clip.bounds.minY, 1_500)
        XCTAssertTrue(f.status.hasPrefix("opened 12 presses 1 "), "the arrow keys select and press nothing: \(f.status)")
        for _ in 1...90 { up(in: f) }
        XCTAssertEqual(f.table.selectedRow, 12)
        XCTAssertEqual(f.cells[12]?.row.map(f.item), 12)

        // Home and End are the list's, and leave the selection.
        key(119, "\u{F72B}", [.function], in: f)
        f.settle()
        XCTAssertGreaterThan(f.clip.bounds.minY, 300_000, "End is the list's end, as the runner has it")
        XCTAssertEqual(f.table.selectedRow, 12)
        key(115, "\u{F729}", [.function], in: f)
        XCTAssertEqual(f.clip.bounds.minY, 0)

        // A row put before it: the selection is the item's, one row down.
        f.presenter.press(try f.node("prepend").id)
        f.settle()
        XCTAssertEqual(f.table.numberOfRows, 10_001)
        XCTAssertEqual(f.table.selectedRow, 13, "the selection moved with its item")
        XCTAssertEqual(f.cells[13]?.row.map(f.item), 12)
    }

    func testTheSelectionIsDrawnByTheWindowsKeyState() throws {
        let f = try keyed()
        click(try f.node("title-4"), in: f)
        XCTAssertEqual(f.table.selectedRow, 4)
        let view = try XCTUnwrap(f.table.rowView(atRow: 4, makeIfNecessary: false))
        XCTAssertTrue(view.isSelected && view.isEmphasized, "key, and the table first responder")
        let emphasized = try fill(4, in: f)
        XCTAssertGreaterThan(chroma(emphasized), 0.4, "the emphasized selection is the accent's: \(emphasized)")
        // Another window takes key: the same row, unemphasized.
        let other = NSWindow(contentRect: NSRect(x: 700, y: 300, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        self.other = other
        other.makeKeyAndOrderFront(nil)
        pump { other.isKeyWindow }
        XCTAssertFalse(f.window.isKeyWindow)
        XCTAssertTrue(view.isSelected && !view.isEmphasized, "not key")
        let unemphasized = try fill(4, in: f)
        XCTAssertLessThan(chroma(unemphasized), 0.02, "the unemphasized selection is grey: \(unemphasized)")
        XCTAssertLessThan(unemphasized.whiteComponentOrLuma, 0.95, "and it is drawn")
        f.window.makeKeyAndOrderFront(nil)
        pump { f.window.isKeyWindow }
        XCTAssertTrue(view.isEmphasized, "key again")
        XCTAssertGreaterThan(chroma(try fill(4, in: f)), 0.4, "emphasized again")
    }

    func testAControlInARowTakesItsOwnClicksAndTheRowTakesTheRest() throws {
        let f = try keyed()
        // The button, the checkbox and the field in row 3, each clicked:
        // each acts, the row's press does not run, the row is not selected.
        click(try f.node("bump-3"), in: f)
        XCTAssertTrue(f.status.contains("presses 0 bumps 1 on false"), f.status)
        XCTAssertEqual(f.table.selectedRow, -1, "a click on a button in a row does not select the row")
        click(try f.node("check-3"), in: f)
        XCTAssertTrue(f.status.contains("presses 0 bumps 1 on true"), f.status)
        XCTAssertEqual(f.table.selectedRow, -1)
        let field = try XCTUnwrap(try f.node("field-3").field)
        click(field, in: f)
        XCTAssertNotNil(field.currentEditor(), "the field took its click and the keyboard")
        XCTAssertEqual(f.table.selectedRow, -1)
        key(4, "h", in: f)
        key(34, "i", in: f)
        XCTAssertTrue(f.status.hasSuffix("draft hi"), f.status)
        XCTAssertTrue(f.status.contains("presses 0 "), f.status)
        // Where no control is, the same row's click is the row's: selected, and its press.
        click(try f.node("title-3"), in: f)
        XCTAssertEqual(f.table.selectedRow, 3)
        XCTAssertTrue(f.status.hasPrefix("opened 3 presses 1 bumps 1 on true"), f.status)
        XCTAssertTrue(f.window.firstResponder === f.table, "and the table has the keyboard again")
        // A row with no controls.
        click(try f.node("title-6"), in: f)
        XCTAssertEqual(f.table.selectedRow, 6)
        XCTAssertTrue(f.status.hasPrefix("opened 6 presses 2 "), f.status)
        // The button again, with another row selected: the selection stays.
        click(try f.node("bump-3"), in: f)
        XCTAssertTrue(f.status.contains("presses 2 bumps 2 "), f.status)
        XCTAssertEqual(f.table.selectedRow, 6)
    }
}

private extension NSColor {
    var whiteComponentOrLuma: CGFloat { (redComponent + greenComponent + blueComponent) / 3 }
}
#endif
