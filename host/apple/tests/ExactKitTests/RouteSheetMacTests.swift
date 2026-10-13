// @ref LLP 1115.002 D7.4 — a modal route as a sheet, in a whole session: the
// route's own subtree is in a sheet attached to the window, laid out in a
// containing block that is a maximum (or the size, where the route has an
// authored one); its header's heading is the sheet's title row and its two
// actions the sheet's buttons at the bottom right, each pressing its
// authored button, and what the rows do not stand for stays in the header; a
// header the sheet cannot take its title out of keeps the route painted in
// the window; the route's `closedby` says what dismisses it, as the stack
// goes back; the driver reaches it; and in a window really made key it
// takes the keyboard. `SheetHostMacTests` holds the sheet a `dialog` is,
// which this one is built on.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class RouteSheetMacTests: XCTestCase {
    /// One stack: a home route, and modal routes the app mounts as it
    /// selects them: one of Shelf's shape (Cancel, a heading, a prominent
    /// Save), one over it, one with an authored size, one whose header also
    /// holds a menu's button and a search field, one whose heading is
    /// pressed, one for each `closedby`, one with no header, and one that
    /// stays mounted while it is not shown.
    static let page = """
    component Proof
      state screen = "home"
      state saved = 0
      state cancelled = 0
      state name = ""
      state query = ""
      state rows = [0]
      action open(s: string)
        screen = s
      action cancel
        cancelled = cancelled + 1
        screen = "home"
      action save
        saved = saved + 1
        screen = "home"
      action rename(v: string)
        name = v
      action search(v: string)
        query = v
      action more
        rows = concat(rows, [length(rows)])
      view
        column testId="root" navigationKey=screen navigationBack="back" width="100%" height="100%" position="relative"
          column navigationKey="home" testId="home" position="absolute" inset=0 padding=20 gap=12
            input testId="page-field" value="on the page" width=240
            row gap=8 flex-wrap="wrap"
              button testId="open-add" press=open("add")
                text "Add"
              button testId="open-sized" press=open("sized")
                text "Sized"
              button testId="open-search" press=open("search")
                text "Search"
              button testId="open-pressable" press=open("pressable")
                text "Pressable"
              button testId="open-none" press=open("none")
                text "None"
              button testId="open-any" press=open("any")
                text "Any"
              button testId="open-plain" press=open("plain")
                text "Plain"
              button testId="open-kept" press=open("kept")
                text "Kept"
            text `saved ${saved} cancelled ${cancelled} name ${name} query ${query}` testId="status"
          when screen == "add" or screen == "over"
            column navigationKey="add" navigationPresentation="modal" testId="add" position="absolute" inset=0 display="flex" flex-direction="column"
              header testId="add-header" display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
                button id="back" press=cancel testId="add-cancel"
                  text "Cancel"
                text "New Book" role="heading" testId="add-heading"
                button press=save buttonStyle="bordered-prominent" testId="add-save"
                  text "Save"
              column padding=20 gap=12 testId="add-body"
                input testId="add-title" placeholder="Title" value=name input=rename width=380 height=24 autofocus=true
                input testId="add-author" placeholder="Author" width=380 height=24
                each i in rows key=i
                  box height=30 testId=`row-${i}`
                row gap=8
                  button testId="add-more" press=more
                    text "More"
                  button testId="add-over" press=open("over")
                    text "Over"
                  button testId="add-ask" commandfor="confirm" command="show-modal"
                    text "Ask"
          when screen == "over"
            column navigationKey="over" navigationPresentation="modal" testId="over" position="absolute" inset=0
              header display="flex" padding="8px 16px"
                text "Over" role="heading"
                button id="back" press=open("add") testId="over-done"
                  text "Done"
              box width=200 height=80 testId="over-body"
          when screen == "sized"
            column navigationKey="sized" navigationPresentation="modal" testId="sized" position="absolute" inset=0 width=480 height=300
              header display="flex" padding="8px 16px"
                button id="back" press=cancel testId="sized-cancel"
                  text "Cancel"
                text "Sized" role="heading"
              box flex=1 testId="sized-body"
          when screen == "search"
            column navigationKey="search" navigationPresentation="modal" testId="search" position="absolute" inset=0
              header testId="search-header" display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
                button id="back" press=cancel testId="search-cancel"
                  text "Cancel"
                text "Find" role="heading" testId="search-heading" flex=1
                button popovertarget="search-menu" aria-label="More" testId="search-more"
                  image "symbol:more"
                button press=save buttonStyle="filled" testId="search-save"
                  text "Done"
                input type="search" value=query input=search placeholder="Search" testId="search-field" width=300
              box width=340 height=60 testId="search-body"
              column id="search-menu" popover="auto" role="menu" aria-label="More"
                button press=more popovertarget="search-menu" popovertargetaction="hide" testId="search-menu-more"
                  text "More rows"
          when screen == "pressable"
            column navigationKey="pressable" navigationPresentation="modal" testId="pressable" position="absolute" inset=0
              header display="flex" padding="8px 16px"
                button id="back" press=cancel testId="pressable-cancel"
                  text "Cancel"
                box press=more testId="pressable-title"
                  text "Pressed" role="heading"
              box width=300 height=80 testId="pressable-body"
          when screen == "none"
            column navigationKey="none" navigationPresentation="modal" closedby="none" testId="none" position="absolute" inset=0
              header display="flex" padding="8px 16px"
                button id="back" press=cancel testId="none-cancel"
                  text "Cancel"
                text "None" role="heading"
              box width=300 height=80
          when screen == "any"
            column navigationKey="any" navigationPresentation="modal" closedby="any" testId="any" position="absolute" inset=0
              header display="flex" padding="8px 16px"
                button id="back" press=cancel testId="any-cancel"
                  text "Cancel"
                text "Any" role="heading"
              box width=300 height=80
          when screen == "plain"
            column navigationKey="plain" navigationPresentation="modal" testId="plain" position="absolute" inset=0 padding=20 gap=12
              text "No header here" testId="plain-text"
              button id="back" press=cancel testId="plain-close"
                text "Close"
          column navigationKey="kept" navigationPresentation="modal" testId="kept" position="absolute" inset=0
            header testId="kept-header" display="flex" padding="8px 16px"
              button id="back" press=cancel testId="kept-cancel"
                text "Cancel"
              text "Kept" role="heading"
            box width=320 height=100 testId="kept-body"
          dialog id="confirm" role="alertdialog" aria-label="Discard this book?"
            text "What you typed will be lost."
            button press=cancel destructive=true commandfor="confirm" command="close" testId="confirm-discard"
              text "Discard"
            button commandfor="confirm" command="close" testId="confirm-cancel"
              text "Cancel"

    """

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }
    private func make(_ size: CGSize = CGSize(width: 900, height: 640)) throws -> SheetFixture {
        let f = try SheetFixture(page: Self.page, size: size)
        fixture = f
        return f
    }
    /// The sheet's own rows: the title's row and the buttons' row, each with its hairline.
    private let rows = RouteChrome.titleRow + 1 + RouteChrome.buttonRow + 1
    /// The open route's sheet and its rows.
    private func sheet(_ f: SheetFixture, file: StaticString = #filePath, line: UInt = #line) throws -> (window: NSWindow, chrome: RouteChrome) {
        let window = try XCTUnwrap(f.presenter.sheets(on: f.window).last, file: file, line: line)
        return (window, try XCTUnwrap(window.contentView as? RouteChrome, file: file, line: line))
    }
    /// Events from the application's queue, delivered, until `done`.
    private func pump(_ seconds: Double = 2, until done: () -> Bool = { true }) {
        let limit = Date().addingTimeInterval(seconds)
        while Date() < limit {
            if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.01), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
            else if done() { return }
        }
    }
    private func keyEvent(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], _ type: NSEvent.EventType = .keyDown, in window: NSWindow) -> NSEvent {
        NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                         context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
    }
    /// A key the session takes in its front window, as its monitor routes one.
    private func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], in f: SheetFixture) -> Bool {
        let taken = f.presenter.routeKey(keyEvent(code, characters, flags, in: f.front), focused: true, in: f.front)
        f.settle()
        return taken
    }
    /// A click on the page's window, behind whatever is attached to it.
    private func clickBehind(_ f: SheetFixture) -> Bool {
        var taken = false
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let click = NSEvent.mouseEvent(with: type, location: NSPoint(x: 30, y: 30), modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber,
                                           context: nil, eventNumber: 3, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
            taken = f.presenter.dialogs.pointer(click) || taken
        }
        f.settle()
        return taken
    }

    /// Shelf's shape. The route is in a sheet on the window, as large as its
    /// content; its heading is the sheet's title row and Cancel and Save the
    /// sheet's buttons at the bottom right, Save the default; the header is
    /// the platform's box; the page beneath is shown, inert, laid out as it
    /// was, and no ground is painted. Save's row presses Save, and the sheet
    /// leaves with the route.
    func testAModalRouteIsASheetWithItsTitleAndItsTwoActionsAsItsOwnRows() throws {
        let f = try make(), p = f.presenter
        let home = try f.node("home"), pageFrames = try ["home", "page-field", "status"].map { try f.node($0).frame }
        XCTAssertNil(f.window.attachedSheet)

        try f.press("open-add")
        let route = try f.node("add"), header = try f.node("add-header")
        let (window, chrome) = try sheet(f)
        XCTAssertTrue(window is SheetWindow && f.window.attachedSheet === window)
        XCTAssertTrue(route.window === window, "the route's own subtree is in the sheet")
        XCTAssertTrue(p.dialogs.active === route && p.dialogs.routeActive)
        XCTAssertNil(p.dialogs.observation, "it is no dialog to the app")
        XCTAssertEqual(p.dialogs.routeObservation?["presentation"] as? String, "sheet")
        XCTAssertFalse(route.isHidden)
        XCTAssertFalse(route.subviews.contains { $0 is ModalGround })
        XCTAssertTrue(route.standsOnSheetGround, "the ground the compiler gave it is the sheet's own there, which AppKit draws")
        XCTAssertNil(route.layer?.backgroundColor)
        // The page beneath: shown under the sheet, inert, where it was.
        XCTAssertFalse(home.isHidden)
        XCTAssertTrue(try f.node("page-field").inert)
        XCTAssertEqual(try ["home", "page-field", "status"].map { try f.node($0).frame }, pageFrames)

        // The header's heading and its two actions are the sheet's rows.
        XCTAssertEqual(chrome.title.stringValue, "New Book")
        XCTAssertEqual([chrome.title.isHidden, chrome.cancel.isHidden, chrome.confirm.isHidden], [false, false, false])
        XCTAssertEqual([chrome.cancel.title, chrome.confirm.title], ["Cancel", "Save"])
        XCTAssertEqual(chrome.confirm.keyEquivalent, "\r", "the one that confirms is the default button")
        XCTAssertEqual(chrome.cancel.keyEquivalent, "\u{1b}", "Escape is the cancel's")
        XCTAssertTrue(header.isHidden, "the header shows nothing else: it is the platform's, whole")
        XCTAssertEqual(p.covers.delivered[header.id], .whole)
        XCTAssertEqual(try f.node("add-body").frame.minY, 0, "the page lays out without the header")
        XCTAssertTrue(p.dialogs.defaultButton(of: route) === (try f.node("add-save")))

        // Its size: the content's, and the rows round it; AppKit centres it.
        let used = try XCTUnwrap(p.blocks.used[route.id])
        XCTAssertEqual(used.width, try f.node("add-title").frame.width + 40, "its widest child, a field, in 20 of padding")
        XCTAssertEqual(route.frame, NSRect(origin: .zero, size: used))
        XCTAssertEqual(chrome.frame.size, CGSize(width: used.width, height: used.height + rows))
        XCTAssertEqual(window.frame.midX, f.window.frame.midX, accuracy: 0.5)
        // The rows, as SwiftUI lays a sheet out: a 48-point title row, a 64-point button row,
        // the buttons one width, 8 apart, 20 from the edges, cancel first.
        chrome.layoutSubtreeIfNeeded()
        let cancel = chrome.cancel.alignmentRect(forFrame: chrome.cancel.frame), save = chrome.confirm.alignmentRect(forFrame: chrome.confirm.frame)
        XCTAssertEqual(route.superview?.frame.minY, RouteChrome.titleRow + 1)
        XCTAssertEqual(chrome.title.frame.midY, RouteChrome.titleRow / 2, accuracy: 1)
        XCTAssertEqual(save.maxX, chrome.bounds.maxX - 20)
        XCTAssertEqual(cancel.maxX, save.minX - 8)
        XCTAssertEqual(cancel.width, save.width)
        XCTAssertEqual(save.midY, chrome.bounds.maxY - RouteChrome.buttonRow / 2, accuracy: 1)

        // The focus arrives in the route's `autofocus` field.
        XCTAssertTrue(window.firstResponder === (try f.node("add-title")).field?.currentEditor())
        // Its content grows by a row: the sheet follows.
        try f.press("add-more")
        XCTAssertEqual(p.blocks.used[route.id], CGSize(width: used.width, height: used.height + 42))
        XCTAssertEqual(chrome.frame.height, used.height + 42 + rows)

        // Save's row: the authored Save is pressed, and the sheet leaves with the route, showing what it showed.
        chrome.confirm.performClick(nil)
        f.settle()
        XCTAssertTrue(f.status.contains("saved 1 cancelled 0"), f.status)
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertNil(p.dialogs.active)
        XCTAssertNil(p.views[route.id], "the app took the route away")
        XCTAssertTrue(chrome.subviews.contains { $0 is NSImageView }, "the leaving sheet kept its picture")
        XCTAssertEqual(p.covers.delivered, [:])
        XCTAssertFalse(try f.node("page-field").inert)
        // Cancel's row.
        try f.press("open-add")
        try sheet(f).chrome.cancel.performClick(nil)
        f.settle()
        XCTAssertTrue(f.status.contains("saved 1 cancelled 1"), f.status)
        XCTAssertNil(f.window.attachedSheet)
    }

    /// With no authored size the route fits its content under a maximum,
    /// the window's content area less the sheet's own rows; an authored
    /// width and height are used as written, whatever the content, and
    /// stop at that maximum too.
    func testItFitsItsContentUnderAMaximumAndAnAuthoredSizeWins() throws {
        let f = try make(CGSize(width: 640, height: 420)), p = f.presenter
        try f.press("open-add")
        let route = try f.node("add"), width = try XCTUnwrap(p.blocks.used[route.id]).width
        for _ in 0..<12 { try f.press("add-more") }
        let most = f.window.contentLayoutRect.size
        XCTAssertEqual(p.blocks.used[route.id]?.height, most.height - rows, "it stops at the window's content area")
        XCTAssertEqual(try sheet(f).chrome.frame.height, most.height)
        XCTAssertEqual(p.blocks.used[route.id]?.width, width)
        try sheet(f).chrome.cancel.performClick(nil)
        f.settle()
        XCTAssertNil(f.window.attachedSheet)

        try f.press("open-sized")
        let sized = try f.node("sized"), (window, chrome) = try sheet(f)
        XCTAssertEqual(p.blocks.used[sized.id], CGSize(width: 480, height: 300), "as written, not its content's")
        XCTAssertEqual(sized.frame, NSRect(x: 0, y: 0, width: 480, height: 300), "its `inset` places nothing: AppKit places the sheet")
        // No confirming action: the one row of buttons holds Cancel alone.
        XCTAssertEqual([chrome.cancel.isHidden, chrome.confirm.isHidden], [false, true])
        XCTAssertEqual(chrome.frame.size, CGSize(width: 480, height: 300 + rows))
        XCTAssertEqual(window.frame.midX, f.window.frame.midX, accuracy: 0.5)
        XCTAssertEqual(try f.node("sized-body").frame.height, 300, "its content fills the size it was given")
        // A window narrower than it: the route stops at the window's content area.
        f.window.setContentSize(NSSize(width: 440, height: 420))
        f.window.layoutIfNeeded(); f.settle()
        XCTAssertEqual(p.blocks.used[sized.id]?.width, f.window.contentLayoutRect.width)
        XCTAssertEqual(chrome.frame.width, f.window.contentLayoutRect.width)
    }

    /// What the sheet's rows do not stand for stays in the header, where
    /// the author put it: a menu's button and a search field, at the top of
    /// the sheet's content, still the authored nodes.
    func testAnythingElseInTheHeaderStaysWhereTheAuthorPutIt() throws {
        let f = try make(), p = f.presenter
        try f.press("open-search")
        let (window, chrome) = try sheet(f)
        let header = try f.node("search-header"), field = try f.node("search-field"), more = try f.node("search-more")
        XCTAssertEqual([chrome.title.stringValue, chrome.cancel.title, chrome.confirm.title], ["Find", "Cancel", "Done"])
        XCTAssertFalse(header.isHidden)
        XCTAssertEqual(try ["search-heading", "search-cancel", "search-save"].map { try f.node($0).isHidden }, [true, true, true])
        XCTAssertEqual(Set(p.covers.delivered.keys), Set(try ["search-heading", "search-cancel", "search-save"].map { try f.node($0).id }))
        XCTAssertEqual(p.dialogs.routeObservation?["header"] as? String, "partly")
        XCTAssertTrue(field.window === window && more.window === window)
        XCTAssertFalse(field.isHiddenOrHasHiddenAncestor || more.isHiddenOrHasHiddenAncestor)
        XCTAssertGreaterThan(field.frame.width, 0)
        // The field is the authored input: what is typed is the app's.
        let typed = f.session.agentInstance.type(["id": Int(field.id), "text": "eliot"])
        XCTAssertEqual(typed["value"] as? String, "eliot")
        f.settle()
        XCTAssertTrue(f.status.hasSuffix("query eliot"), f.status)
        chrome.confirm.performClick(nil)
        f.settle()
        XCTAssertTrue(f.status.contains("saved 1 "), f.status)
    }

    /// A header whose heading cannot be taken out of it keeps the route
    /// painted in the window, on its ground, and says why; so does the
    /// driver's own chrome. A route with no header is a sheet of its content.
    func testARefusedHeaderAndTheDriversChromeKeepTheRoutePainted() throws {
        let f = try make(), p = f.presenter
        try f.press("open-pressable")
        let route = try f.node("pressable")
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(route.window === f.window)
        XCTAssertFalse(route.standsOnSheetGround)
        XCTAssertGreaterThan(route.cgColor("background_color")?.alpha ?? 0, 0, "painted as before, on the ground the compiler gives a modal route (`ModalGroundMacTests`: one it gave none)")
        XCTAssertTrue(p.navigation.keptSheets[route.id]?.contains("inside a pressable") == true)
        XCTAssertEqual(p.dialogs.routeObservation?["presentation"] as? String, "painted")
        let log = f.session.agent("{\"op\":\"logs\"}")
        XCTAssertTrue(log.contains("stays painted in the window: its heading is inside a pressable"), "the session's log says why")
        XCTAssertFalse(try f.node("pressable-cancel").isHidden)
        try f.press("pressable-cancel")
        XCTAssertTrue(f.status.contains("cancelled 1"), f.status)

        // No header: the route is a sheet with no rows of its own.
        try f.press("open-plain")
        let (_, chrome) = try sheet(f)
        XCTAssertEqual([chrome.title.isHidden, chrome.cancel.isHidden, chrome.confirm.isHidden], [true, true, true])
        let plain = try XCTUnwrap(p.blocks.used[try f.node("plain").id])
        XCTAssertEqual(chrome.frame.size, CGSize(width: plain.width.rounded(.up), height: plain.height.rounded(.up)), "its content's size, in whole points")
        try f.press("plain-close")
        XCTAssertNil(f.window.attachedSheet)

        // The driver's own chrome: painted, its header too.
        p.dialogs.painted = true
        try f.press("open-add")
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(try f.node("add").window === f.window)
        XCTAssertNotNil(try f.node("add").layer?.backgroundColor, "in the window it paints its ground")
        XCTAssertFalse(try f.node("add-header").isHidden)
        XCTAssertEqual(p.covers.delivered, [:])
    }

    /// What dismisses it is its `closedby`, and the dismissal is the
    /// stack's way back (the Back control's press), so the app decides.
    func testItsClosedbySaysWhatDismissesIt() throws {
        let f = try make(), p = f.presenter
        // Unwritten: Escape and Command-period; a click on the window behind does nothing.
        try f.press("open-add")
        XCTAssertFalse(clickBehind(f))
        XCTAssertNotNil(f.window.attachedSheet)
        XCTAssertTrue(key(53, "\u{1b}", in: f))
        XCTAssertNil(f.window.attachedSheet, "Escape dismisses it")
        XCTAssertTrue(f.status.contains("cancelled 1"), "as its Back control does: \(f.status)")
        try f.press("open-add")
        XCTAssertTrue(key(47, ".", .command, in: f))
        XCTAssertNil(f.window.attachedSheet, "Command-period dismisses it")
        XCTAssertTrue(f.status.contains("cancelled 2"), f.status)

        // `none`: neither key, and its Cancel has no Escape; its own button still closes it.
        try f.press("open-none")
        XCTAssertEqual(try sheet(f).chrome.cancel.keyEquivalent, "")
        XCTAssertTrue(key(53, "\u{1b}", in: f), "the key is the sheet's all the same")
        _ = key(47, ".", .command, in: f)
        XCTAssertFalse(clickBehind(f))
        XCTAssertNotNil(f.window.attachedSheet, "only its own buttons close it")
        XCTAssertTrue(f.status.contains("cancelled 2"), f.status)
        XCTAssertEqual(p.navigation.agentBack()?.refused, "its closedby is \"none\"")
        try sheet(f).chrome.cancel.performClick(nil)
        f.settle()
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(f.status.contains("cancelled 3"), f.status)

        // `any`: a click on the window behind it too.
        try f.press("open-any")
        XCTAssertTrue(clickBehind(f))
        XCTAssertNil(f.window.attachedSheet, "a click on the window behind dismisses it")
        XCTAssertTrue(f.status.contains("cancelled 4"), f.status)
    }

    /// A modal route over a modal route is a sheet on its sheet; an alert
    /// asked for from the route is on the route's sheet; and a route that
    /// stays mounted goes back under its authored parent, its header as
    /// authored, when its sheet has gone.
    func testWhatNestsAndARouteThatStaysMounted() throws {
        let f = try make(), p = f.presenter
        try f.press("open-add")
        let add = try f.node("add"), first = try sheet(f).window
        try f.press("add-over")
        let over = try f.node("over"), second = try sheet(f).window
        XCTAssertEqual(p.sheets(on: f.window).count, 2)
        XCTAssertTrue(first.attachedSheet === second, "a sheet on the sheet")
        XCTAssertTrue(add.window === first && over.window === second)
        XCTAssertFalse(add.isHidden, "the route under it is still shown in its sheet")
        XCTAssertTrue(try f.node("add-title").inert)
        XCTAssertEqual(try sheet(f).chrome.title.stringValue, "Over")
        // Its Back control is no prominent button: it cancels, in the cancel's place.
        XCTAssertEqual(try sheet(f).chrome.cancel.title, "Done")
        XCTAssertTrue(key(53, "\u{1b}", in: f))
        XCTAssertEqual(p.sheets(on: f.window).count, 1, "the top one leaves and the one under it stays")
        XCTAssertTrue(p.dialogs.active === add)
        XCTAssertFalse(try f.node("add-title").inert)

        // An alert from the route: on the route's sheet.
        try f.press("add-ask")
        let alert = try XCTUnwrap(p.alerts.presented)
        XCTAssertTrue(alert.window === first)
        XCTAssertTrue(p.alerts.key("Escape"))
        XCTAssertTrue(p.dialogs.active === add)
        try sheet(f).chrome.cancel.performClick(nil)
        f.settle()
        XCTAssertNil(f.window.attachedSheet)

        // A route that is only deselected: back where it was written, hidden, its header shown as authored.
        let kept = try f.node("kept"), header = try f.node("kept-header"), parent = kept.superview
        XCTAssertTrue(kept.isHidden)
        try f.press("open-kept")
        XCTAssertTrue(kept.window === (try sheet(f).window))
        XCTAssertTrue(header.isHidden)
        XCTAssertEqual(p.blocks.used[kept.id]?.width, 320)
        XCTAssertTrue(key(53, "\u{1b}", in: f))
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(kept.superview === parent && kept.window === f.window)
        XCTAssertTrue(kept.isHidden, "navigation hides it again")
        XCTAssertFalse(header.isHidden, "its header is as authored again")
        XCTAssertNil(p.blocks.used[kept.id])
        XCTAssertEqual(p.covers.delivered, [:])
        XCTAssertEqual(kept.frame.size, f.window.contentLayoutRect.size, "the page lays it out again, at its `inset`")
    }

    /// The driver: a tap on an authored button its sheet's row stands for
    /// is the row's click; a node in the sheet is where a person sees it and
    /// takes its taps and its typing; what is behind takes none; `back` is
    /// its dismissal.
    func testTheDriverReachesARouteInItsSheet() throws {
        let f = try make(), p = f.presenter
        NodeView.everyClickCounts = true
        addTeardownBlock { NodeView.everyClickCounts = ExactEnv.agentMode }
        let agent = f.session.agentInstance
        try f.press("open-add")
        let route = try f.node("add"), author = try f.node("add-author"), (window, _) = try sheet(f)
        let navigation = try XCTUnwrap(agent.stateSections()["navigation"] as? [String: Any])
        XCTAssertEqual(navigation["presentation"] as? String, "modal")
        XCTAssertEqual((navigation["stack"] as? [String])?.last, "add", "the route in its sheet is still its stack's")
        XCTAssertEqual((navigation["sheet"] as? [String: Any])?["title"] as? String, "New Book")
        // Where a person sees it: the sheet is centred on its window, under the title row.
        let box = agent.box(route)
        XCTAssertEqual(box.midX, p.viewport.bounds.midX, accuracy: 1)
        XCTAssertEqual(agent.box(author).minX, box.minX + 20, accuracy: 0.5)
        let tapped = agent.tap(["id": Int(author.id)])
        XCTAssertNil(tapped["error"], "\(tapped)")
        XCTAssertTrue(window.firstResponder === author.field?.currentEditor())
        XCTAssertEqual(agent.type(["id": Int(author.id), "text": "Eliot"])["value"] as? String, "Eliot")
        XCTAssertNotNil(agent.tap(["id": Int(try f.node("open-sized").id)])["error"], "what is behind the sheet takes no tap")
        // Save, by its authored id: the sheet's row.
        let saved = agent.tap(["id": Int(try f.node("add-save").id)])
        XCTAssertEqual(saved["native"] as? String, "NSButton", "\(saved)")
        f.settle()
        XCTAssertTrue(f.status.contains("saved 1 "), f.status)
        XCTAssertNil(f.window.attachedSheet)
        // Cancel, by its authored id; then the platform's Back.
        try f.press("open-add")
        XCTAssertEqual(agent.tap(["id": Int(try f.node("add-cancel").id)])["native"] as? String, "NSButton")
        f.settle()
        XCTAssertTrue(f.status.contains("cancelled 1"), f.status)
        try f.press("open-add")
        XCTAssertEqual(agent.back()["back"] as? String, "sheet")
        f.settle()
        XCTAssertTrue(f.status.contains("cancelled 2"), f.status)
        XCTAssertNil(f.window.attachedSheet)
    }

    /// In a window really made key: the sheet is the key window, the caret
    /// arrives in its field, Return from the field presses the confirming
    /// row, Escape cancels, and the page has the keyboard back.
    func testInAKeyWindowReturnConfirmsAndEscapeCancels() throws {
        KeyboardNavigation.agent = false
        addTeardownBlock { KeyboardNavigation.agent = nil }
        let f = try make()
        NSApp.setActivationPolicy(.accessory)
        f.window.setFrameTopLeftPoint(NSPoint(x: 80, y: (NSScreen.main?.visibleFrame.maxY ?? 800) - 40))
        NSApp.activate(ignoringOtherApps: true)
        f.window.makeKeyAndOrderFront(nil)
        pump(3) { f.window.isKeyWindow && NSApp.isActive }
        try XCTSkipUnless(f.window.isKeyWindow && NSApp.isActive, "this process cannot bring a window to the front and make it key")
        func send(_ code: UInt16, _ characters: String) {
            for type in [NSEvent.EventType.keyDown, .keyUp] { NSApp.sendEvent(keyEvent(code, characters, [], type, in: f.front)) }
            f.settle()
        }
        try f.press("open-add")
        let (window, chrome) = try sheet(f)
        pump(1) { window.isKeyWindow }
        XCTAssertTrue(window.isKeyWindow, "the sheet is the key window")
        XCTAssertTrue(f.window.isMainWindow && !f.window.isKeyWindow)
        let title = try XCTUnwrap(try f.node("add-title").field)
        XCTAssertTrue(window.firstResponder === title.currentEditor(), "the caret arrives in its field")
        XCTAssertEqual(chrome.confirm.keyEquivalent, "\r")
        send(0, "a"); send(11, "b")
        XCTAssertTrue(f.status.contains("name ab "), f.status)
        send(36, "\r")
        XCTAssertTrue(f.status.contains("saved 1 "), "Return from a field presses the default button: \(f.status)")
        XCTAssertNil(f.window.attachedSheet)
        pump(1) { f.window.isKeyWindow }
        XCTAssertTrue(f.window.isKeyWindow, "the page's window has the keyboard again")

        try f.press("open-add")
        pump(1) { f.window.attachedSheet?.isKeyWindow == true }
        send(53, "\u{1b}")
        XCTAssertNil(f.window.attachedSheet, "Escape cancels")
        XCTAssertTrue(f.status.contains("cancelled 1"), f.status)
    }
}
#endif
