// The desktop shell's context menu in the window (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/window/DesktopWindow.ts `installContextMenu`; the template is T3ShellMenu.swift). Electron raises
// `context-menu` for every right-click the page did not take, and the shell answers it. Contract has no window-wide
// context-menu hook, so this module finds the same clicks itself (realinput-1010d RD-4, shell-context-menu):
//
// - A text input (an `<input>`'s field, a `<textarea>`, the composer): AppKit's own editing menu would open, so a local
//   right-click monitor takes the click, focuses the input as Chromium's mousedown does, selects the word (or the
//   misspelling) under the pointer unless the click is inside the selection, and pops the shell's menu for the input:
//   suggestions from NSSpellChecker where the input checks spelling, Cut, Copy, Paste and Select All by its state.
// - Selected page text: ExactKit would show the menu an NSTextView shows for read-only text (Look Up, Copy, Speech,
//   Services; its menu carries the Look Up action), so the monitor takes that click too and pops the shell's on the same
//   text node: Copy (the node's `copy:`, its `copy` event first) and Select All (the node's `selectAll:`).
// - Anywhere else on the page: the click goes on to ExactKit. A node with a `contextmenu` (or a context popover) answers
//   it and ends it there, as `preventDefault` keeps Electron's event from firing; a click no node answers is passed up
//   the responder chain to the window, and a responder placed after the window's content view (`T3ShellMenuTail`) pops
//   the shell's menu for the page: Copy Link over a link (the inline link's accessibility URL), Copy Image over an image
//   (the image layer's bitmap), Copy while page text is selected (ExactKit enables Edit ▸ Speech ▸ Start Speaking exactly
//   then), Select All.
// - A Browser page answers in its web view (T3ShellWebView.swift); other web views (the terminal) keep their own menus.
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
    /// ExactKit's read-only text menu's Look Up item (TextSelectionMac.swift `lookUpSelection(_:)`).
    static let hostTextMenuAction = NSSelectorFromString("lookUpSelection:")
    /// ExactView's Edit ▸ Speech ▸ Start Speaking, which it enables exactly while page text is selected.
    static let startSpeaking = NSSelectorFromString("startSpeaking:")

    private let agent: Bool
    private var monitor: Any?
    private var tails: [ObjectIdentifier: T3ShellMenuTail] = [:]
    /// What the last answered click showed (the agent's log line, and the tests).
    private(set) var lastShown: [String] = []
    /// Pops the shell's menu (tracks until the menu closes); a test records it instead. Without context-menu plug-ins:
    /// `popUpContextMenu` appends Services for a view that answers `validRequestor`, which Electron's menu has not
    /// (realinput-1010f RF-1).
    var present: (NSMenu, NSEvent, NSView) -> Void = { menu, event, view in
        menu.allowsContextMenuPlugIns = false
        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }
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
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in
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
        guard event.type == .rightMouseDown, let window = event.window, let content = window.contentView else { return event }
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
        if Self.page(hit) != nil { attachTail(to: window) }
        return event
    }

    // MARK: Selected page text (RD-4)

    /// The shell's menu for a right-click on selected page text and the text node it acts on, or nil where ExactKit shows
    /// no text menu (no selection under the pointer, a field, a node that wrote its own `contextmenu`, another window's view).
    static func replacement(for event: NSEvent) -> (NSMenu, NSView)? {
        guard event.type == .rightMouseDown, let window = event.window, let content = window.contentView else { return nil }
        let point = content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow
        guard let view = content.hitTest(point), let hostMenu = view.menu(for: event),
              hostMenu.items.contains(where: { $0.action == hostTextMenuAction }) else { return nil }
        return (pageMenu(view, event, selected: true), view)
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

    /// A click no page node answered (`T3ShellMenuTail`): the shell's menu for the page.
    func pageClick(_ event: NSEvent) -> Bool {
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
        let point = hit.convert(event.locationInWindow, from: nil)
        for case let drawn as T3ShellImageView in hit.subviews where !drawn.isHidden && drawn.frame.contains(point) {
            if let image = drawn.shellImage { return image }
        }
        guard let layer = hit.layer else { return nil }
        for candidate in [layer] + (layer.sublayers ?? []) {
            guard let contents = candidate.contents, CFGetTypeID(contents as CFTypeRef) == CGImage.typeID else { continue }
            let frame = candidate === layer ? hit.bounds : candidate.frame
            if frame.contains(point) { return (contents as! CGImage) }
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
/// opens. AppKit does not retain a next responder, so the menu keeps it, and puts it back if the content view's chain was
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
}
#endif
