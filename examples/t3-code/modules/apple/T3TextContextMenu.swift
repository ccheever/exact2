// The desktop shell's context menu in the window (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/window/DesktopWindow.ts `installContextMenu`; the template is T3ShellMenu.swift). Electron raises
// `context-menu` for every secondary click the page did not take, and the shell answers it. Contract has no window-wide
// context-menu hook, so this module finds the same clicks itself (realinput-1010d RD-4, shell-context-menu). A secondary
// click is a right-click or, on a Mac, a Control-click (Chromium sends both as `contextmenu`; ExactKit runs a node's
// `contextmenu` for a Control-click since main #487, X85, adopted in round 11), and each case below takes both:
//
// - A text input (an `<input>`'s field, a `<textarea>`, the composer): AppKit's own editing menu would open, so a local
//   right-click monitor takes the click, focuses the input as Chromium's mousedown does, selects the word (or the
//   misspelling) under the pointer unless the click is inside the selection, and pops the shell's menu for the input:
//   suggestions from NSSpellChecker where the input checks spelling, Cut, Copy, Paste and Select All by its state.
// - Selected page text: ExactKit would show the menu an NSTextView shows for read-only text (since main `6a7f16c6f`
//   AppKit's own, from a text view holding the selection: Look Up, Translate, Search With, Copy, Share, Writing Tools,
//   Speech, Services), so the monitor takes that click too and pops the shell's on the same text node: Copy (the node's
//   `copy:`, its `copy` event first) and Select All (the node's `selectAll:`).
// - Anywhere else on the page: the click goes on to ExactKit. A node with a `contextmenu` (or a context popover) answers
//   it and ends it there, as `preventDefault` keeps Electron's event from firing; a click no node answers is passed up
//   the responder chain to the window, and a responder placed after the window's content view (`T3ShellMenuTail`) pops
//   the shell's menu for the page: Copy Link over a link (the inline link's accessibility URL), Copy Image over an image
//   (the image layer's bitmap), Copy while page text is selected (ExactKit enables Edit ▸ Speech ▸ Start Speaking exactly
//   then), Select All.
// - A Browser page answers in its web view (T3ShellWebView.swift); other web views (the terminal) keep their own menus.
// - A node whose `contextmenu` leaves the click to the shell (a reply's mailto, irc, xmpp or fragment link: ChatMarkdown's
//   anchor returns before `preventDefault` when the link has no web host) asks for the shell's menu of that click
//   (`shellMenu`, external-link-menu.ts; reply-links RL-1, RL-2), as Electron's `context-menu` then fires. The click is
//   answered once, and only while it is the last input: a key or another click ends it, and so does a menu that answered
//   it (the tail's, or the app's own through T3ContextMenu: `pageClickAnswered`), for a path that sends no event (the
//   agent's keys). The ContextMenu key (ExactKit raises the focused node's `contextmenu` for it, MouseEventsMac
//   `contextMenuKey`) is answered the same way: the monitor keeps the key, its window and the focused node, and
//   `shellMenu` opens the shell's menu at that node, once, while it is the last input and still focused, and only for the
//   link the request names when the node exposes its URL (`focusMenu`, reply-links-reference-behavior RL-6). A
//   `contextmenu` that neither a fresh click nor a fresh ContextMenu key raised opens nothing. The agent's keys reach its
//   window past NSApplication (AgentMac `win.sendEvent`), so under the agent a `contextmenu` with no click is taken as its
//   ContextMenu key at the focus of the window it drives (the agent's right-click goes through NSApplication and is a
//   click here). VoiceOver's show menu (AXShowMenu) runs the node's `contextmenu` with no event (main #486, X84, adopted in
//   round 11), as Chromium fires it at the element: while an assistive technology that sends it is on, a `contextmenu` that
//   neither a click nor a key raised is that request, and the shell's menu opens for the link it names, at the pointer as
//   Electron pops its template (`assistiveMenu`).
//
// The monitor ends every click it answers (it returns nil, never `self?.handle(event) ?? event`: optional chaining
// would flatten handle's nil back into the event and the host's menu would pop after the shell's). Under the agent
// nothing pops up (its window is never key, so a menu would track until a real click, as T3ContextMenu.perform notes):
// the items go to the log (`t3.textmenu:`) instead.
#if os(macOS)
import AppKit
import WebKit

/// The spelling checks a text input's menu asks (NSSpellChecker; tests stub them).
struct T3ShellSpelling {
    var misspelled: (String, NSTextView) -> Bool
    var guesses: (String, NSTextView) -> [String]
    static let system = T3ShellSpelling(misspelled: { word, view in
        let found = NSSpellChecker.shared.checkSpelling(of: word, startingAt: 0, language: nil, wrap: false,
                                                        inSpellDocumentWithTag: view.spellCheckerDocumentTag, wordCount: nil)
        return found.location == 0 && found.length == (word as NSString).length
    }, guesses: { word, view in
        NSSpellChecker.shared.guesses(forWordRange: NSRange(location: 0, length: (word as NSString).length), in: word,
                                      language: NSSpellChecker.shared.language(), inSpellDocumentWithTag: view.spellCheckerDocumentTag) ?? []
    })
}

final class T3TextContextMenu: NSObject {
    /// The text view whose own menu ExactKit gives for selected page text (TextMenuMac.swift, main `6a7f16c6f`): AppKit's
    /// menu for a read-only text view holding the selection, its items pointed at that view.
    static let hostTextMenuView = "SelectionMenuText"
    /// ExactView's Edit ▸ Speech ▸ Start Speaking, which it enables exactly while page text is selected.
    static let startSpeaking = NSSelectorFromString("startSpeaking:")

    private let agent: Bool
    private var monitor: Any?
    private var tails: [ObjectIdentifier: T3ShellMenuTail] = [:]
    /// What the last answered click showed (the agent's log line, and the tests).
    private(set) var lastShown: [String] = []
    /// The last right-click on the page that went on to ExactKit, and when (`shellMenu` answers it once, while fresh, and
    /// only while no key or other click has come after it and no menu has answered it).
    private var lastPageClick: (event: NSEvent, at: TimeInterval)?
    /// The last ContextMenu key with no modifier (the one ExactKit answers), the window it went to and the node that had the
    /// focus, and when: `shellMenu` with no fresh click answers it once, while fresh, last and that node still focused.
    private var lastMenuKey: MenuKey?
    private struct MenuKey {
        weak var window: NSWindow?
        weak var focus: NSView?
        let at: TimeInterval
    }
    /// The ContextMenu key's virtual key (Carbon's kVK_ContextualMenu), which ExactKit names "ContextMenu" (KeyCodes.swift).
    static let contextMenuKeyCode: UInt16 = 0x6E
    /// Pops the shell's menu (tracks until the menu closes); a test records it instead. Without context-menu plug-ins:
    /// `popUpContextMenu` appends Services for a view that answers `validRequestor`, and AutoFill for a text view, which
    /// Electron's menu (`popUpMenuPositioningItem`, DesktopWindow's template) has neither of (realinput-1010f RF-1,
    /// realinput-1010g RG-3); nor Writing Tools, which macOS 15.2 and later add to a text view's menu where Apple
    /// Intelligence is on.
    var present: (NSMenu, NSEvent, NSView) -> Void = { menu, event, view in
        T3TextContextMenu.withoutSystemItems(menu)
        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }

    /// The shell's menu holds only DesktopWindow's items: no Services, AutoFill or Writing Tools.
    static func withoutSystemItems(_ menu: NSMenu) {
        menu.allowsContextMenuPlugIns = false
        if #available(macOS 15.2, *) { menu.automaticallyInsertsWritingToolsItems = false }
    }
    /// Whether an assistive technology that sends AXShowMenu is on (VoiceOver, Switch Control); the tests inject it.
    var assistive: () -> Bool = { NSWorkspace.shared.isVoiceOverEnabled || NSWorkspace.shared.isSwitchControlEnabled }
    /// The editors that are a contenteditable in the reference (the composer, Settings › Appearance's prompt sample):
    /// their Select All stays enabled when they are empty, a text control's does not.
    var richEditors: () -> [NSTextView] = { [] }
    var spelling = T3ShellSpelling.system

    init(agent: Bool) {
        self.agent = agent
        T3ShellWebView.agent = agent
    }

    func install() {
        guard monitor == nil else { return }
        // Not `self?.handle(event) ?? event`: optional chaining flattens handle's `NSEvent?`, so its nil ("taken") would
        // turn back into the event and the host's menu would pop after the shell's.
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.rightMouseDown, .leftMouseDown, .otherMouseDown, .keyDown]) { [weak self] event in
            guard let self else { return event }
            return self.handle(event)
        }
    }

    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        for tail in tails.values { tail.detach() }
        tails.removeAll()
    }

    /// The click ends here when the shell answers it (a text input, selected page text); the rest goes on, with the
    /// window's tail in place for a click no page node answers. A web view answers its own clicks.
    func handle(_ event: NSEvent) -> NSEvent? {
        // Any key or click ends the last page click and the last ContextMenu key: a `contextmenu` after it was not raised by
        // either (`shellMenu`).
        lastPageClick = nil
        lastMenuKey = nil
        if event.type == .keyDown, event.keyCode == Self.contextMenuKeyCode,
           event.modifierFlags.intersection([.shift, .control, .option, .command]).isEmpty, let window = event.window {
            lastMenuKey = MenuKey(window: window, focus: window.firstResponder as? NSView, at: ProcessInfo.processInfo.systemUptime)
        }
        guard Self.isSecondaryClick(event), let window = event.window, let content = window.contentView else { return event }
        let point = content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
        guard let hit = content.hitTest(point) else { return event }
        if Self.ancestors(hit).contains(where: { $0 is WKWebView }) { return event }
        if let input = Self.textInput(hit) {
            guard let (menu, view) = textMenu(input, event) else { return event }
            show(menu, event, view)
            return nil
        }
        if let (menu, view) = Self.replacement(for: event) {
            show(menu, event, view)
            return nil
        }
        if Self.page(hit) != nil {
            attachTail(to: window)
            lastPageClick = (event, ProcessInfo.processInfo.systemUptime)
        }
        return event
    }

    /// A secondary click: the right button, or the primary with Control held, which a Mac takes as the secondary click
    /// (Chromium sends it as `contextmenu` and no `click`; ExactKit since main #487, X85).
    static func isSecondaryClick(_ event: NSEvent) -> Bool {
        event.type == .rightMouseDown || (event.type == .leftMouseDown && event.modifierFlags.contains(.control))
    }

    /// The app showed its own menu for a node (`contextMenu`): that node took the last page click, as `preventDefault` keeps
    /// Electron's `context-menu` from firing, so `shellMenu` no longer answers it.
    func pageClickAnswered() { lastPageClick = nil; lastMenuKey = nil }

    /// `shellMenu`: the node that took the last page click left it to the shell; the shell's menu for that click, as the
    /// page shows it (`pageClick`). With no such click (none, one already answered, or one older than a few seconds), the
    /// last ContextMenu key's, for which Chromium raises `context-menu` at the focused anchor and the shell's menu opens
    /// (reply-links-reference-behavior RL-6): the menu for the node it went to, while that node keeps the focus
    /// (`focusMenu`). Under the agent, whose keys leave no record, the focus of the window it drives (`windows`, the
    /// frontmost first). Otherwise a `contextmenu` neither raised is an assistive technology's show menu (main #486, X84):
    /// while one is on, the shell's menu for the link it names (`assistiveMenu`); else nothing. `href` is the link the
    /// request names.
    func shellMenuForLastClick(href: String = "", now: TimeInterval = ProcessInfo.processInfo.systemUptime, windows: [NSWindow]? = nil) -> Bool {
        let click = lastPageClick, key = lastMenuKey
        lastPageClick = nil
        lastMenuKey = nil
        if let click, now - click.at < 5 { return pageClick(click.event) }
        if let key, now - key.at < 5 {
            guard let window = key.window, let focus = key.focus, window.firstResponder === focus else { return false }
            return focusMenu(focus, in: window, href: href)
        }
        if agent {
            for window in windows ?? NSApp.orderedWindows where window.isVisible {
                guard let focus = window.firstResponder as? NSView else { continue }
                if focusMenu(focus, in: window, href: href) { return true }
            }
            return false
        }
        guard assistive() else { return false }
        return assistiveMenu(href: href, windows: windows ?? NSApp.orderedWindows)
    }

    /// VoiceOver's show menu on a link whose `contextmenu` leaves it to the shell: Chromium fires `contextmenu` at the
    /// element and Electron's `context-menu` follows with the element's link (`params.linkURL`); DesktopWindow pops the
    /// template with no position, so at the pointer. Here the frontmost window with a page: Copy Link for `href` where the
    /// shell allows the URL, Copy while page text is selected, the roles on the page.
    func assistiveMenu(href: String, windows: [NSWindow]) -> Bool {
        for window in windows where window.isVisible {
            guard let content = window.contentView, let page = Self.pageView(in: content),
                  let event = NSEvent.mouseEvent(with: .rightMouseDown, location: window.mouseLocationOutsideOfEventStream, modifierFlags: [],
                                                 timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                                 context: nil, eventNumber: 0, clickCount: 1, pressure: 1) else { continue }
            var params = T3ShellMenuParams()
            params.link = href.isEmpty ? nil : href
            params.canCopy = Self.selectionExists(page)
            show(T3ShellMenu.menu(params, T3ShellMenuActions(editTarget: page)), event, page)
            return true
        }
        return false
    }

    /// The shell's menu for the focused page node `focus` (not the page itself or a text input, whose own menu is the
    /// right-click's), as Chromium's keyboard `context-menu` has it: at the node's middle, Copy while page text is selected,
    /// no Copy Link for a URL the shell refuses. Nothing when the node exposes a URL other than `href`: a `link` node's
    /// `AXURL` is its `href` (X78, fixed on main by #455), so a focused link that is not the one whose `contextmenu` asked
    /// opens nothing. The reference pops it with no position, so Electron places it at the pointer; a keyboard menu here
    /// opens at the focus, as ExactKit's ContextMenu key and T3ContextMenu's `keyboard` do.
    func focusMenu(_ focus: NSView, in window: NSWindow, href: String = "") -> Bool {
        guard window.isVisible, let content = window.contentView, focus.isDescendant(of: content), let page = Self.page(focus),
              focus !== page, Self.textInput(focus) == nil else { return false }
        if !href.isEmpty, let url = focus.accessibilityURL(), url != URL(string: href) { return false }
        // Where ExactKit's ContextMenu key raises the `contextmenu` (MouseEventsMac `contextMenuKey`): the middle of the node
        // as clipped by the ancestors that clip (an `overflow: hidden` node's `clipsToBounds`, a scroll view's clip view),
        // which `visibleRect` follows from macOS 14 (it can reach past the node's bounds there, hence the intersection);
        // ExactKit also follows a CSS transform (its `plane`, which an app module cannot read), and a reply's links have none.
        let visible = focus.visibleRect.intersection(focus.bounds)
        guard !visible.isEmpty,
              let event = NSEvent.mouseEvent(with: .rightMouseDown, location: focus.convert(NSPoint(x: visible.midX, y: visible.midY), to: nil),
                                             modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                             context: nil, eventNumber: 0, clickCount: 1, pressure: 1) else { return false }
        show(Self.pageMenu(focus, event, selected: Self.selectionExists(page)), event, focus)
        return true
    }

    // MARK: Selected page text (RD-4)

    /// The shell's menu for a right-click on selected page text and the text node it acts on, or nil where ExactKit shows
    /// no text menu (no selection under the pointer, a field, a node that wrote its own `contextmenu`, another window's view).
    static func replacement(for event: NSEvent) -> (NSMenu, NSView)? {
        guard isSecondaryClick(event), let window = event.window, let content = window.contentView else { return nil }
        let point = content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
        guard let view = content.hitTest(point), let hostMenu = view.menu(for: event), isHostTextMenu(hostMenu) else { return nil }
        return (pageMenu(view, event, selected: true), view)
    }

    /// Whether `menu` is ExactKit's for selected page text: an item of it (a submenu's included) aims at the text view
    /// ExactKit asked for AppKit's menu (`hostTextMenuView`). Under the agent ExactKit gives none (its tracking would hold
    /// the agent's click), so there the click goes on to the tail's page menu, Copy enabled by the selection.
    static func isHostTextMenu(_ menu: NSMenu) -> Bool {
        menu.items.contains { item in
            item.target.map { String(describing: type(of: $0)) == hostTextMenuView } == true || item.submenu.map(isHostTextMenu) == true
        }
    }

    // MARK: The page

    /// The shell's menu for a right-click on the page at `hit`: read-only, so Cut and Paste are disabled; Copy follows the
    /// page's selection; Copy Link and Copy Image follow what is under the pointer.
    static func pageMenu(_ hit: NSView, _ event: NSEvent, selected: Bool) -> NSMenu {
        var params = T3ShellMenuParams()
        params.link = link(at: event, in: hit)
        let image = image(at: event, in: hit)
        params.image = image != nil
        params.canCopy = selected
        let target: AnyObject? = hit.responds(to: #selector(NSText.copy(_:))) ? hit : page(hit)
        return T3ShellMenu.menu(params, T3ShellMenuActions(editTarget: target, image: image))
    }

    /// A click no page node answered (`T3ShellMenuTail`): the shell's menu for the page. It answers the last page click,
    /// which `shellMenu` then leaves alone.
    func pageClick(_ event: NSEvent) -> Bool {
        lastPageClick = nil
        guard let window = event.window, let content = window.contentView else { return false }
        let point = content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
        guard let hit = content.hitTest(point), let page = Self.page(hit) else { return false }
        show(Self.pageMenu(hit, event, selected: Self.selectionExists(page)), event, hit)
        return true
    }

    /// ExactKit's root view (`ExactView`) above `view`, if the click is on an Exact page.
    static func page(_ view: NSView) -> NSView? {
        ancestors(view).first { String(describing: type(of: $0)) == "ExactView" }
    }

    /// ExactKit's root view at or under `view` (a window's content view).
    static func pageView(in view: NSView) -> NSView? {
        if String(describing: type(of: view)) == "ExactView" { return view }
        for child in view.subviews { if let page = pageView(in: child) { return page } }
        return nil
    }

    /// Whether page text is selected: ExactView validates Start Speaking by exactly that.
    static func selectionExists(_ page: NSView) -> Bool {
        guard page.responds(to: startSpeaking), let validation = page as? NSMenuItemValidation else { return false }
        return validation.validateMenuItem(NSMenuItem(title: "", action: startSpeaking, keyEquivalent: ""))
    }

    /// The link under the pointer: an inline link run's accessibility element (ExactKit gives it the run's `href` as its
    /// URL), else a link node's own URL.
    static func link(at event: NSEvent, in hit: NSView) -> String? {
        guard let window = hit.window else { return nil }
        let screen = window.convertPoint(toScreen: event.locationInWindow)
        for child in hit.accessibilityChildren() ?? [] {
            guard let element = (child as AnyObject) as? NSAccessibilityProtocol,
                  element.accessibilityRole() == .link, element.accessibilityFrame().contains(screen),
                  let url = element.accessibilityURL() else { continue }
            return url.absoluteString
        }
        for view in ancestors(hit).prefix(while: { String(describing: type(of: $0)) != "ExactView" }) where view.accessibilityRole() == .link {
            if let url = view.accessibilityURL() { return url.absoluteString }
        }
        return nil
    }

    /// The image under the pointer: an app view that draws one (`T3ShellImageView`), or a layer of the node that holds a
    /// bitmap (ExactKit's image layer); and that bitmap.
    static func image(at event: NSEvent, in hit: NSView) -> CGImage? {
        if let image = drawnImage(at: event.locationInWindow, in: hit) { return image }
        let point = hit.convert(event.locationInWindow, from: nil)
        guard let layer = hit.layer else { return nil }
        for candidate in [layer] + (layer.sublayers ?? []) {
            guard let contents = candidate.contents, CFGetTypeID(contents as CFTypeRef) == CGImage.typeID else { continue }
            let frame = candidate === layer ? hit.bounds : candidate.frame
            if frame.contains(point) { return (contents as! CGImage) }
        }
        return nil
    }

    /// An app-drawn image under `location` (window coordinates) anywhere below the node that took the click, topmost first:
    /// the image lets presses through, and so may the node that holds it. The work group's tool icon is two levels down
    /// (ToolActivityIcon's box › its `pointer-events="none"` hook box › `T3ToolActivityIcon`'s view), so the click is the
    /// outer box's (realinput-1010g RG-1); Chromium's hit test for Copy Image passes `pointer-events: none` the same way.
    static func drawnImage(at location: NSPoint, in view: NSView) -> CGImage? {
        for child in view.subviews.reversed() where !child.isHidden && child.frame.contains(view.convert(location, from: nil)) {
            if let drawn = child as? T3ShellImageView, let image = drawn.shellImage { return image }
            if let image = drawnImage(at: location, in: child) { return image }
        }
        return nil
    }

    private func attachTail(to window: NSWindow) {
        guard let content = window.contentView else { return }
        let key = ObjectIdentifier(window)
        let tail: T3ShellMenuTail
        if let existing = tails[key], existing.window === window { tail = existing } else {
            tail = T3ShellMenuTail(owner: self, window: window)
            tails[key] = tail
        }
        tail.attach(after: content)
    }

    // MARK: Text inputs

    /// The text input under the pointer: an editable or selectable text view (a textarea, the composer, a field being
    /// edited) or text field; a text view in a scroll view answers for the scroller's empty part.
    static func textInput(_ hit: NSView) -> NSView? {
        for view in ancestors(hit).prefix(4) {
            if let text = view as? NSTextView, text.isEditable || text.isSelectable { return text }
            if let field = view as? NSTextField, field.isEditable || field.isSelectable { return field }
            if let scroll = view as? NSScrollView, let text = scroll.documentView as? NSTextView, text.isEditable { return text }
            if let clip = view as? NSClipView, let text = clip.documentView as? NSTextView, text.isEditable { return text }
        }
        return nil
    }

    /// Focuses the input (Chromium's mousedown does, of any button), selects what the click names and builds the input's
    /// menu; the text view it acts on is the field's editor while a field is edited.
    func textMenu(_ input: NSView, _ event: NSEvent) -> (NSMenu, NSView)? {
        guard let window = input.window else { return nil }
        var focused = false
        let text: NSTextView
        if let field = input as? NSTextField {
            if field.currentEditor() == nil { focused = window.makeFirstResponder(field) }
            guard let editor = field.currentEditor() as? NSTextView else { return nil }
            text = editor
        } else if let view = input as? NSTextView {
            if window.firstResponder !== view, view.acceptsFirstResponder { focused = window.makeFirstResponder(view) }
            text = view
        } else { return nil }
        if text.hasMarkedText() { text.unmarkText() }
        let field = text.isFieldEditor ? text.delegate as? NSTextField : nil
        let params = Self.prepare(text, at: text.convert(event.locationInWindow, from: nil), focused: focused,
                                  rich: richEditors().contains { $0 === text }, secure: field is NSSecureTextField, spelling: spelling)
        let misspelling = text.selectedRange()
        let actions = T3ShellMenuActions(editTarget: text, replace: { [weak text] suggestion in
            // `replaceMisspelling`: the misspelled word the menu was opened on, through the input's own editing path.
            guard let text, NSMaxRange(misspelling) <= (text.string as NSString).length else { return }
            text.insertText(suggestion, replacementRange: misspelling)
        })
        return (T3ShellMenu.menu(params, actions), text)
    }

    /// Selects the word or misspelling under `point` (unless it is inside the selection; a click the focus just moved to
    /// starts from no selection) and reads the input's edit flags.
    static func prepare(_ text: NSTextView, at point: NSPoint, focused: Bool, rich: Bool, secure: Bool, spelling: T3ShellSpelling) -> T3ShellMenuParams {
        let string = text.string as NSString
        var selection = text.selectedRange()
        if focused { selection = NSRange(location: NSNotFound, length: 0) }
        if let index = characterIndex(text, at: point) {
            if !(selection.length > 0 && NSLocationInRange(index, selection)) {
                let word = text.selectionRange(forProposedRange: NSRange(location: index, length: 0), granularity: .selectByWord)
                selection = word.length > 0 ? word : NSRange(location: index, length: 0)
                text.setSelectedRange(selection)
            }
        } else if focused {
            selection = NSRange(location: min(text.characterIndexForInsertion(at: point), string.length), length: 0)
            text.setSelectedRange(selection)
        }
        var params = T3ShellMenuParams()
        let selected = selection.location != NSNotFound && selection.length > 0
        if selected, text.isEditable, text.isContinuousSpellCheckingEnabled, !secure {
            let word = string.substring(with: selection)
            if spelling.misspelled(word, text) {
                params.misspelled = true
                params.suggestions = spelling.guesses(word, text)
            }
        }
        params.canCut = text.isEditable && selected && !secure
        params.canCopy = selected && !secure
        params.canPaste = text.isEditable && T3ShellMenu.clipboardHasContent(readable: text.readablePasteboardTypes)
        params.canSelectAll = rich || string.length > 0
        return params
    }

    /// The character whose glyph is under `point` (the view's coordinates), or nil past the text's end or between lines.
    /// NSTextInputClient's rects, which a TextKit 2 view answers without being asked for its layout manager.
    static func characterIndex(_ text: NSTextView, at point: NSPoint) -> Int? {
        let length = (text.string as NSString).length
        guard length > 0, let window = text.window else { return nil }
        let insertion = text.characterIndexForInsertion(at: point)
        guard insertion != NSNotFound else { return nil }
        let screen = window.convertPoint(toScreen: text.convert(point, to: nil))
        for index in [insertion, insertion - 1] where index >= 0 && index < length {
            var actual = NSRange()
            let rect = text.firstRect(forCharacterRange: NSRange(location: index, length: 1), actualRange: &actual)
            if rect.width > 0, rect.insetBy(dx: -0.5, dy: -0.5).contains(screen) { return index }
        }
        return nil
    }

    // MARK: Showing

    private func show(_ menu: NSMenu, _ event: NSEvent, _ view: NSView) {
        lastShown = T3ShellMenu.describe(menu)
        if agent {
            FileHandle.standardError.write(Data("t3.textmenu: \(lastShown.joined(separator: " | "))\n".utf8))
            return
        }
        present(menu, event, view)
    }

    static func describe(_ menu: NSMenu) -> [String] { T3ShellMenu.describe(menu) }

    static func ancestors(_ view: NSView) -> [NSView] { Array(sequence(first: view, next: { $0.superview })) }
}

/// The responder after the window's content view: a right-click no page node answered reaches it on its way to the
/// window (NSView passes an unanswered `rightMouseDown` to its next responder; NSWindow drops it), and the shell's menu
/// opens; so does a Control-click on a node that is not pressable (ExactKit takes a Control-click as a right-click only where
/// a node wrote `contextmenu`, main #487, and passes such a node's `mouseDown` up the chain). AppKit does not retain a next responder, so the menu keeps it, and puts it back if the content view's chain was
/// reset.
final class T3ShellMenuTail: NSResponder {
    private weak var owner: T3TextContextMenu?
    private(set) weak var window: NSWindow?
    private weak var content: NSView?

    init(owner: T3TextContextMenu, window: NSWindow) {
        self.owner = owner
        self.window = window
        super.init()
    }
    required init?(coder: NSCoder) { nil }

    func attach(after content: NSView) {
        guard content.nextResponder !== self else { return }
        nextResponder = content.nextResponder
        content.nextResponder = self
        self.content = content
    }

    func detach() {
        if let content, content.nextResponder === self { content.nextResponder = nextResponder }
        content = nil
    }

    override func rightMouseDown(with event: NSEvent) {
        if owner?.pageClick(event) != true { super.rightMouseDown(with: event) }
    }

    override func mouseDown(with event: NSEvent) {
        if !(T3TextContextMenu.isSecondaryClick(event) && owner?.pageClick(event) == true) { super.mouseDown(with: event) }
    }
}
#endif
