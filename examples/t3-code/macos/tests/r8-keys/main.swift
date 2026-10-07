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
private func key(_ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command], window: NSWindow? = nil) -> NSEvent {
    NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window?.windowNumber ?? 0, context: nil,
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
        // Letters it answers, outside a typing context.
        XCTAssertTrue(launcher.consume(key("f", 3, [], window: window), typing: false))
        XCTAssertTrue(launcher.consume(key("L", 37, [.shift], window: window), typing: false))
        XCTAssertEqual(launcherView.keys, ["f", "L"])
        XCTAssertFalse(launcher.consume(key("x", 7, [], window: window), typing: false), "a letter it does not list types on")
        XCTAssertFalse(launcher.consume(key("f", 3, [], window: window), typing: true), "the composer keeps its letters")
        XCTAssertFalse(launcher.consume(key("f", 3, [.command], window: window), typing: false))
        // From the bare window (focus lost to a stray click) the launcher takes the focus back.
        window.makeFirstResponder(nil)
        XCTAssertTrue(launcher.consume(key("d", 2, [], window: window), typing: false))
        XCTAssertTrue(window.firstResponder === launcherView)
        launcherView.isHidden = true
        XCTAssertFalse(launcher.consume(key("d", 2, [], window: window), typing: false), "a hidden launcher answers nothing")
        launcher.remove(element)
        launcherView.isHidden = false
        XCTAssertFalse(launcher.consume(key("d", 2, [], window: window), typing: false))
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
exit(run.executionCount == 4 && run.totalFailureCount == 0 ? 0 : 1)
