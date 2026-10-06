import AppKit
import WebKit
import XCTest

// The terminal surface spike (task 20261005-terminal-surface): the `t3-terminal` view
// (T3TerminalView.swift) running T3 Code's Ghostty page from the app's assets.
// Needs `bun examples/t3-code/terminal-host/build.mjs` first and `T3_APP_DIR` (the example
// directory, whose `assets/` holds the page). `T3_TERMINAL_TEST_DIR` receives PNGs.
// `T3_TERMINAL_SCALE=1` also runs the 1/4/11/44-view cost table (S2), which takes a minute.

/// Records the key events WebKit hands back to the application (an unhandled key is resent
/// through `NSApp.sendEvent`), without activating the app or taking the screen's focus.
final class TestApplication: NSApplication {
    var resent: [NSEvent] = []
    override func sendEvent(_ event: NSEvent) {
        if event.type == .keyDown || event.type == .keyUp { resent.append(event) }
        super.sendEvent(event)
        // This test app is never active (it must not take the screen's focus): log what comes back.
        if event.type == .keyDown, keyWindow == nil, !event.modifierFlags.intersection([.command, .control]).isEmpty {
            print("terminal S1 resent \(event.charactersIgnoringModifiers ?? "") code \(event.keyCode) flags \(event.modifierFlags.rawValue)")
        }
    }
}

/// The responder above the web view: an encoded key must never reach it.
final class Container: NSView {
    var keys: [String] = []
    override var acceptsFirstResponder: Bool { true }
    override func keyDown(with event: NSEvent) { keys.append(event.charactersIgnoringModifiers ?? "?") }
}

final class MenuTarget: NSObject {
    var palette = 0
    @objc func openPalette(_ sender: Any?) { palette += 1 }
}

private var messages: [String] = []
private let recordEvent: ExactNativeEventFn = { _, _, kind, bytes, length in
    let text = bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
    messages.append("\(kind):\(text)")
}

final class TerminalSurfaceTests: XCTestCase {
    private var window: NSWindow!
    private var container: Container!
    private var views: [T3TerminalView] = []
    private let target = MenuTarget()

    override func setUpWithError() throws {
        let app = try XCTUnwrap(ProcessInfo.processInfo.environment["T3_APP_DIR"], "set T3_APP_DIR to examples/t3-code")
        setenv("EXACT_ASSETS", app, 1)
        try XCTSkipUnless(T3TerminalAssets.fileURL("terminal-host.js") != nil, "run terminal-host/build.mjs first")
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        container = Container(frame: window.contentView!.bounds)
        container.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(container)
        window.orderFrontRegardless()
        let menu = NSMenu(), appItem = NSMenuItem(), appMenu = NSMenu()
        let palette = NSMenuItem(title: "Command palette", action: #selector(MenuTarget.openPalette(_:)), keyEquivalent: "k")
        palette.target = target
        appMenu.addItem(palette)
        appItem.submenu = appMenu
        menu.addItem(appItem)
        NSApp.mainMenu = menu
        messages = []
        (NSApp as? TestApplication)?.resent = []
    }

    override func tearDown() {
        for view in views { view.destroy() }
        views = []
        window.orderOut(nil)
    }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 15) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }

    private func mount(_ props: [String: String], frame: NSRect? = nil) -> T3TerminalView {
        // As under the agent: the test window may sit behind other windows or on another Space.
        let view = T3TerminalView(props: props, events: ExactNativeEvents(fn: recordEvent, ctx: nil, nonce: 1), agent: true)
        view.web.frame = frame ?? container.bounds
        container.addSubview(view.web)
        views.append(view)
        return view
    }

    private func waitReady(_ view: T3TerminalView, timeout: TimeInterval = 20) {
        spin(until: { view.ready || !view.lastError.isEmpty }, timeout: timeout)
        if !view.ready { print("terminal not ready: error \(view.lastError) served \(view.assets.served) refused \(view.assets.refusedURLs) bridge \(view.bridgeLog) loaded \(view.loaded)") }
    }

    private func debug(_ view: T3TerminalView) -> [String: Any] {
        var result: [String: Any]?
        view.debug { result = $0 }
        spin(until: { result != nil }, timeout: 5)
        return result ?? [:]
    }

    private func evaluate(_ view: T3TerminalView, _ script: String) -> String {
        var value: String?
        view.web.callAsyncJavaScript(script, arguments: [:], in: nil, in: .page) { result in
            switch result {
            case .success(let any): value = (any as? String) ?? "\(any)"
            case .failure(let error): value = "error: \(error.localizedDescription)"
            }
        }
        spin(until: { value != nil }, timeout: 10)
        return value ?? "timeout"
    }

    private func save(_ png: Data, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_TERMINAL_TEST_DIR"] else { return }
        try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }

    private func typed(_ view: T3TerminalView) -> String {
        view.bridgeLog.filter { $0.hasPrefix("data ") }.compactMap { line -> String? in
            let json = String(line.dropFirst(5))
            return (try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed])) as? String
        }.joined()
    }

    /// S3: the page, script, WASM and font come from the assets through the scheme; nothing else loads.
    /// S4: the snapshot shows the canvas and debug() returns the visible text.
    func testLoadsOfflineAndRendersHarnessBytes() throws {
        let view = mount(["terminal": "render", "fixture": "render"])
        waitReady(view)
        XCTAssertTrue(view.ready, "not ready: \(view.lastError)")
        XCTAssertEqual(Set(view.assets.served), ["terminal-host.html", "terminal-host.js", "ghostty-vt.wasm", "ghostty-write-pty.wasm", "SymbolsNerdFontMono-Regular.woff2"])
        XCTAssertEqual(view.assets.refusedURLs, [])
        spin(until: { (self.debug(view)["text"] as? [String])?.contains { $0.hasPrefix("wide:") } == true })
        let page = debug(view)
        let text = page["text"] as? [String] ?? []
        print("terminal S4 visibility \(page["visibility"] ?? "") grid \(page["cols"] ?? 0)x\(page["rows"] ?? 0) received \(page["received"] ?? 0) last rows \(text.suffix(4))")
        // A wide cell's spacer reads as a space in the row text.
        XCTAssertTrue(text.contains { $0.hasPrefix("wide: 界 面  한 국 어  emoji: 🙂 🚀  nerd:") }, "\(text)")
        XCTAssertTrue(text.contains { $0.contains("scrollback line 2000") }, "\(text)")
        XCTAssertFalse(text.contains("alternate screen"))
        let png = try view.snapshot()
        let rep = try XCTUnwrap(NSBitmapImageRep(data: png))
        var colours = Set<UInt32>()
        for y in stride(from: 0, to: rep.pixelsHigh, by: 7) { for x in stride(from: 0, to: rep.pixelsWide, by: 7) {
            if let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) { colours.insert(UInt32(c.redComponent * 255) << 16 | UInt32(c.greenComponent * 255) << 8 | UInt32(c.blueComponent * 255)) }
        } }
        XCTAssertGreaterThan(colours.count, 8, "the snapshot is blank")
        save(png, "terminal-render.png")

        // No navigation away, no network, no window.
        _ = evaluate(view, "location.href = 'https://example.com/'; return 'sent'")
        _ = evaluate(view, "window.open('https://example.com/x'); return 'sent'")
        spin(until: { view.assets.refusedURLs.count >= 2 }, timeout: 5)
        XCTAssertTrue(view.assets.refusedURLs.contains("https://example.com/"), "\(view.assets.refusedURLs)")
        let fetched = evaluate(view, "try { await fetch('https://example.com/'); return 'loaded' } catch (e) { return 'blocked' }")
        XCTAssertEqual(fetched, "blocked")
        XCTAssertEqual(debug(view)["ready"] as? Bool, true)
    }

    /// S1 and S5: a chord the page leaves to the app reaches the menu; an encoded key stays in
    /// the terminal; agent text and keys arrive as the bytes Ghostty's encoder produces.
    func testKeysReachThePageOrTheApp() throws {
        let view = mount(["terminal": "keys", "chords": "Meta+K"])
        waitReady(view)
        XCTAssertTrue(window.makeFirstResponder(view.web))
        spin(until: { view.focused }, timeout: 5)
        XCTAssertTrue(view.focused, "the page's input did not take the focus")

        try view.agentInput(.text("ls -la"))
        spin(until: { self.typed(view) == "ls -la" }, timeout: 5)
        XCTAssertEqual(typed(view), "ls -la")
        let expected: [(String, String)] = [("Enter", "\r"), ("ArrowUp", "\u{1b}[A"), ("Backspace", "\u{7f}"), ("Tab", "\t"), ("Control+C", "\u{3}"), ("Escape", "\u{1b}")]
        for (chord, bytes) in expected {
            let before = typed(view)
            try view.agentInput(.key(chord, phase: nil))
            spin(until: { self.typed(view).count > before.count }, timeout: 5)
            XCTAssertEqual(String(typed(view).dropFirst(before.count)), bytes, chord)
        }
        let resent = (NSApp as? TestApplication)?.resent.count ?? 0
        XCTAssertEqual(target.palette, 0)
        XCTAssertEqual(container.keys, [], "an encoded key reached the responder above the terminal")

        try view.agentInput(.key("Meta+K", phase: nil))
        // The page leaves ⌘K alone, and WebKit hands the unhandled key back to the application
        // (NSApp.sendEvent), which offers it to the key window's equivalents and the menu. This
        // test app is never active, so AppKit itself skips the menu here; the live app drive shows
        // ⌘K opening the palette from a focused terminal.
        let app = try XCTUnwrap(NSApp as? TestApplication)
        spin(until: { view.declined.contains("Meta+KeyK") && app.resent.count > resent }, timeout: 5)
        XCTAssertEqual(view.declined.last, "Meta+KeyK")
        let back = try XCTUnwrap(app.resent.last)
        XCTAssertEqual(app.resent.count, resent + 1)
        XCTAssertEqual(back.keyCode, 40)
        XCTAssertTrue(back.modifierFlags.contains(.command))
        XCTAssertEqual(typed(view), "ls -la\r\u{1b}[A\u{7f}\t\u{3}\u{1b}", "the declined chord was encoded")
        print("terminal S1 delivery \(view.agentDelivery) declined \(view.declined) palette \(target.palette) container \(container.keys) resent-before \(resent) after \((NSApp as? TestApplication)?.resent.count ?? 0)")
    }

    func testNativePasteShortcutsPreserveUnicodeBracketsAndLaterInputAndOutput() throws {
        let board = NSPasteboard.general
        let saved = (board.pasteboardItems ?? []).map { item in
            item.types.compactMap { type in item.data(forType: type).map { (type, $0) } }
        }
        defer {
            board.clearContents()
            let restored = saved.map { values in
                let item = NSPasteboardItem()
                for (type, data) in values { item.setData(data, forType: type) }
                return item
            }
            if !restored.isEmpty { board.writeObjects(restored) }
        }
        let payload = "alpha 한글😀\nsecondline"
        board.clearContents(); XCTAssertTrue(board.setString(payload, forType: .string))
        let view = mount(["terminal": "native-paste"])
        waitReady(view)
        view.write("\u{1b}[?2004h")
        try view.agentInput(.key("Meta+V", phase: nil))
        let bracketed = "\u{1b}[200~" + payload + "\u{1b}[201~"
        spin(until: { self.typed(view) == bracketed }, timeout: 5)
        XCTAssertEqual(typed(view), bracketed)
        try view.agentInput(.key("Meta+Shift+V", phase: nil))
        spin(until: { self.typed(view) == bracketed + bracketed }, timeout: 5)
        XCTAssertEqual(typed(view), bracketed + bracketed)
        try view.agentInput(.key("Control+D", phase: nil))
        spin(until: { self.typed(view).hasSuffix("\u{4}") }, timeout: 5)
        XCTAssertEqual(typed(view), bracketed + bracketed + "\u{4}")
        view.write("\r\nAFTER_PASTE\r\n")
        spin(until: { (self.debug(view)["text"] as? [String] ?? []).contains { $0.contains("AFTER_PASTE") } }, timeout: 5)
        XCTAssertTrue((debug(view)["text"] as? [String] ?? []).contains { $0.contains("AFTER_PASTE") })
    }

    /// S6 (host path): text the app read from the clipboard reaches the shell as one paste.
    func testHostPaste() {
        let view = mount(["terminal": "paste"])
        waitReady(view)
        view.send(["type": "paste", "text": "echo pasted"])
        spin(until: { self.typed(view) == "echo pasted" }, timeout: 5)
        XCTAssertEqual(typed(view), "echo pasted")
        view.write("\u{1b}[?2004h")
        view.send(["type": "paste", "text": "two\nlines"])
        spin(until: { self.typed(view).hasSuffix("\u{1b}[201~") }, timeout: 5)
        // Ghostty keeps the newline inside the brackets.
        XCTAssertEqual(typed(view), "echo pasted\u{1b}[200~two\nlines\u{1b}[201~")
    }

    /// S7 (partial): composition through the web view's text input client commits once.
    func testCompositionCommitsOnce() throws {
        let view = mount(["terminal": "ime"])
        waitReady(view)
        window.makeFirstResponder(view.web)
        spin(until: { view.focused }, timeout: 5)
        let conforms = (view.web as AnyObject) as? NSTextInputClient
        print("terminal S7 web view is NSTextInputClient: \(conforms != nil); responds to setMarkedText: \(view.web.responds(to: Selector(("setMarkedText:selectedRange:replacementRange:"))))")
        let client = try XCTUnwrap(conforms, "WKWebView is not an NSTextInputClient at run time")
        client.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        client.setMarkedText("하", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        client.setMarkedText("한", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        client.insertText("한", replacementRange: NSRange(location: NSNotFound, length: 0))
        spin(until: { !self.typed(view).isEmpty }, timeout: 5)
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        print("terminal S7 composed \(typed(view).unicodeScalars.map { String($0.value, radix: 16) })")
        XCTAssertEqual(typed(view), "한")
    }

    /// Evidence pictures: the render fixture in dark, and the loopback after typed input.
    func testEvidencePictures() throws {
        let dark = mount(["terminal": "dark", "fixture": "render", "scheme": "dark"])
        waitReady(dark)
        spin(until: { (self.debug(dark)["text"] as? [String])?.contains { $0.hasPrefix("wide:") } == true })
        save(try dark.snapshot(), "terminal-render-dark.png")
        dark.destroy()
        let loop = mount(["terminal": "loop", "fixture": "loopback"])
        waitReady(loop)
        window.makeFirstResponder(loop.web)
        spin(until: { loop.focused }, timeout: 5)
        try loop.agentInput(.text("echo hello"))
        try loop.agentInput(.key("Enter", phase: nil))
        spin(until: { (self.debug(loop)["text"] as? [String])?.contains("$ echo hello") == true }, timeout: 5)
        let text = debug(loop)["text"] as? [String] ?? []
        print("terminal loopback rows \(text.filter { !$0.isEmpty })")
        XCTAssertTrue(text.contains("$ echo hello"), "\(text)")
        RunLoop.main.run(until: Date().addingTimeInterval(0.6))
        save(try loop.snapshot(), "terminal-loopback.png")
    }

    /// Theme/font changes repaint existing output, including changes made during page load.
    func testThemeChangesPreserveOutputAndResizeFont() throws {
        let view = mount(["terminal": "themes", "fixture": "render"])
        let themes: [(String, Bool, [Int], [Int], [Int], String)] = [
            ("light", false, [252,252,252], [39,39,42], [38,56,78], "rgba(37,63,99,0.2)"),
            ("dark", true, [10,10,10], [245,245,245], [180,203,255], "rgba(180,203,255,0.25)"),
            ("grove", false, [243,247,244], [36,21,35], [27,125,80], "#cce1d7"),
            ("custom", true, [18,52,86], [171,205,239], [254,220,186], "#11223380"),
        ]
        var originalCols = 0
        for (name, dark, bg, fg, cursor, selection) in themes {
            func rgb(_ v: [Int]) -> [String: Int] { ["r": v[0], "g": v[1], "b": v[2]] }
            let theme: [String: Any] = ["background": rgb(bg), "foreground": rgb(fg), "cursor": rgb(cursor), "selectionBackground": selection]
            try view.setProps(["terminal": "themes", "fixture": "render", "scheme": dark ? "dark" : "light",
                               "terminal-theme": T3TerminalView.json(theme), "terminal-font-size": "13"])
            waitReady(view)
            spin(until: { (self.debug(view)["text"] as? [String])?.contains { $0.hasPrefix("wide:") } == true })
            let expected = "rgb(\(bg[0]), \(bg[1]), \(bg[2]))"
            spin(until: { self.evaluate(view, "return getComputedStyle(document.body).backgroundColor") == expected })
            XCTAssertEqual(evaluate(view, "return getComputedStyle(document.body).backgroundColor"), expected)
            // Sample a canvas pixel well away from text, not just the surrounding DOM background.
            let pixel = evaluate(view, "const c=document.querySelector('canvas'); return Array.from(c.getContext('2d').getImageData(c.width-20,c.height-10,1,1).data).slice(0,3).join(',')")
            XCTAssertEqual(pixel, bg.map(String.init).joined(separator: ","))
            XCTAssertTrue((debug(view)["text"] as? [String] ?? []).contains { $0.hasPrefix("wide:") })
            save(try view.snapshot(), "theme-\(name).png")
            print("terminal theme \(name) background \(pixel) grid \(view.cols)x\(view.rows) text retained")
            originalCols = view.cols
        }
        var props = view.props
        props["terminal-font"] = "Menlo"; props["terminal-font-size"] = "18"
        try view.setProps(props)
        spin(until: { view.cols < originalCols })
        XCTAssertLessThan(view.cols, originalCols)
        XCTAssertTrue((debug(view)["text"] as? [String] ?? []).contains { $0.hasPrefix("wide:") })
        save(try view.snapshot(), "theme-custom-font18.png")
        print("terminal font resize \(originalCols) -> \(view.cols) cols")
    }

    /// Hidden: paint stops, output still parses; shown again it repaints what arrived.
    func testHiddenKeepsParsing() {
        let view = mount(["terminal": "hidden"])
        waitReady(view)
        try? view.setProps(["terminal": "hidden", "active": "false"])
        view.write("while hidden\r\n")
        spin(until: { (self.debug(view)["received"] as? Int ?? 0) > 0 }, timeout: 5)
        XCTAssertEqual(debug(view)["visible"] as? Bool, false)
        try? view.setProps(["terminal": "hidden", "active": "true"])
        spin(until: { (self.debug(view)["text"] as? [String])?.first == "while hidden" }, timeout: 5)
        XCTAssertEqual((debug(view)["text"] as? [String])?.first, "while hidden")
    }

    /// S2: a 5 MB flood arrives whole, and the main thread keeps turning.
    func testFloodArrivesWhole() {
        let view = mount(["terminal": "flood"])
        waitReady(view)
        var lastTick = Date(), maxGap: TimeInterval = 0
        let timer = Timer.scheduledTimer(withTimeInterval: 0.005, repeats: true) { _ in
            let now = Date(); maxGap = max(maxGap, now.timeIntervalSince(lastTick)); lastTick = now
        }
        let start = Date()
        T3TerminalFixtures.flood(view)
        spin(until: { (self.debug(view)["text"] as? [String])?.contains { $0.hasPrefix("flood done") } == true }, timeout: 60)
        let elapsed = Date().timeIntervalSince(start)
        timer.invalidate()
        let page = debug(view)
        let text = page["text"] as? [String] ?? []
        print("terminal S2 flood sent \(view.sentBytes) received \(page["received"] ?? 0) chunks \(view.sentChunks) seconds \(String(format: "%.2f", elapsed)) max-main-gap-ms \(Int(maxGap * 1000)) last \(text.filter { !$0.isEmpty }.suffix(2))")
        XCTAssertEqual(page["received"] as? Int, view.sentBytes)
        let done = text.first { $0.hasPrefix("flood done") } ?? ""
        let lines = done.split(separator: " ").dropFirst(2).first.flatMap { Int($0) } ?? -1
        XCTAssertTrue(text.contains { $0.hasPrefix("flood \(String(format: "%07d", lines)) ") }, "the last numbered line is missing: \(text.suffix(4))")
        XCTAssertLessThan(elapsed, 30)
    }

    /// S2: process, memory and idle CPU cost of 1, 4, 11 and 44 views.
    func testCostByViewCount() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["T3_TERMINAL_SCALE"] == "1", "set T3_TERMINAL_SCALE=1")
        let baseline = Set(Self.webContent())
        for count in [1, 4, 11, 44] {
            let before = Set(Self.webContent())
            let t0 = Date()
            // Each turn's autoreleased references go with its pool, as in the app's run loop.
            var mounted = autoreleasepool { (0..<count).map { index in
                mount(["terminal": "scale-\(index)", "fixture": "render"], frame: NSRect(x: (index % 11) * 60, y: (index / 11) * 90, width: 300, height: 160))
            } }
            spin(until: { mounted.allSatisfy(\.ready) }, timeout: 120)
            let ready = mounted.filter(\.ready).count, readySeconds = Date().timeIntervalSince(t0)
            RunLoop.main.run(until: Date().addingTimeInterval(2))
            let spawned = Set(Self.webContent()).subtracting(before)
            let pids = Array(spawned) + [Int(getpid())]
            let cpu0 = Self.cpuSeconds(pids)
            RunLoop.main.run(until: Date().addingTimeInterval(5))
            let cpu1 = Self.cpuSeconds(pids)
            let rss = Self.rssKB(Array(spawned)), own = Self.rssKB([Int(getpid())])
            let footprint = Self.footprintMB(Array(spawned))
            print("terminal S2 views \(count) ready \(ready) in \(String(format: "%.1f", readySeconds))s new-web-content-processes \(spawned.count) their-rss-MB \(rss / 1024) (\(spawned.isEmpty ? 0 : rss / 1024 / spawned.count) each) footprint-MB \(footprint) app-rss-MB \(own / 1024) idle-cpu-percent \(String(format: "%.1f", (cpu1 - cpu0) / 5 * 100))")
            final class Weak { weak var web: WKWebView?; weak var view: T3TerminalView?; init(_ v: T3TerminalView) { web = v.web; view = v } }
            let watch = mounted.map(Weak.init)
            autoreleasepool {
                for view in mounted { view.destroy() }
                views.removeAll { view in mounted.contains { $0 === view } }
                mounted = []
            }
            RunLoop.main.run(until: Date().addingTimeInterval(10))
            print("terminal S2 views \(count) released: views alive \(watch.filter { $0.view != nil }.count) web views alive \(watch.filter { $0.web != nil }.count)")
            let left = Set(Self.webContent()).subtracting(baseline)
            print("terminal S2 views \(count) closed and released: web-content-processes above baseline \(left.count)")
            XCTAssertEqual(ready, count)
        }
    }

    static func ps(_ arguments: [String]) -> String {
        let process = Process(), pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/ps")
        process.arguments = arguments
        process.standardOutput = pipe
        try? process.run()
        // Read before waiting: a full pipe would block ps forever.
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return String(decoding: data, as: UTF8.self)
    }
    static func webContent() -> [Int] {
        ps(["-axo", "pid=,comm="]).split(separator: "\n").compactMap { line in
            let parts = line.split(separator: " ", maxSplits: 1)
            return parts.count == 2 && parts[1].contains("com.apple.WebKit.WebContent") ? Int(parts[0]) : nil
        }
    }
    /// phys_footprint (what Activity Monitor calls Memory), summed, from /usr/bin/footprint.
    static func footprintMB(_ pids: [Int]) -> Int {
        pids.map { pid -> Int in
            let process = Process(), pipe = Pipe()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/footprint")
            process.arguments = [String(pid)]
            process.standardOutput = pipe; process.standardError = Pipe()
            try? process.run()
            let text = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            guard let line = text.split(separator: "\n").first(where: { $0.contains("phys_footprint:") }) else { return 0 }
            let parts = line.split(separator: " ")
            guard parts.count >= 3, let value = Double(parts[1]) else { return 0 }
            return Int(parts[2].hasPrefix("GB") ? value * 1024 : parts[2].hasPrefix("KB") ? value / 1024 : value)
        }.reduce(0, +)
    }
    static func rssKB(_ pids: [Int]) -> Int {
        guard !pids.isEmpty else { return 0 }
        return ps(["-o", "rss=", "-p", pids.map(String.init).joined(separator: ",")]).split(separator: "\n").compactMap { Int($0.trimmingCharacters(in: .whitespaces)) }.reduce(0, +)
    }
    static func cpuSeconds(_ pids: [Int]) -> Double {
        ps(["-o", "time=", "-p", pids.map(String.init).joined(separator: ",")]).split(separator: "\n").map { field -> Double in
            let parts = field.trimmingCharacters(in: .whitespaces).split(separator: ":").map { Double($0) ?? 0 }
            return parts.reduce(0) { $0 * 60 + $1 }
        }.reduce(0, +)
    }
}

setvbuf(stdout, nil, _IOLBF, 0)
_ = TestApplication.shared
NSApp.setActivationPolicy(.accessory)
var suite = XCTestSuite(name: "terminal")
suite.addTest(XCTestSuite(forTestCaseClass: TerminalSurfaceTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: TerminalDrawerTests.self)) // terminal-drawer (drawer.swift)
suite.addTest(XCTestSuite(forTestCaseClass: TerminalAuthTests.self))
if let only = ProcessInfo.processInfo.environment["T3_TERMINAL_ONLY"] {
    let picked = XCTestSuite(name: only)
    for case let group as XCTestSuite in suite.tests { for test in group.tests where test.name.contains(only) { picked.addTest(test) } }
    suite = picked
}
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
