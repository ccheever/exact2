#if os(macOS)
import AppKit

/// Stands in as the composer text view's delegate. Exact's node stays the
/// real delegate: every message this object does not answer is forwarded to
/// it, so its change, selection and editing notifications arrive unchanged.
/// Only `doCommandBy` (the key commands the editor's behaviours claim) and
/// the change hook (undo grouping, large pastes) are answered here first.
final class T3ComposerTextDelegate: NSObject, NSTextViewDelegate {
    weak var inner: NSObject?
    weak var editor: T3ComposerEditor?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || inner?.responds(to: selector) == true
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        inner?.responds(to: selector) == true ? inner : super.forwardingTarget(for: selector)
    }
    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if editor?.command(selector, in: textView) == true { return true }
        return (inner as? NSTextViewDelegate)?.textView?(textView, doCommandBy: selector) ?? false
    }
    func textView(_ textView: NSTextView, shouldChangeTextIn range: NSRange, replacementString text: String?) -> Bool {
        if editor?.shouldChange(textView, range: range, text: text) == false { return false }
        return (inner as? NSTextViewDelegate)?.textView?(textView, shouldChangeTextIn: range, replacementString: text) ?? true
    }
}

/// The prompt editor's behaviours on Exact's ordinary textarea (T3's
/// ChatComposer onComposerCommandKey, ComposerPromptEditorTiptap): trigger
/// detection for the command menus, the menus' keys, list continuation, Tab
/// indent, prompt-history recall, the plan toggle chord, atomic chips and
/// undo grouping. The Contract owns the menus' rows and highlight; this
/// reports the active trigger and presses the menu's hidden key buttons.
final class T3ComposerEditor {
    private let changed: (String) -> Void
    private(set) weak var textView: NSTextView?
    private weak var element: ExactElement?
    private let proxy = T3ComposerTextDelegate()
    private var observers: [NSObjectProtocol] = []
    private final class Key { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var keys: [String: Key] = [:]

    private(set) var trigger: T3ComposerTrigger?
    private var dismissed: T3ComposerTrigger?
    private(set) var owner = ""
    private var seq = 0
    private var reported = ""

    var history: [T3ComposerText.HistoryEntry] = []
    /// The thread whose sent prompts `history` holds.
    var historyOwner = ""
    private var historyPosition: T3ComposerText.HistoryPosition?
    let styler = T3ComposerStyler()

    // Undo grouping (composer-undo-grouping.ts): a new step after 1 s idle
    // and whenever typing switches between inserting and deleting.
    static let undoGroupDelay: TimeInterval = 1.0
    private var lastEditKind = ""
    private var lastEditAt: TimeInterval = 0
    var now: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    /// Set while ⇧⌘V pastes, so a large paste stays inline.
    var pastingAsText = false
    /// Whether an insertion is a paste (injectable for tests, which never touch the pasteboard).
    var pasteDetector: (String) -> Bool = T3ComposerEditor.isPaste
    /// Toasts the editor raised (title, description), taken by the next sync.
    private(set) var notices: [[String: String]] = []
    func takeNotices() -> [[String: String]] { defer { notices.removeAll() }; return notices }

    init(changed: @escaping (String) -> Void) {
        self.changed = changed
        proxy.editor = self
        styler.editor = self
    }

    // MARK: Elements

    func install(_ element: ExactElement) {
        if element.hook == .t3ComposerKey {
            let name = element.data[.anchor] ?? ""
            if !name.isEmpty { keys[name] = Key(element) }
            return
        }
        guard element.hook == .t3Composer else { return }
        self.element = element
        owner = element.data[.snapshotOwner] ?? ""
        guard let view = element.textView else { return }
        if textView !== view { attach(view) }
        refresh()
    }

    func remove(_ element: ExactElement) {
        for (name, key) in keys where key.element == nil || key.element === element { keys[name] = nil }
        if self.element === element { detach(); self.element = nil }
    }

    func destroy() { detach(); keys.removeAll() }

    private func attach(_ view: NSTextView) {
        detach()
        textView = view
        // r5-integrate: the reference composer is a browser textarea, which never substitutes; AppKit's smart
        // dashes turned a typed `-->` into `—>` (a Mermaid prompt failed to parse). Same switch-off as the Files editor.
        T3PlainText.apply(view)
        if view.delegate !== proxy {
            proxy.inner = view.delegate as? NSObject
            view.delegate = proxy
        }
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: NSText.didChangeNotification, object: view, queue: nil) { [weak self] _ in
            self?.textChanged()
        })
        observers.append(center.addObserver(forName: NSTextView.didChangeSelectionNotification, object: view, queue: nil) { [weak self] _ in
            self?.selectionChanged()
        })
        if let storage = view.textStorage {
            observers.append(center.addObserver(forName: NSTextStorage.didProcessEditingNotification, object: storage, queue: nil) { [weak self] _ in
                self?.styler.storageEdited()
            })
        }
        styler.attach(view)
    }

    private func detach() {
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        styler.detach()
        if let view = textView, view.delegate === proxy { view.delegate = proxy.inner as? NSTextViewDelegate }
        proxy.inner = nil
        textView = nil
    }

    private func textChanged() {
        styler.restyle()
        refresh()
    }

    private func selectionChanged() {
        if styler.snapSelection() { return }
        styler.restyleIfTextMoved()
        refresh()
    }

    func key(_ name: String) -> ExactElement? {
        guard let element = keys[name]?.element, element.isLive else { return nil }
        return element
    }

    // MARK: Trigger (useComposerTriggerState)

    /// Recompute the trigger at the caret, keeping a dismissed one closed
    /// until the caret leaves its token.
    func refresh() {
        var candidate: T3ComposerTrigger?
        if let view = textView, !view.hasMarkedText() {
            let selection = view.selectedRange()
            if selection.length == 0 { candidate = T3ComposerText.trigger(view.string as NSString, cursor: selection.location) }
        } else if textView?.hasMarkedText() == true { return }
        var active = candidate
        if let candidate, let dismissed, candidate.kind == dismissed.kind, candidate.start == dismissed.start { active = nil }
        if candidate == nil || active != nil { dismissed = nil }
        trigger = active
        report()
    }

    func dismiss() {
        dismissed = trigger
        trigger = nil
        report()
    }

    private func report() {
        let signature = "\(owner)|\(trigger.map { "\($0.kind):\($0.query):\($0.start):\($0.end)" } ?? "")"
        guard signature != reported else { return }
        reported = signature
        seq += 1
        changed("t3.editor")
    }

    var state: [String: Any] {
        var value: [String: Any] = ["owner": owner, "seq": seq, "trigger": NSNull(), "atStart": false, "blankAround": false]
        if let trigger, let view = textView {
            value["trigger"] = trigger.dictionary
            let text = view.string as NSString
            let before = text.substring(to: min(trigger.start, text.length))
            let after = text.substring(from: min(trigger.end, text.length))
            value["atStart"] = trigger.start == 0
            value["blankAround"] = before.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && after.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        value["focused"] = textView.map { $0.window?.firstResponder === $0 } ?? false
        value["empty"] = textView?.string.isEmpty ?? true
        return value
    }

    // MARK: Commands (onComposerCommandKey)

    private func press(_ name: String) -> Bool {
        guard let key = key(name) else { return false }
        key.click()
        return true
    }

    func command(_ selector: Selector, in view: NSTextView) -> Bool {
        guard view === textView, view.isEditable, !view.hasMarkedText() else { return false }
        switch selector {
        case #selector(NSResponder.insertBacktab(_:)):
            // ⇧Tab toggles Build/Plan when the plan toggle is shown.
            return press("plan")
        case #selector(NSResponder.cancelOperation(_:)):
            guard trigger != nil else { return false }
            dismiss()
            return true
        case #selector(NSResponder.moveDown(_:)), #selector(NSResponder.moveUp(_:)):
            let down = selector == #selector(NSResponder.moveDown(_:))
            if trigger != nil, press(down ? "ArrowDown" : "ArrowUp") { return true }
            // The open stash menu takes the arrows (ComposerStashMenu).
            if press(down ? "stash-ArrowDown" : "stash-ArrowUp") { return true }
            return recall(backward: !down, in: view)
        case #selector(NSResponder.deleteToBeginningOfLine(_:)):
            return press("stash-Delete")
        case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)):
            if trigger != nil, press("Enter") { return true }
            if press("stash-Enter") { return true }
            let selection = view.selectedRange()
            guard selection.length == 0, let edit = T3ComposerText.listContinuation(view.string, cursor: selection.location) else { return false }
            return apply(NSRange(location: edit.start, length: edit.end - edit.start), edit.replacement, in: view)
        case #selector(NSResponder.insertTab(_:)):
            if trigger != nil, press("Enter") { return true }
            let selection = view.selectedRange()
            guard let edit = T3ComposerText.listIndent(view.string, start: selection.location, end: selection.location + selection.length) else { return false }
            return apply(NSRange(location: edit.start, length: 0), edit.replacement, in: view, caret: selection.location + 2)
        case #selector(NSResponder.moveLeft(_:)), #selector(NSResponder.moveRight(_:)),
             #selector(NSResponder.deleteBackward(_:)), #selector(NSResponder.deleteForward(_:)):
            return styler.chipCommand(selector, in: view)
        default:
            return false
        }
    }

    // MARK: Prompt history (composerPromptHistory.ts)

    private func recall(backward: Bool, in view: NSTextView) -> Bool {
        if historyPosition == nil && !view.string.isEmpty { return false }
        guard styler.caretOnVisualEdge(first: backward, in: view) else { return false }
        guard let step = T3ComposerText.historyStep(backward: backward, entries: history, position: historyPosition, current: view.string) else { return false }
        historyPosition = step.position
        let whole = NSRange(location: 0, length: (view.string as NSString).length)
        _ = apply(whole, step.prompt, in: view)
        return true
    }

    /// The composer's owner changed (another thread): recall restarts.
    func resetHistory() { historyPosition = nil }

    // MARK: Edits

    /// One programmatic edit as one undo step, announced like typing.
    @discardableResult
    func apply(_ range: NSRange, _ text: String, in view: NSTextView, caret: Int? = nil) -> Bool {
        guard view.isEditable, !view.hasMarkedText(), NSMaxRange(range) <= (view.string as NSString).length else { return false }
        view.breakUndoCoalescing()
        guard view.shouldChangeText(in: range, replacementString: text) else { return false }
        view.textStorage?.replaceCharacters(in: range, with: NSAttributedString(string: text, attributes: view.typingAttributes))
        view.didChangeText()
        view.setSelectedRange(NSRange(location: caret ?? (range.location + (text as NSString).length), length: 0))
        view.breakUndoCoalescing()
        lastEditKind = ""
        view.scrollRangeToVisible(view.selectedRange())
        return true
    }

    /// TypeScript's replace (a picked menu row, an inserted chip): only when
    /// the range still holds what the menu saw.
    func edit(_ request: [String: Any]) -> [String: Any] {
        if let fold = request["fold"] as? Int { folds.removeAll { $0["id"] as? Int == fold } }
        guard let view = textView else { return ["applied": false, "reason": "no composer"] }
        let text = view.string as NSString
        let all = request["all"] as? Bool == true
        let start = all ? 0 : (request["start"] as? Int) ?? text.length
        var end = all ? text.length : (request["end"] as? Int) ?? start
        let expect = request["expect"] as? String
        guard start >= 0, start <= end, end <= text.length else { return ["applied": false, "reason": "range"] }
        if let expect, text.substring(with: NSRange(location: start, length: end - start)) != expect { return ["applied": false, "reason": "changed"] }
        // A menu pick applies only to the trigger the menu was built for.
        if let kind = request["kind"] as? String {
            guard let trigger, trigger.kind == kind, trigger.start == start, trigger.end == end else { return ["applied": false, "reason": "changed"] }
        }
        var replacement = request["text"] as? String ?? ""
        // A chip placed into running text keeps a space before it.
        if request["pad"] as? Bool == true, start > 0, !T3ComposerText.isBlank(text.character(at: start - 1)) { replacement = " " + replacement }
        if request["extendSpace"] as? Bool == true, replacement.hasSuffix(" "), end < text.length, text.character(at: end) == 32 { end += 1 }
        if request["focus"] as? Bool != false, let window = view.window, window.firstResponder !== view { window.makeFirstResponder(view) }
        let applied = apply(NSRange(location: start, length: end - start), replacement, in: view)
        if applied, let then = request["then"] as? String, !then.isEmpty { _ = press(then) }
        return ["applied": applied, "caret": view.selectedRange().location]
    }

    /// Insert at the caret, padding with spaces only where words would join
    /// (insertInlineContextReference).
    func insert(_ request: [String: Any]) -> [String: Any] {
        guard let view = textView else { return ["applied": false, "reason": "no composer"] }
        let text = view.string as NSString
        // Terminal/file menus take focus, but the editor retains the user's insertion range.
        let selection = view.selectedRange()
        let start = min(selection.location, text.length)
        let end = start + min(selection.length, text.length - start)
        let lead = start > 0 && !T3ComposerText.isBlank(text.character(at: start - 1)) ? " " : ""
        let replaceEnd = end < text.length && text.character(at: end) == 32 ? end + 1 : end
        let body = request["text"] as? String ?? ""
        if let window = view.window, window.firstResponder !== view { window.makeFirstResponder(view) }
        let applied = apply(NSRange(location: start, length: replaceEnd - start), lead + body + " ", in: view)
        return ["applied": applied]
    }

    /// Tiptap's toggleBold / toggleItalic on Markdown source: unwrap a
    /// selection already wrapped by the marker, else wrap it; an empty
    /// selection opens a marked span for what is typed next.
    func toggleMark(_ marker: String, in view: NSTextView) {
        let text = view.string as NSString
        let selection = view.selectedRange()
        let width = (marker as NSString).length
        let before = selection.location >= width ? text.substring(with: NSRange(location: selection.location - width, length: width)) : ""
        let after = NSMaxRange(selection) + width <= text.length ? text.substring(with: NSRange(location: NSMaxRange(selection), length: width)) : ""
        // A lone `*` beside a `**` is bold's, not italic's.
        let bolder = width == 1 && (selection.location >= 2 && text.substring(with: NSRange(location: selection.location - 2, length: 2)) == "**")
        if before == marker, after == marker, !bolder {
            let outer = NSRange(location: selection.location - width, length: selection.length + width * 2)
            let inner = text.substring(with: selection)
            apply(outer, inner, in: view, caret: selection.location - width)
            view.setSelectedRange(NSRange(location: selection.location - width, length: selection.length))
            return
        }
        let inner = text.substring(with: selection)
        apply(selection, marker + inner + marker, in: view, caret: selection.location + width)
        view.setSelectedRange(NSRange(location: selection.location + width, length: selection.length))
    }

    // MARK: Folder drops (ChatComposer addDroppedFolders)

    /// Whether the T3 server runs on this Mac (TypeScript's sync says).
    var localEnvironment = true
    /// The drag pasteboard (injectable for tests).
    var dragPasteboard: () -> NSPasteboard = { NSPasteboard(name: .drag) }

    /// NSTextView inserts a dropped file's path; when every dropped URL is a
    /// folder and the insertion is exactly their paths, the drop is ours.
    func droppedFolders(_ text: String) -> [URL]? {
        if let event = NSApp.currentEvent, event.type == .keyDown { return nil }
        guard let urls = dragPasteboard().readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL], !urls.isEmpty else { return nil }
        let paths = urls.map(\.path)
        guard paths.contains(text) || paths.joined(separator: "\n") == text || paths.joined(separator: " ") == text else { return nil }
        let folders = urls.filter { var directory: ObjCBool = false; return FileManager.default.fileExists(atPath: $0.path, isDirectory: &directory) && directory.boolValue }
        return folders.count == urls.count ? folders : nil
    }

    func dropFolders(_ folders: [URL], in view: NSView) {
        guard localEnvironment else {
            notices.append(["kind": "error", "title": "Folders can't be dropped into remote environments", "description": "Type the folder path with @ instead."])
            changed("t3.editor")
            return
        }
        DispatchQueue.main.async { [weak self] in
            guard let self, let editor = self.textView else { return }
            for folder in folders {
                let text = editor.string as NSString
                let lead = text.length > 0 && !T3ComposerText.isBlank(text.character(at: text.length - 1)) ? " " : ""
                self.apply(NSRange(location: text.length, length: 0), lead + T3ComposerEditor.fileLink(folder.path) + " ", in: editor)
            }
            editor.window?.makeFirstResponder(editor)
        }
    }

    /// serializeComposerFileLink: `[basename](encodeURI(path))` with ( ) # ? \ escaped.
    static func fileLink(_ path: String) -> String {
        let label = T3ComposerText.basename(path).replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "[", with: "\\[").replacingOccurrences(of: "]", with: "\\]")
        var allowed = CharacterSet.alphanumerics.intersection(CharacterSet(charactersIn: Unicode.Scalar(0)...Unicode.Scalar(127)))
        allowed.insert(charactersIn: ";,/:@&=+$-_.!~*'")
        let destination = path.addingPercentEncoding(withAllowedCharacters: allowed) ?? path
        return "[\(label)](\(destination))"
    }

    // MARK: Change hook

    func shouldChange(_ view: NSTextView, range: NSRange, text: String?) -> Bool {
        guard view === textView, let text else { return true }
        if let folders = droppedFolders(text) { dropFolders(folders, in: view); return false }
        let length = (text as NSString).length
        if !pastingAsText, length > 0, let allowed = foldPaste(view, range: range, text: text) { return allowed }
        if !view.hasMarkedText() {
            let kind = length == 0 ? "delete" : range.length == 0 ? "insert" : "replace"
            let at = now()
            if kind != lastEditKind || at - lastEditAt > T3ComposerEditor.undoGroupDelay { view.breakUndoCoalescing() }
            lastEditKind = kind
            lastEditAt = at
        }
        styler.noteEdit(range, replacement: length)
        return true
    }

    static let promptLimit = 120_000
    /// PASTED_TEXT_ATTACHMENT_THRESHOLD_BYTES.
    static let foldThreshold = 32 * 1024
    /// The largest paste TypeScript can stage as a file (0: this server or
    /// draft takes none), from the sync.
    var foldLimit = 0
    /// Pastes held back for TypeScript to fold into pasted-text.txt. Each
    /// stays until an edit names it, so a sync whose answer is lost retries.
    private(set) var folds: [[String: Any]] = []
    private var foldSeq = 0

    /// foldPastedText (pastedTextDisposition): a paste of 32 KiB or more, or
    /// one that would pass the prompt limit, becomes a file attachment; one
    /// that cannot fold stays inline unless it would pass the limit, which is
    /// refused with the reference's toast. Nil lets the paste through.
    func foldPaste(_ view: NSTextView, range: NSRange, text: String) -> Bool? {
        let length = (text as NSString).length, bytes = text.utf8.count
        let exceeds = (view.string as NSString).length - range.length + length > T3ComposerEditor.promptLimit
        let large = length >= T3ComposerEditor.foldThreshold || bytes >= T3ComposerEditor.foldThreshold
        guard large || exceeds, pasteDetector(text) else { return nil }
        if foldLimit <= 0 || bytes > foldLimit {
            guard exceeds else { return nil }
            let tooLarge = foldLimit > 0
            notices.append(["kind": "error", "title": tooLarge ? "Pasted text is too large to attach" : "Pasted text is too large for this message",
                            "description": tooLarge ? "Reduce the clipboard contents or save a smaller excerpt as a file." : "Remove some text or an attachment, then paste again."])
            changed("t3.editor")
            return false
        }
        let expect = (view.string as NSString).substring(with: range)
        foldSeq += 1
        folds.append(["id": foldSeq, "text": text, "start": range.location, "end": NSMaxRange(range), "expect": expect])
        if folds.count > 4 { folds.removeFirst() }
        changed("t3.editor")
        return false
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
