import AppKit
import XCTest

// Compile with the production native-module facade, its generated app keys,
// T3WindowChrome.swift and T3Menus.swift (see README, palette lane). Real AppKit
// menus, windows and key events; the quit timers run on an injected clock.
let exactModule: ExactModule.Type = ExactModule.self

/// A clock and timer queue the tests advance by hand, kept in whole milliseconds (as vitest's
/// fake timers are) so a 1.2 s hold reached by twelve 100 ms steps is exactly 1.2 s.
private final class Clock {
    var ms: Double = 100_000
    var time: TimeInterval { ms / 1000 }
    var timers: [(at: Double, id: Int, work: () -> Void)] = []
    var serial = 0
    func schedule(_ delay: TimeInterval, _ work: @escaping () -> Void) -> () -> Void {
        serial += 1
        let id = serial
        timers.append((ms + (delay * 1000).rounded(), id, work))
        return { [weak self] in self?.timers.removeAll { $0.id == id } }
    }
    func advance(_ seconds: TimeInterval) {
        let end = ms + (seconds * 1000).rounded()
        while let next = timers.filter({ $0.at <= end }).min(by: { $0.at < $1.at }) {
            timers.removeAll { $0.id == next.id }
            ms = next.at
            next.work()
        }
        ms = end
    }
}

/// QuitHold.test.ts makeHarness / makeInput: the reference's inputs on the injected clock.
private struct Input {
    var type = "keyDown", key = "q", meta = true, alt = false, shift = false, isAutoRepeat = false
}
private final class Harness {
    let clock = Clock()
    let hold = T3QuitHold()
    var notifications: [String] = []
    var concealed = 0
    var quits = 0
    var prevented = 0
    /// Pending mode reads (a nil mode leaves each read for the test to answer).
    var resolvers: [(String?) -> Void] = []
    init(mode: String? = "hold") {
        hold.now = { [unowned self] in self.clock.time }
        hold.schedule = { [unowned self] delay, work in self.clock.schedule(delay, work) }
        hold.getMode = { [unowned self] done in if let mode { done(mode) } else { self.resolvers.append(done) } }
        hold.notify = { [unowned self] down, mode in self.notifications.append(down ? "down:\(mode)" : "up") }
        hold.quit = { [unowned self] in self.quits += 1 }
        hold.conceal = { [unowned self] in self.concealed += 1 }
    }
    func send(_ input: Input) {
        if input.type == "keyUp" { hold.keyUp(key: input.key.lowercased()); return }
        if hold.keyDown(key: input.key.lowercased(), command: input.meta, option: input.alt, shift: input.shift, isRepeat: input.isAutoRepeat) { prevented += 1 }
    }
    func send(_ build: (inout Input) -> Void = { _ in }) { var input = Input(); build(&input); send(input) }
    /// The OS auto-repeating the held shortcut every `interval` ms for `duration` ms.
    func holdFor(_ duration: Double, interval: Double = 100, _ build: (inout Input) -> Void = { _ in }) {
        var elapsed = 0.0
        while elapsed < duration {
            clock.advance(interval / 1000)
            send { $0.isAutoRepeat = true; build(&$0) }
            elapsed += interval
        }
    }
    func advance(_ ms: Double) { clock.advance(ms / 1000) }
}
private let HOLD_MS = T3QuitHold.holdDuration * 1000, DOUBLE_MS = T3QuitHold.doublePress * 1000, GRACE_MS = T3QuitHold.releaseGrace * 1000
private let HOLD_DOWN = "down:hold", DOUBLE_CLICK_DOWN = "down:double-click", UP = "up"

/// A window stand-in for concealPendingQuitWindow's test.
private final class FakeWindow: T3ConcealableWindow {
    var isConcealable = true, isFullScreenWindow = false
    var calls: [String] = []
    func leaveFullScreen() { calls.append("setFullScreen(false)") }
    var alphaValue: CGFloat = 1 { didSet { calls.append("setOpacity(\(alphaValue))") } }
}

/// QuitHold.test.ts, case by case (names kept; "uses control on non-mac platforms" does not apply on macOS).
final class QuitHoldTests: XCTestCase {
    func test_shows_the_hint_on_a_tap_without_quitting_even_when_the_release_is_never_seen() {
        let h = Harness()
        h.send()
        XCTAssertEqual(h.prevented, 1)
        XCTAssertEqual(h.notifications, [HOLD_DOWN])
        h.advance(HOLD_MS + GRACE_MS)
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func test_conceals_a_completed_hold_then_quits_after_release() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS + 200)
        XCTAssertEqual(h.concealed, 1)
        XCTAssertEqual(h.quits, 0)
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        XCTAssertEqual(h.quits, 0)
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func test_keeps_a_concealed_hold_committed_when_another_key_is_pressed() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS)
        h.send { $0.key = "Shift"; $0.shift = true }
        XCTAssertEqual(h.concealed, 1)
        XCTAssertEqual(h.quits, 0)
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 1)
    }
    func test_keeps_a_concealed_hold_committed_through_a_fresh_Cmd_Q_press() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS)
        h.send()
        XCTAssertEqual(h.concealed, 1)
        XCTAssertEqual(h.quits, 0)
        h.send { $0.type = "keyUp" }
        XCTAssertEqual(h.quits, 1)
    }
    func test_quits_when_a_completed_hold_goes_quiet_without_release_events() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS + GRACE_MS * 2)
        XCTAssertEqual(h.quits, 0)
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func test_waits_for_slow_repeats_to_stop_before_quitting() {
        let h = Harness()
        h.send()
        h.advance(300); h.send { $0.isAutoRepeat = true }
        h.advance(900); h.send { $0.isAutoRepeat = true }
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 0)
        h.advance(300); h.send { $0.isAutoRepeat = true }
        h.advance(1_799)
        XCTAssertEqual(h.quits, 0)
        h.advance(1)
        XCTAssertEqual(h.quits, 1)
    }
    func test_uses_the_initial_repeat_delay_when_the_first_repeat_completes_the_hold() {
        let h = Harness()
        h.send()
        h.advance(HOLD_MS + 100); h.send { $0.isAutoRepeat = true }
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 0)
        h.advance(1_999)
        XCTAssertEqual(h.quits, 0)
        h.advance(1)
        XCTAssertEqual(h.quits, 1)
    }
    func test_waits_for_Q_release_when_Cmd_is_released_first() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS + 200)
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        h.prevented = 0
        h.holdFor(GRACE_MS * 2) { $0.meta = false }
        XCTAssertGreaterThan(h.prevented, 0)
        XCTAssertEqual(h.quits, 0)
        h.send { $0.type = "keyUp"; $0.meta = false }
        XCTAssertEqual(h.quits, 1)
    }
    func test_commits_a_concealed_hold_when_the_last_Q_repeat_is_never_released() {
        let h = Harness()
        h.send(); h.holdFor(HOLD_MS + 200)
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        h.send { $0.meta = false; $0.isAutoRepeat = true }
        h.advance(GRACE_MS)
        XCTAssertEqual(h.quits, 1)
        h.quits = 0
        h.send { $0.key = "Meta" }
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        h.advance(GRACE_MS * 4)
        XCTAssertEqual(h.quits, 0)
    }
    func test_does_not_quit_when_the_hold_stops_before_the_duration() {
        let h = Harness()
        h.send(); h.holdFor(500)
        h.send { $0.type = "keyUp" }
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
        h.advance((HOLD_MS + GRACE_MS) * 2)
        XCTAssertEqual(h.concealed, 0)
        XCTAssertEqual(h.quits, 0)
    }
    func test_cancels_the_hold_when_the_modifier_is_released_first() {
        let h = Harness()
        h.send()
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
        h.advance((HOLD_MS + GRACE_MS) * 2)
        XCTAssertEqual(h.quits, 0)
    }
    func test_quits_without_showing_a_hint_in_direct_mode() {
        let h = Harness(mode: "direct")
        h.send()
        XCTAssertEqual(h.concealed, 0)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [])
    }
    func test_honors_direct_mode_when_the_key_is_released_before_its_mode_read_settles() {
        let h = Harness(mode: nil)
        h.send(); h.send { $0.type = "keyUp" }
        h.resolvers.first?("direct")
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [])
    }
    func test_does_not_arm_hold_mode_after_a_released_keys_mode_read_settles() {
        let h = Harness(mode: nil)
        h.send(); h.send { $0.type = "keyUp" }
        h.resolvers.first?("hold")
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [])
    }
    private func quickSecondPress(pending mode: String) {
        let h = Harness(mode: nil)
        h.send(); h.send { $0.type = "keyUp" }
        h.advance(DOUBLE_MS - 100)
        h.send()
        XCTAssertEqual(h.quits, 1, mode)
        XCTAssertEqual(h.notifications, [], mode)
        h.send { $0.type = "keyUp" }
        h.resolvers.first?(mode)
        XCTAssertEqual(h.quits, 1, mode)
        XCTAssertEqual(h.notifications, [], mode)
    }
    func test_quits_on_a_quick_second_press_without_waiting_for_a_pending_direct_mode_read() { quickSecondPress(pending: "direct") }
    func test_quits_on_a_quick_second_press_without_waiting_for_a_pending_hold_mode_read() { quickSecondPress(pending: "hold") }
    func test_quits_on_a_quick_second_press_without_waiting_for_a_pending_double_click_mode_read() { quickSecondPress(pending: "double-click") }
    func test_discards_a_stale_mode_resolution_from_a_superseded_press() {
        let h = Harness(mode: nil)
        h.send(); h.send { $0.type = "keyUp" }
        h.advance(DOUBLE_MS + 100)
        h.send()
        XCTAssertEqual(h.resolvers.count, 2)
        h.resolvers[0]("direct")
        XCTAssertEqual(h.quits, 0)
        h.resolvers[1]("hold")
        h.holdFor(HOLD_MS + 200)
        h.send { $0.type = "keyUp" }
        XCTAssertEqual(h.quits, 1)
    }
    func test_quits_on_a_quick_double_press_in_double_click_mode_when_the_first_release_is_unseen() {
        let h = Harness(mode: "double-click")
        h.send(); h.advance(DOUBLE_MS - 100); h.send()
        XCTAssertEqual(h.concealed, 0)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP])
    }
    func test_keeps_the_double_press_hint_visible_after_key_release_until_the_window_ends() {
        let h = Harness(mode: "double-click")
        h.send(); h.advance(100); h.send { $0.type = "keyUp" }
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN])
        h.advance(DOUBLE_MS - 101)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN])
        h.advance(1)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP])
    }
    func test_accepts_a_second_full_shortcut_after_the_modifier_is_released_and_pressed_again() {
        let h = Harness(mode: "double-click")
        h.send(); h.send { $0.type = "keyUp" }
        h.send { $0.type = "keyUp"; $0.key = "Meta"; $0.meta = false }
        h.advance(100)
        h.send { $0.key = "Meta" }
        h.send()
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP])
    }
    func test_expires_a_delayed_double_press_hint_from_keydown_rather_than_mode_resolution() {
        let h = Harness(mode: nil)
        h.send(); h.advance(100); h.send { $0.type = "keyUp" }
        h.advance(100)
        h.resolvers.first?("double-click")
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN])
        h.advance(DOUBLE_MS - 201)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN])
        h.advance(1)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP])
    }
    func test_treats_two_slow_presses_as_separate_attempts_in_double_click_mode() {
        let h = Harness(mode: "double-click")
        h.send(); h.send { $0.type = "keyUp" }
        h.advance(DOUBLE_MS + 100)
        h.send()
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP, DOUBLE_CLICK_DOWN])
    }
    func test_quits_on_a_quick_second_press_in_hold_mode() {
        let h = Harness()
        h.send(); h.send { $0.type = "keyUp" }
        h.advance(DOUBLE_MS - 100); h.send()
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func test_quits_on_a_quick_second_press_in_hold_mode_when_the_first_release_is_unseen() {
        let h = Harness()
        h.send(); h.advance(DOUBLE_MS - 100); h.send()
        XCTAssertEqual(h.concealed, 0)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func test_does_not_count_auto_repeat_as_a_second_press() {
        let h = Harness()
        h.send(); h.holdFor(DOUBLE_MS - 100)
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [HOLD_DOWN])
    }
    func test_does_not_count_a_released_tap_after_another_shortcut_interrupts_it() {
        let h = Harness()
        h.send(); h.send { $0.type = "keyUp" }
        h.send { $0.key = "c" }
        h.advance(100); h.send()
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP, HOLD_DOWN])
    }
    func test_cancels_the_hold_when_another_key_interrupts_it() {
        let h = Harness()
        h.send(); h.holdFor(500)
        h.send { $0.shift = true }
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
        h.holdFor(HOLD_MS)
        XCTAssertEqual(h.quits, 0)
    }
    func test_does_not_count_an_interrupted_press_toward_a_double_press() {
        let h = Harness(mode: "double-click")
        h.send(); h.send { $0.shift = true }
        h.send()
        XCTAssertEqual(h.quits, 0)
        XCTAssertEqual(h.notifications, [DOUBLE_CLICK_DOWN, UP, DOUBLE_CLICK_DOWN])
    }
    func test_ignores_other_shortcuts() {
        let h = Harness()
        h.send { $0.key = "w" }
        h.send { $0.shift = true }
        h.send { $0.meta = false }
        XCTAssertEqual(h.prevented, 0)
        XCTAssertEqual(h.notifications, [])
    }
    /// The clone's own: a failed settings read quits at once (getMode's rejection branch).
    func testAFailedModeReadQuitsAtOnce() {
        let h = Harness(mode: nil)
        h.send()
        h.resolvers.first?(nil)
        XCTAssertEqual(h.quits, 1)
        XCTAssertEqual(h.notifications, [])
    }
    /// DesktopWindow.test.ts "leaves fullscreen before concealing a pending quit".
    func test_leaves_fullscreen_before_concealing_a_pending_quit() {
        let window = FakeWindow()
        T3Menus.concealPendingQuit(window)
        XCTAssertEqual(window.calls, ["setOpacity(0.0)"])
        window.calls = []; window.isFullScreenWindow = true
        T3Menus.concealPendingQuit(window)
        XCTAssertEqual(window.calls, ["setFullScreen(false)", "setOpacity(0.0)"])
        window.calls = []; window.isFullScreenWindow = false; window.isConcealable = false
        T3Menus.concealPendingQuit(window)
        XCTAssertEqual(window.calls, [])
    }
    /// The menus' conceal keeps the real window ordered in and key-able: only its alpha changes.
    func testConcealTurnsTheRealWindowTransparentWithoutOrderingItOut() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFront(nil)
        defer { window.orderOut(nil) }
        let menus = T3Menus()
        menus.attach(window)
        menus.quit.conceal()
        XCTAssertEqual(window.alphaValue, 0)
        XCTAssertTrue(window.isVisible)
    }
}

private func keyEvent(_ type: NSEvent.EventType, _ characters: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [.command], repeating: Bool = false) -> NSEvent {
    NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
                     characters: characters, charactersIgnoringModifiers: characters, isARepeat: repeating, keyCode: code)!
}

final class MenuTests: XCTestCase {
    func testRealKeyEventsReachTheStateMachine() {
        let h = Harness()
        XCTAssertTrue(h.hold.handle(keyEvent(.keyDown, "q", 12)))
        XCTAssertFalse(h.hold.handle(keyEvent(.keyDown, "w", 13)))
        // Korean 2-Set types ㅂ on Q's key: the quit shortcut still holds (X15).
        let korean = Harness()
        XCTAssertTrue(korean.hold.handle(keyEvent(.keyDown, "ㅂ", 12)))
        XCTAssertEqual(korean.notifications, [HOLD_DOWN])
    }
    func testAModifierPressedMidHoldCancelsItLikeAnotherKey() {
        let h = Harness()
        XCTAssertFalse(h.hold.handle(keyEvent(.flagsChanged, "", 55)))
        XCTAssertTrue(h.hold.handle(keyEvent(.keyDown, "q", 12)))
        XCTAssertFalse(h.hold.handle(keyEvent(.flagsChanged, "", 56, [.command, .shift])), "modifier events are never consumed")
        XCTAssertEqual(h.notifications, [HOLD_DOWN, UP])
    }
    func testHeldCommandWRepeatsAreDroppedButAPressIsNot() {
        XCTAssertFalse(T3Menus.dropsHeldClose(keyEvent(.keyDown, "w", 13)), "a deliberate press reaches Close Window")
        XCTAssertTrue(T3Menus.dropsHeldClose(keyEvent(.keyDown, "w", 13, repeating: true)))
        XCTAssertTrue(T3Menus.dropsHeldClose(keyEvent(.keyDown, "ㅈ", 13, repeating: true)), "Korean 2-Set: by key code")
        XCTAssertFalse(T3Menus.dropsHeldClose(keyEvent(.keyDown, "w", 13, [.command, .option], repeating: true)))
        XCTAssertFalse(T3Menus.dropsHeldClose(keyEvent(.keyDown, "w", 13, [.command, .shift], repeating: true)))
        XCTAssertFalse(T3Menus.dropsHeldClose(keyEvent(.keyDown, "w", 13, [], repeating: true)))
        XCTAssertFalse(T3Menus.dropsHeldClose(keyEvent(.keyDown, "e", 14, repeating: true)))
    }
    /// pickProjectFavicon: one image (WORKSPACE_IMAGE_PREVIEW_EXTENSIONS), starting at the workspace root.
    func testFaviconPanelPicksOneImageFromTheWorkspaceRoot() throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-favicon-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let panel = T3Menus.faviconPanel(startingAt: root.path)
        XCTAssertEqual(panel.directoryURL?.standardizedFileURL.path, root.standardizedFileURL.path)
        XCTAssertFalse(panel.allowsMultipleSelection)
        XCTAssertTrue(panel.canChooseFiles)
        XCTAssertFalse(panel.canChooseDirectories)
        XCTAssertEqual(Set(panel.allowedContentTypes.compactMap(\.preferredFilenameExtension)).isSuperset(of: ["png", "svg", "gif", "jpeg", "webp", "ico"]), true)
        // Under the agent: the imports folder's first image, or nothing.
        let imports = root.appendingPathComponent("imports", isDirectory: true)
        try FileManager.default.createDirectory(at: imports, withIntermediateDirectories: true)
        try Data("x".utf8).write(to: imports.appendingPathComponent("notes.txt"))
        var picked: String?? = .none
        T3Menus().pickProjectFavicon(startingAt: root.path, importsRoot: imports) { picked = .some($0) }
        XCTAssertEqual(picked, .some(nil))
        try Data("x".utf8).write(to: imports.appendingPathComponent("icon.png"))
        T3Menus().pickProjectFavicon(startingAt: root.path, importsRoot: imports) { picked = .some($0) }
        XCTAssertEqual(picked.flatMap { $0 }.map { URL(fileURLWithPath: $0).resolvingSymlinksInPath().path }, imports.appendingPathComponent("icon.png").resolvingSymlinksInPath().path)
    }
    /// The full-screen fact: the style mask at attach, then did-enter / did-exit, each published.
    func testFullScreenFactFollowsTheWindowNotifications() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        let fact = T3FullScreen()
        var published = 0
        fact.changed = { published += 1 }
        fact.attach(window)
        XCTAssertFalse(fact.fullScreen)
        NotificationCenter.default.post(name: NSWindow.didEnterFullScreenNotification, object: window)
        XCTAssertTrue(fact.fullScreen)
        NotificationCenter.default.post(name: NSWindow.didEnterFullScreenNotification, object: window)
        NotificationCenter.default.post(name: NSWindow.didExitFullScreenNotification, object: NSWindow())
        XCTAssertTrue(fact.fullScreen, "another window's exit is not this one's")
        NotificationCenter.default.post(name: NSWindow.didExitFullScreenNotification, object: window)
        XCTAssertFalse(fact.fullScreen)
        XCTAssertEqual(published, 3, "attach, enter, exit")
        XCTAssertEqual(T3Locale.systemLocale(Locale(identifier: "ko_KR")), "ko-KR")
        XCTAssertEqual(T3Locale.systemLocale(Locale(identifier: "en_GB")), "en-GB")
    }

    func testMenusGainTheReferenceItems() {
        _ = NSApplication.shared
        let bar = NSMenu()
        let edit = NSMenu(title: "Edit")
        for title in ["Undo", "Redo"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(.separator())
        for title in ["Cut", "Copy", "Paste", "Delete", "Select All"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        // The host's Edit ends with its own Speech since exact2 #226 (DevMenuMac.swift).
        edit.addItem(.separator())
        let speech = NSMenu(title: "Speech")
        for title in ["Start Speaking", "Stop Speaking"] { speech.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(withTitle: "Speech", action: nil, keyEquivalent: "").submenu = speech
        bar.addItem(withTitle: "Edit", action: nil, keyEquivalent: "").submenu = edit
        let view = NSMenu(title: "View")
        view.addItem(withTitle: "Enter Full Screen", action: nil, keyEquivalent: "f")
        bar.addItem(withTitle: "View", action: nil, keyEquivalent: "").submenu = view
        let menus = T3Menus()
        menus.augment(bar)
        menus.augment(bar)
        // DesktopApplicationMenu.ts's Edit: one Speech, the host's.
        XCTAssertEqual(edit.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Paste as Text", "Delete", "—", "Select All", "—", "Speech"])
        XCTAssertEqual(edit.items.filter { $0.title == "Speech" }.count, 1)
        let pasteText = edit.item(withTitle: "Paste as Text")!
        XCTAssertEqual(pasteText.keyEquivalent, "v")
        XCTAssertEqual(pasteText.keyEquivalentModifierMask, [.command, .shift])
        XCTAssertEqual(pasteText.action, #selector(NSTextView.pasteAsPlainText(_:)))
        XCTAssertEqual(view.items.map { $0.isSeparatorItem ? "—" : "\($0.title)\($0.isHidden ? " (hidden)" : "") \($0.keyEquivalent)" },
                       ["Actual Size 0", "Zoom In =", "Zoom In (hidden) +", "Zoom Out -", "—", "Enter Full Screen f"])
    }
    func testEditKeepsOnlyTheReferenceItemsAndAppCommandsStayKeyEquivalents() {
        _ = NSApplication.shared
        // The host's Edit since exact2 #226: an app's ⇧⌘G button after Select All, behind its own separator.
        let bar = NSMenu(), edit = NSMenu(title: "Edit"), command = ShortcutHost()
        for title in ["Undo", "Redo"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(.separator())
        for title in ["Cut", "Copy", "Paste", "Delete", "Select All"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        let undo = NSMenuItem(title: "Undo", action: #selector(ShortcutHost.fire(_:)), keyEquivalent: "z"); undo.target = command
        edit.removeItem(at: 0); edit.insertItem(undo, at: 0) // the app's Undo stands in for the host's
        edit.addItem(.separator())
        let branch = NSMenuItem(title: "Branch", action: #selector(ShortcutHost.fire(_:)), keyEquivalent: "G"); branch.target = command
        branch.keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(branch)
        edit.addItem(.separator())
        edit.addItem(withTitle: "Speech", action: nil, keyEquivalent: "")
        bar.addItem(withTitle: "Edit", action: nil, keyEquivalent: "").submenu = edit
        let keys = R8KeysMenus()
        keys.arrange(bar)
        let shown = { edit.items.filter { !$0.isHidden }.map { $0.isSeparatorItem ? "—" : $0.title } }
        XCTAssertEqual(shown(), ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Delete", "Select All", "—", "Speech"])
        XCTAssertTrue(branch.isHidden)
        XCTAssertTrue(branch.allowsKeyEquivalentWhenHidden, "⇧⌘G still reaches the branch picker")
        XCTAssertFalse(edit.items[0].isHidden, "the app's Undo keeps Edit ▸ Undo's place")
        // A command the host files later is hidden when Edit opens.
        let find = NSMenuItem(title: "Find", action: #selector(ShortcutHost.fire(_:)), keyEquivalent: "f"); find.target = command
        edit.insertItem(find, at: edit.index(of: branch))
        keys.menuNeedsUpdate(edit)
        XCTAssertEqual(shown(), ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Delete", "Select All", "—", "Speech"])
    }
    /// The host's Edit and Help since LLP 1115 D8 (DevMenuMac.swift): Xcode's template, which DesktopApplicationMenu.ts's
    /// Edit lacks, and a Help menu with the help-book item (adopt-main-fixes-r7).
    func testTheHostTemplateEditAndHelpShowOnlyTheReferenceItems() {
        _ = NSApplication.shared
        let bar = NSMenu(), edit = NSMenu(title: "Edit"), command = ShortcutHost()
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        let matchStyle = edit.addItem(withTitle: "Paste and Match Style", action: #selector(NSTextView.pasteAsPlainText(_:)), keyEquivalent: "v")
        matchStyle.keyEquivalentModifierMask = [.command, .option, .shift]
        edit.addItem(withTitle: "Delete", action: #selector(NSText.delete(_:)), keyEquivalent: "")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        edit.addItem(.separator())
        // Find, with the app's ⇧⌘G (the branch picker) standing in Find Previous's place (ShortcutsMac placeEdit).
        let find = NSMenu(title: "Find")
        find.addItem(withTitle: "Find…", action: #selector(NSResponder.performTextFinderAction(_:)), keyEquivalent: "f")
        let branch = NSMenuItem(title: "Branch", action: #selector(ShortcutHost.fire(_:)), keyEquivalent: "g"); branch.target = command
        branch.keyEquivalentModifierMask = [.command, .shift]
        find.addItem(branch)
        let selection = find.addItem(withTitle: "Use Selection for Find", action: #selector(NSResponder.performTextFinderAction(_:)), keyEquivalent: "e")
        edit.addItem(withTitle: "Find", action: nil, keyEquivalent: "").submenu = find
        for title in ["Spelling and Grammar", "Substitutions", "Transformations"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "").submenu = NSMenu(title: title) }
        let speech = NSMenu(title: "Speech")
        speech.addItem(withTitle: "Start Speaking", action: nil, keyEquivalent: "")
        edit.addItem(withTitle: "Speech", action: nil, keyEquivalent: "").submenu = speech
        bar.addItem(withTitle: "Edit", action: nil, keyEquivalent: "").submenu = edit
        let help = NSMenu(title: "Help")
        let book = help.addItem(withTitle: "T3 Code Help", action: #selector(NSApplication.showHelp(_:)), keyEquivalent: "?")
        bar.addItem(withTitle: "Help", action: nil, keyEquivalent: "").submenu = help
        let menus = T3Menus()
        menus.updatesDisabledReason = { "Automatic updates are not available because no update feed is configured." }
        menus.augment(bar)
        menus.augment(bar)
        let shown = { (menu: NSMenu) in menu.items.filter { !$0.isHidden }.map { $0.isSeparatorItem ? "—" : $0.title } }
        XCTAssertEqual(shown(edit), ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Paste as Text", "Delete", "—", "Select All", "—", "Speech"])
        XCTAssertTrue(matchStyle.isHidden)
        XCTAssertFalse(matchStyle.allowsKeyEquivalentWhenHidden, "⌥⇧⌘V is no item in the reference")
        XCTAssertTrue(branch.allowsKeyEquivalentWhenHidden, "⇧⌘G still reaches the branch picker from Find")
        XCTAssertFalse(selection.allowsKeyEquivalentWhenHidden, "the host's own ⌘E stays out with its menu")
        let before = ShortcutHost.fired
        let key = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [.command, .shift], timestamp: 0, windowNumber: 0, context: nil,
                                   characters: "G", charactersIgnoringModifiers: "g", isARepeat: false, keyCode: 5)!
        XCTAssertTrue(bar.performKeyEquivalent(with: key), "a hidden Find still answers the app's chord")
        XCTAssertEqual(ShortcutHost.fired, before + 1)
        // DesktopApplicationMenu.ts's Help: Check for Updates... alone; the help book is not offered.
        XCTAssertEqual(shown(help), ["Check for Updates..."])
        XCTAssertTrue(book.isHidden)
        XCTAssertFalse(book.allowsKeyEquivalentWhenHidden, "⇧⌘/ stays AppKit's help search")
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

    /// QuitHoldOverlay's text-2xl px-8 py-4 follow the Interface font size (T3RootFont.swift).
    func testPillFollowsTheInterfaceFontSize() {
        defer { T3RootFont.set(16) }
        T3RootFont.set(20)
        let pill = T3QuitOverlay().makePill(mode: "hold", dark: false)
        XCTAssertEqual(pill.frame.height, 80)
        let label = pill.subviews.compactMap { $0 as? NSTextField }.first
        XCTAssertEqual(label?.font?.pointSize, 30)
        XCTAssertEqual(label?.frame.minX, 40)
        T3RootFont.set(12)
        XCTAssertEqual(T3QuitOverlay().makePill(mode: "hold", dark: false).frame.height, 48)
        T3RootFont.set(Double.nan)
        XCTAssertEqual(T3RootFont.size, 16)
    }
}

let suite = XCTestSuite(name: "Menus")
suite.addTest(QuitHoldTests.defaultTestSuite)
suite.addTest(MenuTests.defaultTestSuite)
suite.run()
guard let run = suite.testRun, run.executionCount == 46 else { print("Menus: unexpected test count \(suite.testRun?.executionCount ?? 0)"); exit(1) }
print("Menus: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)

/// The host's command target (ExactKit's ShortcutHost), named as R8KeysMenus finds it.
final class ShortcutHost: NSObject { nonisolated(unsafe) static var fired = 0; @objc func fire(_ sender: Any?) { Self.fired += 1 } }
