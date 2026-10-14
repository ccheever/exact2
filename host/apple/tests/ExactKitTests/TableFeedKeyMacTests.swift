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
    private func keyed(count: Int = 10_000, source: String? = nil) throws -> TableFixture {
        let f = try TableFixture(count: count, source: source)
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
    /// A key through the application's queue, as a person's arrives: it is `NSApp.currentEvent` while it is handled.
    private func queued(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], in f: TableFixture) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            NSApp.postEvent(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber,
                                             context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!, atStart: false)
        }
        pump(1)
        f.settle()
    }
    /// Letters, digits and spaces typed one key after another, quicker than a type-select forgets.
    private func typing(_ text: String, in f: TableFixture) {
        let codes: [Character: UInt16] = ["r": 15, "o": 31, "w": 13, " ": 49, "1": 18, "2": 19, "9": 25]
        for character in text { key(codes[character]!, String(character), in: f) }
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

    /// Stable keyed rows whose data can change while their selected row is
    /// outside the runner's window and the table has no keyboard focus.
    private static let selectionChanges = """
    component Rows
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      derive d1 = seed.concat(seed)
      derive d2 = d1.concat(d1)
      derive d3 = d2.concat(d2)
      derive d4 = d3.concat(d3)
      derive base = d4.map((x, i) => i)
      state inserted = false
      state rotated = false
      state missing = false
      state label = "Row"
      state opened = 0 - 1
      state presses = 0
      derive withInsertion = inserted ? [0 - 1].concat(base) : base
      derive moved = withInsertion.slice(20).concat(withInsertion.slice(0, 20))
      derive ordered = rotated ? moved : withInsertion
      derive rows = missing ? ordered.filter(r => r != 12) : ordered
      action open(r: number)
        opened = r
        presses = presses + 1
      action rename
        label = "Changed"
      action insertBefore
        inserted = true
      action removeBefore
        inserted = false
      action moveRows
        rotated = true
      action removeSelected
        missing = true
      action readdSelected
        missing = false
      view
        column width="100%" height="100%"
          row gap=8 padding=4
            text `opened ${opened} presses ${presses} label ${label}` testId="status"
            button press=rename testId="rename"
              text "Rename"
            button press=insertBefore testId="insert-before"
              text "Insert"
            button press=removeBefore testId="remove-before"
              text "Remove"
            button press=moveRows testId="move-rows"
              text "Move"
            button press=removeSelected testId="remove-selected"
              text "Delete"
            button press=readdSelected testId="readd-selected"
              text "Re-add"
          list virtualized=true estimated-item-height=32 flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" testId="rows"
            for r in rows key=`k${r}`
              row press=open(r) testId=`row-${r}` padding="6px 0" width="100%"
                text (r == 12 ? `${label} ${r}` : `Row ${r}`) testId=`title-${r}`

    """

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

        // Scrolled away while the table has the keyboard: the selected row
        // is where the keys go, and the runner keeps a row that has the
        // focus, so the selection stays its item's however far the list
        // goes. The table has no view for it there: it is in the list's document.
        f.scroll(to: 20_000)
        XCTAssertTrue(f.presenter.views[first.id] === first, "the runner kept the selected row")
        XCTAssertNil(f.table.rowView(atRow: 12, makeIfNecessary: false))
        XCTAssertTrue(first.superview === f.document)
        XCTAssertEqual(f.table.selectedRow, 12)
        f.scroll(to: 0)
        XCTAssertTrue(f.cells[12]?.row === first, "back: the same row, in its cell again")
        XCTAssertEqual(f.table.rowView(atRow: 12, makeIfNecessary: false)?.isSelected, true)
        // With the keyboard elsewhere the row is no one's focus. Scrolled
        // away: the runner destroys it and the table lets its view go.
        // Back: another row for the same item, selected.
        XCTAssertTrue(f.window.makeFirstResponder(nil))
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
        XCTAssertTrue(f.window.makeFirstResponder(f.table))

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

    func testAnUnmountedSelectionFollowsItsKeyWhileTheTableIsUnfocused() throws {
        let f = try keyed(count: 160, source: Self.selectionChanges)
        click(try f.node("title-12"), in: f)
        XCTAssertEqual(f.table.selectedRow, 12)
        XCTAssertEqual(f.status, "opened 12 presses 1 label Row")
        let selected = try XCTUnwrap(f.cells[12]?.row)
        let key = try XCTUnwrap(f.feed.selectedKey)
        XCTAssertEqual(selected.props["listItemKey"], key)

        // With the keyboard elsewhere, leaving the runner's window destroys
        // the row. Subsequent data changes must use its key, without bringing
        // the row or the table's port back to it.
        XCTAssertTrue(f.window.makeFirstResponder(nil))
        f.scroll(to: 3_000)
        XCTAssertNil(f.presenter.views[selected.id])
        XCTAssertNil(f.table.rowView(atRow: 12, makeIfNecessary: false))
        XCTAssertGreaterThan(f.clip.bounds.minY, 2_000)

        func change(_ control: String, selectedRow: Int, count: Int, _ note: String) throws {
            f.presenter.press(try f.node(control).id)
            f.settle()
            XCTAssertEqual(f.table.numberOfRows, count, note)
            XCTAssertEqual(f.table.selectedRow, selectedRow, note)
            XCTAssertEqual(f.feed.selectedKey, key, note)
            XCTAssertFalse(f.mounted.values.contains { $0.props["listItemKey"] == key }, "\(note): the selected item stayed unmounted")
            XCTAssertNil(f.table.rowView(atRow: selectedRow, makeIfNecessary: false), "\(note): AppKit made no row view for the selection")
            XCTAssertGreaterThan(f.clip.bounds.minY, 2_000, "\(note): the port did not reveal the selection")
            XCTAssertEqual(f.status, "opened 12 presses 1 label Changed", "\(note): no row action ran")
        }

        try change("rename", selectedRow: 12, count: 160, "a payload change")
        try change("insert-before", selectedRow: 13, count: 161, "an insertion before the key")
        try change("remove-before", selectedRow: 12, count: 160, "removing the item before the key")
        try change("move-rows", selectedRow: 152, count: 160, "a data reorder")

        f.presenter.press(try f.node("remove-selected").id)
        f.settle()
        XCTAssertEqual(f.table.numberOfRows, 159)
        XCTAssertEqual(f.table.selectedRow, -1, "the item now at the old index was not adopted")
        XCTAssertNil(f.feed.selectedKey, "the vanished key ended the selection")
        XCTAssertGreaterThan(f.clip.bounds.minY, 2_000)
        XCTAssertEqual(f.status, "opened 12 presses 1 label Changed", "removing the selected item pressed nothing")

        f.presenter.press(try f.node("readd-selected").id)
        f.settle()
        XCTAssertEqual(f.table.numberOfRows, 160)
        XCTAssertEqual(f.presenter.onListIndex?(f.list.id, key), 152, "the same key exists again")
        XCTAssertEqual(f.table.selectedRow, -1, "re-adding the key did not restore an ended selection")
        XCTAssertNil(f.feed.selectedKey)
        XCTAssertFalse(f.mounted.values.contains { $0.props["listItemKey"] == key })
        XCTAssertGreaterThan(f.clip.bounds.minY, 2_000)
        XCTAssertEqual(f.status, "opened 12 presses 1 label Changed", "re-adding the item pressed nothing")
    }

    func testThePageKeysScrollTypingSelectsAMountedRowAndReturnSpaceAndEscapePressNothing() throws {
        let f = try keyed()
        click(try f.node("title-5"), in: f)
        XCTAssertEqual(f.table.selectedRow, 5)
        XCTAssertTrue(f.status.hasPrefix("opened 5 presses 1 "), f.status)
        // Page Down and Page Up: the list moves by a page and the selection
        // stays, as a stock table's does.
        key(121, "\u{F72D}", [.function], in: f)
        pump(1) { f.clip.bounds.minY > 300 }
        f.settle()
        XCTAssertGreaterThan(f.clip.bounds.minY, 300, "Page Down scrolled the list")
        XCTAssertLessThan(f.clip.bounds.minY, f.clip.bounds.height, "by no more than a page")
        XCTAssertEqual(f.table.selectedRow, 5)
        key(116, "\u{F72C}", [.function], in: f)
        pump(1) { f.clip.bounds.minY == 0 }
        f.settle()
        XCTAssertEqual(f.clip.bounds.minY, 0, "Page Up scrolled it back")
        // Return, Enter, Space and Escape on a selected row: the selection
        // stays and the row's press does not run. A stock table and SwiftUI's
        // list do nothing with them either (measured beside this).
        for (code, characters) in [(UInt16(36), "\r"), (76, "\u{3}"), (49, " "), (53, "\u{1B}")] {
            key(code, characters, in: f)
            XCTAssertEqual(f.table.selectedRow, 5, "key \(code)")
            XCTAssertTrue(f.status.hasPrefix("opened 5 presses 1 "), "key \(code): \(f.status)")
            XCTAssertTrue(f.window.firstResponder === f.table, "key \(code)")
        }
        XCTAssertEqual(f.clip.bounds.minY, 0, "and Space did not scroll")
        // Typing selects the next row whose text begins with what was typed,
        // among the rows that are mounted: the feed has no text for the others.
        XCTAssertTrue(f.table.allowsTypeSelect)
        typing("row 2", in: f)
        let found = try XCTUnwrap(f.cells[f.table.selectedRow]?.row, "typing selected a mounted row (\(f.table.selectedRow))")
        XCTAssertEqual(f.table.selectedRow, 20, "the next row after the selected one whose text begins \"Row 2\"")
        XCTAssertTrue(try f.node("title-\(f.item(found))").accessibleText.hasPrefix("Row 2"))
        XCTAssertTrue(f.status.hasPrefix("opened 5 presses 1 "), "typing selects and presses nothing: \(f.status)")
        // A row that is not mounted is not found: a stock table, which has
        // every row's text, would select row 9,999.
        XCTAssertNil(f.mounted[9_999])
        RunLoop.main.run(until: Date().addingTimeInterval(1.5)) // the type-select forgets what was typed
        typing("row 9999", in: f)
        XCTAssertNotEqual(f.table.selectedRow, 9_999)
        XCTAssertNotNil(f.mounted[f.table.selectedRow], "the selection is on a mounted row (\(f.table.selectedRow))")
    }

    func testAKeyThatJumpsToTheListsEndLandsOnTheRunnersLastRow() throws {
        let f = try keyed()
        click(try f.node("title-5"), in: f)
        XCTAssertNil(f.mounted[9_999], "the last row is not made while the list is at its start")
        /// The selected row is `row`: the runner's row for that item, in its cell, whole in the port.
        func landed(_ row: Int, _ note: String) throws {
            f.settle()
            XCTAssertEqual(f.table.selectedRow, row, note)
            let node = try XCTUnwrap(f.cells[row]?.row, "\(note): row \(row) is mounted and in its cell")
            XCTAssertEqual(f.item(node), row, note)
            XCTAssertTrue(f.clip.bounds.insetBy(dx: 0, dy: -0.5).contains(f.inDocument(node)), "\(note): row \(row) is whole in the port (\(f.inDocument(node)) in \(f.clip.bounds))")
            XCTAssertEqual(f.feed.selectedKey, node.props["listItemKey"], "\(note): the selection is that item's")
            XCTAssertTrue(f.status.hasPrefix("opened 5 presses 1 "), "\(note) pressed nothing: \(f.status)")
        }
        // Option-Down is the last row and Option-Up the first, as in a stock
        // table. AppKit shows the last row where it guesses it is; the feed
        // shows it again when the runner has made it and layout has measured it.
        key(125, "\u{F701}", [.function, .numericPad, .option], in: f)
        try landed(9_999, "Option-Down")
        down(in: f)
        try landed(9_999, "Down on the last row")
        key(126, "\u{F700}", [.function, .numericPad, .option], in: f)
        try landed(0, "Option-Up")
        XCTAssertEqual(f.clip.bounds.minY, 0)
        // Up with nothing selected is the last row too.
        f.table.deselectAll(nil)
        up(in: f)
        try landed(9_999, "Up with nothing selected")
        // A row selected before it was made is its item's once it is: a row put before it moves the selection with it.
        f.presenter.press(try f.node("prepend").id)
        f.settle()
        XCTAssertEqual(f.table.numberOfRows, 10_001)
        XCTAssertEqual(f.table.selectedRow, 10_000)
        XCTAssertEqual(f.cells[10_000]?.row.map(f.item), 9_999)
    }

    /// A list of `count` rows whose press pushes a route for the row, and
    /// that route's Back.
    private static func pushing(count: Int, links: Bool = false) -> String {
        // The row: a `press` that pushes, or a link to the row's route.
        let row = links ? "link href=`/item/${r}` display=\"block\"" : "row press=open(r)"
        return """
        routes nav
          items "/"
            item "/item/:id"

        component Rows
          state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
          derive d1 = seed.concat(seed)
          derive d2 = d1.concat(d1)
          derive d3 = d2.concat(d2)
          derive d4 = d3.concat(d3)
          derive rows = d4.map((x, i) => i).slice(0, \(count))
          action open(r: number)
            nav = push(nav, path("item", `${r}`))
          action back
            nav = back(nav)
          action follow(url: string)
            nav = go(nav, url)
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
              for e in stack(nav) key=e.id
                if e.name == "items"
                  column navigationKey=`${e.id}` position="absolute" inset=0
                    list virtualized=true estimated-item-height=32 flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" testId="rows"
                      for r in rows key=`k${r}`
                        \(row) testId=`row-${r}` padding="6px 0" width="100%"
                          text `Row ${r}` testId=`title-${r}`
                if e.name == "item"
                  column navigationKey=`${e.id}` position="absolute" inset=0 padding=16 gap=8
                    text `Item ${e.params.id}` testId="detail"
                    button id="back" press=back testId="back"
                      text "Back"

        """
    }

    func testTheSelectionIsStillTheRowsWhenThePersonComesBackFromItsPush() throws {
        let f = try keyed(count: 160, source: Self.pushing(count: 160))
        // A click on a row pushes its route when the mouse comes up.
        click(try f.node("title-7"), in: f)
        XCTAssertEqual(try f.node("detail").accessibleText, "Item 7")
        XCTAssertNotNil(f.presenter.navigation.pushedRoute, "the row's press pushed a route")
        XCTAssertEqual(f.table.selectedRow, 7, "under the pushed route the row is still selected")
        let row = try XCTUnwrap(f.cells[7]?.row)
        // Back: the list's table is the one it was, the row the runner's
        // same row, selected, as SwiftUI's stack leaves its list (measured
        // beside this: selected on return and still selected two seconds on).
        f.presenter.press(try f.node("back").id)
        f.settle()
        pump(0.6) { false }
        f.settle()
        XCTAssertNil(f.presenter.navigation.pushedRoute)
        XCTAssertFalse(f.presenter.views.values.contains { $0.props["testId"] == "detail" }, "the pushed route is gone")
        XCTAssertTrue(f.presenter.tables.feeds[f.list.id] === f.feed, "the same feed")
        XCTAssertEqual(f.table.selectedRow, 7)
        XCTAssertTrue(f.cells[7]?.row === row, "the same row, in its cell")
        XCTAssertEqual(f.table.rowView(atRow: 7, makeIfNecessary: false)?.isSelected, true)
        // And the next arrow key goes on from it, once the table has the keyboard.
        XCTAssertTrue(f.window.makeFirstResponder(f.table))
        down(in: f)
        XCTAssertEqual(f.table.selectedRow, 8)
    }

    /// The arrow keys, ruled by the references (measured on macOS 27.0, key
    /// window): a stock table's action is the mouse's alone, and SwiftUI's
    /// list of navigation links pushes the row an arrow key reaches. So a
    /// row that is a link to one of the app's routes is pressed by the key
    /// that reaches it. A row whose `press` is an action is only selected
    /// (`testTheSelectionStaysWithItsItemAcrossARowsReuseAndTheKeyboard`):
    /// whether an action pushes is the compiler's to say, and no host is told yet.
    func testAnArrowKeyPushesTheNavigationRowItReaches() throws {
        let f = try keyed(count: 160, source: Self.pushing(count: 160, links: true))
        click(try f.node("title-7"), in: f)
        XCTAssertEqual(try f.node("detail").accessibleText, "Item 7", "a click on a link row follows it")
        XCTAssertEqual(f.table.selectedRow, 7)
        f.presenter.press(try f.node("back").id)
        f.settle()
        pump(0.6) { false }
        f.settle()
        XCTAssertNil(f.presenter.navigation.pushedRoute)
        XCTAssertTrue(f.window.makeFirstResponder(f.table))
        // Through the application's queue, as a person's key arrives.
        queued(125, "\u{F701}", [.function, .numericPad], in: f)
        XCTAssertEqual(f.table.selectedRow, 8)
        XCTAssertEqual(try f.node("detail").accessibleText, "Item 8", "Down selected the next row and pushed its route")
        f.presenter.press(try f.node("back").id)
        f.settle()
        pump(0.6) { false }
        f.settle()
        XCTAssertEqual(f.table.selectedRow, 8, "and it is still selected on the way back")
    }

    /// Twenty rows that press, answer a context click and a double click and
    /// hear keys, in a list that hears keys too. Each holds a symbol, a
    /// title in the unsaid colour, a line in the secondary label's and a
    /// word in a colour of the author's.
    private static let acting = """
    component Rows
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      derive rows = seed.concat(seed.map(x => x + 10))
      state log = ""
      action open(r: number)
        log = log + `press ${r};`
      action aim(r: number)
        log = log + `menu ${r};`
      action twice(r: number)
        log = log + `double ${r};`
      action rowKey(k: string)
        log = log + `row ${k};`
      action listKey(k: string)
        log = log + `list ${k};`
      view
        column width="100%" height="100%"
          text `log ${log}` testId="status"
          list virtualized=true estimated-item-height=44 flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" key=listKey aria-label="Twenty rows" testId="rows"
            for r in rows key=`k${r}`
              row press=open(r) contextmenu=aim(r) dblclick=twice(r) key=rowKey testId=`row-${r}` align-items="center" gap=8 padding="6px 0" width="100%"
                image "symbol:star" testId=`star-${r}`
                column flex=1 min-width=0
                  text `Row ${r}` testId=`title-${r}`
                  text `The second line of row ${r}` color="-exact-secondary-label" font-size=11 testId=`sub-${r}`
                text "Fixed" color="#cc2222" testId=`fixed-${r}`

    """

    /// The ink a paragraph resolved: its first run's, else the paragraph's own.
    private func ink(_ id: String, in f: TableFixture) throws -> [Double] {
        let spec = try f.node(id).paragraphSpec()
        return (spec.runs.first?.color ?? spec.color).map { ($0 * 100).rounded() / 100 }
    }

    func testWhatExactPaintsInASelectedRowTakesTheSelectedItemsInk() throws {
        let f = try keyed(count: 20, source: Self.acting)
        f.window.appearance = NSAppearance(named: .aqua)
        f.settle()
        let plain = try (title: ink("title-3", in: f), sub: ink("sub-3", in: f), fixed: ink("fixed-3", in: f))
        XCTAssertEqual(Array(plain.title.prefix(3)), [0, 0, 0], "in a light window an unsaid colour is dark ink")
        XCTAssertEqual(plain.fixed, [204, 34, 34, 255])
        XCTAssertEqual(try f.node("star-3").symbolView?.cell?.backgroundStyle, .normal)
        // Selected, in a key window with the table first responder: AppKit
        // draws the row emphasized, and a role's ink is the selected item's
        // text, a label colour at the share a stock table draws it at.
        click(try f.node("title-3"), in: f)
        XCTAssertEqual(f.table.selectedRow, 3)
        let cell = try XCTUnwrap(f.cells[3])
        XCTAssertEqual(cell.backgroundStyle, .emphasized)
        XCTAssertEqual(try ink("title-3", in: f), [255, 255, 255, 255], "the unsaid colour is the selected item's text")
        XCTAssertEqual(try ink("sub-3", in: f), [255, 255, 255, 234.6], "the secondary label's is that at 0.92")
        XCTAssertEqual(try ink("fixed-3", in: f), plain.fixed, "a colour the author wrote stays the author's")
        XCTAssertEqual(try f.node("star-3").symbolView?.cell?.backgroundStyle, .emphasized, "a symbol is drawn as a cell's image is")
        XCTAssertEqual(try ink("title-4", in: f), plain.title, "the row beside it is as it was")
        // What a person sees: light ink on the accent, where stage 3a's picture had black.
        /// The lightest and the darkest pixel drawn in a node's box, in its row's own picture.
        func extremes(_ id: String) throws -> (lightest: CGFloat, darkest: CGFloat) {
            let view = try XCTUnwrap(f.table.rowView(atRow: 3, makeIfNecessary: false)), node = try f.node(id)
            view.window?.displayIfNeeded()
            let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
            view.cacheDisplay(in: view.bounds, to: rep)
            let box = view.convert(node.bounds, from: node)
            var most: CGFloat = 0, least: CGFloat = 1
            for y in Int(box.minY)..<Int(box.maxY) {
                for x in Int(box.minX)..<Int(box.maxX) {
                    guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
                    let luma = (c.redComponent + c.greenComponent + c.blueComponent) / 3
                    most = max(most, luma); least = min(least, luma)
                }
            }
            return (most, least)
        }
        XCTAssertGreaterThan(try extremes("title-3").lightest, 0.9, "the title's glyphs are drawn light on the selection")
        // Another window takes key: the same row, selected and not
        // emphasized, in its own colours again, as a stock table's cell is.
        let other = NSWindow(contentRect: NSRect(x: 700, y: 300, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        self.other = other
        other.makeKeyAndOrderFront(nil)
        pump { other.isKeyWindow }
        f.settle()
        XCTAssertEqual(cell.backgroundStyle, .normal)
        XCTAssertEqual(f.table.selectedRow, 3)
        XCTAssertEqual(try ink("title-3", in: f), plain.title)
        XCTAssertEqual(try ink("sub-3", in: f), plain.sub)
        XCTAssertEqual(try f.node("star-3").symbolView?.cell?.backgroundStyle, .normal)
        XCTAssertLessThan(try extremes("title-3").darkest, 0.2, "and its glyphs are dark again")
    }

    func testAKeyIsHeardByTheSelectedRowAndTheListWhileTheTableHasTheKeyboard() throws {
        let f = try keyed(count: 20, source: Self.acting)
        // Nothing selected: the list hears the key, and what is above it.
        key(7, "x", in: f)
        XCTAssertEqual(f.status, "log list x;")
        // A row selected: its authored root hears first, then the list.
        click(try f.node("title-3"), in: f)
        XCTAssertEqual(f.status, "log list x;press 3;")
        key(7, "x", in: f)
        XCTAssertEqual(f.status, "log list x;press 3;row x;list x;")
        // The table's own keys are heard too, and still the table's.
        down(in: f)
        XCTAssertEqual(f.table.selectedRow, 4)
        XCTAssertTrue(f.status.hasSuffix("row ArrowDown;list ArrowDown;"), f.status)
        XCTAssertFalse(f.status.contains("press 4"), "an arrow selects a row whose press is an action, and presses nothing")
    }

    func testAContextClickAndADoubleClickOnARowAreTheRowsOwn() throws {
        let f = try keyed(count: 20, source: Self.acting)
        /// A press of a mouse button and its release in the middle of `view`, as `click` sends the left one.
        func press(_ down: NSEvent.EventType, _ up: NSEvent.EventType, _ view: NSView, flags: NSEvent.ModifierFlags = [], clicks: Int = 1, number: Int) {
            let point = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
            func event(_ type: NSEvent.EventType) -> NSEvent {
                NSEvent.mouseEvent(with: type, location: point, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber,
                                   context: nil, eventNumber: number, clickCount: clicks, pressure: type == down ? 1 : 0)!
            }
            var sent = false
            let timer = Timer(timeInterval: 0.05, repeats: false) { _ in NSApp.postEvent(event(up), atStart: false); sent = true }
            RunLoop.main.add(timer, forMode: .common)
            NSApp.sendEvent(event(down))
            pump { sent }
            f.settle()
        }
        // The right button on a row: its `contextmenu` hears it, and the row is not pressed.
        press(.rightMouseDown, .rightMouseUp, try f.node("title-5"), number: 11)
        XCTAssertEqual(f.status, "log menu 5;")
        // Control and the left button is the Mac's secondary click: the same, and no press follows it.
        press(.leftMouseDown, .leftMouseUp, try f.node("title-6"), flags: .control, number: 12)
        XCTAssertEqual(f.status, "log menu 5;menu 6;")
        // Two clicks: the first presses the row and the second is its
        // `dblclick`. A table sends its action for the first click of two
        // and its double action for the second, so the row is pressed
        // once: a row that pushes is not pushed twice.
        press(.leftMouseDown, .leftMouseUp, try f.node("title-7"), number: 13)
        press(.leftMouseDown, .leftMouseUp, try f.node("title-7"), clicks: 2, number: 14)
        XCTAssertEqual(f.status, "log menu 5;menu 6;press 7;double 7;")
        XCTAssertEqual(f.table.selectedRow, 7)
    }

    func testTheTableIsOneTabStopAndWhatIsInItsRowsIsStillReached() throws {
        let f = try keyed()
        let was = KeyboardNavigation.agent
        defer { KeyboardNavigation.agent = was }
        /// The Tab loop from the table, as AppKit walks it: each stop's next, until it comes round.
        func loop() -> [NSView] {
            f.presenter.syncKeyViewLoop()
            var stops: [NSView] = [f.table], next = f.table.nextKeyView
            while let view = next, view !== f.table, stops.count < 400 { stops.append(view); next = view.nextKeyView }
            return stops
        }
        func ids(_ stops: [NSView]) -> [String] {
            stops.compactMap { stop in f.presenter.views.values.first { f.presenter.keyView(of: $0) === stop }?.props["testId"] }
        }
        XCTAssertTrue(f.presenter.keyView(of: f.list) === f.table, "the list's key view is its table")
        // Keyboard Navigation off, the system's default: Tab moves between
        // text fields and lists. The table is a stop, and so is the field
        // in a row, which is in a cell and no subview of the list.
        KeyboardNavigation.agent = false
        var stops = ids(loop())
        XCTAssertEqual(stops.first, "rows", "the table stands in the loop where its list is")
        XCTAssertTrue(stops.contains("field-3"), "a field in a row is reached: \(stops)")
        XCTAssertFalse(stops.contains { $0.hasPrefix("row-") || $0.hasPrefix("bump-") }, "and no row or button is a stop: \(stops)")
        // On: every control is a stop, a button in a row too, but not the rows: the table selects those.
        KeyboardNavigation.agent = true
        stops = ids(loop())
        XCTAssertTrue(stops.contains("rows") && stops.contains("bump-3") && stops.contains("field-3") && stops.contains("grow"), "\(stops)")
        XCTAssertFalse(stops.contains { $0.hasPrefix("row-") }, "a row that presses is the table's to select, not a stop of its own: \(stops)")
        // Tab from the table goes to the first stop in its rows.
        KeyboardNavigation.agent = false
        _ = loop()
        XCTAssertTrue(f.window.makeFirstResponder(f.table))
        key(48, "\t", in: f)
        XCTAssertNotNil(try f.node("field-3").field?.currentEditor(), "Tab from the table reached the field in row 3")
    }

    /// What VoiceOver is told: AppKit's own table, rows and cells (a stock
    /// table's, whose tree is kept beside the audit's records), named by
    /// the list, with what the runner's row says inside each cell.
    func testVoiceOverIsToldATableOfRowsAndCellsNamedByTheList() throws {
        let f = try keyed(count: 20, source: Self.acting)
        click(try f.node("title-3"), in: f)
        XCTAssertEqual(f.table.accessibilityRole(), .table)
        XCTAssertEqual(f.table.accessibilityLabel(), "Twenty rows", "the list's name is the table's")
        // AppKit's row elements are its own objects, which Swift's typed arrays of the protocol refuse: read as plain arrays.
        func elements(_ name: String) throws -> [AnyObject] {
            try XCTUnwrap(f.table.perform(NSSelectorFromString(name))?.takeUnretainedValue() as? NSArray).map { $0 as AnyObject }
        }
        let rows = try elements("accessibilityRows")
        XCTAssertEqual(rows.count, 20, "a row for every item")
        XCTAssertEqual(Presenter.axFacts(rows[3])?.role, "AXRow")
        XCTAssertEqual(Presenter.axFacts(rows[3])?.selected, true, "the selected row is said selected")
        XCTAssertEqual(Presenter.axFacts(rows[4])?.selected, false)
        // The rows that show are the port's: the table keeps a view for every mounted row, and all twenty are mounted.
        let port = f.table.rows(in: f.table.convert(f.clip.bounds, from: f.document))
        XCTAssertEqual(f.feed.rowViews, 20)
        XCTAssertLessThan(port.length, 20)
        XCTAssertEqual(try elements("accessibilityVisibleRows").count, port.length, "VoiceOver is told the rows the port shows")
        /// Every element under one, with its role and what it says, as the driver's `tree --ax` reads them.
        func said(_ element: Any, depth: Int = 0) -> [String] {
            guard depth < 8, let facts = Presenter.axFacts(element as AnyObject) else { return [] }
            let says = [facts.label, facts.title, facts.value as? String].compactMap { $0 }.first { !$0.isEmpty } ?? ""
            return ["\(facts.role ?? "?") \(says)".trimmingCharacters(in: .whitespaces)] + facts.children.flatMap { said($0, depth: depth + 1) }
        }
        let tree = said(rows[3])
        XCTAssertEqual(tree.first, "AXRow")
        XCTAssertTrue(tree.contains("AXCell"), "\(tree)")
        XCTAssertTrue(tree.contains("AXStaticText Row 3"), "the row's title is in its cell: \(tree)")
        XCTAssertTrue(tree.contains("AXStaticText The second line of row 3"), "\(tree)")
        XCTAssertFalse(said(rows[4]).isEmpty)
        if let dir = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"] {
            try? (["the table: \(f.table.accessibilityRole()?.rawValue ?? "?") \"\(f.table.accessibilityLabel() ?? "")\", \(rows.count) rows"] + tree.map { "  " + $0 })
                .joined(separator: "\n").write(toFile: dir + "/table-ax.txt", atomically: true, encoding: .utf8)
        }
    }

    /// The window server's picture of the fixture's window, saved where `EXACT_PROOF_SHOTS` says.
    private func shot(_ f: TableFixture, _ name: String) {
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        guard let dir = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"], let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage") else { return }
        f.window.displayIfNeeded()
        CATransaction.flush()
        pump(0.3) { false }
        guard let image = unsafeBitCast(symbol, to: Create.self)(.null, 1 << 3, UInt32(f.window.windowNumber), 1 << 0 | 1 << 3)?.takeRetainedValue() else { return }
        try? NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }

    func testTheSelectionIsDrawnByTheWindowsKeyState() throws {
        let f = try keyed()
        click(try f.node("title-4"), in: f)
        XCTAssertEqual(f.table.selectedRow, 4)
        shot(f, "table-gate-key.png")
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
        shot(f, "table-gate-not-key.png")
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

    /// The gate's other clicks are made in a key window. These are the
    /// click that brings a window forward, which AppKit gives a view only
    /// if it accepts the first mouse: a table does, a button does, a field does.
    func testTheClickThatBringsTheWindowForwardSelectsARowAndPressesAControlInIt() throws {
        let f = try keyed()
        let other = NSWindow(contentRect: NSRect(x: 700, y: 300, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        self.other = other
        /// A click on `view` while another window is key.
        func first(_ view: NSView) {
            other.makeKeyAndOrderFront(nil)
            pump { other.isKeyWindow }
            XCTAssertFalse(f.window.isKeyWindow)
            click(view, in: f)
            pump(1) { f.window.isKeyWindow }
            XCTAssertTrue(f.window.isKeyWindow, "the click brought the window forward")
        }
        // A person's clicks first, then the driver's rule, under which every click counts (its window is never key).
        let was = NodeView.everyClickCounts
        defer { NodeView.everyClickCounts = was }
        for (round, every) in [false, true].enumerated() {
            NodeView.everyClickCounts = every
            let note = every ? "every click counts" : "AppKit's rule"
            let row = 6 + round, presses = 1 + round, bumps = 1 + round
            first(try f.node("title-\(row)"))
            XCTAssertEqual(f.table.selectedRow, row, "\(note): the click selected the row")
            XCTAssertTrue(f.status.hasPrefix("opened \(row) presses \(presses) "), "\(note): and pressed it, \(f.status)")
            first(try f.node("bump-3"))
            XCTAssertTrue(f.status.contains("presses \(presses) bumps \(bumps) "), "\(note): a button in a row took the click, \(f.status)")
            XCTAssertEqual(f.table.selectedRow, row, note)
            first(try f.node("check-3"))
            XCTAssertTrue(f.status.contains("bumps \(bumps) on \(round == 0) "), "\(note): a checkbox in a row took the click, \(f.status)")
            let field = try XCTUnwrap(try f.node("field-3").field)
            first(field)
            XCTAssertNotNil(field.currentEditor(), "\(note): a field in a row took the click and the keyboard")
            XCTAssertEqual(f.table.selectedRow, row, note)
            XCTAssertTrue(f.window.makeFirstResponder(f.table))
        }
    }
}

private extension NSColor {
    var whiteComponentOrLuma: CGFloat { (redComponent + greenComponent + blueComponent) / 3 }
}
#endif
