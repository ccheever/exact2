// @ref LLP 1115.002 D7.2 — a `dialog` as a sheet, in a whole session: the
// kernel lays the dialog out in its sheet's containing block and the sheet's
// window is the size that comes back; in a window really made key the sheet
// takes the keyboard, and Escape, Command-period and the default button's
// Return work there (D7's condition for stage 6c); an alert goes over the
// sheet and a popover opens from it; the driver finds, clicks and reads what
// is in it. `DialogSheetMacTests` holds what a dialog is to the app in
// either presentation; `PopoverNativeMacTests` the popover.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// One session in a window, running a page that has a `dialog` (a field,
/// rows an action adds, Cancel and a `filled` Save), a second that asks for
/// all the width, two popovers and an alert.
final class SheetFixture {
    let session: ExactSession
    let window: NSWindow
    let view: ExactView
    var presenter: Presenter { session.presenter }

    static let page = """
    component Proof
      state notes = [0]
      state saved = 0
      state name = ""
      state tag = ""
      state discarded = 0
      action more
        notes = concat(notes, [length(notes)])
      action save
        saved = saved + 1
        close("book")
      action rename(v: string)
        name = v
      action retag(v: string)
        tag = v
      action discard
        discarded = discarded + 1
        close("book")
      view
        column testId="page" width="100%" height="100%" padding=20 gap=12
          input testId="page-field" value="on the page" width=240
          row gap=8
            button testId="open" commandfor="book" command="show-modal"
              text "New Book"
            button testId="tags" popovertarget="tags-pop"
              text "Tags"
            button testId="pinned" popovertarget="pinned-pop"
              text "Pinned"
            button testId="wide-open" commandfor="wide" command="show-modal"
              text "Wide"
          text `saved ${saved} name ${name} tag ${tag} discarded ${discarded}` testId="status"
          dialog id="book" testId="book" padding=20 width=420
            column gap=12
              text "New Book" testId="heading"
              input testId="title" placeholder="Title" value=name input=rename width=380 height=24
              input testId="author" placeholder="Author" width=380 height=24
              each i in notes key=i
                box height=30 testId=`note-${i}`
              row gap=8 justify-content="flex-end"
                button testId="more" press=more width=60 height=28
                  text "More"
                button testId="in-sheet-tags" popovertarget="sheet-pop" width=60 height=28
                  text "Tags"
                button testId="ask" commandfor="confirm" command="show-modal" width=70 height=28
                  text "Discard"
                button testId="cancel" commandfor="book" command="close" width=70 height=28
                  text "Cancel"
                button testId="save" press=save buttonStyle="filled" width=60 height=28
                  text "Save"
              column id="sheet-pop" testId="sheet-pop" popover="auto" position="absolute" padding=16 width=200
                input testId="sheet-tag" placeholder="Tag" width=168 height=24
          dialog id="wide" testId="wide" padding=20 width="100%" box-sizing="border-box" closedby="any"
            box height=40
          column id="tags-pop" testId="tags-pop" popover="auto" padding=16 gap=8 width=260
            input testId="tag" placeholder="Tag" value=tag input=retag width=228 height=24
            button testId="done" popovertarget="tags-pop" popovertargetaction="hide" width=60 height=28
              text "Done"
          column id="pinned-pop" testId="pinned-pop" popover="manual" position-area="top span-all" margin-bottom=12 padding=16 width=200
            box height=40
            button testId="unpin" popovertarget="pinned-pop" popovertargetaction="hide" width=60 height=28
              text "Close"
          dialog id="confirm" role="alertdialog" aria-label="Discard this book?"
            text "What you typed will be lost."
            button press=discard destructive=true commandfor="confirm" command="close" testId="confirm-discard"
              text "Discard"
            button commandfor="confirm" command="close" testId="confirm-cancel"
              text "Cancel"

    """

    init(size: CGSize = CGSize(width: 900, height: 640)) throws {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try Self.page.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        compiler.standardError = FileHandle.nullDevice // its colour advice
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        session = ExactApp.shared.makeSession(label: "sheet-\(UUID().uuidString.prefix(6))")
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: size).error)
        window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        view = ExactView(session: session)
        view.frame = NSRect(origin: .zero, size: size)
        window.contentView = view
        window.layoutIfNeeded()
        settle()
    }
    func close() {
        presenter.reset()
        session.destroy()
        window.close()
    }
    /// What a batch left to a later turn lands.
    func settle() {
        for _ in 0..<3 {
            presenter.settlePump()
            session.drainFill()
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.002))
        }
        window.layoutIfNeeded()
    }
    func node(_ id: String) throws -> NodeView { try XCTUnwrap(presenter.views.values.first { $0.props["testId"] == id }, id) }
    func press(_ id: String) throws { presenter.press(try node(id).id); settle() }
    /// The status line: what the app's state is now.
    var status: String { (try? node("status"))?.paragraphSpec().runs.map(\.text).joined() ?? "" }
    /// The window the keyboard is in: the open dialog's sheet, else the page's.
    var front: NSWindow { presenter.frontWindow ?? window }
}

final class SheetHostMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }
    private func make(_ size: CGSize = CGSize(width: 900, height: 640)) throws -> SheetFixture {
        let f = try SheetFixture(size: size)
        fixture = f
        return f
    }
    /// Events from the application's queue, delivered, until `done`.
    private func pump(_ seconds: Double = 2, until done: () -> Bool = { true }) {
        let limit = Date().addingTimeInterval(seconds)
        while Date() < limit {
            if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.01), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
            else if done() { return }
        }
    }
    /// A key through the application, as a person's comes: the session's monitor first, then the key window.
    private func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], in f: SheetFixture) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            NSApp.sendEvent(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.front.windowNumber,
                                             context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!)
        }
        f.settle()
    }

    /// The dialog is laid out in its sheet's block: as large as its
    /// content, an authored width as written, at most the window's content
    /// area; the sheet's window is that size and AppKit centres it; the page
    /// behind is laid out as it was; and it all goes back when it closes.
    func testTheKernelLaysTheDialogOutInItsSheetAndTheSheetIsThatSize() throws {
        let f = try make(), p = f.presenter
        let dialog = try f.node("book"), field = try XCTUnwrap(try f.node("title").field)
        let behind = try ["page", "page-field", "open", "status"].map(f.node)
        let pageFrames = behind.map(\.frame), home = dialog.superview
        XCTAssertTrue(dialog.isHidden)
        field.stringValue = "kept"

        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet)
        XCTAssertTrue(p.dialogs.active === dialog)
        XCTAssertTrue(dialog.window === sheet && sheet is SheetWindow)
        XCTAssertFalse(dialog.isHidden)
        XCTAssertEqual(p.dialogs.observation?["presentation"] as? String, "sheet")
        // 420 as written and its padding beside it (a content box); its height the content's.
        let fitted = try XCTUnwrap(p.blocks.used[dialog.id])
        XCTAssertEqual(fitted.width, 460)
        XCTAssertLessThan(fitted.height, 400)
        XCTAssertEqual(dialog.frame, NSRect(origin: .zero, size: fitted), "its frame is in the sheet's own space")
        XCTAssertEqual(sheet.contentView?.frame.size, fitted, "the sheet's window is the size layout returned")
        XCTAssertEqual(sheet.frame.midX, f.window.frame.midX, accuracy: 0.5, "AppKit centres it on its window")
        XCTAssertEqual(behind.map(\.frame), pageFrames, "the page behind it is laid out as it was")
        XCTAssertTrue(try f.node("title").field === field, "the same field, in the sheet")
        XCTAssertEqual(field.stringValue, "kept")
        XCTAssertTrue(field.window === sheet)
        XCTAssertEqual(try f.node("title").convert(NSPoint.zero, to: dialog).x, 20, "inside the dialog's padding")

        // Its content grows by a row: the sheet follows, still centred.
        try f.press("more")
        XCTAssertEqual(p.blocks.used[dialog.id], CGSize(width: 460, height: fitted.height + 30 + 12))
        XCTAssertEqual(sheet.contentView?.frame.height, fitted.height + 42)
        XCTAssertEqual(sheet.frame.midX, f.window.frame.midX, accuracy: 0.5)
        // Past the window's content area it stops there.
        let maximum = f.window.contentLayoutRect.size
        for _ in 0..<20 { try f.press("more") }
        XCTAssertEqual(p.blocks.used[dialog.id]?.height, maximum.height)
        XCTAssertEqual(sheet.contentView?.frame.height, maximum.height)
        XCTAssertEqual(behind.map(\.frame), pageFrames)
        // The window gets shorter: so does the sheet's maximum.
        f.window.setContentSize(NSSize(width: 900, height: 500))
        f.window.layoutIfNeeded(); f.settle()
        XCTAssertEqual(p.blocks.used[dialog.id]?.height, f.window.contentLayoutRect.height)

        // Closed by its own button: back under its authored parent, hidden, no block.
        try f.press("cancel")
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertNil(p.dialogs.active)
        XCTAssertNil(p.blocks.used[dialog.id])
        XCTAssertTrue(dialog.superview === home)
        XCTAssertTrue(dialog.isHidden)
        XCTAssertFalse(sheet.isVisible)

        // A width of 100% is all of the maximum, and `closedby="any"` takes a click on the window behind.
        try f.press("wide-open")
        let wide = try f.node("wide"), wideSheet = try XCTUnwrap(f.window.attachedSheet)
        XCTAssertEqual(p.blocks.used[wide.id]?.width, f.window.contentLayoutRect.width)
        XCTAssertEqual(wideSheet.frame.width, f.window.frame.width)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let click = try XCTUnwrap(NSEvent.mouseEvent(with: type, location: NSPoint(x: 30, y: 30), modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber,
                                                         context: nil, eventNumber: 3, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0))
            XCTAssertTrue(p.dialogs.pointer(click))
        }
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(wide.isHidden)
    }

    /// D7's condition for stage 6c, in a window really made key: the sheet
    /// is the key window and the page's stays main; the caret arrives in its
    /// first field and typing goes to the authored input; Tab goes round its
    /// fields; Return presses the one `filled` button; Escape and
    /// Command-period close it; and the page has the keyboard and its focus
    /// back each time.
    func testInAKeyWindowTheSheetTakesTheKeyboardAndReturnAndEscapeWorkThere() throws {
        KeyboardNavigation.agent = false
        addTeardownBlock { KeyboardNavigation.agent = nil }
        let f = try make()
        NSApp.setActivationPolicy(.accessory)
        f.window.setFrameTopLeftPoint(NSPoint(x: 80, y: (NSScreen.main?.visibleFrame.maxY ?? 800) - 40))
        NSApp.activate(ignoringOtherApps: true)
        f.window.makeKeyAndOrderFront(nil)
        pump(3) { f.window.isKeyWindow && NSApp.isActive }
        try XCTSkipUnless(f.window.isKeyWindow && NSApp.isActive, "this process cannot bring a window to the front and make it key")
        let pageField = try XCTUnwrap(try f.node("page-field").field)
        XCTAssertTrue(f.window.makeFirstResponder(pageField))
        (pageField.currentEditor() as? NSTextView)?.setSelectedRange(NSRange(location: 3, length: 4))

        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet)
        pump(1) { sheet.isKeyWindow }
        XCTAssertTrue(sheet.isKeyWindow, "the sheet is the key window")
        XCTAssertTrue(f.window.isMainWindow && !f.window.isKeyWindow, "and the page's stays main")
        let title = try XCTUnwrap(try f.node("title").field), author = try XCTUnwrap(try f.node("author").field)
        XCTAssertTrue(sheet.firstResponder === title.currentEditor(), "the caret arrives in its first field")
        XCTAssertNil(pageField.currentEditor(), "the page gave its focus up")
        // Typed at the keyboard: the authored input hears it.
        key(0, "a", in: f); key(11, "b", in: f)
        XCTAssertEqual(title.stringValue, "ab")
        XCTAssertTrue(f.status.contains("name ab "), f.status)
        // Tab: its two fields, and round again (no button is a stop with Keyboard Navigation off).
        key(48, "\t", in: f)
        XCTAssertTrue(sheet.firstResponder === author.currentEditor())
        key(48, "\t", in: f)
        XCTAssertTrue(sheet.firstResponder === title.currentEditor())
        key(48, "\u{19}", .shift, in: f)
        XCTAssertTrue(sheet.firstResponder === author.currentEditor())
        // The default button is drawn so: its key equivalent is Return.
        let save = try f.node("save")
        XCTAssertTrue(f.presenter.dialogs.defaultButton(of: try f.node("book")) === save)
        // Return, the caret in a field: Save is pressed, which closes the dialog.
        key(36, "\r", in: f)
        XCTAssertTrue(f.status.contains("saved 1 "), f.status)
        XCTAssertNil(f.window.attachedSheet)
        pump(1) { f.window.isKeyWindow }
        XCTAssertTrue(f.window.isKeyWindow, "the page's window has the keyboard again")
        XCTAssertTrue(f.window.firstResponder === pageField.currentEditor(), "and its field the focus")
        XCTAssertEqual((pageField.currentEditor() as? NSTextView)?.selectedRange(), NSRange(location: 3, length: 4), "with its selection")

        // Escape, the caret in a field.
        try f.press("open")
        pump(1) { f.window.attachedSheet?.isKeyWindow == true }
        key(53, "\u{1b}", in: f)
        XCTAssertNil(f.window.attachedSheet, "Escape closes it")
        XCTAssertTrue(f.status.contains("saved 1 "), "and presses nothing")
        pump(1) { f.window.isKeyWindow }
        XCTAssertTrue(f.window.isKeyWindow)
        // Command-period, AppKit's other cancel.
        try f.press("open")
        pump(1) { f.window.attachedSheet?.isKeyWindow == true }
        key(47, ".", .command, in: f)
        XCTAssertNil(f.window.attachedSheet, "Command-period closes it")
        // `closedby="none"`: neither does.
        let book = try f.node("book")
        try f.press("open")
        book.props["closedby"] = "none"
        pump(1) { f.window.attachedSheet?.isKeyWindow == true }
        key(53, "\u{1b}", in: f)
        key(47, ".", .command, in: f)
        XCTAssertNotNil(f.window.attachedSheet, "only its own buttons close it")
        try f.press("cancel")
        XCTAssertNil(f.window.attachedSheet)
    }

    /// What nests: an alert over the sheet is a sheet on the sheet's own
    /// window (on the page's it would wait behind it), a popover opens from
    /// a button in the sheet as AppKit's popover, and closing the dialog
    /// takes both with it.
    func testAnAlertGoesOverTheSheetAndAPopoverOpensFromIt() throws {
        let f = try make(), p = f.presenter
        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet), book = try f.node("book")
        // The alert: over the sheet.
        try f.press("ask")
        let alert = try XCTUnwrap(p.alerts.presented)
        XCTAssertTrue(alert.window === sheet, "the alert is on the sheet's window")
        XCTAssertTrue(sheet.attachedSheet === alert.alert.window)
        XCTAssertTrue(f.window.attachedSheet === sheet, "and the dialog's sheet is still the page's")
        XCTAssertEqual(p.sheets(on: f.window).count, 2)
        XCTAssertTrue(p.alerts.key("Escape"), "Escape is the alert's cancel")
        XCTAssertNil(p.alerts.presented)
        XCTAssertTrue(p.dialogs.active === book, "the dialog under it stays")
        // Its Discard closes the dialog too, through the app's own action.
        try f.press("ask")
        XCTAssertEqual(p.alerts.click(try f.node("confirm-discard")), true)
        let limit = Date().addingTimeInterval(2)
        while p.dialogs.active != nil, Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        f.settle()
        XCTAssertTrue(f.status.contains("discarded 1"), f.status)
        XCTAssertNil(p.dialogs.active)
        XCTAssertNil(f.window.attachedSheet)

        // A popover from a button in the sheet.
        try f.press("open")
        let again = try XCTUnwrap(f.window.attachedSheet)
        try f.press("in-sheet-tags")
        let pop = try f.node("sheet-pop"), native = try XCTUnwrap(p.menus.windowed.first)
        XCTAssertTrue(pop.superview === native.content)
        XCTAssertTrue(native.popover.isShown)
        XCTAssertTrue(p.hosts(native.window) && native.window !== again && native.window !== f.window)
        XCTAssertFalse(try f.node("sheet-tag").inert, "what is in a popover of the dialog's is the dialog's")
        XCTAssertTrue(try f.node("page-field").inert)
        XCTAssertTrue(p.dialogs.active === book, "the popover is no dialog: the sheet under it is still the modal")
        let home = try f.node("book").container
        try f.press("cancel")
        XCTAssertTrue(p.menus.windowed.isEmpty, "the dialog's popover closes with it")
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertTrue(pop.isHidden)
        // AppKit takes its popover away in its own time; its subtree goes home when it has.
        let gone = Date().addingTimeInterval(3)
        while native.popover.isShown, Date() < gone { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertFalse(native.popover.isShown)
        XCTAssertTrue(pop.isDescendant(of: home), "back under its authored parent")
        XCTAssertNil(p.blocks.used[pop.id])
    }

    /// The driver: a node in the sheet is where a person sees it, in the
    /// viewport's space; a click by its id lands on it in the sheet's
    /// window, which is never key under the driver; `back` cancels it.
    func testTheDriverFindsAndClicksWhatIsInTheSheet() throws {
        let f = try make(), p = f.presenter
        NodeView.everyClickCounts = true
        addTeardownBlock { NodeView.everyClickCounts = ExactEnv.agentMode }
        let agent = f.session.agentInstance
        try f.press("open")
        let sheet = try XCTUnwrap(f.window.attachedSheet)
        let author = try f.node("author"), book = try f.node("book")
        // The sheet is centred on its window: its dialog's box is too.
        let box = agent.box(book)
        XCTAssertEqual(box.size, book.frame.size)
        XCTAssertEqual(box.midX, p.viewport.bounds.midX, accuracy: 1)
        XCTAssertEqual(agent.box(author).minX, box.minX + 20, accuracy: 0.5)
        // A click by id: through the application into the sheet's window.
        let reply = agent.tap(["id": Int(author.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(sheet.firstResponder === author.field?.currentEditor(), "the click landed on the field in the sheet")
        // Typed by id: the same field.
        let typed = agent.type(["id": Int(author.id), "text": "Eliot"])
        XCTAssertEqual(typed["value"] as? String, "Eliot")
        // What is behind the sheet takes no tap.
        let blocked = agent.tap(["id": Int(try f.node("open").id)])
        XCTAssertNotNil(blocked["error"])
        // The platform's Back is its cancel.
        XCTAssertEqual(agent.back()["back"] as? String, "dialog")
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertNil(p.dialogs.active)
    }
}
#endif
