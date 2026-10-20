#if os(macOS)
import AppKit

/// Retains OS focus/paste routing and toolbar geometry around the shared field.
/// Contract owns Return and formatting; an open command menu's keys press its
/// hidden key buttons (T3ComposerEditor.menuCommand).
final class T3Composer {
    /// The surface launcher (hatch `t3-launcher`): its letters are its rows' `aria-keyshortcuts`, which Exact's key
    /// route presses (shell-panels.contract SurfaceRow), so type-to-focus leaves them alone (`launcherLetter`).
    private weak var launcher: ExactElement?
    private let changed: (String) -> Void
    private weak var composer: ExactElement?
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
    /// OS paste and drop facts around the shared field.
    let editor: T3ComposerEditor

    init(changed: @escaping (String) -> Void = { _ in }) {
        self.changed = changed
        editor = T3ComposerEditor(changed: changed)
    }

    var status: [String: Any] { ["anchors": anchorFrames] }

    func install(_ element: ExactElement) {
        editor.install(element)
        if element.hatch == .t3Anchor {
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
        } else if element.hatch == .t3Launcher {
            launcher = element
        } else if element.hatch == .t3Composer {
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
        if launcher === element { launcher = nil }
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
        launcher = nil
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
        if queuedEditKey(event) { return nil } // ⌥↑ with the caret at the start (T3ComposerQueueKey.swift)
        if let routed = editorKey(event) { return routed.event }
        return event
    }

    // MARK: Keys the text view never sees (ChatView.tsx type-to-focus)

    struct Routed { let event: NSEvent? }

    /// ⇧⌘V is Paste as Text (T3ComposerPasteHook); a printable key or ⌘V with nothing editable
    /// focused goes to the composer, which takes focus. Nil leaves the event to
    /// the others; `Routed` holds what goes on (nil: the key is taken).
    func editorKey(_ event: NSEvent) -> Routed? {
        guard event.type == .keyDown, let composer, composer.isLive, let view = composer.textView, view.isEditable,
              let window = view.window, event.window === window, window.attachedSheet == nil,
              view.isHiddenOrHasHiddenAncestor == false else { return nil }
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        if flags == [.command, .shift], key == "v", window.firstResponder === view {
            view.pasteAsPlainText(nil)
            return Routed(event: nil)
        }
        // A launcher letter goes on to Exact's key route (never type-to-focus), where its
        // row's shortcut takes it (AppKit runs local monitors in no fixed order).
        if let launcher, let shown = launcher.view, Self.launcherLetter(event, letters: launcher.data[.surfaceLauncherKeys] ?? "",
                                                                          live: launcher.isLive && shown.window === window && !shown.isHiddenOrHasHiddenAncestor) {
            return Routed(event: event)
        }
        guard T3Composer.redirectable(window.firstResponder, in: window), T3Composer.interactive(composer) else { return nil }
        if flags == .command, key == "v" {
            guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else { return nil }
            window.makeFirstResponder(view)
            view.paste(nil)
            return Routed(event: nil)
        }
        guard flags.subtracting(.shift).isEmpty, let characters = event.characters, !characters.isEmpty,
              characters.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) && ($0.value < 0xF700 || $0.value > 0xF8FF) }) else { return nil }
        // The key itself goes on to the prompt, now the first responder, rather than its characters: an input method
        // composes it there. Inserted as text, the first jamo of Korean 2-Set's 한 stood alone and the input method
        // went on from the next key (ㅎㅏㄴ).
        window.makeFirstResponder(view)
        return Routed(event: event)
    }

    /// A plain letter (no ⌘, ⌃ or ⌥) the live launcher lists (`data-surface-launcher-keys`, its available rows' letters).
    static func launcherLetter(_ event: NSEvent, letters: String, live: Bool) -> Bool {
        guard live, event.type == .keyDown, event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let typed = event.charactersIgnoringModifiers, typed.count == 1 else { return false }
        // A key that types no ASCII letter (Korean 2-Set's ㄹ) is its physical key's, as the shortcut matches it (ShortcutsMac).
        let key = typed.unicodeScalars.allSatisfy(\.isASCII) ? typed : (T3LatinChords.physicalKeys[event.keyCode] ?? typed)
        return letters.uppercased().contains(key.uppercased())
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
        case "editorSync": value = editor.sync(request)
        case "editorForgetFold": editor.forgetFold(request["id"] as? Int ?? -1)
        case "editorPoke":
            let delay = max(0, min(2000, request["afterMs"] as? Int ?? 0))
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(delay)) { [weak self] in self?.changed("t3.editor") }
        default: return ["ok": false, "generation": generation, "error": ["kind": "Editor", "message": "Unknown editor request.", "uncertain": false]]
        }
        return ["ok": true, "generation": generation, "value": value]
    }

    deinit { destroy() }
}

/// Empty-editor Select All needs a weak registry, not a second editing owner.
final class T3ComposerPreview {
    private weak var element: ExactElement?
    var textView: NSTextView? { element?.isLive == true ? element?.textView : nil }
    func install(_ element: ExactElement) { if element.hatch == .t3PromptPreview { self.element = element } }
    func remove(_ element: ExactElement) { if self.element === element { self.element = nil } }
    func destroy() { element = nil }
}
#endif
