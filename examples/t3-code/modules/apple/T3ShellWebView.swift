// The desktop shell's context menu in a web view the page shows (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/window/DesktopWindow.ts `installContextMenu`, which the shell installs on every attached
// `<webview>`, the Browser panel's pages, and on the windows a page opens). WebKit builds its own menu for a right-click
// the page did not take (a `contextmenu` listener that called `preventDefault` gets none, as in Electron) and hands it
// to `willOpenMenu` before it opens; this view rebuilds that menu as the shell's from what WebKit found under the
// pointer: WebKit's own spelling guesses (the first five; "No suggestions" for its "No Guesses Found"), its Copy Link
// and Copy Image (which act on the hit element), then Cut, Copy, Paste and Select All on the page. WebKit enables Paste
// in any editable field, Chromium only with something on the clipboard; WebKit selects a link's text, as Chromium does,
// but lists no Copy for it; Select All is disabled in an empty `<input>` or `<textarea>` (read from the page, so it can
// turn off a moment after the menu opens). Under the agent the items go to the log (`t3.textmenu:`) and nothing tracks.
#if os(macOS)
import AppKit
import WebKit

class T3ShellWebView: WKWebView {
    /// Whether the session runs under the agent (set by the module's T3TextContextMenu).
    static var agent = false
    /// What the last menu showed (the agent's log line, and the tests).
    private(set) var lastShown: [String] = []
    /// Called once the menu's items are final (tests).
    var shown: (([String]) -> Void)?

    override func willOpenMenu(_ menu: NSMenu, with event: NSEvent) {
        let selectAll = T3ShellWebMenu.reshape(menu, target: self)
        lastShown = T3ShellMenu.describe(menu)
        if Self.agent {
            menu.removeAllItems()
            RunLoop.main.perform(inModes: [.eventTracking]) { menu.cancelTracking() }
        }
        super.willOpenMenu(menu, with: event)
        evaluateJavaScript(T3ShellWebMenu.emptyTextControl) { [weak self] result, _ in
            guard let self else { return }
            if result as? Bool == true {
                selectAll.isEnabled = false
                if let last = self.lastShown.indices.last { self.lastShown[last] = "Select All (disabled)" }
            }
            if Self.agent { FileHandle.standardError.write(Data("t3.textmenu: \(self.lastShown.joined(separator: " | "))\n".utf8)) }
            self.shown?(self.lastShown)
        }
    }
}

enum T3ShellWebMenu {
    /// WebKit's context menu items act through this action, each named by its WebCore `ContextMenuAction` tag.
    static let forward = NSSelectorFromString("forwardContextMenuAction:")
    /// WebCore ContextMenuItem.h: CopyLinkToClipboard 3, CopyImageToClipboard 6, Copy 8, Cut 13, Paste 14,
    /// SpellingGuess 15, NoGuessesFound 16 (read back from WebKit's menus on macOS 26, shell-context-menu).
    enum Tag { static let copyLink = 3, copyImage = 6, copy = 8, cut = 13, paste = 14, guess = 15, noGuesses = 16 }
    /// Chromium's `canSelectAll` is false in an empty text control.
    static let emptyTextControl = "(() => { const e = document.activeElement; return !!e && (e instanceof HTMLInputElement || e instanceof HTMLTextAreaElement) && e.value === ''; })()"

    /// Rebuilds WebKit's `menu` as the shell's; returns its Select All. The editing roles go to `target` (the web view).
    @discardableResult
    static func reshape(_ menu: NSMenu, target: AnyObject?, clipboard: Bool = T3ShellMenu.clipboardHasContent()) -> NSMenuItem {
        let items = menu.items
        func webKit(_ tag: Int) -> [NSMenuItem] { items.filter { $0.action == forward && $0.tag == tag } }
        let guesses = webKit(Tag.guess), noGuesses = webKit(Tag.noGuesses).first
        let copyLink = webKit(Tag.copyLink).first, copyImage = webKit(Tag.copyImage).first
        let cut = webKit(Tag.cut).first, copy = webKit(Tag.copy).first, paste = webKit(Tag.paste).first
        menu.removeAllItems()
        menu.autoenablesItems = false
        if !guesses.isEmpty || noGuesses != nil {
            for guess in guesses.prefix(5) { menu.addItem(guess) }
            if guesses.isEmpty {
                let none = NSMenuItem(title: "No suggestions", action: nil, keyEquivalent: "")
                none.isEnabled = false
                menu.addItem(none)
            }
            menu.addItem(.separator())
        }
        if let copyLink {
            copyLink.title = "Copy Link"
            menu.addItem(copyLink)
            menu.addItem(.separator())
        }
        if let copyImage {
            copyImage.title = "Copy Image"
            menu.addItem(copyImage)
            menu.addItem(.separator())
        }
        let canCut = cut?.isEnabled == true
        let canCopy = copy?.isEnabled == true || copyLink != nil
        let canPaste = paste != nil && clipboard
        T3ShellMenu.addRoles(to: menu, target: target, enabled: [canCut, canCopy, canPaste, true])
        return menu.items[menu.items.count - 1]
    }
}
#endif
