// The desktop shell's context menu (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/window/DesktopWindow.ts `installContextMenu`, electron/ElectronShell.ts `parseSafeExternalUrl`).
// Electron raises `context-menu` for a right-click the page did not take (no `contextmenu` listener called
// `preventDefault`) on the window, on windows the page opens and on every attached `<webview>`, and the shell pops,
// in order: on a misspelled word up to five dictionary suggestions (each replaces the word) or "No suggestions"
// (disabled), then a separator; over a safe external link Copy Link and a separator; over an image Copy Image and a
// separator; and always Cut, Copy, Paste and Select All, each enabled by Chromium's edit flags.
//
// The flags as the reference reports them (shell-context-menu, measured over CDP with the main process's menu read
// back): a page selection enables Copy; editable content enables Cut with a selection and Paste with something on the
// clipboard; Select All is enabled everywhere except in an empty text control (an `<input>` or `<textarea>`; the
// composer, a contenteditable, keeps it). A right-click on text first selects the word under it (Chromium on a Mac),
// on a misspelled word the misspelling, on a link its text; inside an existing selection it keeps that selection.
#if os(macOS)
import AppKit

/// Electron's `context-menu` params, as far as a Mac window can know them.
struct T3ShellMenuParams: Equatable {
    var misspelled = false
    var suggestions: [String] = []
    var link: String?
    var image = false
    var canCut = false
    var canCopy = false
    var canPaste = false
    var canSelectAll = true
}

/// An app view that draws an image (an `<img>` in the reference) under a node and lets presses through to it (its
/// `hitTest` is nil), such as a tool's icon (T3ToolActivityIcon.swift): the shell's Copy Image copies its bitmap.
protocol T3ShellImageView: NSView {
    var shellImage: CGImage? { get }
}

/// What the items act on. The four editing roles go to `editTarget` (nil: the first responder), as Electron's roles go
/// to the focused contents; a suggestion calls `replace`; Copy Link writes the link as text, Copy Image the image.
struct T3ShellMenuActions {
    var editTarget: AnyObject?
    var replace: ((String) -> Void)?
    var image: CGImage?
}

/// An item's action that is not a responder action (a suggestion, Copy Link, Copy Image). `NSMenuItem.target` is weak,
/// so the item keeps it as its `representedObject`.
final class T3ShellMenuAction: NSObject {
    private let run: () -> Void
    init(_ run: @escaping () -> Void) { self.run = run }
    @objc func choose(_ sender: Any?) { run() }
}

enum T3ShellMenu {
    /// The clipboard the items write and the flags read (tests use a private one).
    static var pasteboard = NSPasteboard.general

    /// ElectronShell `parseSafeExternalUrl`: http(s), or a remote editor's SSH deep link
    /// (`<scheme>://vscode-remote/ssh-remote+<host>…`, `zed://ssh/<host>/<path>`, no user info).
    static func safeExternalURL(_ raw: String?) -> String? {
        guard let raw, let url = URL(string: raw), let scheme = url.scheme?.lowercased() else { return nil }
        if scheme == "http" || scheme == "https" {
            // Chromium's `linkURL` is the canonical URL: an empty path is "/".
            guard url.host?.isEmpty == false, var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return nil }
            if parts.path.isEmpty { parts.path = "/" }
            return parts.string
        }
        guard remoteEditorSchemes.contains(scheme), url.user == nil, url.password == nil else { return nil }
        let path = url.path
        if scheme == "zed" { return url.host == "ssh" && path.range(of: #"^/[^/@:]+/.*$"#, options: .regularExpression) != nil ? url.absoluteString : nil }
        return url.host == "vscode-remote" && path.hasPrefix("/ssh-remote+") && path.count > "/ssh-remote+".count ? url.absoluteString : nil
    }
    /// packages/contracts editor.ts: the editors with a `remoteScheme`.
    static let remoteEditorSchemes: Set<String> = ["cursor", "vscode", "vscode-insiders", "vscodium", "zed"]

    /// The editing roles with the accelerators Electron shows for them.
    static let roles: [(title: String, action: Selector, key: String)] = [
        ("Cut", #selector(NSText.cut(_:)), "x"),
        ("Copy", #selector(NSText.copy(_:)), "c"),
        ("Paste", #selector(NSText.paste(_:)), "v"),
        ("Select All", #selector(NSResponder.selectAll(_:)), "a"),
    ]

    /// DesktopWindow's template for these params.
    static func menu(_ params: T3ShellMenuParams, _ actions: T3ShellMenuActions) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        if params.misspelled {
            for suggestion in params.suggestions.prefix(5) {
                menu.addItem(action(suggestion) { actions.replace?(suggestion) })
            }
            if params.suggestions.isEmpty {
                let none = NSMenuItem(title: "No suggestions", action: nil, keyEquivalent: "")
                none.isEnabled = false
                menu.addItem(none)
            }
            menu.addItem(.separator())
        }
        if let link = safeExternalURL(params.link) {
            menu.addItem(action("Copy Link") { copyText(link) })
            menu.addItem(.separator())
        }
        if params.image {
            menu.addItem(action("Copy Image") { if let image = actions.image { copyImage(image) } })
            menu.addItem(.separator())
        }
        addRoles(to: menu, target: actions.editTarget, enabled: [params.canCut, params.canCopy, params.canPaste, params.canSelectAll])
        return menu
    }

    static func addRoles(to menu: NSMenu, target: AnyObject?, enabled: [Bool]) {
        for (index, role) in roles.enumerated() {
            let item = NSMenuItem(title: role.title, action: role.action, keyEquivalent: role.key)
            item.keyEquivalentModifierMask = .command
            item.target = target
            item.isEnabled = enabled[index]
            menu.addItem(item)
        }
    }

    private static func action(_ title: String, _ run: @escaping () -> Void) -> NSMenuItem {
        let holder = T3ShellMenuAction(run)
        let item = NSMenuItem(title: title, action: #selector(T3ShellMenuAction.choose(_:)), keyEquivalent: "")
        item.target = holder
        item.representedObject = holder
        item.isEnabled = true
        return item
    }

    /// Electron `clipboard.writeText`.
    static func copyText(_ text: String, to pasteboard: NSPasteboard = T3ShellMenu.pasteboard) {
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    /// `webContents.copyImageAt`: the image's own bitmap, not the pixels on screen.
    static func copyImage(_ image: CGImage, to pasteboard: NSPasteboard = T3ShellMenu.pasteboard) {
        pasteboard.clearContents()
        pasteboard.writeObjects([NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))])
    }

    /// Whether the clipboard holds something a paste takes (Chromium's `canPaste` is false on an empty clipboard).
    static func clipboardHasContent(_ pasteboard: NSPasteboard = T3ShellMenu.pasteboard, readable: [NSPasteboard.PasteboardType]? = nil) -> Bool {
        let types = readable ?? [.string, .rtf, .rtfd, .html, .png, .tiff, .fileURL, .URL]
        return pasteboard.availableType(from: types) != nil
    }

    /// The menu as the agent logs it: a separator is "—", a disabled item "<title> (disabled)".
    static func describe(_ menu: NSMenu) -> [String] {
        menu.items.map { $0.isSeparatorItem ? "—" : $0.isEnabled ? $0.title : "\($0.title) (disabled)" }
    }
}
#endif
