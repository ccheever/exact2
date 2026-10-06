#if os(macOS)
import AppKit

/// Adds chat Return semantics to Exact's ordinary textarea and reports where
/// the composer's menu triggers sit. Exact keeps its text view, delegate,
/// editing state and authored Send action; Contract owns the menus, the
/// model picker's keyboard highlight and their dismissal.
final class T3Composer {
    var sendShortcut = "enter"
    /// Lane r8-keys: the surface launcher's letters come before type-to-focus (R8KeysLauncher.swift).
    var launcher: R8KeysLauncher?
    private let changed: (String) -> Void
    private weak var composer: ExactElement?
    private weak var send: ExactElement?
    private final class Anchor {
        weak var element: ExactElement?
        var observer: NSObjectProtocol?
        init(_ element: ExactElement) { self.element = element }
    }
    private var anchors: [String: Anchor] = [:]
    /// Each trigger's [x, width] in its toolbar row, in points (T3's menus
    /// open above the trigger, start-aligned, so the view needs both edges).
    private(set) var anchorFrames: [String: [Double]] = [:]
    private var keyMonitor: Any?
    /// The prompt editor's behaviours (T3ComposerEditor.swift).
    let editor: T3ComposerEditor

    init(changed: @escaping (String) -> Void = { _ in }) {
        self.changed = changed
        editor = T3ComposerEditor(changed: changed)
    }

    var status: [String: Any] { ["anchors": anchorFrames] }

    func install(_ element: ExactElement) {
        editor.install(element)
        if element.hook == .t3Anchor {
            let name = element.data[.anchor] ?? ""
            guard !name.isEmpty else { return }
            if anchors[name]?.element !== element {
                if let old = anchors[name]?.observer { NotificationCenter.default.removeObserver(old) }
                let anchor = Anchor(element)
                if let view = element.view {
                    view.postsFrameChangedNotifications = true
                    anchor.observer = NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification,
                        object: view, queue: .main) { [weak self] _ in self?.measure() }
                }
                anchors[name] = anchor
            }
            measure()
        } else if element.hook == .t3Send {
            send = element
        } else if element.hook == .t3Composer {
            composer = element
            if keyMonitor == nil {
                keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                    guard let self else { return event }
                    return self.handle(event)
                }
            }
        }
    }

    func remove(_ element: ExactElement) {
        editor.remove(element)
        let ended = anchors.filter { $0.value.element == nil || $0.value.element === element }
        for (name, anchor) in ended {
            if let observer = anchor.observer { NotificationCenter.default.removeObserver(observer) }
            anchors[name] = nil
        }
        if !ended.isEmpty { measure() }
        if send === element { send = nil }
        if composer === element {
            composer = nil
            stopMonitoring()
        }
    }

    func destroy() {
        editor.destroy()
        stopMonitoring()
        for anchor in anchors.values { if let observer = anchor.observer { NotificationCenter.default.removeObserver(observer) } }
        anchors.removeAll()
        composer = nil
        send = nil
    }

    /// Internal so tests can drive it after moving a trigger.
    func measure() {
        var next: [String: [Double]] = [:]
        for (name, anchor) in anchors {
            guard let element = anchor.element, element.isLive, let view = element.view, let row = view.superview else { continue }
            let box = view.convert(view.bounds, to: row)
            next[name] = [(Double(box.minX) * 2).rounded() / 2, (Double(box.width) * 2).rounded() / 2]
        }
        guard next != anchorFrames else { return }
        anchorFrames = next
        changed("t3.status")
    }

    private func stopMonitoring() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
    }

    /// Internal so focused tests can exercise the actual window/focus guard.
    func handle(_ event: NSEvent) -> NSEvent? {
        if queuedEditKey(event) { return nil } // thread-commands-and-keys: ⌥↑ with the caret at the start (T3ComposerQueueKey.swift)
        if let routed = editorKey(event) { return routed.event }
        switch action(for: event) {
        case .passThrough: return event
        case .send:
            // click() queues the Contract action after any pending input batch;
            // its existing disabled state and canSend validation remain in charge.
            send?.click()
            return nil
        case .consumeRepeat: return nil
        }
    }

    private func action(for event: NSEvent) -> ReturnAction {
        // An open command menu takes Return (its Enter key button) through
        // the text view's own command path.
        if self.editor.trigger != nil, self.editor.key("Enter") != nil { return .passThrough }
        if self.editor.key("stash-Enter") != nil { return .passThrough }
        guard let composer, composer.isLive, let editor = composer.textView,
              let window = editor.window, event.window === window,
              window.firstResponder === editor, window.attachedSheet == nil,
              editor.isEditable, let send, send.isLive, send.view?.window === window else { return .passThrough }
        return Self.returnAction(event, hasMarkedText: editor.hasMarkedText(), shortcut: sendShortcut, prompt: editor.string)
    }

    enum ReturnAction { case passThrough, send, consumeRepeat }

    static func returnAction(_ event: NSEvent, hasMarkedText: Bool, shortcut: String = "enter", prompt: String = "") -> ReturnAction {
        guard event.type == .keyDown, event.keyCode == 36 || event.keyCode == 76,
              !hasMarkedText else {
            return .passThrough
        }
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        let requiresCommand = shortcut == "mod-enter" || (shortcut == "mod-enter-multiline" && prompt.contains(where: { $0 == "\n" || $0 == "\r" }))
        guard flags == (requiresCommand ? .command : []) else { return .passThrough }
        // Holding Return must neither dispatch twice nor insert a newline after
        // the first send. Return used to commit IME text always reaches AppKit.
        return event.isARepeat ? .consumeRepeat : .send
    }

    // MARK: Keys the text view never sees (ChatView.tsx type-to-focus)

    struct Routed { let event: NSEvent? }

    /// ⇧⌘V pastes inline; a printable key or ⌘V with nothing editable
    /// focused goes to the composer, which takes focus. Nil passes the event on.
    func editorKey(_ event: NSEvent) -> Routed? {
        guard event.type == .keyDown, let composer, composer.isLive, let view = composer.textView, view.isEditable,
              let window = view.window, event.window === window, window.attachedSheet == nil,
              view.isHiddenOrHasHiddenAncestor == false else { return nil }
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        // ⌘B / ⌘I toggle bold and italic when the rich text composer is on (lane r9-input: by key code
        // under a non-Latin source, ending composition first, R9Input.swift).
        let chord = R9Input.chordKey(event)
        if flags == .command, chord == "b" || chord == "i", window.firstResponder === view, editor.styler.richText, R9Input.commitMarkedText(view) {
            editor.toggleMark(chord == "b" ? "**" : "*", in: view)
            return Routed(event: nil)
        }
        if flags == [.command, .shift], key == "v", window.firstResponder === view {
            editor.pastingAsText = true
            view.pasteAsPlainText(nil)
            editor.pastingAsText = false
            return Routed(event: nil)
        }
        if let launcher, launcher.consume(event, typing: window.firstResponder is NSText || window.firstResponder is NSTextField) { return Routed(event: nil) }
        guard T3Composer.redirectable(window.firstResponder, in: window), T3Composer.interactive(composer) else { return nil }
        if flags == .command, key == "v" {
            guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else { return nil }
            window.makeFirstResponder(view)
            view.setSelectedRange(NSRange(location: (view.string as NSString).length, length: 0))
            view.paste(nil)
            return Routed(event: nil)
        }
        guard flags.subtracting(.shift).isEmpty, let characters = event.characters, !characters.isEmpty,
              characters.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) && ($0.value < 0xF700 || $0.value > 0xF8FF) }) else { return nil }
        window.makeFirstResponder(view)
        view.setSelectedRange(NSRange(location: (view.string as NSString).length, length: 0))
        view.insertText(characters, replacementRange: view.selectedRange())
        return Routed(event: nil)
    }

    /// shouldRedirectInputToComposer: nothing editable or interactive holds
    /// the keyboard (a bare window, Exact's root view or an inert surface).
    static func redirectable(_ responder: NSResponder?, in window: NSWindow) -> Bool {
        guard let responder, responder !== window, responder !== window.contentView else { return true }
        // WKWebView (the terminal's native responder) is an NSTextInputClient even though
        // its accessibility container is AXGroup. It already owns printable keys and paste.
        if responder is NSTextInputClient || responder is NSText || responder is NSTextField || responder is NSControl { return false }
        guard let view = responder as? NSView else { return false }
        let blocked: [NSAccessibility.Role] = [.button, .checkBox, .radioButton, .menuItem, .menuButton, .popUpButton, .link,
            .textField, .textArea, .comboBox, .slider, .disclosureTriangle, .tabGroup]
        return !blocked.contains(view.accessibilityRole() ?? .unknown)
    }

    /// Exact's own hit test refuses an inert or hidden node: an open dialog or
    /// settings page leaves the composer behind it untouched.
    static func interactive(_ element: ExactElement) -> Bool {
        guard let node = element.view, node.superview != nil else { return false }
        return node.hitTest(NSPoint(x: node.frame.midX, y: node.frame.midY)) != nil
    }

    /// TypeScript's requests for the prompt editor (main thread).
    func perform(_ request: [String: Any]) -> [String: Any] {
        let generation = request["generation"] as? Int ?? 0
        var value: [String: Any] = [:]
        switch request["op"] as? String {
        case "editorState": value = editor.state; value["text"] = editor.textView?.string ?? ""
        case "editorSync":
            if let entries = request["history"] as? [[String: Any]] {
                editor.history = entries.compactMap { entry in
                    guard let id = entry["id"] as? String, let prompt = entry["prompt"] as? String else { return nil }
                    return T3ComposerText.HistoryEntry(id: id, prompt: prompt)
                }
            }
            if let rich = request["richText"] as? Bool { editor.styler.richText = rich }
            if let local = request["localEnvironment"] as? Bool { editor.localEnvironment = local }
            if let contexts = request["contexts"] as? [String: String] { editor.styler.contexts = contexts }
            if let limit = request["foldLimit"] as? Int { editor.foldLimit = limit }
            if let owner = request["owner"] as? String, owner != editor.historyOwner { editor.historyOwner = owner; editor.resetHistory() }
            value = editor.state
            value["notices"] = editor.takeNotices()
            value["folds"] = editor.folds
        case "editorEdit": value = editor.edit(request)
        case "editorInsert": value = editor.insert(request)
        case "editorPoke":
            let delay = max(0, min(2000, request["afterMs"] as? Int ?? 0))
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(delay)) { [weak self] in self?.changed("t3.editor") }
        case "editorFocus":
            if let view = editor.textView, let window = view.window {
                window.makeFirstResponder(view)
                if request["end"] as? Bool == true { view.setSelectedRange(NSRange(location: (view.string as NSString).length, length: 0)) }
            }
        default: return ["ok": false, "generation": generation, "error": ["kind": "Editor", "message": "Unknown editor request.", "uncertain": false]]
        }
        return ["ok": true, "generation": generation, "value": value]
    }

    deinit { destroy() }
}
#endif
