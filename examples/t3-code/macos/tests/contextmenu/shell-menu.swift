import AppKit
import WebKit
import XCTest

// shell-context-menu: the desktop shell's menu (DesktopWindow.ts installContextMenu) wherever the page shows none, per
// case as the reference reports it (measured over CDP with the main process's menu read back, 2026-10-10). Clicks go
// through `install()` and `NSApp.sendEvent`, as the window server and the agent (AgentMouseMac.otherClick) deliver them.
// `ExactView` stands in for ExactKit's root view (its Start Speaking is enabled exactly while page text is selected) and
// plain views for its nodes; a test pasteboard stands in for the clipboard.

final class ExactView: NSView, NSMenuItemValidation {
    var selected = false
    private(set) var copies = 0, selections = 0
    @objc func startSpeaking(_ sender: Any?) {}
    @objc func copy(_ sender: Any?) { copies += 1 }
    override func selectAll(_ sender: Any?) { selections += 1 }
    func validateMenuItem(_ item: NSMenuItem) -> Bool { item.action == NSSelectorFromString("startSpeaking:") ? selected : true }
}

/// A node that wrote its own `contextmenu`: NodeViewMac's `dispatchContextMenu` answers and nothing goes up.
private final class OwnMenuNode: NSView {
    private(set) var menus = 0
    override func rightMouseDown(with event: NSEvent) { menus += 1 }
}

/// A paragraph with an inline link run, exposed as ExactKit exposes it (TextInteraction.swift InlineAccessibility).
private final class LinkParagraph: NSView {
    var href = "https://example.test"
    override func accessibilityChildren() -> [Any]? {
        let run = NSAccessibilityElement()
        run.setAccessibilityParent(self)
        run.setAccessibilityRole(.link)
        run.setAccessibilityURL(URL(string: href))
        run.setAccessibilityFrame(window!.convertToScreen(convert(NSRect(x: 0, y: 0, width: 60, height: bounds.height), to: nil)))
        return [run]
    }
}

/// A text view that counts the menu AppKit would have shown (its own editing menu).
private final class CountingTextView: NSTextView {
    private(set) var ownMenus = 0
    override func rightMouseDown(with event: NSEvent) { ownMenus += 1 }
}

/// An app-drawn image under a node (T3ToolActivityIcon's IconView): presses go through it to the node.
private final class DrawnIcon: NSView, T3ShellImageView {
    var shellImage: CGImage?
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private func pixel() -> CGImage {
    let context = CGContext(data: nil, width: 4, height: 4, bitsPerComponent: 8, bytesPerRow: 16, space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(NSColor.systemBlue.cgColor)
    context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
    return context.makeImage()!
}

class ShellMenuCase: XCTestCase {
    var window: NSWindow!
    var page: ExactView!
    var shell: T3TextContextMenu!
    var popped: [(titles: [String], view: NSView)] = []
    let clipboard = NSPasteboard(name: NSPasteboard.Name("t3-shell-menu-tests"))

    override func setUp() {
        _ = NSApplication.shared
        clipboard.clearContents()
        T3ShellMenu.pasteboard = clipboard
        window = NSWindow(contentRect: NSRect(x: 240, y: 240, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        page = ExactView(frame: NSRect(x: 0, y: 0, width: 500, height: 300))
        window.contentView!.addSubview(page)
        window.orderFrontRegardless()
        shell = T3TextContextMenu(agent: false)
        popped = []
        shell.present = { [unowned self] menu, _, view in popped.append((T3ShellMenu.describe(menu), view)) }
        shell.install()
    }
    override func tearDown() {
        shell.destroy()
        window.orderOut(nil)
        T3ShellMenu.pasteboard = .general
    }

    func rightClick(at point: NSPoint) {
        let event = NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        NSApp.sendEvent(event)
    }
    /// The window point of `local` in `view`.
    func at(_ view: NSView, _ local: NSPoint) -> NSPoint { view.convert(local, to: nil) }
}

final class ShellPageMenuTests: ShellMenuCase {
    func testAnEmptyAreaGetsCutCopyPasteDisabledAndSelectAll() {
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]], "the reference's empty timeline and sidebar")
        XCTAssertTrue(popped.first?.view === page)
    }

    func testPageTextSelectedElsewhereEnablesCopyAndTheRolesActOnThePage() throws {
        page.selected = true
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"]])
        let menu = try XCTUnwrap(lastMenu(at: NSPoint(x: 400, y: 200)))
        XCTAssertEqual(menu.items.map(\.keyEquivalent), ["x", "c", "v", "a"], "Electron shows each role's accelerator")
        menu.performActionForItem(at: 1)
        menu.performActionForItem(at: 3)
        XCTAssertEqual(page.copies, 1, "Copy is the page's copy: (ExactView: the selection)")
        XCTAssertEqual(page.selections, 1, "Select All is the page's selectAll:")
    }

    func testANodeWithItsOwnContextmenuKeepsItAndTheShellStaysOut() {
        let node = OwnMenuNode(frame: NSRect(x: 20, y: 20, width: 100, height: 40))
        page.addSubview(node)
        rightClick(at: at(node, NSPoint(x: 10, y: 10)))
        XCTAssertEqual(node.menus, 1, "the app's own menu (a thread row, a file chip, the header)")
        XCTAssertTrue(popped.isEmpty, "preventDefault: Electron's context-menu never fires")
    }

    func testAnInlineLinkGetsCopyLinkWhichCopiesTheCanonicalURL() throws {
        let paragraph = LinkParagraph(frame: NSRect(x: 20, y: 100, width: 300, height: 20))
        page.addSubview(paragraph)
        rightClick(at: at(paragraph, NSPoint(x: 30, y: 10)))
        XCTAssertEqual(popped.map(\.titles), [["Copy Link", "—", "Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
        let menu = try XCTUnwrap(lastMenu(at: at(paragraph, NSPoint(x: 30, y: 10))))
        menu.performActionForItem(at: 0)
        XCTAssertEqual(clipboard.string(forType: .string), "https://example.test/", "params.linkURL is Chromium's canonical URL")
        popped = []
        rightClick(at: at(paragraph, NSPoint(x: 200, y: 10)))
        XCTAssertEqual(popped.first?.titles.first, "Cut (disabled)", "past the run: no link")
        paragraph.href = "javascript:alert(1)"
        popped = []
        rightClick(at: at(paragraph, NSPoint(x: 30, y: 10)))
        XCTAssertEqual(popped.first?.titles.first, "Cut (disabled)", "parseSafeExternalUrl refuses a non-web link")
    }

    func testAnImageGetsCopyImageWhichCopiesItsBitmap() throws {
        let node = NSView(frame: NSRect(x: 300, y: 20, width: 80, height: 80))
        node.wantsLayer = true
        let image = CALayer()
        image.frame = NSRect(x: 0, y: 0, width: 80, height: 80)
        image.contents = pixel()
        node.layer!.addSublayer(image)
        page.addSubview(node)
        rightClick(at: at(node, NSPoint(x: 40, y: 40)))
        XCTAssertEqual(popped.map(\.titles), [["Copy Image", "—", "Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
        let menu = try XCTUnwrap(lastMenu(at: at(node, NSPoint(x: 40, y: 40))))
        menu.performActionForItem(at: 0)
        let copied = try XCTUnwrap(NSImage(pasteboard: clipboard))
        XCTAssertEqual(copied.size, NSSize(width: 4, height: 4), "the image's own bitmap, not the 80 pt view")
    }

    func testAnAppDrawnIconUnderANodeGetsCopyImage() throws {
        let host = NSView(frame: NSRect(x: 20, y: 200, width: 16, height: 16))
        let icon = DrawnIcon(frame: host.bounds)
        icon.shellImage = pixel()
        host.addSubview(icon)
        page.addSubview(host)
        rightClick(at: at(host, NSPoint(x: 8, y: 8)))
        XCTAssertEqual(popped.first?.titles.first, "Copy Image", "the work group's tool icon (an <img> in the reference)")
        XCTAssertTrue(popped.first?.view === host, "the click is the node's: the icon lets it through")
        icon.shellImage = nil
        popped = []
        rightClick(at: at(host, NSPoint(x: 8, y: 8)))
        XCTAssertEqual(popped.first?.titles.first, "Cut (disabled)", "an icon not loaded yet (the fallback glyph): no image")
    }

    func testSelectedTextKeepsRD4sMenuThroughTheSameTemplate() {
        // RD-4's case (text-menu.swift) through the page template: Copy enabled, no Look Up, the click ends in the monitor.
        XCTAssertEqual(T3ShellMenu.describe(T3TextContextMenu.pageMenu(page, rightClickEvent(NSPoint(x: 400, y: 200)), selected: true)),
                       ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"])
    }

    func testUnderTheAgentTheTailLogsTheMenuAndNothingTracks() {
        shell.destroy()
        let agent = T3TextContextMenu(agent: true)
        var tracked = 0
        agent.present = { _, _, _ in tracked += 1 }
        agent.install()
        defer { agent.destroy(); T3ShellWebView.agent = false }
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(agent.lastShown, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"])
        XCTAssertEqual(tracked, 0)
    }

    func testDestroyTakesTheTailOutOfTheChain() {
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertTrue(window.contentView!.nextResponder is T3ShellMenuTail)
        shell.destroy()
        XCTAssertTrue(window.contentView!.nextResponder === window, "the content view's chain is the window's again")
        popped = []
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertTrue(popped.isEmpty)
    }

    func testAWindowWithoutAnExactPageGetsNoMenu() {
        page.removeFromSuperview()
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertTrue(popped.isEmpty, "an alert or panel: Electron's native dialogs have no shell menu")
    }

    private func rightClickEvent(_ point: NSPoint) -> NSEvent {
        NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
    private func lastMenu(at point: NSPoint) -> NSMenu? {
        var menu: NSMenu?
        let recorder = shell.present
        shell.present = { shown, _, _ in menu = shown }
        rightClick(at: point)
        shell.present = recorder
        return menu
    }
}

final class ShellTextMenuTests: ShellMenuCase {
    private var field: NSTextField!
    private var area: CountingTextView!

    override func setUp() {
        super.setUp()
        field = NSTextField(frame: NSRect(x: 20, y: 240, width: 260, height: 24))
        field.font = .systemFont(ofSize: 14)
        page.addSubview(field)
        area = CountingTextView(frame: NSRect(x: 20, y: 120, width: 400, height: 60))
        area.font = .systemFont(ofSize: 14)
        area.isContinuousSpellCheckingEnabled = true
        page.addSubview(area)
        shell.spelling = T3ShellSpelling(misspelled: { word, _ in ["chek", "qzxwvk"].contains(word) },
                                         guesses: { word, _ in word == "chek" ? ["check", "chef", "chew", "chez", "cheek", "whek"] : [] })
    }

    /// The window point over `range`'s first character in `text`.
    private func over(_ text: NSTextView, _ range: NSRange) -> NSPoint {
        var actual = NSRange()
        let screen = text.firstRect(forCharacterRange: NSRange(location: range.location, length: 1), actualRange: &actual)
        return window.convertPoint(fromScreen: NSPoint(x: screen.midX, y: screen.midY))
    }

    func testAnEmptyFieldGetsEveryRoleDisabledAndTakesTheFocus() {
        rightClick(at: at(field, NSPoint(x: 100, y: 12)))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All (disabled)"]], "Settings' empty search field")
        XCTAssertNotNil(field.currentEditor(), "the right-click focuses the input, as Chromium's mousedown does")
    }

    func testAWordInAFieldIsSelectedAndCutAndCopyAreEnabled() throws {
        field.stringValue = "theme colors"
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        editor.setSelectedRange(NSRange(location: 12, length: 0)) // typed: the caret at the end, as the reference's "theme" case
        rightClick(at: over(editor, NSRange(location: 1, length: 1)))
        XCTAssertEqual(popped.map(\.titles), [["Cut", "Copy", "Paste (disabled)", "Select All"]])
        XCTAssertEqual((editor.string as NSString).substring(with: editor.selectedRange()), "theme", "the word under the pointer is selected")
        XCTAssertTrue(popped.first?.view === editor, "the roles act on the field's editor")
    }

    func testPastTheTextNothingIsSelectedAndPasteFollowsTheClipboard() {
        field.stringValue = "theme"
        clipboard.setString("copied", forType: .string)
        rightClick(at: at(field, NSPoint(x: 240, y: 12)))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste", "Select All"]])
    }

    func testAMisspelledWordGetsFiveSuggestionsAndOneReplacesIt() throws {
        area.string = "Plese chek the parser"
        rightClick(at: over(area, NSRange(location: 7, length: 1)))
        XCTAssertEqual(popped.map(\.titles), [["check", "chef", "chew", "chez", "cheek", "—", "Cut", "Copy", "Paste (disabled)", "Select All"]], "the reference's composer on \"chek\"")
        XCTAssertEqual(area.ownMenus, 0, "the click ends in the monitor: AppKit's editing menu never follows")
        var menu: NSMenu?
        shell.present = { shown, _, _ in menu = shown }
        rightClick(at: over(area, NSRange(location: 7, length: 1)))
        try XCTUnwrap(menu).performActionForItem(at: 0)
        XCTAssertEqual(area.string, "Plese check the parser", "replaceMisspelling")
    }

    func testAMisspellingWithoutGuessesSaysNoSuggestions() {
        area.string = "abc qzxwvk"
        rightClick(at: over(area, NSRange(location: 6, length: 1)))
        XCTAssertEqual(popped.first?.titles, ["No suggestions (disabled)", "—", "Cut", "Copy", "Paste (disabled)", "Select All"])
        area.isContinuousSpellCheckingEnabled = false
        popped = []
        rightClick(at: over(area, NSRange(location: 6, length: 1)))
        XCTAssertEqual(popped.first?.titles, ["Cut", "Copy", "Paste (disabled)", "Select All"], "spellcheck=\"false\": no suggestions")
    }

    func testAClickInsideTheSelectionKeepsIt() {
        area.string = "Plese chek the parser"
        window.makeFirstResponder(area)
        area.setSelectedRange(NSRange(location: 0, length: 21))
        rightClick(at: over(area, NSRange(location: 16, length: 1)))
        XCTAssertEqual(area.selectedRange(), NSRange(location: 0, length: 21))
        XCTAssertEqual(popped.first?.titles, ["Cut", "Copy", "Paste (disabled)", "Select All"])
    }

    func testAnEmptyTextareaDisablesSelectAllButAnEmptyComposerKeepsIt() {
        rightClick(at: at(area, NSPoint(x: 200, y: 30)))
        XCTAssertEqual(popped.first?.titles, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All (disabled)"], "an empty <textarea>")
        shell.richEditors = { [unowned self] in [area] }
        popped = []
        rightClick(at: at(area, NSPoint(x: 200, y: 30)))
        XCTAssertEqual(popped.first?.titles, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"], "the composer, a contenteditable")
    }

    func testASecureFieldNeverCutsOrCopies() throws {
        let secure = NSSecureTextField(frame: NSRect(x: 300, y: 240, width: 150, height: 24))
        secure.stringValue = "hunter22"
        page.addSubview(secure)
        window.makeFirstResponder(secure)
        let editor = try XCTUnwrap(secure.currentEditor() as? NSTextView)
        editor.selectAll(nil)
        rightClick(at: at(secure, NSPoint(x: 140, y: 12)))
        XCTAssertEqual(popped.first?.titles, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"])
    }

    func testUnderTheAgentTheInputsMenuIsLoggedAndNothingTracks() {
        shell.destroy()
        let agent = T3TextContextMenu(agent: true)
        var tracked = 0
        agent.present = { _, _, _ in tracked += 1 }
        agent.install()
        defer { agent.destroy(); T3ShellWebView.agent = false }
        rightClick(at: at(field, NSPoint(x: 100, y: 12)))
        XCTAssertEqual(agent.lastShown, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All (disabled)"])
        XCTAssertEqual(tracked, 0)
    }
}

final class ShellTemplateTests: XCTestCase {
    func testSafeExternalURLsAreWebAndRemoteEditorLinks() {
        XCTAssertEqual(T3ShellMenu.safeExternalURL("https://example.com/docs"), "https://example.com/docs")
        XCTAssertEqual(T3ShellMenu.safeExternalURL("http://example.test"), "http://example.test/")
        XCTAssertEqual(T3ShellMenu.safeExternalURL("vscode://vscode-remote/ssh-remote+box/home/a"), "vscode://vscode-remote/ssh-remote+box/home/a")
        XCTAssertEqual(T3ShellMenu.safeExternalURL("zed://ssh/box/home/a"), "zed://ssh/box/home/a")
        for refused in ["javascript:alert(1)", "file:///etc/hosts", "t3code://app/#/", "/#/", "mailto:a@b.c", "vscode://vscode-remote/ssh-remote+", "zed://ssh/u@box/a", ""] {
            XCTAssertNil(T3ShellMenu.safeExternalURL(refused), refused)
        }
    }

    func testTheTemplateOrdersSuggestionsLinkImageThenTheRoles() {
        var params = T3ShellMenuParams(misspelled: true, suggestions: ["a", "b"], link: "https://x.test/", image: true)
        params.canCopy = true
        XCTAssertEqual(T3ShellMenu.describe(T3ShellMenu.menu(params, T3ShellMenuActions())),
                       ["a", "b", "—", "Copy Link", "—", "Copy Image", "—", "Cut (disabled)", "Copy", "Paste (disabled)", "Select All"])
    }
}

/// Browser pages (T3ShellWebView, the Browser panel's and an HTML attachment's web view): WebKit's menu rebuilt as the
/// shell's, read back from a real WKWebView under the agent (the menu is cancelled before it tracks).
final class ShellWebMenuTests: XCTestCase {
    private var window: NSWindow!
    private var web: T3ShellWebView!
    private let clipboard = NSPasteboard(name: NSPasteboard.Name("t3-shell-web-menu-tests"))

    override func setUp() {
        _ = NSApplication.shared
        clipboard.clearContents()
        T3ShellMenu.pasteboard = clipboard
        T3ShellWebView.agent = true
        window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        web = T3ShellWebView(frame: NSRect(x: 0, y: 0, width: 600, height: 400))
        window.contentView = web
        window.orderFrontRegardless()
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR4nGNgYGBgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg=="
        web.loadHTMLString("""
            <!doctype html><body style="margin:0;font:16px sans-serif">
            <p style="position:absolute;left:10px;top:10px;margin:0">Plain words here</p>
            <a href="https://example.com/x" style="position:absolute;left:10px;top:50px">Example link</a>
            <img src="data:image/png;base64,\(png)" style="position:absolute;left:10px;top:90px;width:60px;height:60px">
            <textarea id=a spellcheck=true style="position:absolute;left:10px;top:170px;width:400px;height:30px;font:16px sans-serif">Plese chek the parsr</textarea>
            <input id=e style="position:absolute;left:10px;top:220px;width:200px">
            <p style="position:absolute;left:10px;top:260px;margin:0" oncontextmenu="event.preventDefault()">Own menu</p>
            </body>
            """, baseURL: URL(string: "http://localhost/"))
        spin(2)
    }
    override func tearDown() {
        window.orderOut(nil)
        T3ShellWebView.agent = false
        T3ShellMenu.pasteboard = .general
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    /// Right-clicks the page at (x, top-down y) and waits for the reshaped menu, or nil when the page took the click.
    private func menu(_ x: CGFloat, _ y: CGFloat, prepare: String? = nil) -> [String]? {
        if let prepare { web.evaluateJavaScript(prepare); spin(0.5) }
        var shown: [String]?
        web.shown = { shown = $0 }
        let point = NSPoint(x: x, y: 400 - y)
        for type in [NSEvent.EventType.rightMouseDown, .rightMouseUp] {
            NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
        }
        let deadline = Date().addingTimeInterval(3)
        while shown == nil, Date() < deadline { spin(0.05) }
        return shown
    }

    func testEachPageCaseGetsTheReferencesItems() {
        XCTAssertEqual(menu(30, 18), ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"], "a word: selected, Copy enabled")
        XCTAssertEqual(menu(40, 58, prepare: "getSelection().removeAllRanges()"),
                       ["Copy Link", "—", "Cut (disabled)", "Copy", "Paste (disabled)", "Select All"], "a link: Copy Link, its text selected")
        XCTAssertEqual(menu(30, 110, prepare: "getSelection().removeAllRanges()"),
                       ["Copy Image", "—", "Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"])
        XCTAssertEqual(menu(60, 228), ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All (disabled)"], "an empty <input>")
        XCTAssertNil(menu(30, 268), "the page's own contextmenu (preventDefault): no menu")
        XCTAssertEqual(menu(500, 360, prepare: "document.activeElement.blur(); getSelection().removeAllRanges()"),
                       ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"], "an empty area")
    }

    func testAMisspelledWordInAFieldGetsWebKitsFirstFiveGuesses() throws {
        web.evaluateJavaScript("const a = document.getElementById('a'); a.focus(); a.setSelectionRange(a.value.length, a.value.length); document.execCommand('insertText', false, ' ')")
        spin(1.5)
        let shown = try XCTUnwrap(menu(68, 185))
        let separator = try XCTUnwrap(shown.firstIndex(of: "—"))
        XCTAssertLessThanOrEqual(separator, 5, "at most five suggestions (WebKit lists more)")
        XCTAssertEqual(shown.first, "check", "NSSpellChecker's first guess for \"chek\", as the reference's")
        XCTAssertEqual(Array(shown[(separator + 1)...]), ["Cut", "Copy", "Paste (disabled)", "Select All"])
    }

    func testReshapeKeepsWebKitsOwnItemsForTheHitAndDropsTheRest() {
        let menu = NSMenu()
        func item(_ title: String, _ tag: Int, enabled: Bool = true) -> NSMenuItem {
            let entry = NSMenuItem(title: title, action: T3ShellWebMenu.forward, keyEquivalent: "")
            entry.tag = tag
            entry.isEnabled = enabled
            return entry
        }
        let guess = item("check", 15), link = item("Copy Link", 3)
        for entry in [guess, item("chef", 15), item("Ignore Spelling", 17), item("Open Link", 33), link, item("Look Up", 22),
                      item("Cut", 13), item("Copy", 8, enabled: false), item("Paste", 14)] { menu.addItem(entry) }
        T3ShellWebMenu.reshape(menu, target: nil, clipboard: false)
        XCTAssertEqual(T3ShellMenu.describe(menu), ["check", "chef", "—", "Copy Link", "—", "Cut", "Copy", "Paste (disabled)", "Select All"])
        XCTAssertTrue(menu.items[0] === guess && menu.items[3] === link, "WebKit's items keep their actions (replace the misspelling, copy the hit link)")
    }
}
