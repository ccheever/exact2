#if os(macOS)
import AppKit

/// Intercepts large text pastes and the open command menus' keys. Exact
/// remains the text/selection delegate and every other command's; its shared
/// field owns every source edit.
final class T3ComposerTextDelegate: NSObject, NSTextViewDelegate {
    weak var inner: NSObject?
    weak var editor: T3ComposerEditor?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || inner?.responds(to: selector) == true
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        inner?.responds(to: selector) == true ? inner : super.forwardingTarget(for: selector)
    }
    func textView(_ textView: NSTextView, shouldChangeTextIn range: NSRange, replacementString text: String?) -> Bool {
        if editor?.shouldChange(textView, range: range, text: text) == false { return false }
        return (inner as? NSTextViewDelegate)?.textView?(textView, shouldChangeTextIn: range, replacementString: text) ?? true
    }
    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if editor?.menuCommand(selector, in: textView) == true { return true }
        return (inner as? NSTextViewDelegate)?.textView?(textView, doCommandBy: selector) ?? false
    }
}

/// Observes the mounted composer and holds large pastes until the shared field
/// accepts or refuses them. It never edits text, selection, styles or undo.
/// A paste's files and a drop are Contract's (`paste`, `drop`), staged in
/// TypeScript (composer-controls-attach.ts).
final class T3ComposerEditor {
    private let changed: (String) -> Void
    private(set) weak var textView: NSTextView?
    private weak var element: ExactElement?
    private let proxy = T3ComposerTextDelegate()
    private final class Key { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var keys: [String: Key] = [:]
    private struct Owner: Equatable {
        let snapshot: String
        let field: String
        var valid: Bool { !snapshot.isEmpty && !field.isEmpty }
    }
    private var owner = Owner(snapshot: "", field: "")
    static let maxRecordID = 9_007_199_254_740_991
    static func nextRecordID(after current: Int) -> Int? {
        current >= 0 && current < maxRecordID ? current + 1 : nil
    }
    private var recordID = 0
    private(set) var folds: [[String: Any]] = []
    private(set) var notices: [[String: String]] = []
    private(set) var foldLimit = 0
    var pastingAsText = false
    var pasteDetector: (String) -> Bool = T3ComposerEditor.isPaste
    /// Where Paste as Text's paste waits for its key's press (the tests run it themselves).
    var nextTurn: (@escaping () -> Void) -> Void = { DispatchQueue.main.async(execute: $0) }

    init(changed: @escaping (String) -> Void) {
        self.changed = changed
        proxy.editor = self
    }

    func install(_ element: ExactElement) {
        if element.hatch == .t3ComposerKey {
            let name = element.data[.anchor] ?? ""
            if !name.isEmpty { keys[name] = Key(element) }
            return
        }
        guard element.hatch == .t3Composer else { return }
        if self.element !== element || textView !== element.textView {
            detach()
            self.element = element
            if let view = element.textView {
                textView = view
                proxy.inner = view.delegate as? NSObject
                view.delegate = proxy
            }
        }
        refreshOwner()
    }

    func remove(_ element: ExactElement) {
        for (name, key) in keys where key.element == nil || key.element === element { keys[name] = nil }
        if self.element === element { detach() }
    }
    func destroy() { detach(); keys.removeAll() }

    private func detach() {
        if let view = textView {
            if view.delegate === proxy { view.delegate = proxy.inner as? NSTextViewDelegate }
        }
        proxy.inner = nil
        textView = nil
        element = nil
        resetOwner(Owner(snapshot: "", field: ""))
    }

    func key(_ name: String) -> ExactElement? {
        guard let element = keys[name]?.element, element.isLive else { return nil }
        return element
    }

    /// An open command or stash menu's keys (ChatComposer onComposerCommandKey, ComposerStashMenu):
    /// its hidden key buttons are mounted only while it is open, and take ↑, ↓,
    /// Return, Tab (the highlighted item) and Escape before the field's own command.
    func menuCommand(_ selector: Selector, in view: NSTextView) -> Bool {
        guard view === textView, view.isEditable, !view.hasMarkedText() else { return false }
        if let step = Self.arrow(selector, NSApp.currentEvent) {
            // ⌥↓ is two commands (moveForward:, moveToEndOfParagraph:): its first press holds the second.
            if let event = NSApp.currentEvent, event === arrowEvent { return true }
            let pressed = step > 0 ? press("ArrowDown") || press("stash-ArrowDown") : press("ArrowUp") || press("stash-ArrowUp")
            arrowEvent = pressed ? NSApp.currentEvent : nil
            return pressed
        }
        switch selector {
        case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)):
            return press("Enter") || press("stash-Enter")
        case #selector(NSResponder.insertTab(_:)): return press("Enter")
        case #selector(NSResponder.cancelOperation(_:)): return press("Escape") || press("stash-Escape")
        case #selector(NSResponder.deleteToBeginningOfLine(_:)): return press("stash-Delete")
        default: return false
        }
    }
    private weak var arrowEvent: NSEvent?
    /// The menus read `event.key` (onComposerCommandKey, ComposerStashMenu), so ↑ and ↓ move the highlight with
    /// any modifier held, ⇧ included, while ⌃N and ⌃P, which AppKit binds to moveDown: and moveUp: too, move the
    /// caret. With no key event (a command sent directly), the selector alone decides.
    static func arrow(_ selector: Selector, _ event: NSEvent?) -> Int? {
        if let event, event.type == .keyDown {
            switch event.charactersIgnoringModifiers?.unicodeScalars.first.map({ Int($0.value) }) {
            case NSDownArrowFunctionKey: return 1
            case NSUpArrowFunctionKey: return -1
            default: return nil
            }
        }
        switch selector {
        case #selector(NSResponder.moveDown(_:)): return 1
        case #selector(NSResponder.moveUp(_:)): return -1
        default: return nil
        }
    }
    private func press(_ name: String) -> Bool {
        guard let key = key(name) else { return false }
        key.click()
        return true
    }

    private func refreshOwner() {
        let mounted = element.flatMap { $0.isLive ? $0 : nil }
        let next = Owner(snapshot: mounted?.data[.snapshotOwner] ?? "", field: mounted?.data[.owner] ?? "")
        if next != owner { resetOwner(next) }
    }
    private func resetOwner(_ next: Owner) {
        folds.removeAll(); notices.removeAll()
        foldLimit = 0
        owner = next
    }
    private func matches(_ request: [String: Any]) -> Bool {
        owner.valid && request["snapshotOwner"] as? String == owner.snapshot && request["fieldOwner"] as? String == owner.field
    }

    func sync(_ request: [String: Any]) -> [String: Any] {
        refreshOwner()
        if matches(request), let limit = request["foldLimit"] as? Int { foldLimit = max(0, limit) }
        let result: [String: Any] = ["snapshotOwner": owner.snapshot, "fieldOwner": owner.field,
            "focused": textView.map { $0.window?.firstResponder === $0 } ?? false,
            "composing": textView?.hasMarkedText() ?? false,
            "notices": notices, "folds": folds]
        if matches(request) { notices.removeAll() }
        return result
    }

    func forgetFold(_ id: Int) {
        refreshOwner()
        folds.removeAll { $0["id"] as? Int == id }
    }

    private func refuse(_ title: String, _ description: String) {
        notices.append(["kind": "error", "title": title, "description": description])
        changed("t3.editor")
    }

    /// The observed full source and UTF-16 range are evidence for a later
    /// guarded shared-field edit, never authority to mutate a replacement field.
    private func record(_ view: NSTextView, range: NSRange) -> [String: Any]? {
        refreshOwner()
        let value = view.string as NSString
        guard view === textView, owner.valid, range.location >= 0, range.length >= 0,
              range.location <= value.length, range.length <= value.length - range.location else { return nil }
        guard folds.count < 4 else {
            refuse("Too many pending pastes", "Wait for the previous attachments, then try again.")
            return nil
        }
        guard let next = Self.nextRecordID(after: recordID) else {
            refuse("Attachment queue is unavailable", "Close and reopen this window before pasting more attachments.")
            return nil
        }
        recordID = next
        return ["id": recordID, "snapshotOwner": owner.snapshot, "fieldOwner": owner.field,
            "value": view.string, "start": range.location, "end": NSMaxRange(range), "expect": value.substring(with: range)]
    }

    func shouldChange(_ view: NSTextView, range: NSRange, text: String?) -> Bool {
        guard view === textView, let text, view.isEditable, !view.hasMarkedText() else { return true }
        if !pastingAsText, !text.isEmpty, let allowed = foldPaste(view, range: range, text: text) { return allowed }
        return true
    }

    static let promptLimit = 120_000
    static let foldThreshold = 32 * 1024

    func foldPaste(_ view: NSTextView, range: NSRange, text: String) -> Bool? {
        refreshOwner()
        let source = view.string as NSString
        guard range.location >= 0, range.length >= 0, range.location <= source.length,
              range.length <= source.length - range.location else { return nil }
        let length = (text as NSString).length, bytes = text.utf8.count
        let exceeds = source.length - range.length + length > Self.promptLimit
        let large = length >= Self.foldThreshold || bytes >= Self.foldThreshold
        guard large || exceeds, pasteDetector(text) else { return nil }
        if foldLimit <= 0 || bytes > foldLimit {
            guard exceeds else { return nil }
            let tooLarge = foldLimit > 0
            refuse(tooLarge ? "Pasted text is too large to attach" : "Pasted text is too large for this message",
                tooLarge ? "Reduce the clipboard contents or save a smaller excerpt as a file." : "Remove some text or an attachment, then paste again.")
            return false
        }
        guard var fact = record(view, range: range) else { return false }
        fact["text"] = text
        folds.append(fact)
        changed("t3.editor")
        return false
    }

    /// Paste as Text (⇧⌘V, Edit › Paste as Text; ChatComposer isPasteAsTextShortcut): the text view's own paste, so
    /// Exact's `paste` carries the pasteboard's files, after a press of the hidden `paste-as-text` key that tells the
    /// Contract this paste is Paste as Text (composer.contract `pasted`). AppKit's `pasteAsPlainText:` fires no `paste`.
    /// The key's press runs on the next turn (ExactElement.click), so the paste follows it there. Its text stays inline
    /// however large (`pastingAsText`).
    func pasteAsText(_ view: NSTextView) {
        key("paste-as-text")?.click()
        nextTurn { [weak self, weak view] in
            guard let self, let view, view === self.textView, view.isEditable, !view.hasMarkedText() else { return }
            let was = self.pastingAsText
            self.pastingAsText = true
            view.paste(nil)
            self.pastingAsText = was
        }
    }

    /// A pasteboard AppKit's plain text view will not paste from (its Paste is disabled: no type it reads), which still
    /// carries files for Exact's `paste`: image data alone, as a screenshot copied to the clipboard.
    static func pastesOnlyFiles(_ pasteboard: NSPasteboard) -> Bool {
        pasteboard.string(forType: .string) == nil && (pasteboard.data(forType: .png) != nil || pasteboard.data(forType: .tiff) != nil)
    }

    static func isPaste(_ text: String) -> Bool {
        guard let event = NSApp.currentEvent, event.type == .keyDown,
              event.modifierFlags.intersection([.command, .control, .option]) == .command,
              event.charactersIgnoringModifiers?.lowercased() == "v" else {
            return NSPasteboard.general.string(forType: .string) == text
        }
        return true
    }

    deinit { destroy() }
}
#endif
