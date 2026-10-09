import AppKit
import XCTest

// Lane r8-keys: the reference menu bar over the host's (R8KeysMenus.swift), routed
// keystrokes through real NSMenu key-equivalent lookup with the bar installed as
// the main menu, and the surface launcher's focus and letters (R8KeysLauncher.swift)
// in a real window. Compiled with every file in modules/apple (T3Module included).

private var acted: [(node: UInt32, action: UInt32)] = []
private let r8Resolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let r8Act: ExactHatches.ActFn = { _, node, action in acted.append((node, action)); return 0 }
private let r8Log: ExactHatches.LogFn = { _, _, _ in }
private let r8Delegate: ExactHatches.DelegateFn = { _, _, _ in }

private func makeHooks() -> ExactHatches {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(r8Resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r8Act, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r8Log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(r8Delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHatches(host: nil, table: UnsafeRawPointer(table))!
}

/// Stands in for the host's command items (ShortcutsMac.swift ShortcutHost): one
/// action for every item, the label as the title, enabled while its button is.
final class ShortcutHost: NSObject, NSMenuItemValidation {
    var pressed: [String] = []
    var disabled: Set<String> = []
    @objc func activate(_ item: NSMenuItem) { if validateMenuItem(item) { pressed.append(item.title) } }
    func validateMenuItem(_ item: NSMenuItem) -> Bool { !disabled.contains(item.title) }
}
final class DevTarget: NSObject {
    var reloads = 0, infos = 0
    @objc func reload(_ sender: Any?) { reloads += 1 }
    @objc func info(_ sender: Any?) { infos += 1 }
}
final class Recorder: NSObject { var closes = 0; @objc func performClose(_ sender: Any?) { closes += 1 } }

/// The bar DevMenuMac.makeMenu builds, with the host's command items already filed.
private func hostBar(_ host: ShortcutHost, _ dev: DevTarget) -> NSMenu {
    let bar = NSMenu()
    func add(_ title: String) -> NSMenu { let item = NSMenuItem(title: title, action: nil, keyEquivalent: ""); let menu = NSMenu(title: title); item.submenu = menu; bar.addItem(item); return menu }
    let app = add("T3 Code")
    app.addItem(withTitle: "About T3 Code", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
    app.addItem(withTitle: "Quit T3 Code", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
    let file = add("File")
    for title in ["Thread 1", "Undo", "Close Right Panel", "Toggle Diff", "Open in Editor", "Medium"] {
        let item = NSMenuItem(title: title, action: #selector(ShortcutHost.activate(_:)), keyEquivalent: "")
        item.target = host
        file.addItem(item)
    }
    file.addItem(.separator())
    file.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
    let edit = add("Edit")
    edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
    let redo = edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
    redo.keyEquivalentModifierMask = [.command, .shift]
    let view = add("View")
    let full = view.addItem(withTitle: "Enter Full Screen", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
    full.keyEquivalentModifierMask = [.command, .control]
    let go = add("Go")
    let back = NSMenuItem(title: "Back", action: #selector(ShortcutHost.activate(_:)), keyEquivalent: "[")
    back.target = host
    go.addItem(back)
    _ = add("Window")
    let develop = add("Develop")
    develop.addItem(withTitle: "Reload", action: #selector(DevTarget.reload(_:)), keyEquivalent: "r").target = dev
    develop.addItem(withTitle: "Open Project…", action: #selector(DevTarget.info(_:)), keyEquivalent: "o").target = dev
    develop.addItem(withTitle: "App Info…", action: #selector(DevTarget.info(_:)), keyEquivalent: "d").target = dev
    return bar
}
private func key(_ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command], window: NSWindow? = nil, at time: TimeInterval = 0) -> NSEvent {
    NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: time, windowNumber: window?.windowNumber ?? 0, context: nil,
        characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
}
/// What ShortcutHost.sync does on each plan batch: the declared chord onto its item.
private func sync(_ bar: NSMenu, _ chords: [String: (String, NSEvent.ModifierFlags)]) {
    let file = bar.items.first { $0.submenu?.title == "File" }!.submenu!
    for item in file.items where item.target is ShortcutHost {
        guard let (key, mask) = chords[item.title] else { continue }
        item.keyEquivalent = key
        item.keyEquivalentModifierMask = mask
    }
}

final class R8KeysTests: XCTestCase {
    let chords: [String: (String, NSEvent.ModifierFlags)] = ["Thread 1": ("1", .command), "Undo": ("z", .command), "Close Right Panel": ("w", .command),
                                                            "Toggle Diff": ("d", .command), "Open in Editor": ("o", .command), "Medium": ("e", [.command, .shift])]

    func testTheBarMatchesTheReferenceDesktopMenus() {
        _ = NSApplication.shared
        let host = ShortcutHost(), dev = DevTarget()
        let bar = hostBar(host, dev)
        let menus = T3Menus()
        menus.updatesDisabledReason = { "no feed" }
        menus.augment(bar)
        menus.augment(bar)
        XCTAssertEqual(bar.items.map(\.title), ["T3 Code", "File", "Edit", "View", "Window", "Help"], "no Develop or Go menu")
        let file = bar.items[1].submenu!, view = bar.items[3].submenu!
        XCTAssertEqual(file.items.filter { !$0.isHidden }.map(\.title), ["Close Window"], "File shows only Close Window (DesktopApplicationMenu.ts darwin)")
        XCTAssertTrue(file.items.filter { $0.isHidden }.allSatisfy(\.allowsKeyEquivalentWhenHidden), "the host's command items stay key equivalents")
        XCTAssertTrue(file.delegate === menus.keys)
        // A later plan batch files a new item: it is concealed before the menu shows.
        let later = NSMenuItem(title: "Thread 2", action: #selector(ShortcutHost.activate(_:)), keyEquivalent: "2")
        later.target = host
        file.insertItem(later, at: 0)
        menus.keys.menuNeedsUpdate(file)
        XCTAssertEqual(file.items.filter { !$0.isHidden }.map(\.title), ["Close Window"])
        XCTAssertEqual(view.items.prefix(3).map { $0.isSeparatorItem ? "—" : "\($0.title) \($0.keyEquivalent) \($0.keyEquivalentModifierMask.rawValue)" },
                       ["Reload r \(NSEvent.ModifierFlags.command.rawValue)", "Force Reload r \(NSEvent.ModifierFlags([.command, .shift]).rawValue)", "—"])
        XCTAssertEqual(view.items.filter { !$0.isHidden && !$0.isSeparatorItem }.map(\.title), ["Reload", "Force Reload", "Actual Size", "Zoom In", "Zoom Out", "Enter Full Screen"])
        let reload = view.items[0]
        XCTAssertTrue(menus.keys.validateMenuItem(reload))
        _ = reload.target!.perform(reload.action!, with: reload)
        XCTAssertEqual(dev.reloads, 1, "View › Reload runs the host's reload")
    }

    func testKeystrokesReachTheWindowsCommands() {
        _ = NSApplication.shared
        let host = ShortcutHost(), dev = DevTarget()
        let bar = hostBar(host, dev)
        NSApp.mainMenu = bar
        let menus = T3Menus()
        menus.updatesDisabledReason = { "no feed" }
        menus.augment(bar)
        menus.keys.keystroke = { true }
        sync(bar, chords)
        let file = bar.items.first { $0.submenu?.title == "File" }!.submenu!
        // ⌘D and ⌘O are free once Develop is gone: AppKit keeps them on the window's items.
        XCTAssertEqual(file.item(withTitle: "Toggle Diff")?.keyEquivalent, "d")
        XCTAssertEqual(file.item(withTitle: "Open in Editor")?.keyEquivalent, "o")
        XCTAssertTrue(bar.performKeyEquivalent(with: key("d", 2)))
        XCTAssertTrue(bar.performKeyEquivalent(with: key("o", 31)))
        XCTAssertTrue(bar.performKeyEquivalent(with: key("1", 18)))
        XCTAssertTrue(bar.performKeyEquivalent(with: key("e", 14, [.command, .shift])))
        XCTAssertEqual(host.pressed, ["Toggle Diff", "Open in Editor", "Thread 1", "Medium"])
        XCTAssertEqual(dev.infos, 0, "App Info… no longer takes ⌘D")
        // ⌘Z: Edit › Undo keeps the chord; with no editable text focused, a keystroke is thread.undo.
        host.pressed = []
        XCTAssertTrue(bar.performKeyEquivalent(with: key("z", 6)))
        XCTAssertEqual(host.pressed, ["Undo"])
        let undo = bar.items.first { $0.submenu?.title == "Edit" }!.submenu!.items[0]
        XCTAssertTrue(menus.keys.validateMenuItem(undo), "enabled while the undo notice is live")
        host.disabled = ["Undo"]
        XCTAssertFalse(menus.keys.validateMenuItem(undo), "nothing to undo")
        file.item(withTitle: "Undo").map { $0.menu?.removeItem($0) }
        XCTAssertFalse(menus.keys.validateMenuItem(undo))
        // ⌘W: an open right panel closes first; without one the window closes.
        host.pressed = []
        XCTAssertTrue(bar.performKeyEquivalent(with: key("w", 13)))
        XCTAssertEqual(host.pressed, ["Close Right Panel"])
        host.disabled.insert("Close Right Panel")
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.orderFront(nil)
        XCTAssertTrue(window.isVisible)
        menus.keys.closeTarget = { window }
        XCTAssertTrue(bar.performKeyEquivalent(with: key("w", 13)))
        XCTAssertFalse(window.isVisible, "no panel to close: Close Window closes the window")
        // A click on the menu item (no keystroke) is always Close Window.
        menus.keys.keystroke = { false }
        host.disabled = []
        let other = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        other.orderFront(nil)
        menus.keys.closeTarget = { other }
        host.pressed = []
        menus.keys.closeWindow(nil)
        XCTAssertEqual(host.pressed, [])
        XCTAssertFalse(other.isVisible)
        NSApp.mainMenu = nil
    }

    func testTheLauncherTakesFocusAndItsLetters() {
        _ = NSApplication.shared
        acted = []
        let hooks = makeHooks()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let editor = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
        editor.isEditable = true
        window.contentView!.addSubview(editor)
        let launcherView = LauncherView(frame: .zero)
        window.contentView!.addSubview(launcherView)
        window.makeFirstResponder(editor)
        let launcher = R8KeysLauncher()
        let element = ExactElement(hatch: .t3Launcher, id: "surface-chooser", node: 7, hatches: hooks)
        element.view = launcherView
        element.data = ExactData(["surface-launcher-keys": "FLD"])
        launcher.install(element)
        XCTAssertTrue(window.firstResponder === editor, "a launcher with no size yet waits")
        launcherView.frame = NSRect(x: 220, y: 0, width: 260, height: 300) // laid out
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertTrue(acted.contains { $0.node == 7 && $0.action == 1 }, "focus() on mount, as focusOnMount does")
        XCTAssertTrue(window.firstResponder === launcherView)
        // Letters it answers, outside a typing context: with the focus on it they go on to Exact's key route,
        // which runs its `key` handlers (a view's keyDown runs none), never to the view or to type-to-focus.
        XCTAssertEqual(launcher.route(key("f", 3, [], window: window), typing: false), .pass)
        XCTAssertEqual(launcher.route(key("L", 37, [.shift], window: window), typing: false), .pass)
        XCTAssertEqual(launcherView.keys, [], "nothing is handed to the view's keyDown")
        XCTAssertEqual(launcher.route(key("x", 7, [], window: window), typing: false), .none, "a letter it does not list types on")
        XCTAssertEqual(launcher.route(key("f", 3, [], window: window), typing: true), .none, "the composer keeps its letters")
        XCTAssertEqual(launcher.route(key("f", 3, [.command], window: window), typing: false), .none)
        // From the bare window (focus lost to a stray click) the launcher takes the focus back and the key comes again,
        // at the head of the queue, for Exact's route to hear at the launcher.
        window.makeFirstResponder(nil)
        let d = key("d", 2, [], window: window)
        XCTAssertEqual(launcher.route(d, typing: false), .taken)
        XCTAssertTrue(window.firstResponder === launcherView)
        let again = NSApp.nextEvent(matching: .keyDown, until: Date(), inMode: .default, dequeue: true)
        XCTAssertEqual(again?.characters, "d", "posted again")
        // The copy posted again is never posted a second time, even when the focus left meanwhile.
        window.makeFirstResponder(nil)
        XCTAssertEqual(launcher.route(again!, typing: false), .none)
        XCTAssertNil(NSApp.nextEvent(matching: .keyDown, until: Date(), inMode: .default, dequeue: true))
        window.makeFirstResponder(launcherView)
        launcherView.isHidden = true
        XCTAssertEqual(launcher.route(d, typing: false), .none, "a hidden launcher answers nothing")
        launcher.remove(element)
        launcherView.isHidden = false
        XCTAssertEqual(launcher.route(d, typing: false), .none)
    }
    /// realinput-1010-fixes RI-1: AppKit calls a window's local key monitors in no fixed order (it changes as monitors
    /// come and go: the panel's reopen did it). A real launcher letter must reach Exact's key route (ExactViewMac's
    /// session monitor; the launcher's `key` handlers run only from there) with the launcher focused, whether the
    /// composer's monitor (type-to-focus, T3Composer.handle) runs before Exact's or after it.
    func testALauncherLetterReachesExactsKeyRouteInEitherMonitorOrder() {
        _ = NSApplication.shared
        acted = []
        let hooks = makeHooks()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let text = NSTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
        text.isEditable = true
        window.contentView!.addSubview(text)
        let launcherView = LauncherView(frame: NSRect(x: 220, y: 0, width: 260, height: 300))
        window.contentView!.addSubview(launcherView)
        let composer = T3Composer()
        let field = ExactElement(hatch: .t3Composer, id: "composer", node: 3, hatches: hooks)
        field.view = text
        field.platform = text
        composer.install(field)
        let launcher = R8KeysLauncher()
        composer.launcher = launcher
        let element = ExactElement(hatch: .t3Launcher, id: "surface-chooser", node: 7, hatches: hooks)
        element.view = launcherView
        element.data = ExactData(["surface-launcher-keys": "BTFDL"])
        launcher.install(element)
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertTrue(window.firstResponder === launcherView, "focused on mount")
        // Exact's monitor (Presenter.routeKey): the `key` handlers at the first responder hear the key; the launcher's
        // prevent a letter it answers, so the event goes no further.
        var heard: [String] = []
        func exact(_ event: NSEvent) -> NSEvent? {
            guard window.firstResponder === launcherView else { return event }
            heard.append(event.characters ?? "")
            return nil
        }
        func press(_ event: NSEvent, composerFirst: Bool) {
            var next: NSEvent? = event
            while let current = next {
                if composerFirst { _ = composer.handle(current).flatMap(exact) } else { _ = exact(current).flatMap(composer.handle) }
                next = NSApp.nextEvent(matching: .keyDown, until: Date(), inMode: .default, dequeue: true)
            }
        }
        var time: TimeInterval = 100 // each press its own timestamp, as a keyboard's
        for composerFirst in [false, true] {
            heard = []
            time += 1; press(key("f", 3, [], window: window, at: time), composerFirst: composerFirst)
            XCTAssertEqual(heard, ["f"], "F reaches the launcher's key handlers once, composer's monitor first: \(composerFirst)")
            // Focus lost to a stray click: the launcher takes it back and still hears the letter once.
            window.makeFirstResponder(nil)
            heard = []
            time += 1; press(key("d", 2, [], window: window, at: time), composerFirst: composerFirst)
            XCTAssertEqual(heard, ["d"], "D from the bare window, composer's monitor first: \(composerFirst)")
            XCTAssertTrue(window.firstResponder === launcherView)
        }
        XCTAssertEqual(text.string, "", "type-to-focus never takes a launcher letter")
        XCTAssertEqual(launcherView.keys, [])
        // A letter the launcher does not list still goes to the composer (ChatView's type-to-focus).
        heard = []
        XCTAssertNil(composer.handle(key("x", 7, [], window: window, at: time + 1)))
        XCTAssertEqual(text.string, "x")
        composer.destroy()
    }
    func testMeasureReportsTheDrawnFrame() {
        _ = NSApplication.shared
        let hooks = makeHooks()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let scroll = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        window.contentView!.addSubview(scroll)
        let button = NSView(frame: NSRect(x: 300, y: 200, width: 24, height: 24))
        scroll.addSubview(button)
        let measure = R8KeysMeasure()
        let element = ExactElement(hatch: .t3Measure, id: "", node: 9, hatches: hooks)
        element.view = button
        element.data = ExactData(["frame": "table-copy-1"])
        measure.install(element)
        let first = measure.perform(["name": "table-copy-1", "generation": 3])
        XCTAssertEqual(first["ok"] as? Bool, true)
        let box = first["value"] as? [String: Double] ?? [:]
        XCTAssertEqual(box["x"], 300); XCTAssertEqual(box["y"], 300 - 224); XCTAssertEqual(box["width"], 24); XCTAssertEqual(box["windowHeight"], 300)
        scroll.setFrameOrigin(NSPoint(x: 0, y: 46)) // the transcript scrolled: the drawn position moves
        XCTAssertEqual((measure.perform(["name": "table-copy-1"])["value"] as? [String: Double])?["y"], 300 - 270)
        measure.remove(element)
        XCTAssertEqual(measure.perform(["name": "table-copy-1"])["ok"] as? Bool, false)
    }
    /// fix-misc-batch (#298 bug 3): ⌘Z, ⇧⌘Z and Edit › Undo undo typing in an Exact textarea, whose text view keeps
    /// its own history (host TextAreaMac.swift `textUndo`), as the composer and the prompt preview are.
    func testUndoAndRedoActOnTheFocusedTextsOwnHistory() {
        _ = NSApplication.shared
        let host = ShortcutHost(), dev = DevTarget()
        let bar = hostBar(host, dev)
        NSApp.mainMenu = bar
        let menus = T3Menus()
        menus.updatesDisabledReason = { "no feed" }
        menus.augment(bar)
        menus.keys.keystroke = { true }
        sync(bar, chords)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let text = OwnHistoryText(frame: NSRect(x: 0, y: 0, width: 300, height: 100))
        text.allowsUndo = true
        window.contentView = text
        window.makeFirstResponder(text)
        // Why the old route did nothing: `undo:` up the responder chain is NSWindow's, on the window's manager.
        text.insertText("stable", replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(text.tryToPerform(Selector(("undo:")), with: nil))
        XCTAssertEqual(text.string, "stable", "the window's manager holds none of the text's steps")
        menus.keys.focusedText = { text }
        let edit = bar.items.first { $0.submenu?.title == "Edit" }!.submenu!
        let undo = edit.items.first { $0.title == "Undo" }!, redo = edit.items.first { $0.title == "Redo" }!
        XCTAssertTrue(menus.keys.validateMenuItem(undo))
        XCTAssertFalse(menus.keys.validateMenuItem(redo), "nothing undone yet")
        host.pressed = []
        XCTAssertTrue(bar.performKeyEquivalent(with: key("z", 6, window: window)))
        XCTAssertEqual(text.string, "", "⌘Z undoes the typing")
        XCTAssertEqual(host.pressed, [], "and is not thread.undo")
        XCTAssertTrue(menus.keys.validateMenuItem(redo))
        // ⇧⌘Z is Edit › Redo's own chord, now on this target (a synthetic shifted event does not match a menu
        // item reliably, so the item's action is sent as the menu sends it; the real chord is in the live drive).
        XCTAssertTrue(redo.target === menus.keys)
        XCTAssertEqual(redo.keyEquivalent, "z"); XCTAssertEqual(redo.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask), [.command, .shift])
        XCTAssertTrue(NSApp.sendAction(redo.action!, to: redo.target, from: redo))
        XCTAssertEqual(text.string, "stable", "⇧⌘Z redoes it")
        menus.keys.keystroke = { false } // a click on the item
        menus.keys.undo(undo)
        XCTAssertEqual(text.string, "", "Edit › Undo from the menu")
        menus.keys.redo(redo)
        XCTAssertEqual(text.string, "stable", "Edit › Redo from the menu")
        // While it composes (marked text, a Korean or Japanese input source), the history is left alone.
        menus.keys.keystroke = { true }
        text.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertTrue(text.hasMarkedText())
        XCTAssertFalse(menus.keys.validateMenuItem(undo))
        XCTAssertTrue(bar.performKeyEquivalent(with: key("z", 6, window: window)))
        XCTAssertTrue(text.string.hasPrefix("stable"), "no undo under a composition")
        text.unmarkText()
        // Editable text with the focus and nothing to undo: ⌘Z is not thread.undo (`!editableFocus`).
        text.undoManager?.removeAllActions()
        XCTAssertTrue(bar.performKeyEquivalent(with: key("z", 6, window: window)))
        XCTAssertEqual(host.pressed, [])
        // No editable text with the focus: the keystroke is the window's thread.undo.
        menus.keys.focusedText = { nil }
        XCTAssertTrue(bar.performKeyEquivalent(with: key("z", 6, window: window)))
        XCTAssertEqual(host.pressed, ["Undo"])
        NSApp.mainMenu = nil
    }
}
/// Exact's textarea (host TextAreaMac.swift `TextArea`): its own undo manager, not the window's.
final class OwnHistoryText: NSTextView {
    private let history = UndoManager()
    override var undoManager: UndoManager? { history }
}
final class LauncherView: NSView {
    var keys: [String] = []
    override var acceptsFirstResponder: Bool { true }
    override func keyDown(with event: NSEvent) { keys.append(event.characters ?? "") }
}

let suite = XCTestSuite(forTestCaseClass: R8KeysTests.self)
suite.run()
let run = suite.testRun!
print("R8 keys tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.executionCount == 6 && run.totalFailureCount == 0 ? 0 : 1)
