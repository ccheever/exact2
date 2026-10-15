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
///   method keeps composing (WebKit's Korean ⌃C) or consumes it (Hanja's ⌥Return, Kotoeri's ⌃K).
final class FakeHangulInputMethod {
    enum Answer { case commitsAndDeclines, keepsAndDeclines, consumes }
    var control = Answer.commitsAndDeclines
    var optionReturn = Answer.commitsAndDeclines
    private(set) var marked = ""
    /// Key-down codes offered to the input method, and the times the view asked the input context itself.
    private(set) var offered: [UInt16] = []
    private(set) var synchronousAsks = 0
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
    private func answer(_ event: NSEvent, _ client: NSTextInputClient) -> Bool {
        guard event.type == .keyDown else { return false }
        offered.append(event.keyCode)
        let none = NSRange(location: NSNotFound, length: 0)
        let flags = event.modifierFlags.intersection([.command, .control, .option])
        if flags.isEmpty, let jamo = Self.jamo[event.keyCode] {
            let (commit, next) = compose(jamo)
            if !commit.isEmpty { client.insertText(commit, replacementRange: none) }
            if !next.isEmpty || !marked.isEmpty { client.setMarkedText(next, selectedRange: NSRange(location: (next as NSString).length, length: 0), replacementRange: none) }
            marked = next
            return true
        }
        guard !marked.isEmpty else { return false }
        let returnKey = event.keyCode == 36 || event.keyCode == 76
        switch flags.contains(.control) ? control : flags.contains(.option) && returnKey ? optionReturn : .commitsAndDeclines {
        case .consumes: return true
        case .keepsAndDeclines: return false
        case .commitsAndDeclines:
            client.insertText(marked, replacementRange: none)
            marked = ""
            return false
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
            DispatchQueue.main.async { done(self.answer(event, context.client)) }
        }
        _ = replace("handleEventByInputMethod:completionHandler:", byInputMethod)
        typealias Layout = @convention(c) (AnyObject, Selector, NSEvent) -> Bool
        let layoutSelector = NSSelectorFromString("handleEventByKeyboardLayout:")
        let layout = unsafeBitCast(method_getImplementation(class_getInstanceMethod(context, layoutSelector)!), to: Layout.self)
        let synchronous: @convention(block) (NSTextInputContext, NSEvent) -> Bool = { [unowned self] context, event in
            synchronousAsks += 1
            DispatchQueue.main.async { if !self.answer(event, context.client) { _ = layout(context, layoutSelector, event) } }
            return false
        }
        _ = replace("handleEvent:", synchronous)
        typealias Discard = @convention(c) (AnyObject, Selector) -> Void
        var discardOriginal: Discard?
        let discard: @convention(block) (NSTextInputContext) -> Void = { [unowned self] context in
            marked = ""
            discardOriginal?(context, NSSelectorFromString("discardMarkedText"))
        }
        discardOriginal = unsafeBitCast(replace("discardMarkedText", discard), to: Discard.self)
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
    }

    override func tearDown() {
        method?.uninstall()
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

    /// A key down and up as AppKit dispatches a person's: the terminal's local monitor, then the window.
    private func press(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], plain: String? = nil) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: window.windowNumber, context: nil, characters: characters,
                                         charactersIgnoringModifiers: plain ?? characters, isARepeat: false, keyCode: code)!
            if !view.routeTerminalKey(event) { window.sendEvent(event) }
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
    /// commits ㅎ itself or keeps composing (as the Korean one does under WebKit), the result is the same.
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

    /// ⌃L with 하 composing commits 하 and clears the screen (the reference's handleBeforeKey still
    /// runs on a "Process" key); nothing else goes out.
    func testControlLCommitsAndClears() {
        compose("gk")
        press(37, "\u{c}", .control, plain: "l")
        settle()
        print("terminal ⌃L while 하 composes: typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed.sorted(), "하\u{c}".sorted())
        XCTAssertFalse(view.composing)
    }

    /// ⌥Return with 한 composing is the input method's (its Hanja list): the terminal never encodes it as
    /// ESC CR, and the input method sees it once. One that commits and declines it sends the syllable alone.
    func testOptionReturnIsTheInputMethods() {
        method.optionReturn = .consumes
        compose("gks")
        press(36, "\r", .option)
        settle()
        print("terminal ⌥Return while 한 composes (Hanja): typed \(hex(typed)) composing \(view.composing) offered \(method.offered)")
        XCTAssertEqual(typed, "")
        XCTAssertTrue(view.composing, "the Hanja list keeps 한 composing")
        XCTAssertEqual(method.offered.filter { $0 == 36 }.count, 1)
        // The person picks nothing and types on: r commits 한.
        press(15, "r")
        settle()
        XCTAssertEqual(typed, "한")

        method.optionReturn = .commitsAndDeclines
        press(46, "m"); press(3, "f")
        settle()
        press(36, "\r", .option)
        settle()
        print("terminal ⌥Return while 글 composes (declined): typed \(hex(typed)) composing \(view.composing)")
        XCTAssertEqual(typed, "한글", "⌥Return reached the shell (ESC CR is the terminal's encoding, a line break the layout's)")
        XCTAssertEqual(method.offered.filter { $0 == 36 }.count, 2)

        // Without a composition ⌥Return is the terminal's again: Ghostty's ESC CR.
        let before = typed
        press(36, "\r", .option)
        settle()
        XCTAssertEqual(String(typed.dropFirst(before.count)), "\u{1b}\r")
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
}
