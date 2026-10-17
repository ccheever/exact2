import AppKit
import XCTest

// A real key reaches the window's commands only through
// NSApplication.sendEvent, the window server's path: the app's local key monitors, the key window's
// performKeyEquivalent (the composer's text view), then the main menu, where the host files every button that declares
// a ⌘ chord as a hidden key equivalent (ShortcutsMac.swift ShortcutHost.sync). The session's own monitor presses the
// button first when the chord matches the typed characters; the hidden item answers the rest (⇧⌘[ types "{", which
// the session's match misses and AppKit's menu matches). An agent's key goes straight to the window, so only this
// path can lose a chord. Every ⌘ chord of the reference's defaults that holds in an open thread is sent here as the
// window server sends it (a CGEvent-backed NSEvent through NSApp.sendEvent), from the composer and with nothing
// focused, past the clone's own monitors (T3Menus, T3Composer with its launcher-letter guard, T3ComposerInput, T3LatinChords, T3Sidebar,
// T3TextContextMenu, T3ComposerIntent, T3Timeline, RightPanelTabsInput, T3ActivityReporter) and its menu bar
// (T3Menus.augment, T3MenuBar.arrange) over the host's, and must reach the host's monitor and the host's item for
// that chord, leaving the composer's text and the focus alone. The host's parts are stand-ins, as in menu-bar: its bar
// (DevMenuMac.makeMenu's items), a session monitor that only records (the host's presses and consumes), and
// ShortcutHost's items, filed after the clone's augment as each plan batch files them, with the chords they take from
// other items (`claim`). Compiled with every file in modules/apple (T3Module included); it runs NSApplication's own
// event loop, so the app is active and its window key, as under real keys, where the session allows it (`screenLocked`).
// A coverage guard, not a reproduction: ⇧⌘F was once lost before the app, so this passed on the modules from before
// its fix too; it fails when a later module or menu change takes one
// of these chords on the way.

/// The reference's ⌘ chords (packages/shared/src/keybindings.ts DEFAULT_KEYBINDINGS; keybindingDefaults) whose
/// `when` holds in an open thread with nothing focused or the composer focused, the composer's own send chords
/// (`composerFocus && …`) aside. search-key-and-timeline-commit-title.test.ts checks this list against them.
let referenceChords = ["mod+b", "mod+[", "mod+]", "mod+j", "mod+alt+b", "mod+w", "mod+d", "mod+shift+j", "mod+k", "mod+p",
                       "mod+shift+f", "mod+u", "mod+alt+a", "mod+alt+shift+a", "mod+alt+shift+t", "mod+s", "mod+shift+enter",
                       "mod+n", "mod+shift+o", "mod+shift+n", "mod+alt+n", "mod+shift+m", "mod+shift+h", "mod+shift+e",
                       "mod+shift+a", "mod+shift+x", "mod+shift+g", "mod+shift+l", "mod+shift+k", "mod+o", "mod+shift+[",
                       "mod+shift+]", "mod+shift+c", "mod+shift+s", "mod+shift+p", "mod+z", "mod+1", "mod+2", "mod+3",
                       "mod+4", "mod+5", "mod+6", "mod+7", "mod+8", "mod+9"]
/// ⌘Z is Edit › Undo's: the focused text's own history, else the window's thread.undo (T3MenuBar.undo), which
/// menu-bar's testKeystrokesReachTheWindowsCommands and testUndoAndRedoActOnTheFocusedTextsOwnHistory drive.
let editMenuChords: Set<String> = ["mod+z"]

/// ANSI key codes, as a US keyboard sends them.
private let keyCodes: [String: UInt16] = ["a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11,
                                          "q": 12, "w": 13, "e": 14, "r": 15, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22,
                                          "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31, "u": 32,
                                          "[": 33, "i": 34, "p": 35, "enter": 36, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46]

struct Chord {
    let key: String
    private var tokens: [String] { key.split(separator: "+").map(String.init) }
    var code: UInt16 { keyCodes[tokens.last!]! }
    var keyEquivalent: String { tokens.last == "enter" ? "\r" : tokens.last! }
    var mask: NSEvent.ModifierFlags {
        var mask: NSEvent.ModifierFlags = []
        for token in tokens.dropLast() {
            switch token { case "mod": mask.insert(.command); case "shift": mask.insert(.shift); case "alt": mask.insert(.option); case "ctrl": mask.insert(.control); default: break }
        }
        return mask
    }
    var flags: CGEventFlags {
        var flags: CGEventFlags = []
        if mask.contains(.command) { flags.insert(.maskCommand) }; if mask.contains(.shift) { flags.insert(.maskShift) }
        if mask.contains(.option) { flags.insert(.maskAlternate) }; if mask.contains(.control) { flags.insert(.maskControl) }
        return flags
    }
}
private let chordMask: NSEvent.ModifierFlags = [.command, .shift, .option, .control]

/// Stands in for the host's ShortcutHost (ShortcutsMac.swift; the name T3MenuBar.isCommand reads): one item per
/// declared chord, pressed by the menu.
final class ShortcutHost: NSObject, NSMenuItemValidation {
    var pressed: [String] = []
    @objc func activate(_ item: NSMenuItem) { pressed.append(item.title) }
    func validateMenuItem(_ item: NSMenuItem) -> Bool { true }
    /// ShortcutHost.sync on a plan batch: an item elsewhere in the bar with a declared chord gives it up (`claim`),
    /// and each command's item goes into File with its chord, where T3MenuBar keeps it a hidden key equivalent.
    func file(_ chords: [Chord], in bar: NSMenu) {
        var menus = bar.items.compactMap(\.submenu)
        while let menu = menus.popLast() {
            for item in menu.items where !(item.target is ShortcutHost) {
                if let sub = item.submenu { menus.append(sub) }
                if !item.keyEquivalent.isEmpty, chords.contains(where: { Self.sameChord(item, key: $0.keyEquivalent, $0.mask) }) {
                    item.keyEquivalent = ""
                }
            }
        }
        let file = bar.items.first { $0.submenu?.title == "File" }!.submenu!
        for (index, chord) in chords.enumerated() {
            let item = NSMenuItem(title: chord.key, action: #selector(activate(_:)), keyEquivalent: chord.keyEquivalent)
            item.keyEquivalentModifierMask = chord.mask
            item.target = self
            item.isHidden = true
            item.allowsKeyEquivalentWhenHidden = true
            file.insertItem(item, at: index)
        }
    }
    /// ShortcutsMac.swift's `sameChord` and `same`, as they are: the item's key equivalent lowercased, its modifiers
    /// against the command's. An uppercase equivalent is not read as ⇧ there (AppKit reads it so), so not here either:
    /// a bar item with one would keep its chord beside a command's, which `testNoBarItemHasAnUppercaseKeyEquivalent`
    /// rules out.
    static func sameChord(_ item: NSMenuItem, key: String, _ mask: NSEvent.ModifierFlags) -> Bool {
        !key.isEmpty && item.keyEquivalent.lowercased() == key.lowercased()
            && item.keyEquivalentModifierMask.intersection(chordMask) == mask.intersection(chordMask)
    }
}
final class DevTarget: NSObject { @objc func reload(_ sender: Any?) {}; @objc func other(_ sender: Any?) {} }

/// DevMenuMac.makeMenu's bar: every item with a chord, in its menu.
private func hostBar(_ dev: DevTarget) -> NSMenu {
    let bar = NSMenu()
    func add(_ title: String, _ items: [(String, Selector?, String, NSEvent.ModifierFlags)]) -> NSMenu {
        let menu = NSMenu(title: title)
        for (name, action, key, mask) in items {
            let item = menu.addItem(withTitle: name, action: action, keyEquivalent: key)
            item.keyEquivalentModifierMask = mask
        }
        bar.addItem(withTitle: title, action: nil, keyEquivalent: "").submenu = menu
        return menu
    }
    _ = add("T3 Code", [("About T3 Code", #selector(NSApplication.orderFrontStandardAboutPanel(_:)), "", []),
                        ("Hide T3 Code", #selector(NSApplication.hide(_:)), "h", .command), ("Hide Others", #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]),
                        ("Quit T3 Code", #selector(NSApplication.terminate(_:)), "q", .command)])
    _ = add("File", [("Close Window", #selector(NSWindow.performClose(_:)), "w", .command)])
    let edit = add("Edit", [("Undo", Selector(("undo:")), "z", .command), ("Redo", Selector(("redo:")), "z", [.command, .shift]),
                            ("Cut", #selector(NSText.cut(_:)), "x", .command), ("Copy", #selector(NSText.copy(_:)), "c", .command),
                            ("Paste", #selector(NSText.paste(_:)), "v", .command), ("Paste and Match Style", #selector(NSTextView.pasteAsPlainText(_:)), "v", [.command, .option, .shift]),
                            ("Select All", #selector(NSText.selectAll(_:)), "a", .command)])
    let find = NSMenu(title: "Find")
    for (name, key, mask) in [("Find…", "f", NSEvent.ModifierFlags.command), ("Find and Replace…", "f", [.command, .option]), ("Find Next", "g", .command),
                              ("Find Previous", "g", [.command, .shift]), ("Use Selection for Find", "e", .command), ("Jump to Selection", "j", .command)] {
        find.addItem(withTitle: name, action: #selector(NSResponder.performTextFinderAction(_:)), keyEquivalent: key).keyEquivalentModifierMask = mask
    }
    edit.addItem(withTitle: "Find", action: nil, keyEquivalent: "").submenu = find
    _ = add("View", [("Enter Full Screen", #selector(NSWindow.toggleFullScreen(_:)), "f", [.command, .control])])
    _ = add("Go", [])
    _ = add("Window", [("Minimize", #selector(NSWindow.performMiniaturize(_:)), "m", .command)])
    let develop = add("Develop", [("Reload", #selector(DevTarget.reload(_:)), "r", .command), ("Open Project…", #selector(DevTarget.other(_:)), "o", .command),
                                  ("App Info…", #selector(DevTarget.other(_:)), "d", .command), ("Save Trace", #selector(DevTarget.other(_:)), "t", [.command, .option])])
    for item in develop.items { item.target = dev }
    _ = add("Help", [("T3 Code Help", #selector(NSApplication.showHelp(_:)), "?", .command)])
    return bar
}

private let resolveFn: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let actFn: ExactHatches.ActFn = { _, _, _ in 0 }
private let logFn: ExactHatches.LogFn = { _, _, _ in }
private let delegateFn: ExactHatches.DelegateFn = { _, _, _ in }
private func makeHooks() -> ExactHatches {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(resolveFn, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(actFn, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(logFn, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(delegateFn, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHatches(host: nil, table: UnsafeRawPointer(table))!
}
/// Runs the app's event loop for a while: activation and window ordering arrive as events, as main-queue work runs.
private func tick(_ seconds: TimeInterval = 0.05) {
    let end = Date(timeIntervalSinceNow: seconds)
    while Date() < end { if let event = NSApp.nextEvent(matching: .any, until: end, inMode: .default, dequeue: true) { NSApp.sendEvent(event) } }
}
final class Flipped: NSView { override var isFlipped: Bool { true } }

/// An open thread's window with the clone's modules mounted as T3Module mounts them.
final class Fixture {
    let hooks = makeHooks()
    let host = ShortcutHost(), dev = DevTarget()
    let window = NSWindow(contentRect: NSRect(x: 240, y: 240, width: 640, height: 420), styleMask: [.titled, .closable], backing: .buffered, defer: false)
    let composerText = NSTextView(frame: NSRect(x: 20, y: 300, width: 600, height: 80))
    let composer = T3Composer(), menus = T3Menus()
    let composerInput = T3ComposerInput(), latinChords = T3LatinChords(agent: false), sidebar = T3Sidebar(agent: false, changed: { _ in })
    let textMenu = T3TextContextMenu(agent: false), intent = T3ComposerIntent(), timeline = T3Timeline(changed: { _ in })
    let panelTabs = RightPanelTabsInput(), activity = T3ActivityReporter(persistent: false, dataDirectory: nil, pageFacts: true)
    var elements: [ExactElement] = []
    /// What the host's session monitor saw: each keydown's key code and modifiers.
    var session: [(UInt16, NSEvent.ModifierFlags)] = []
    private var monitor: Any?

    init() {
        window.isReleasedWhenClosed = false
        let content = Flipped(frame: NSRect(x: 0, y: 0, width: 640, height: 420))
        window.contentView = content
        NSApp.mainMenu = hostBar(dev)
        // The host's monitor is the window's first (ExactViewMac.viewDidMoveToWindow), before any module mounts.
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if let self, event.window === self.window { self.session.append((event.keyCode, event.modifierFlags.intersection(chordMask))) }
            return event
        }
        let transcript = NSView(frame: NSRect(x: 0, y: 0, width: 640, height: 280)), send = NSButton(frame: NSRect(x: 580, y: 384, width: 40, height: 24))
        let tab = NSView(frame: NSRect(x: 600, y: 0, width: 20, height: 20))
        composerText.isEditable = true
        composerText.allowsUndo = true
        for view in [transcript, composerText, send, tab] { content.addSubview(view) }
        func element(_ hatch: ExactHatchKey, _ id: String, _ node: UInt32, _ view: NSView, _ data: [String: String] = [:]) -> ExactElement {
            let element = ExactElement(hatch: hatch, id: id, node: node, hatches: hooks)
            element.view = view; element.platform = view; element.data = ExactData(data)
            elements.append(element)
            return element
        }
        let composerElement = element(.t3Composer, "composer", 3, composerText, ["snapshot-owner": "t1"])
        let sendElement = element(.t3Send, "send", 4, send)
        let transcriptElement = element(.t3Transcript, "transcript", 5, transcript, ["timeline-owner": "t1", "timeline-rest": "yes"])
        let tabElement = element(.t3Anchor, "", 6, tab, ["anchor": "tab-editor:files"])
        activate()
        menus.updatesDisabledReason = { "no feed" }
        sidebar.install(); textMenu.install()
        activity.connect(UUID(), environment: "env") { _, done in done() }
        // T3Module.element's order.
        for element in [composerElement, sendElement, transcriptElement, tabElement] {
            panelTabs.install(element); composer.install(element); timeline.install(element); menus.install(element); composerInput.install(element)
        }
        tick(0.2)
        // The next plan batch files the window's commands over the clone's bar.
        host.file(referenceChords.filter { !editMenuChords.contains($0) }.map(Chord.init), in: NSApp.mainMenu!)
        tick()
    }

    /// The app active and the window key, as under real keys (another app may take the activation between tests).
    @discardableResult func activate() -> Bool {
        for _ in 0..<60 where !(NSApp.isActive && NSApp.keyWindow === window) {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            tick(0.05)
        }
        return NSApp.isActive && NSApp.keyWindow === window
    }

    /// One press of `chord` as the window server delivers it.
    func press(_ chord: Chord) {
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: chord.code, keyDown: down)!
            event.flags = chord.flags
            event.setIntegerValueField(CGEventField(rawValue: 51)!, value: Int64(window.windowNumber)) // the window under the key
            NSApp.sendEvent(NSEvent(cgEvent: event)!)
        }
        tick(0.02)
    }

    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        composer.destroy(); latinChords.destroy(); timeline.destroy(); panelTabs.destroy(); textMenu.destroy(); activity.destroy()
        window.orderOut(nil)
        NSApp.mainMenu = nil
    }
}

final class RealKeyChordsTests: XCTestCase {
    static var shared: Fixture?
    var fixture: Fixture { Self.shared! }

    func checkEveryChord(focus: NSResponder?, label: String, except taken: Set<String> = []) {
        let f = fixture
        XCTAssertTrue(f.activate() || screenLocked, "the app is active and its window key, as under real keys")
        XCTAssertEqual(NSApp.mainMenu?.items.map(\.title), ["T3 Code", "File", "Edit", "View", "Window", "Help"], "the clone's bar (no Develop, no Go)")
        for key in referenceChords where !editMenuChords.contains(key) && !taken.contains(key) {
            let chord = Chord(key: key)
            f.window.makeFirstResponder(focus)
            f.composerText.string = "draft"
            f.composerText.setSelectedRange(NSRange(location: 5, length: 0))
            f.host.pressed = []; f.session = []
            f.press(chord)
            XCTAssertEqual(f.host.pressed, [key], "\(key) \(label): the window's command item")
            XCTAssertEqual(f.session.filter { $0.0 == chord.code && $0.1 == chord.mask }.count, 1, "\(key) \(label): the host's session monitor saw it")
            XCTAssertEqual(f.composerText.string, "draft", "\(key) \(label): nothing typed")
            XCTAssertTrue(f.window.firstResponder === (focus ?? f.window), "\(key) \(label): the focus stays")
        }
    }

    func testEveryChordFromTheComposer() {
        // The reference's rich text composer claims ⌘B for bold (AppSidebarLayout.tsx); below.
        checkEveryChord(focus: fixture.composerText, label: "from the composer", except: ["mod+b"])
    }

    func testEveryChordWithNothingFocused() {
        checkEveryChord(focus: nil, label: "with nothing focused")
    }

    /// AppKit reads an uppercase letter key equivalent as ⇧ with it; ShortcutHost.sync's claim compares it lowercased
    /// with the item's own modifiers, so such an item in the clone's bar would keep a command's ⇧ chord beside the
    /// command's item. None has one.
    func testNoBarItemHasAnUppercaseKeyEquivalent() {
        var menus = NSApp.mainMenu?.items.compactMap(\.submenu) ?? [], upper: [String] = [], chords = 0
        while let menu = menus.popLast() {
            for item in menu.items {
                if let sub = item.submenu { menus.append(sub) }
                if !item.keyEquivalent.isEmpty { chords += 1 }
                if item.keyEquivalent.count == 1, item.keyEquivalent != item.keyEquivalent.lowercased() { upper.append("\(menu.title) › \(item.title) \(item.keyEquivalent)") }
            }
        }
        XCTAssertGreaterThan(chords, referenceChords.count, "the host's, the clone's and the commands' items")
        XCTAssertEqual(upper, [])
    }

    func testCommandBIsTheRichTextComposersBold() {
        let f = fixture
        XCTAssertTrue(f.activate() || screenLocked)
        f.window.makeFirstResponder(f.composerText)
        f.composerText.string = "draft"
        f.composerText.setSelectedRange(NSRange(location: 0, length: 5))
        f.host.pressed = []
        f.press(Chord(key: "mod+b"))
        XCTAssertEqual(f.host.pressed, [], "the composer's bold, not sidebar.toggle")
        XCTAssertEqual(f.composerText.string, "**draft**")
        f.composer.editor.styler.richText = false
        f.composerText.string = "draft"
        f.press(Chord(key: "mod+b"))
        XCTAssertEqual(f.host.pressed, ["mod+b"], "the plain text composer leaves ⌘B to sidebar.toggle")
        f.composer.editor.styler.richText = true
    }
}

/// A locked screen lets no app become active, so there the key window's own performKeyEquivalent (the composer's text
/// view) is not on the path; the monitors and the main menu still are. The run says which it had.
let screenLocked = (CGSessionCopyCurrentDictionary() as? [String: Any])?["CGSSessionScreenIsLocked"] as? Bool ?? false

final class Delegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        DispatchQueue.main.async {
            RealKeyChordsTests.shared = Fixture()
            let suite = XCTestSuite(forTestCaseClass: RealKeyChordsTests.self)
            suite.run()
            RealKeyChordsTests.shared?.destroy()
            let run = suite.testRun!
            print("Real key chords tests: \(run.executionCount) run, \(run.totalFailureCount) failed (\(screenLocked ? "screen locked: the app was not active, so no key window" : "the app active, its window key"))")
            exit(run.executionCount == suite.testCaseCount && run.totalFailureCount == 0 ? 0 : 1)
        }
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
