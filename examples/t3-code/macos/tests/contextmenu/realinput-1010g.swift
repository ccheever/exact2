import AppKit
import WebKit
import XCTest

// realinput-1010g-followups: what the real-input session realinput-1010g found in the shell's menu (#407).
// RG-1: the work group's tool icon sits two views below the node that takes its click, so Copy Image must look below
// the hit view, not only at its own subviews. RG-3: the menu AppKit draws holds DesktopWindow's items only, read back
// from the menu window while it is open (AppKit adds AutoFill and Services to the drawn menu, never to `menu.items`).

/// T3ToolActivityIcon's view: draws the icon's bitmap and lets every press through.
private final class ToolIconView: NSView, T3ShellImageView {
    var shellImage: CGImage?
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A node with `pointer-events: none` (NodeViewMac's `refusesOwnHit`): never the hit itself.
private final class PassThroughNode: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

/// The window's field editor, writing its selection to the test's clipboard (NSText's `cut:` writes to the general one).
private final class ClipboardFieldEditor: NSTextView {
    var board: NSPasteboard?
    override func writeSelection(to pboard: NSPasteboard, types: [NSPasteboard.PasteboardType]) -> Bool {
        super.writeSelection(to: board ?? pboard, types: types)
    }
}

private final class FieldEditorSource: NSObject, NSWindowDelegate {
    let editor = ClipboardFieldEditor()
    override init() { super.init(); editor.isFieldEditor = true }
    func windowWillReturnFieldEditor(_ sender: NSWindow, to client: Any?) -> Any? { client is NSTextField ? editor : nil }
}

private func bluePixel() -> CGImage {
    let context = CGContext(data: nil, width: 4, height: 4, bitsPerComponent: 8, bytesPerRow: 16, space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(NSColor.systemBlue.cgColor)
    context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
    return context.makeImage()!
}

/// The titles of the menu AppKit draws, read from its menu window's accessibility tree while it is open; then it closes.
final class DrawnMenuReader: NSObject, NSMenuDelegate {
    private(set) var drawn: [String]?
    func menuWillOpen(_ menu: NSMenu) {
        RunLoop.main.add(Timer(timeInterval: 0.4, repeats: false) { [weak self] _ in
            self?.drawn = Self.drawnItems()
            menu.cancelTrackingWithoutAnimation()
        }, forMode: .common)
    }
    /// The item labels AppKit drew, in order (each item row's text field; a separator draws none; a key equivalent's
    /// label, after a left-to-right mark, and a submenu's arrow, an attachment character, are left out).
    static func drawnItems() -> [String] {
        var titles: [String] = []
        func walk(_ view: NSView) {
            if let label = view as? NSTextField, !label.stringValue.isEmpty, label.stringValue != "\u{FFFC}", !label.stringValue.hasPrefix("\u{200E}") { titles.append(label.stringValue) }
            for child in view.subviews { walk(child) }
        }
        for window in NSApp.windows where window.isVisible && String(describing: type(of: window)).contains("Menu") {
            if let content = window.contentView { walk(content) }
        }
        return titles
    }
}

final class RealInput1010gTests: ShellMenuCase {
    /// RG-1: ToolActivityIcon (timeline-icons.contract) is a 1rem box › a `pointer-events="none"` hook box ›
    /// T3ToolActivityIcon's view. The click lands on the outer box; Copy Image copies the icon's own bitmap, as the
    /// reference's `<img>` gives Copy Image (DesktopWindow.ts `mediaType === "image"`).
    func testTheToolIconUnderItsPassThroughHookGetsCopyImage() throws {
        let box = NSView(frame: NSRect(x: 20, y: 200, width: 16, height: 16))
        let hook = PassThroughNode(frame: box.bounds)
        let icon = ToolIconView(frame: hook.bounds)
        icon.shellImage = bluePixel()
        hook.addSubview(icon)
        box.addSubview(hook)
        page.addSubview(box)
        let point = at(box, NSPoint(x: 8, y: 8))
        XCTAssertTrue(window.contentView!.hitTest(window.contentView!.superview!.convert(point, from: nil)) === box, "the click is the outer box's")
        rightClick(at: point)
        XCTAssertEqual(popped.map(\.titles), [["Copy Image", "—", "Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]],
                       "the reference's menu on the work group's tool icon")
        XCTAssertTrue(popped.first?.view === box)
        var menu: NSMenu?
        shell.present = { shown, _, _ in menu = shown }
        rightClick(at: point)
        try XCTUnwrap(menu).performActionForItem(at: 0)
        let copied = try XCTUnwrap(NSImage(pasteboard: clipboard))
        XCTAssertEqual(copied.size, NSSize(width: 4, height: 4), "the icon's own bitmap")
        icon.shellImage = nil
        popped = []
        shell.present = { [unowned self] shown, _, view in popped.append((T3ShellMenu.describe(shown), view)) }
        rightClick(at: point)
        XCTAssertEqual(popped.first?.titles.first, "Cut (disabled)", "the fallback glyph (no image loaded): no Copy Image")
        icon.shellImage = bluePixel()
        icon.isHidden = true
        popped = []
        rightClick(at: point)
        XCTAssertEqual(popped.first?.titles.first, "Cut (disabled)", "a hidden icon is not under the pointer")
    }

    /// RG-2: a typed word in a field is selected by the right-click and Cut cuts it (the reference's Settings search).
    func testCutTakesTheWordTheRightClickSelectedInAField() throws {
        let source = FieldEditorSource()
        source.editor.board = clipboard
        window.delegate = source
        defer { window.delegate = nil }
        let field = NSTextField(frame: NSRect(x: 20, y: 240, width: 260, height: 24))
        field.font = .systemFont(ofSize: 14)
        page.addSubview(field)
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        XCTAssertTrue(editor === source.editor)
        for character in "theme" { editor.insertText(String(character), replacementRange: editor.selectedRange()) }
        var menu: NSMenu?
        shell.present = { shown, _, _ in menu = shown }
        var actual = NSRange()
        let rect = editor.firstRect(forCharacterRange: NSRange(location: 1, length: 1), actualRange: &actual)
        rightClick(at: window.convertPoint(fromScreen: NSPoint(x: rect.midX, y: rect.midY)))
        let shown = try XCTUnwrap(menu)
        XCTAssertEqual(T3ShellMenu.describe(shown), ["Cut", "Copy", "Paste (disabled)", "Select All"])
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 0, length: 5))
        shown.performActionForItem(at: 0)
        XCTAssertEqual(field.stringValue, "", "Cut took the word")
        XCTAssertEqual(clipboard.string(forType: .string), "theme", "and holds it on the clipboard")
    }

    /// RG-3: the drawn menu of the composer (a misspelling) and of a field holds the template's items only; AppKit's own
    /// pop-up for the same menu (6bac646cc's `present`) draws AutoFill and Services after them.
    func testTheDrawnShellMenuHasNoAutoFillServicesOrWritingTools() throws {
        let composer = NSTextView(frame: NSRect(x: 20, y: 120, width: 400, height: 60))
        composer.font = .systemFont(ofSize: 14)
        composer.isContinuousSpellCheckingEnabled = true
        composer.string = "Plese chek the parser"
        page.addSubview(composer)
        let field = NSTextField(frame: NSRect(x: 20, y: 240, width: 260, height: 24))
        field.stringValue = "theme"
        page.addSubview(field)
        shell.spelling = T3ShellSpelling(misspelled: { word, _ in word == "chek" }, guesses: { _, _ in ["check", "chef"] })
        func over(_ text: NSTextView, _ index: Int) -> NSPoint {
            var actual = NSRange()
            let rect = text.firstRect(forCharacterRange: NSRange(location: index, length: 1), actualRange: &actual)
            return window.convertPoint(fromScreen: NSPoint(x: rect.midX, y: rect.midY))
        }
        func drawn(_ pop: @escaping (NSMenu, NSEvent, NSView) -> Void, at point: () -> NSPoint) -> [String]? {
            let reader = DrawnMenuReader()
            shell.present = { menu, event, view in menu.delegate = reader; pop(menu, event, view) }
            rightClick(at: point())
            return reader.drawn
        }
        let shellPresent = T3TextContextMenu(agent: false).present
        let appKitPresent: (NSMenu, NSEvent, NSView) -> Void = { menu, event, view in NSMenu.popUpContextMenu(menu, with: event, for: view) }
        let spellingMenu = ["check", "chef", "Cut", "Copy", "Paste", "Select All"]
        let before = try XCTUnwrap(drawn(appKitPresent) { over(composer, 7) })
        let after = drawn(shellPresent) { over(composer, 7) }
        print("t3.rg3: composer \"chek\", AppKit's pop-up (6bac646cc): \(before); the shell's: \(after ?? [])")
        XCTAssertTrue(before.starts(with: spellingMenu) && before.contains("AutoFill"), "AppKit's pop-up adds AutoFill to a text view's menu: \(before)")
        XCTAssertEqual(after, spellingMenu, "the composer's spelling menu, as the reference's")
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        let fieldBefore = try XCTUnwrap(drawn(appKitPresent) { over(editor, 1) })
        let fieldAfter = drawn(shellPresent) { over(editor, 1) }
        print("t3.rg3: field \"theme\", AppKit's pop-up (6bac646cc): \(fieldBefore); the shell's: \(fieldAfter ?? [])")
        XCTAssertTrue(fieldBefore.contains("AutoFill") && fieldBefore.contains("Services"), "a selected word in a field: \(fieldBefore)")
        XCTAssertEqual(fieldAfter, ["Cut", "Copy", "Paste", "Select All"], "Settings' search field with \"theme\"")
        let menu = NSMenu()
        T3TextContextMenu.withoutSystemItems(menu)
        XCTAssertFalse(menu.allowsContextMenuPlugIns)
        if #available(macOS 15.2, *) { XCTAssertFalse(menu.automaticallyInsertsWritingToolsItems, "no Writing Tools where Apple Intelligence is on") }
    }
}

/// A Browser page's web view that keeps the menu WebKit opens (WebKit's menu has a delegate of its own).
private final class ReadingWebView: T3ShellWebView {
    weak var opened: NSMenu?
    override func willOpenMenu(_ menu: NSMenu, with event: NSEvent) {
        super.willOpenMenu(menu, with: event)
        opened = menu
    }
}

/// RG-3 in a Browser page (T3ShellWebView, #407 step 6 and 7): WebKit pops its menu through AppKit too, so the reshaped
/// menu is drawn without Services or AutoFill, as the reference's `<webview>` menu.
final class RealInput1010gWebTests: XCTestCase {
    private var window: NSWindow!
    private var web: ReadingWebView!

    override func setUp() {
        _ = NSApplication.shared
        T3ShellWebView.agent = false
        window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        web = ReadingWebView(frame: NSRect(x: 0, y: 0, width: 600, height: 400))
        window.contentView = web
        window.orderFrontRegardless()
        web.loadHTMLString("""
            <!doctype html><body style="margin:0;font:16px sans-serif">
            <p style="position:absolute;left:10px;top:10px;margin:0">Plain words here</p>
            <input id=f value="Field text" style="position:absolute;left:10px;top:60px;width:200px;font:16px sans-serif">
            </body>
            """, baseURL: URL(string: "http://localhost/"))
        RunLoop.main.run(until: Date().addingTimeInterval(2))
    }
    override func tearDown() { window.orderOut(nil) }

    /// Right-clicks the page at (x, top-down y) and reads the menu AppKit draws, from a timer in the menu's tracking loop.
    private func drawn(_ x: CGFloat, _ y: CGFloat) -> [String]? {
        var drawn: [String]?
        web.opened = nil
        let reader = Timer(timeInterval: 0.1, repeats: true) { [unowned self] timer in
            guard let menu = web.opened else { return }
            let items = DrawnMenuReader.drawnItems()
            guard !items.isEmpty else { return }
            drawn = items
            timer.invalidate()
            menu.cancelTrackingWithoutAnimation()
        }
        RunLoop.main.add(reader, forMode: .common)
        defer { reader.invalidate() }
        let point = NSPoint(x: x, y: 400 - y)
        for type in [NSEvent.EventType.rightMouseDown, .rightMouseUp] {
            NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
        }
        let deadline = Date().addingTimeInterval(4)
        while drawn == nil, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        return drawn
    }

    func testABrowserPagesMenuIsDrawnWithoutServicesOrAutoFill() {
        let word = drawn(30, 18), field = drawn(40, 70)
        print("t3.rg3: Browser page, a word: \(word ?? []); a word in a field: \(field ?? [])")
        XCTAssertEqual(word, ["Cut", "Copy", "Paste", "Select All"], "a word: selected, Copy (#407 step 6)")
        XCTAssertEqual(field, ["Cut", "Copy", "Paste", "Select All"], "a word in a field (#407 step 7)")
    }
}
