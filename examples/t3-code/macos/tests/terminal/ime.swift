import AppKit
import WebKit
import XCTest

/// A 2-Set Korean input method as the terminal meets it, in the order AppKit and WebKit call it. The
/// tests cannot switch the machine's input source, so this stands in for it:
///
/// - AppKit hands a person's keyDown to the local monitors, then to the window: its key equivalents,
///   then the first responder's keyDown (`press`).
/// - WKWebView's keyDown offers the key, and its keyUp, to the input context with
///   `handleEventByInputMethod:completionHandler:` (WebKit's WebViewImpl::interpretKeyEvent), then
///   collects the keyboard layout's commands for a key the input method declines and sends the keydown
///   to the page: as "Process" (keyCode 229) when the input method took it.
/// - An input method runs in its own process and answers after the call returns, having called the
///   client's setMarkedText or insertText. `NSTextInputContext.handleEvent(_:)` takes the same path: it
///   passes a block to `_handleEvent:options:allowingSyntheticEvent:completionHandler:` and returns the
///   flag that block has set by then, so with an input method it returns false while the input method
///   still acts on the key, and the keyboard layout's command for a declined key follows.
/// - 2-Set: g ㅎ, k ㅏ, s ㄴ, r ㄱ, m ㅡ, f ㄹ compose 한 and 글 (r commits 한 and starts ㄱ); a key
///   the input method does not compose commits the syllable and is declined, unless the test says the
///   method keeps composing (as the Korean one did for a chord re-issued with a Latin letter), consumes it
///   (Kotoeri's ⌃K), or converts (the Korean one's ⌥Return, below).
/// - Hanja (⌥Return), in the order the Korean input method took under WebKit with a person's keys: it
///   commits the syllable, reads the selection and the text before it through WebKit's asynchronous
///   client calls (answered by the page), and, when that text is the syllable, replaces it with its first
///   candidate, marked while the list is open, and consumes the key. Finding no text, it declines the key.
///   Return picks the candidate.
/// - Disconnected (`connected` false): after another process selects the input source, WebKit's input
///   context can be left with no input method, which declines every key until the context is activated
///   again (T3TerminalInputSource). `source` stands in for the selected source and the view's context
///   for the current one.
final class FakeHangulInputMethod {
    enum Answer { case commitsAndDeclines, keepsAndDeclines, consumes, convertsToHanja }
    var control = Answer.commitsAndDeclines
    var optionReturn = Answer.commitsAndDeclines
    private(set) var marked = ""
    private var choosing = false
    /// Key-down codes offered to the input method, and the times the view asked the input context itself.
    private(set) var offered: [UInt16] = []
    private(set) var synchronousAsks = 0
    var connected = true
    var source = "com.apple.inputmethod.Korean.2SetKorean"
    private(set) var activations = 0
    private var restore: [(Method, IMP)] = []
    private static let jamo: [UInt16: String] = [5: "ㅎ", 40: "ㅏ", 1: "ㄴ", 15: "ㄱ", 46: "ㅡ", 3: "ㄹ", 49: " "]

    private func compose(_ jamo: String) -> (commit: String, marked: String) {
        switch (marked, jamo) {
        case (_, " "): return (marked + " ", "")
        case ("", _): return ("", jamo)
        case ("ㅎ", "ㅏ"): return ("", "하")
        case ("하", "ㄴ"): return ("", "한")
        case ("ㄱ", "ㅡ"): return ("", "그")
        case ("그", "ㄹ"): return ("", "글")
        default: return (marked, jamo)
        }
    }

    /// The input method's answer to one key, with the client calls it makes before answering.
    private func answer(_ event: NSEvent, _ client: NSTextInputClient, _ done: @escaping (Bool) -> Void) {
        guard event.type == .keyDown, connected else { return done(false) }
        offered.append(event.keyCode)
        let none = NSRange(location: NSNotFound, length: 0)
        let flags = event.modifierFlags.intersection([.command, .control, .option])
        let returnKey = event.keyCode == 36 || event.keyCode == 76
        let choosing = self.choosing
        self.choosing = false
        if choosing, flags.isEmpty, returnKey {
            client.insertText(marked, replacementRange: none)
            marked = ""
            return done(true)
        }
        if flags.isEmpty, let jamo = Self.jamo[event.keyCode] {
            let (commit, next) = compose(jamo)
            if !commit.isEmpty { client.insertText(commit, replacementRange: none) }
            if !next.isEmpty || !marked.isEmpty { client.setMarkedText(next, selectedRange: NSRange(location: (next as NSString).length, length: 0), replacementRange: none) }
            marked = next
            return done(true)
        }
        guard !marked.isEmpty else { return done(false) }
        switch flags.contains(.control) ? control : flags.contains(.option) && returnKey ? optionReturn : .commitsAndDeclines {
        case .consumes: done(true)
        case .keepsAndDeclines: done(false)
        case .commitsAndDeclines:
            client.insertText(marked, replacementRange: none)
            marked = ""
            done(false)
        case .convertsToHanja:
            let syllable = marked
            client.insertText(syllable, replacementRange: none)
            marked = ""
            readBeforeSelection(client, length: (syllable as NSString).length) { [unowned self] text, range in
                guard text == syllable, let candidate = Self.hanja[syllable] else { return done(false) }
                client.setMarkedText(candidate, selectedRange: NSRange(location: (candidate as NSString).length, length: 0), replacementRange: range)
                marked = candidate
                self.choosing = true
                done(true)
            }
        }
    }

    private static let hanja = ["한": "韓"]

    /// The selection, then the `length` units before it, as WebKit's asynchronous text input client
    /// answers them: from the page, after the calls made before.
    private func readBeforeSelection(_ client: NSTextInputClient, length: Int, _ done: @escaping (String?, NSRange) -> Void) {
        typealias Selected = @convention(c) (AnyObject, Selector, @escaping @convention(block) (NSRange) -> Void) -> Void
        typealias Substring = @convention(c) (AnyObject, Selector, NSRange, @escaping @convention(block) (NSAttributedString?, NSRange) -> Void) -> Void
        let object = client as! NSObject
        let selectedName = NSSelectorFromString("selectedRangeWithCompletionHandler:")
        let substringName = NSSelectorFromString("attributedSubstringForProposedRange:completionHandler:")
        let selected = unsafeBitCast(object.method(for: selectedName), to: Selected.self)
        selected(object, selectedName) { range in
            guard range.location != NSNotFound, range.location >= length else { return done(nil, range) }
            let before = NSRange(location: range.location - length, length: length)
            let substring = unsafeBitCast(object.method(for: substringName), to: Substring.self)
            substring(object, substringName, before) { text, _ in done(text?.string, before) }
        }
    }

    func install() {
        let context: AnyClass = NSTextInputContext.self
        func replace(_ name: String, _ block: Any) -> IMP {
            let method = class_getInstanceMethod(context, NSSelectorFromString(name))!
            let original = method_setImplementation(method, imp_implementationWithBlock(block))
            restore.append((method, original))
            return original
        }
        let byInputMethod: @convention(block) (NSTextInputContext, NSEvent, @escaping (Bool) -> Void) -> Void = { [unowned self] context, event, done in
            DispatchQueue.main.async { self.answer(event, context.client, done) }
        }
        _ = replace("handleEventByInputMethod:completionHandler:", byInputMethod)
        typealias Layout = @convention(c) (AnyObject, Selector, NSEvent) -> Bool
        let layoutSelector = NSSelectorFromString("handleEventByKeyboardLayout:")
        let layout = unsafeBitCast(method_getImplementation(class_getInstanceMethod(context, layoutSelector)!), to: Layout.self)
        let synchronous: @convention(block) (NSTextInputContext, NSEvent) -> Bool = { [unowned self] context, event in
            synchronousAsks += 1
            DispatchQueue.main.async { self.answer(event, context.client) { if !$0 { _ = layout(context, layoutSelector, event) } } }
            return false
        }
        _ = replace("handleEvent:", synchronous)
        typealias Discard = @convention(c) (AnyObject, Selector) -> Void
        var discardOriginal: Discard?
        let discard: @convention(block) (NSTextInputContext) -> Void = { [unowned self] context in
            marked = ""
            choosing = false
            discardOriginal?(context, NSSelectorFromString("discardMarkedText"))
        }
        discardOriginal = unsafeBitCast(replace("discardMarkedText", discard), to: Discard.self)
        let selected: @convention(block) (NSTextInputContext) -> String? = { [unowned self] _ in source }
        _ = replace("selectedKeyboardInputSource", selected)
        typealias Activate = @convention(c) (AnyObject, Selector) -> Void
        var activateOriginal: Activate?
        let activate: @convention(block) (NSTextInputContext) -> Void = { [unowned self] context in
            activations += 1
            connected = true
            activateOriginal?(context, NSSelectorFromString("activate"))
        }
        activateOriginal = unsafeBitCast(replace("activate", activate), to: Activate.self)
    }

    /// `NSTextInputContext.current` answers `context` (the test app is never active, so AppKit's is nil).
    func makeCurrent(_ context: @escaping () -> NSTextInputContext?) {
        let method = class_getClassMethod(NSTextInputContext.self, NSSelectorFromString("currentInputContext"))!
        let current: @convention(block) (AnyObject) -> NSTextInputContext? = { _ in context() }
        restore.append((method, method_setImplementation(method, imp_implementationWithBlock(current))))
    }

    func uninstall() {
        for (method, original) in restore.reversed() { method_setImplementation(method, original) }
        restore = []
    }
}

/// The terminal under that input method: what reaches the PTY for each key a person presses.
final class TerminalInputMethodTests: XCTestCase {
    private var window: NSWindow!
    private var view: T3TerminalView!
    private var method: FakeHangulInputMethod!
    private var latin: T3LatinChords!

    override func setUpWithError() throws {
        let app = try XCTUnwrap(ProcessInfo.processInfo.environment["T3_APP_DIR"], "set T3_APP_DIR to examples/t3-code")
        setenv("EXACT_ASSETS", app, 1)
        try XCTSkipUnless(T3TerminalAssets.fileURL("terminal-host.js") != nil, "run terminal-host/build.mjs first")
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
        view = T3TerminalView(props: ["terminal": "ime"], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        view.web.frame = window.contentView!.bounds
        window.contentView!.addSubview(view.web)
        spin(until: { self.view.ready }, timeout: 20)
        window.makeFirstResponder(view.web)
        // WebKit hands keys to the input context once the page's input is editable.
        spin(until: { self.view.focused && self.view.web.inputContext != nil }, timeout: 5)
        method = FakeHangulInputMethod()
        method.install()
        latin = T3LatinChords(agent: true)
    }

    override func tearDown() {
        method?.uninstall()
        latin?.destroy()
        view?.destroy()
        window?.orderOut(nil)
    }

    private func spin(until condition: () -> Bool, timeout: TimeInterval) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }

    private var typed: String {
        view.bridgeLog.filter { $0.hasPrefix("data ") }.compactMap { line in
            (try? JSONSerialization.jsonObject(with: Data(line.dropFirst(5).utf8), options: [.fragmentsAllowed])) as? String
        }.joined()
    }

    /// A key down and up as AppKit dispatches a person's: the local monitors (the module's re-issue of
    /// a ⌃ or ⌘ chord typed under a non-Latin layout with its key's Latin letter, T3LatinChords, when
    /// `monitors`; then the terminal's), then `-[NSApplication sendEvent:]`, which offers a key down
    /// with ⌘, ⌃ or fn to the key window's `performKeyEquivalent:` first and sends it to the window
    /// (the first responder's keyDown) only if no view takes it. The test app is never active, so its
    /// own sendEvent: would skip that pass.
    private func press(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], plain: String? = nil, monitors: Bool = false) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            var event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: window.windowNumber, context: nil, characters: characters,
                                         charactersIgnoringModifiers: plain ?? characters, isARepeat: false, keyCode: code)!
            if monitors, type == .keyDown { event = latin.route(event) }
            let equivalent = type == .keyDown && !flags.isDisjoint(with: [.command, .control, .function])
            if !view.routeTerminalKey(event), !(equivalent && window.performKeyEquivalent(with: event)) { window.sendEvent(event) }
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        }
    }

    /// Every answer and message has landed: the page has answered a script sent after them.
    private func settle() {
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        var done = false
        view.web.evaluateJavaScript("0") { _, _ in done = true }
        spin(until: { done }, timeout: 5)
    }

    private func compose(_ keys: String) {
        let before = typed
        for key in keys { press(["g": 5, "k": 40, "s": 1][key]!, String(key)) }
        settle()
        XCTAssertTrue(view.composing, "nothing composes")
        XCTAssertEqual(typed, before, "the composition went out early")
    }

    private func hex(_ text: String) -> String { text.unicodeScalars.map { String($0.value, radix: 16) }.joined(separator: " ") }

    func testTypingSendsEachSyllableOnceItIsCommitted() {
        for (code, key) in [(5, "g"), (40, "k"), (1, "s"), (15, "r"), (46, "m"), (3, "f"), (49, " ")] as [(UInt16, String)] { press(code, key) }
        settle()
        print("terminal 2-Set g k s r m f Space: typed \(hex(typed))")
        XCTAssertEqual(typed, "한글 ")
        XCTAssertFalse(view.composing)
    }

    /// ⌃C with ㅎ composing: ㅎ goes out once and ^C does not, as T3 Code's Chromium page gets the key as
    /// "Process" and ignores it; with nothing composing ⌃C is the shell's. Whether the input method
    /// commits ㅎ itself or keeps composing, the result is the same.
    func testControlCCommitsTheCompositionAlone() {
        for answer in [FakeHangulInputMethod.Answer.commitsAndDeclines, .keepsAndDeclines] {
            method.control = answer
            let before = typed, offered = method.offered.count
            compose("g")
            press(8, "\u{3}", .control, plain: "c")
            settle()
            print("terminal ⌃C while ㅎ composes (\(answer)): typed \(hex(String(typed.dropFirst(before.count)))) composing \(view.composing)")
            XCTAssertEqual(String(typed.dropFirst(before.count)), "ㅎ", "\(answer)")
            XCTAssertFalse(view.composing, "\(answer): ㅎ is still composing")
            XCTAssertEqual(Array(method.offered.dropFirst(offered)), [5, 8], "\(answer): the input method sees g, then ⌃C once")
            press(8, "\u{3}", .control, plain: "c")
            settle()
            XCTAssertEqual(String(typed.dropFirst(before.count)), "ㅎ\u{3}", "\(answer): without a composition ⌃C is the shell's")
            XCTAssertEqual(method.synchronousAsks, 0, "the view asked the input context itself")
        }
    }

    /// ⌃L under a Latin layout's input method ("l") with 하 composing commits 하 and clears the screen:
    /// the reference's handleBeforeKey still runs on a "Process" key and reads its `key`.
    func testControlLCommitsAndClears() {
        compose("gk")
        press(37, "\u{c}", .control, plain: "l")
        settle()
        print("terminal ⌃L while 하 composes: typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed.sorted(), "하\u{c}".sorted())
        XCTAssertFalse(view.composing)
    }

    /// ⌃C and ⌃L under Korean 2-Set, in the order a person's keys took in the app: the module's monitor
    /// sees the key down first (its characters are the layout's: ⌃C is "ㅊ", ⌃L "ㅣ"), AppKit offers it
    /// to the terminal's key equivalents, and WebKit to the input method, which commits the syllable and
    /// declines the key. ⌃C sends ㅎ alone. ⌃L sends 하 alone and does not clear: T3 Code's drawer reads
    /// "ㅣ" on its "Process" key, which is not its clear shortcut (keybindings.ts
    /// isTerminalClearShortcut). Without a composition ⌃L clears, as the module re-issues it as "l".
    func testKoreanControlKeysInTheAppsOrder() {
        compose("g")
        press(8, "\u{3}", .control, plain: "ㅊ", monitors: true)
        settle()
        print("terminal 2-Set ⌃C while ㅎ composes, app order: typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed, "ㅎ")
        XCTAssertFalse(view.composing)

        compose("gk")
        press(37, "\u{c}", .control, plain: "ㅣ", monitors: true)
        settle()
        print("terminal 2-Set ⌃L while 하 composes, app order: typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed, "ㅎ하", "⌃L cleared the screen while the input method's key")
        XCTAssertFalse(view.composing)

        press(37, "\u{c}", .control, plain: "ㅣ", monitors: true)
        settle()
        XCTAssertEqual(typed, "ㅎ하\u{c}", "without a composition ⌃L clears")
        XCTAssertEqual(Array(method.offered.filter { [8, 37].contains($0) }), [8, 37], "the input method sees each composing ⌃ key once")
    }

    /// ⌥Return with 한 composing opens the Korean input method's Hanja list. 한 goes out when the input method
    /// commits it and the picked 韓 after it, as under T3 Code's Chromium page, whose surface sends every
    /// commit (surface.ts onCompositionEnd); the input method finds 한 before the caret because the page
    /// leaves it in the input (entry.ts keepConversionContext). The terminal never encodes the key.
    func testOptionReturnOpensTheHanjaList() {
        method.optionReturn = .convertsToHanja
        compose("gks")
        press(36, "\r", .option)
        settle()
        print("terminal ⌥Return while 한 composes (Hanja): typed \(hex(typed)) composing \(view.composing) marked \(method.marked)")
        XCTAssertEqual(typed, "한")
        XCTAssertEqual(method.marked, "韓", "the input method found no 한 before the caret, so no list opened")
        XCTAssertTrue(view.composing, "韓 composes while the list is open")
        XCTAssertEqual(method.offered.filter { $0 == 36 }.count, 1)
        // Return picks 韓; the next syllable goes out alone.
        press(36, "\r")
        settle()
        XCTAssertEqual(typed, "한韓")
        XCTAssertFalse(view.composing)
        press(5, "g"); press(49, " ")
        settle()
        XCTAssertEqual(typed, "한韓ㅎ ")
        XCTAssertEqual(method.synchronousAsks, 0, "the view asked the input context itself")
    }

    /// An input method that commits and declines ⌥Return sends the syllable alone, once: the terminal never
    /// encodes the key (ESC CR is the terminal's encoding, a line break the layout's), and the syllable the
    /// page left for a conversion does not go out again with the next key. Without a composition ⌥Return
    /// is the terminal's: Ghostty's ESC CR.
    func testDeclinedOptionReturnSendsTheSyllableAlone() {
        compose("gks")
        press(36, "\r", .option)
        settle()
        print("terminal ⌥Return while 한 composes (declined): typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed, "한")
        XCTAssertFalse(view.composing)
        XCTAssertEqual(method.offered.filter { $0 == 36 }.count, 1)
        press(49, " ")
        settle()
        XCTAssertEqual(typed, "한 ")

        press(36, "\r", .option)
        settle()
        XCTAssertEqual(typed, "한 \u{1b}\r")
        XCTAssertEqual(method.synchronousAsks, 0, "the view asked the input context itself")
    }

    /// A ⌃ key the input method consumes (Kotoeri's ⌃K converts) keeps the composition and sends nothing.
    func testConsumedControlKeyKeepsTheComposition() {
        method.control = .consumes
        compose("gk")
        press(40, "\u{b}", .control, plain: "k")
        settle()
        XCTAssertTrue(view.composing, "a consumed conversion key must not commit the composition")
        XCTAssertEqual(typed, "")
        XCTAssertEqual(method.offered.filter { $0 == 40 }.count, 2, "g k, then ⌃K once")
    }

    /// Another process selects 2-Set while the terminal is first responder and leaves its input context with
    /// no input method, in the order measured with VoiceOver on (macOS 26): each keydown reaches the page as
    /// the key itself, not "Process", with the 2-Set layout's character, and the surface encodes it, so ㅎ
    /// and ㅏ go out apart. The input source notification reconnects the context once, and 한 composes.
    func testAnInputSourceChosenElsewhereComposesAgain() {
        method.makeCurrent { [unowned self] in view.web.inputContext }
        method.connected = false
        press(5, "ㅎ"); press(40, "ㅏ")
        settle()
        print("terminal 2-Set, input method disconnected: typed \(hex(typed))")
        XCTAssertEqual(typed, "ㅎㅏ")
        XCTAssertEqual(method.offered, [], "a disconnected input method saw a key")

        method.source = "com.apple.keylayout.ABC"
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        method.source = "com.apple.inputmethod.Korean.2SetKorean"
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        XCTAssertEqual(method.activations, 2, "the context is reconnected once per change of source")
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        XCTAssertEqual(method.activations, 2, "the same source again reconnected the context")

        for (code, key) in [(5, "g"), (40, "k"), (1, "s"), (49, " ")] as [(UInt16, String)] { press(code, key) }
        settle()
        XCTAssertEqual(typed, "ㅎㅏ한 ")
    }

    /// A change of source while another view has the focus leaves the context alone: activating it would take
    /// the input method from that view.
    func testAnInputSourceChangeAwayFromTheTerminalLeavesItsContext() {
        method.makeCurrent { [unowned self] in view.web.inputContext }
        window.makeFirstResponder(nil)
        method.source = "com.apple.keylayout.ABC"
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        XCTAssertEqual(method.activations, 0)
        method.makeCurrent { nil }
        window.makeFirstResponder(view.web)
        method.source = "com.apple.inputmethod.Korean.2SetKorean"
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        XCTAssertEqual(method.activations, 0, "the context was not the current one")
    }
}
