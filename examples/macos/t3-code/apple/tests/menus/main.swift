import AppKit
import XCTest

// Compile with the production native-module facade, its generated app keys,
// T3WindowChrome.swift and T3Menus.swift (see README, palette lane). Real AppKit
// menus, windows and key events; the quit timers run on an injected clock.
let exactModule: ExactModule.Type = ExactModule.self

/// A clock and timer queue the tests advance by hand.
private final class Clock {
    var time: TimeInterval = 100
    var timers: [(at: TimeInterval, id: Int, work: () -> Void)] = []
    var serial = 0
    func schedule(_ delay: TimeInterval, _ work: @escaping () -> Void) -> () -> Void {
        serial += 1
        let id = serial
        timers.append((time + delay, id, work))
        return { [weak self] in self?.timers.removeAll { $0.id == id } }
    }
    func advance(_ seconds: TimeInterval) {
        let end = time + seconds
        while let next = timers.filter({ $0.at <= end }).min(by: { $0.at < $1.at }) {
            timers.removeAll { $0.id == next.id }
            time = next.at
            next.work()
        }
        time = end
    }
}

private final class QuitFixture {
    let clock = Clock()
    let hold = T3QuitHold()
    var mode = "hold"
    var hints: [String] = []
    var quits = 0
    var concealed = 0
    init() {
        hold.now = { [unowned self] in self.clock.time }
        hold.schedule = { [unowned self] delay, work in self.clock.schedule(delay, work) }
        hold.mode = { [unowned self] in self.mode }
        hold.notify = { [unowned self] down, mode in self.hints.append(down ? "down:\(mode)" : "up") }
        hold.quit = { [unowned self] in self.quits += 1 }
        hold.conceal = { [unowned self] in self.concealed += 1 }
    }
    @discardableResult func press(repeatKey: Bool = false, shift: Bool = false) -> Bool {
        hold.keyDown(key: "q", command: true, option: false, shift: shift, isRepeat: repeatKey)
    }
}

final class MenuTests: XCTestCase {
    func testHoldModeShowsHintThenQuitsAfterHeldRepeats() {
        let fixture = QuitFixture()
        XCTAssertTrue(fixture.press())
        XCTAssertEqual(fixture.hints, ["down:hold"])
        XCTAssertEqual(fixture.quits, 0)
        // Auto-repeat proves the key is held; 1.2s in, the hold completes.
        for _ in 0..<13 { fixture.clock.advance(0.1); XCTAssertTrue(fixture.press(repeatKey: true)) }
        XCTAssertEqual(fixture.concealed, 1)
        XCTAssertEqual(fixture.quits, 0, "quits only once the repeats stop")
        fixture.clock.advance(0.7)
        XCTAssertEqual(fixture.quits, 1)
    }
    func testHoldModeTapDoesNotQuitAndAcceptsASecondPress() {
        let fixture = QuitFixture()
        fixture.press()
        // The tap's keyUp is suppressed by macOS; the watchdog releases the hint.
        fixture.clock.advance(1.9)
        XCTAssertEqual(fixture.hints, ["down:hold", "up"])
        XCTAssertEqual(fixture.quits, 0)
        fixture.press()
        fixture.clock.advance(0.3)
        fixture.press()
        XCTAssertEqual(fixture.quits, 1, "two presses within 500ms quit")
    }
    func testDoublePressModeAndDirectMode() {
        let fixture = QuitFixture()
        fixture.mode = "double-click"
        fixture.press()
        XCTAssertEqual(fixture.hints, ["down:double-click"])
        fixture.hold.keyUp(key: "q")
        XCTAssertEqual(fixture.hints, ["down:double-click"], "the double-press hint stays for its window")
        fixture.clock.advance(0.6)
        XCTAssertEqual(fixture.hints, ["down:double-click", "up"])
        XCTAssertEqual(fixture.quits, 0)
        fixture.press(); fixture.clock.advance(0.2); fixture.press()
        XCTAssertEqual(fixture.quits, 1)
        let direct = QuitFixture()
        direct.mode = "direct"
        direct.press()
        XCTAssertEqual(direct.quits, 1)
        XCTAssertEqual(direct.hints, [])
    }
    func testOtherKeysCancelAndModifiedChordsPassThrough() {
        let fixture = QuitFixture()
        XCTAssertFalse(fixture.press(shift: true), "⇧⌘Q is not the quit shortcut")
        fixture.press()
        XCTAssertFalse(fixture.hold.keyDown(key: "a", command: true, option: false, shift: false, isRepeat: false))
        XCTAssertEqual(fixture.hints, ["down:hold", "up"])
        fixture.clock.advance(0.2)
        fixture.press()
        XCTAssertEqual(fixture.quits, 0, "a cancelled first press does not count toward a double press")
    }
    func testRealKeyEventsReachTheStateMachine() {
        let fixture = QuitFixture()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command], timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "q", charactersIgnoringModifiers: "q", isARepeat: false, keyCode: 12)!
        XCTAssertTrue(fixture.hold.handle(down))
        let other = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command], timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "w", charactersIgnoringModifiers: "w", isARepeat: false, keyCode: 13)!
        XCTAssertFalse(fixture.hold.handle(other))
    }
    func testMenusGainTheReferenceItems() {
        _ = NSApplication.shared
        let bar = NSMenu()
        let edit = NSMenu(title: "Edit")
        for title in ["Undo", "Redo"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(.separator())
        for title in ["Cut", "Copy", "Paste", "Delete", "Select All"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        bar.addItem(withTitle: "Edit", action: nil, keyEquivalent: "").submenu = edit
        let view = NSMenu(title: "View")
        view.addItem(withTitle: "Enter Full Screen", action: nil, keyEquivalent: "f")
        bar.addItem(withTitle: "View", action: nil, keyEquivalent: "").submenu = view
        let menus = T3Menus()
        menus.augment(bar)
        menus.augment(bar)
        XCTAssertEqual(edit.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Paste as Text", "Delete", "—", "Select All", "—", "Speech"])
        let pasteText = edit.item(withTitle: "Paste as Text")!
        XCTAssertEqual(pasteText.keyEquivalent, "v")
        XCTAssertEqual(pasteText.keyEquivalentModifierMask, [.command, .shift])
        XCTAssertEqual(pasteText.action, #selector(NSTextView.pasteAsPlainText(_:)))
        XCTAssertEqual(view.items.map { $0.isSeparatorItem ? "—" : "\($0.title)\($0.isHidden ? " (hidden)" : "") \($0.keyEquivalent)" },
                       ["Actual Size 0", "Zoom In =", "Zoom In (hidden) +", "Zoom Out -", "—", "Enter Full Screen f"])
    }
    func testCheckForUpdatesJoinsTheAppAndHelpMenus() {
        _ = NSApplication.shared
        let bar = NSMenu()
        let app = NSMenu(title: "T3 Code")
        app.addItem(withTitle: "About T3 Code", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        app.addItem(.separator())
        let settings = NSMenuItem(title: "Settings…", action: nil, keyEquivalent: ",")
        app.addItem(settings)
        app.addItem(.separator())
        app.addItem(withTitle: "Services", action: nil, keyEquivalent: "")
        app.addItem(.separator())
        for title in ["Hide T3 Code", "Hide Others", "Show All"] { app.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        app.addItem(.separator())
        app.addItem(withTitle: "Quit T3 Code", action: nil, keyEquivalent: "q")
        bar.addItem(withTitle: "T3 Code", action: nil, keyEquivalent: "").submenu = app
        for title in ["File", "Edit", "View", "Go", "Window"] { bar.addItem(withTitle: title, action: nil, keyEquivalent: "").submenu = NSMenu(title: title) }
        let menus = T3Menus()
        menus.updatesDisabledReason = { "Automatic updates are not available because no update feed is configured." }
        var shown: [NSAlert] = []
        menus.presentAlert = { shown.append($0) }
        menus.augment(bar)
        menus.augment(bar)
        let expected = ["About T3 Code", "Check for Updates...", "—", "Settings…", "—", "Services", "—", "Hide T3 Code", "Hide Others", "Show All", "—", "Quit T3 Code"]
        let titles = { app.items.map { $0.isSeparatorItem ? "—" : $0.title } }
        XCTAssertEqual(titles(), expected)
        // The host's next batch puts Settings… back at index 2; the menu is ordered again as it opens.
        app.removeItem(settings)
        app.insertItem(settings, at: 2)
        menus.menuNeedsUpdate(app)
        XCTAssertEqual(titles(), expected)
        XCTAssertTrue(app.delegate === menus)
        XCTAssertEqual(bar.items.map(\.title), ["T3 Code", "File", "Edit", "View", "Window", "Help"], "the reference bar has no Go menu (lane r8-keys)")
        let help = bar.items.last!.submenu!
        XCTAssertEqual(help.items.map(\.title), ["Check for Updates..."])
        XCTAssertTrue(NSApp.helpMenu === help)
        let item = help.items[0]
        XCTAssertTrue(menus.validateMenuItem(item), "enabled with no window, as the reference's ensureMain does")
        _ = item.target!.perform(item.action!, with: item)
        XCTAssertEqual(shown.count, 1)
        XCTAssertEqual(shown[0].messageText, "Automatic updates are not available right now.")
        XCTAssertEqual(shown[0].informativeText, "Automatic updates are not available because no update feed is configured.")
        XCTAssertEqual(shown[0].alertStyle, .informational)
        XCTAssertEqual(shown[0].buttons.map(\.title), ["OK"])
        if let directory = ProcessInfo.processInfo.environment["T3_MENUS_EVIDENCE"], let content = { shown[0].layout(); return shown[0].window.contentView }() {
            for dark in [false, true] {
                content.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
                let rep = content.bitmapImageRepForCachingDisplay(in: content.bounds)!
                content.cacheDisplay(in: content.bounds, to: rep)
                try? rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(directory)/updates-unavailable-\(dark ? "dark" : "light").png"))
            }
        }
        // A build linking the update store has no hook here: neither item appears.
        let updating = T3Menus(), other = NSMenu()
        updating.updatesDisabledReason = { nil }
        let otherApp = NSMenu(title: "T3 Code")
        otherApp.addItem(withTitle: "About T3 Code", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        other.addItem(withTitle: "T3 Code", action: nil, keyEquivalent: "").submenu = otherApp
        updating.augment(other)
        XCTAssertEqual(otherApp.items.map(\.title), ["About T3 Code"])
        XCTAssertEqual(other.items.count, 1)
    }
    func testUpdatesDisabledReasonReadsTheReceiptComposition() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-menus-\(ProcessInfo.processInfo.processIdentifier)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let embedded = directory.appendingPathComponent("embedded.json"), updating = directory.appendingPathComponent("updating.json")
        try Data(#"{"composition":"embedded"}"#.utf8).write(to: embedded)
        try Data(#"{"composition":"updating"}"#.utf8).write(to: updating)
        XCTAssertEqual(T3Menus.updatesDisabledReason(receipt: embedded), "Automatic updates are not available because no update feed is configured.")
        XCTAssertNil(T3Menus.updatesDisabledReason(receipt: updating))
        XCTAssertNotNil(T3Menus.updatesDisabledReason(receipt: directory.appendingPathComponent("missing.json")))
    }
    func testZoomScalesTheWindowContent() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        let child = NSView(frame: window.contentView!.bounds)
        child.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(child)
        let menus = T3Menus()
        menus.attach(window)
        menus.zoomIn(nil)
        XCTAssertEqual(menus.zoomLevel, 0.5)
        XCTAssertEqual(window.contentView!.bounds.width, 1200 / pow(1.2, 0.5), accuracy: 0.01)
        XCTAssertEqual(child.frame.width, window.contentView!.bounds.width, accuracy: 0.01, "the page lays out in the zoomed viewport")
        menus.zoomOut(nil); menus.zoomOut(nil)
        XCTAssertEqual(window.contentView!.bounds.width, 1200 * pow(1.2, 0.5), accuracy: 0.01)
        menus.actualSize(nil)
        XCTAssertEqual(window.contentView!.bounds.size, window.contentView!.frame.size)
        for _ in 0..<40 { menus.zoomIn(nil) }
        XCTAssertEqual(pow(1.2, menus.zoomLevel), 5, accuracy: 0.001)
    }
    func testOverlayPillRendersTheReferenceMessages() throws {
        _ = NSApplication.shared
        XCTAssertEqual(T3QuitOverlay.message("hold"), "Hold ⌘Q or press twice to quit")
        XCTAssertEqual(T3QuitOverlay.message("double-click"), "Press ⌘Q again to quit")
        guard let directory = ProcessInfo.processInfo.environment["T3_MENUS_EVIDENCE"] else { return }
        for (mode, dark) in [("hold", false), ("hold", true), ("double-click", false), ("double-click", true)] {
            let overlay = T3QuitOverlay()
            let pill = overlay.makePill(mode: mode, dark: dark)
            let canvas = NSView(frame: NSRect(x: 0, y: 0, width: pill.frame.width + 80, height: pill.frame.height + 80))
            canvas.wantsLayer = true
            canvas.layer?.backgroundColor = (dark ? NSColor(srgbRed: 0.04, green: 0.04, blue: 0.04, alpha: 1) : NSColor(srgbRed: 0.988, green: 0.988, blue: 0.988, alpha: 1)).cgColor
            pill.setFrameOrigin(NSPoint(x: 40, y: 40))
            canvas.addSubview(pill)
            canvas.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
            let rep = canvas.bitmapImageRepForCachingDisplay(in: canvas.bounds)!
            canvas.cacheDisplay(in: canvas.bounds, to: rep)
            let data = rep.representation(using: .png, properties: [:])!
            try data.write(to: URL(fileURLWithPath: "\(directory)/quit-pill-\(mode)-\(dark ? "dark" : "light").png"))
            XCTAssertEqual(pill.frame.height, 64)
        }
    }
}

let suite = MenuTests.defaultTestSuite
suite.run()
guard let run = suite.testRun, run.executionCount == 10 else { exit(1) }
print("Menus: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
