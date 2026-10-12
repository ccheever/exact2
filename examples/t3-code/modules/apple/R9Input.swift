#if os(macOS)
import AppKit

/// Lane r9-input: the composer's focus, composing (marked) text and the transcript's
/// remembered position (MIT reference, see LICENSE-T3).
///
/// - Focus: AppKit posts a text view's begin-editing only with its first edit, so a
///   click (or ⌘-less focus) into an existing draft never told the window the
///   composer was focused, and ⌘B kept toggling the sidebar there. The window's
///   first responder is observed instead; when it becomes the composer's text view,
///   the view's own delegate (Exact's node, behind T3ComposerTextDelegate) hears the
///   begin-editing it would have heard on typing. Resigning already ends editing.
/// - Composing text: a ⌘ chord or a press elsewhere ends composition first, as a
///   browser's compositionend does, so ⌘B bolds under Korean 2-Set and a click is
///   never spent committing a syllable.
/// - Transcript position (components/chat/MessagesTimeline.tsx rememberTimelinePosition /
///   readTimelinePosition, timelineScrollAnchoring.ts): leaving a thread scrolled up
///   remembers the row at the top edge and the offset within it (session memory, at
///   most 100 threads); reopening it restores that row's position instead of the end.
///   A wheel, scroll key or press in the transcript cancels a restoration. The motion
///   after a gesture's last event (momentum, rows measured as they scroll in) is the
///   reader's too: it is committed once quiet, on a press, or when the thread is left.
/// - A field remounted under the focus (the palette's per-appearance subtrees) keeps its caret.
/// - Diagnostics: R9_INPUT_LOG=<file> logs presses, ⌘ keys, focus and restorations.
final class R9Input {
    private let agent: Bool
    init(agent: Bool) {
        self.agent = agent
        if let path = ProcessInfo.processInfo.environment["R9_INPUT_LOG"], !path.isEmpty { logPath = path }
        holdComposition = ProcessInfo.processInfo.environment["R9_INPUT_HOLD_COMPOSITION"] == "1"
    }
    /// Diagnostics only (R9_INPUT_HOLD_COMPOSITION=1): leave composition to AppKit on a press, for an A/B check.
    private var holdComposition = false

    // MARK: Elements

    func install(_ element: ExactElement) {
        if element.hatch == .t3Composer, let view = element.textView { attachComposer(view) }
        if element.hatch == .t3Transcript { installTranscript(element) }
        if element.hatch == .t3Turn, let id = element.data[.turn], !id.isEmpty {
            turns = turns.filter { $0.value.element != nil && $0.value.element !== element }
            turns[id] = Weak(element: element)
        }
        observeWindow()
    }
    func remove(_ element: ExactElement) {
        if element === transcript { stopTranscript() }
        if element.hatch == .t3Turn { turns = turns.filter { $0.value.element != nil && $0.value.element !== element } }
    }
    func destroy() {
        responder = nil; observedWindow = nil
        if let selection { NotificationCenter.default.removeObserver(selection) }
        selection = nil
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        stopTranscript()
        turns.removeAll()
    }

    // MARK: Composer focus

    private weak var composer: NSTextView?
    private weak var observedWindow: NSWindow?
    private var responder: NSKeyValueObservation?
    private var monitor: Any?
    private(set) var composerFocused = false
    /// Begin-editing bridges sent (tests read it).
    private(set) var bridged = 0

    func attachComposer(_ view: NSTextView) {
        if composer !== view { composer = view; composerFocused = false }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown, .scrollWheel, .keyDown]) { [weak self] event in
                self?.handle(event)
                return event
            }
        }
        observeWindow()
    }

    // MARK: A text field remounted under the focus (palette per-scheme subtrees)

    private struct FieldMark { weak var field: NSTextField?; let text: String; let range: NSRange }
    private var fieldMark: FieldMark?
    private var selection: NSObjectProtocol?
    /// Carets carried over to a remounted field (tests read it).
    private(set) var carried = 0

    /// A field whose subtree remounts (one subtree per appearance) takes the focus again on mount
    /// (`autofocus`), and AppKit selects all of a field it focuses. When the field that had the
    /// focus has left the window and the new one holds the same text with all of it selected, the
    /// old caret and selection come back, so typing goes on where it was.
    func fieldSelectionChanged(_ editor: NSTextView) {
        guard editor.isFieldEditor, let field = editor.delegate as? NSTextField else { return }
        let range = editor.selectedRange(), text = field.stringValue
        if let mark = fieldMark, mark.field !== field, mark.field?.window == nil, mark.text == text,
           range.location == 0, range.length == (text as NSString).length, mark.range != range {
            let restored = NSRange(location: min(mark.range.location, range.length), length: min(mark.range.length, range.length - min(mark.range.location, range.length)))
            fieldMark = FieldMark(field: field, text: text, range: restored)
            carried += 1
            log("field-caret restored=\(restored.location),\(restored.length)")
            DispatchQueue.main.async { if field.currentEditor() === editor { editor.setSelectedRange(restored) } }
            return
        }
        fieldMark = FieldMark(field: field, text: text, range: range)
    }

    /// Follows the composer's window's first responder (re-attached when the view changes windows).
    func observeWindow() {
        guard let window = composer?.window ?? transcript?.scrollView?.window, window !== observedWindow else { return }
        observedWindow = window
        if selection == nil {
            selection = NotificationCenter.default.addObserver(forName: NSTextView.didChangeSelectionNotification, object: nil, queue: nil) { [weak self] note in
                guard let editor = note.object as? NSTextView, editor.isFieldEditor, editor.window === self?.observedWindow else { return }
                self?.fieldSelectionChanged(editor)
            }
        }
        responder = window.observe(\.firstResponder, options: [.new]) { [weak self] window, _ in
            DispatchQueue.main.async { self?.responderChanged(window) }
        }
        responderChanged(window)
    }

    /// Internal so tests can call it after moving the first responder.
    func responderChanged(_ window: NSWindow) {
        guard let view = composer, view.window === window else { return }
        let focused = window.firstResponder === view
        defer { composerFocused = focused }
        guard focused, !composerFocused else { return }
        bridged += 1
        log("focus-bridge marked=\(view.hasMarkedText())")
        view.delegate?.textDidBeginEditing?(Notification(name: NSText.didBeginEditingNotification, object: view))
    }

    /// A ⌘ chord's letter: `charactersIgnoringModifiers`, or the ANSI key's letter when a
    /// non-Latin source reports another script for it.
    static func chordKey(_ event: NSEvent) -> String {
        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        if !key.isEmpty, key.unicodeScalars.allSatisfy({ $0.isASCII }) { return key }
        return ansiLetters[event.keyCode] ?? key
    }
    static let ansiLetters: [UInt16: String] = [11: "b", 34: "i", 0: "a", 6: "z", 13: "w", 40: "k"]

    /// Ends composition in `view` (the input method drops its state, the text stays);
    /// true once no marked text is left.
    @discardableResult
    static func commitMarkedText(_ view: NSTextView) -> Bool {
        guard view.hasMarkedText() else { return true }
        view.inputContext?.discardMarkedText()
        if view.hasMarkedText() { view.unmarkText() }
        return !view.hasMarkedText()
    }

    // MARK: Events

    /// Internal so tests can drive it with synthetic events (`window` stands in for the window a
    /// synthesized wheel event cannot carry).
    func handle(_ event: NSEvent, window eventWindow: NSWindow? = nil) {
        let now = self.now()
        let eventWindow = eventWindow ?? event.window
        if event.type == .leftMouseDown || event.type == .rightMouseDown, !holdComposition, let view = composer, view.hasMarkedText(), eventWindow === view.window,
           !view.bounds.contains(view.convert(event.locationInWindow, from: nil)) {
            log("commit-on-press")
            Self.commitMarkedText(view)
        }
        if let scroll = transcript?.scrollView, let window = scroll.window, eventWindow === window {
            let inside = scroll.bounds.contains(scroll.convert(event.locationInWindow, from: nil))
            let responderInside = (window.firstResponder as? NSView)?.isDescendant(of: scroll) == true
            let scrollKey = event.type == .keyDown && [UInt16(115), 119, 116, 121, 125, 126, 49].contains(event.keyCode) && responderInside
            if scrollKey { keyUntil = now + 0.8 }
            if (event.type == .scrollWheel && inside) || scrollKey || (event.type == .leftMouseDragged && inside) {
                gestureUntil = now + 0.8
                if restoring != nil { log("restore-cancel gesture"); restoring = nil }
                // The reader's motion settles after its last event (momentum, a line scroll's
                // animation, rows measured as they come into view): that tail is theirs too.
                if !owner.isEmpty { settleOwner = owner; settleUntil = now + Self.settleTail }
                armSettle(event.type == .scrollWheel && event.momentumPhase == .ended ? 0.1 : Self.settleQuiet)
            }
            if event.type == .leftMouseDown, inside {
                if restoring != nil { log("restore-cancel press"); restoring = nil }
                commitPending("press")
            }
        }
        guard logPath != nil else { return }
        switch event.type {
        case .leftMouseDown, .leftMouseUp, .rightMouseDown:
            let hit = event.window?.contentView?.hitTest(event.locationInWindow)
            log("\(event.type == .leftMouseUp ? "up" : event.type == .leftMouseDown ? "down" : "rdown") at=\(Int(event.locationInWindow.x)),\(Int(event.locationInWindow.y)) clicks=\(event.clickCount) hit=\(hit.map { String(describing: type(of: $0)) } ?? "nil"):\(hit?.accessibilityIdentifier() ?? "") responder=\(event.window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil") marked=\(composer?.hasMarkedText() ?? false) source=\(NSTextInputContext.current?.selectedKeyboardInputSource ?? "-") key=\(NSApp.keyWindow === event.window)")
        case .keyDown where event.modifierFlags.contains(.command):
            log("key chars=\(event.characters ?? "") ignoring=\(event.charactersIgnoringModifiers ?? "") code=\(event.keyCode) chord=\(Self.chordKey(event)) responder=\(event.window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil") composerFocused=\(composerFocused) marked=\(composer?.hasMarkedText() ?? false) source=\(NSTextInputContext.current?.selectedKeyboardInputSource ?? "-")")
        default: break
        }
    }

    // MARK: Transcript position

    struct Position: Equatable { let rowId: String; let offsetWithinRow: CGFloat; let scrollOffset: CGFloat; let atEnd: Bool }
    private struct Weak { weak var element: ExactElement? }
    private struct Sample { let top: CGFloat; let height: CGFloat; let content: CGFloat; let width: CGFloat }
    private struct Restore { let position: Position; let deadline: TimeInterval; var stable: Int; var found: Bool }
    private weak var transcript: ExactElement?
    private var bounds: NSObjectProtocol?
    private var turns: [String: Weak] = [:]
    private(set) var owner = ""
    private var memory: [String: Position] = [:]
    private var order: [String] = []
    private var previous: Sample?
    private var gestureUntil: TimeInterval = 0
    private var keyUntil: TimeInterval = 0
    private var restoring: Restore?
    private var timer: Timer?
    var now: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    /// The settling tail of a reader's gesture: the thread it began in, until when its motion still
    /// counts, the position its last move left (committed once the motion is quiet, on a press in the
    /// transcript, or when the thread is left) and the quiet timer.
    static let settleTail: TimeInterval = 3
    static let settleQuiet: TimeInterval = 0.35
    private var settleOwner = ""
    private var settleUntil: TimeInterval = 0
    private var pending: (owner: String, position: Position)?
    private var settleTimer: Timer?
    /// Tests read it.
    var isSettling: Bool { !settleOwner.isEmpty }

    func remembered(_ owner: String) -> Position? { memory[owner] }
    var isRestoring: Bool { restoring != nil }

    private func installTranscript(_ element: ExactElement) {
        if transcript !== element {
            stopTranscript()
            transcript = element
            if let clip = element.scrollView?.contentView {
                clip.postsBoundsChangedNotifications = true
                bounds = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: clip, queue: .main) { [weak self] _ in self?.sample() }
            }
            if monitor == nil {
                monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown, .scrollWheel, .keyDown]) { [weak self] event in
                    self?.handle(event)
                    return event
                }
            }
        }
        let next = element.data[.timelineOwner] ?? ""
        guard next != owner else { return }
        // Leaving: the rows may already be the next thread's, so the thread left keeps the last
        // position its own rows confirmed (a settling tail not yet committed), never this geometry.
        commitPending("leave")
        owner = next; previous = nil; gestureUntil = 0
        if let position = memory[next], !position.atEnd {
            restoring = Restore(position: position, deadline: now() + 3, stable: 0, found: false)
            log("restore-start owner=\(next) row=\(position.rowId) offset=\(position.offsetWithinRow) scroll=\(position.scrollOffset)")
            startTimer()
        } else { restoring = nil }
    }
    private func stopTranscript() {
        if let bounds { NotificationCenter.default.removeObserver(bounds) }
        bounds = nil; transcript = nil; restoring = nil; previous = nil
        timer?.invalidate(); timer = nil
        commitPending("stop")
    }
    private func startTimer() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] timer in
            guard let self, self.restoring != nil else { timer.invalidate(); return }
            self.step()
        }
    }

    /// The transcript's top edge, rows and end, in the clip view's (flipped) coordinates.
    private func geometry() -> (scroll: NSScrollView, visible: NSRect, content: CGFloat)? {
        guard let scroll = transcript?.scrollView, let document = scroll.documentView, scroll.contentView.isFlipped else { return nil }
        return (scroll, scroll.contentView.bounds, document.frame.height)
    }
    private func rowTop(_ id: String, in scroll: NSScrollView) -> CGFloat? {
        guard let view = turns[id]?.element?.view, view.window != nil, let document = scroll.documentView, view.isDescendant(of: document) else { return nil }
        let rect = view.convert(view.bounds, to: scroll.contentView)
        return rect.height > 0 ? rect.minY : nil
    }

    /// Internal so tests can call it after scrolling.
    func sample() {
        guard let g = geometry() else { return }
        let (scroll, visible, content) = (g.scroll, g.visible, g.content)
        let current = Sample(top: visible.minY, height: visible.height, content: content, width: visible.width)
        defer { previous = current }
        if restoring != nil { step(); return }
        // A wheel moves the top a little per frame; a jump (Edit from here, a thread switch's jump to the
        // end before the rows change) is not the reader's, unless a scroll key (Home, End) made it.
        // While the reader's own wheel or keys move it, a growing turn (content streaming in) still counts.
        let t = now(), reading = t <= gestureUntil
        // After the wheel's last event its motion goes on settling (momentum, a line scroll's
        // animation, rows measured as they come into view, the end-of-transcript pill): moves in that
        // tail are kept as pending and committed once quiet, on a press, or when the thread is left.
        let settling = !reading && !owner.isEmpty && settleOwner == owner && t <= settleUntil
        guard agent || reading || settling, let previous, !owner.isEmpty, abs(previous.top - current.top) > 0.25 else { return }
        guard abs(previous.top - current.top) < current.height * 0.9 || t <= keyUntil else {
            // Not the reader's, unless it is a switch's rows landing (the owner changes in this batch,
            // and leaving commits the tail first): decided on the next turn.
            let sampled = owner
            if settling { DispatchQueue.main.async { [weak self] in
                guard let self, self.owner == sampled, self.settleOwner == sampled else { return }
                self.log("settle-cancel jump"); self.endSettle()
            } }
            return
        }
        guard reading || settling || (abs(previous.height - current.height) < 0.5 && abs(previous.content - current.content) < 0.5 && abs(previous.width - current.width) < 0.5) else { return }
        // A thread switch swaps the rows and the owner in one batch, the rows' layout (and its bounds
        // change) first: a position is kept on the next turn only if the owner is still the same.
        let sampled = owner, commit = agent || reading
        DispatchQueue.main.async { [weak self] in
            guard let self, self.owner == sampled, self.restoring == nil, let g = self.geometry(), g.scroll === scroll else { return }
            let position = self.position(scroll: g.scroll, visible: g.visible, content: g.content)
            if commit { self.remember(position) }
            if self.settleOwner == sampled {
                self.pending = (sampled, position)
                self.armSettle(Self.settleQuiet)
            }
        }
    }

    /// Re-arms the quiet timer that commits a settled position.
    private func armSettle(_ delay: TimeInterval) {
        guard !settleOwner.isEmpty else { return }
        settleTimer?.invalidate()
        settleTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in self?.settle() }
    }

    /// The motion is quiet (the wheel's momentum ended, nothing moved for a moment): the position
    /// now shown is the reader's. Internal so tests can call it in place of the timer.
    func settle() {
        settleTimer?.invalidate(); settleTimer = nil
        guard !settleOwner.isEmpty else { return }
        pending = nil
        guard settleOwner == owner, restoring == nil, let g = geometry() else { endSettle(); return }
        let position = position(scroll: g.scroll, visible: g.visible, content: g.content)
        log("settle owner=\(owner)")
        remember(position)
        if now() > settleUntil { endSettle() }
    }

    /// Commits a settling tail's last confirmed position (it was this owner's rows) and ends the tail.
    private func commitPending(_ reason: String) {
        if let pending, pending.owner == settleOwner, !pending.owner.isEmpty {
            log("settle-commit \(reason)")
            remember(pending.position, owner: pending.owner)
        }
        endSettle()
    }
    private func endSettle() {
        settleTimer?.invalidate(); settleTimer = nil
        settleOwner = ""; pending = nil
    }

    private func position(scroll: NSScrollView, visible: NSRect, content: CGFloat) -> Position {
        let atEnd = content <= visible.height + 2 || visible.minY >= content - visible.height - 2
        var anchor: (id: String, top: CGFloat)?
        if !atEnd {
            let rows = turns.compactMap { entry -> (String, CGFloat)? in rowTop(entry.key, in: scroll).map { (entry.key, $0) } }
            let above = rows.filter { $0.1 <= visible.minY }.max { $0.1 < $1.1 }
            let below = rows.filter { $0.1 > visible.minY }.min { $0.1 < $1.1 }
            anchor = (above ?? below).map { (id: $0.0, top: $0.1) }
        }
        return Position(rowId: anchor?.id ?? "", offsetWithinRow: anchor.map { visible.minY - $0.top } ?? 0, scrollOffset: visible.minY, atEnd: atEnd)
    }

    private func remember(_ position: Position, owner key: String? = nil) {
        let owner = key ?? self.owner
        guard !owner.isEmpty, memory[owner] != position else { return }
        memory[owner] = position
        order.removeAll { $0 == owner }; order.append(owner)
        if order.count > 100 { memory[order.removeFirst()] = nil }
        log("remember owner=\(owner) row=\(position.rowId) offset=\(position.offsetWithinRow) scroll=\(position.scrollOffset) atEnd=\(position.atEnd)")
    }

    /// One restoration pass: the saved row's top plus its offset once the row is mounted,
    /// the saved scroll offset until then; done after the position holds for ~200 ms.
    func step() {
        guard var restore = restoring else { return }
        guard now() <= restore.deadline, let g = geometry() else {
            log("restore-end deadline found=\(restore.found)"); restoring = nil; return
        }
        let (scroll, visible, content) = (g.scroll, g.visible, g.content)
        let maxTop = max(0, content - visible.height)
        let target: CGFloat
        if let top = rowTop(restore.position.rowId, in: scroll) { target = top + restore.position.offsetWithinRow; restore.found = true }
        else { target = restore.position.scrollOffset }
        let clamped = min(max(0, target), maxTop)
        if abs(visible.minY - clamped) > 1 {
            restore.stable = 0
            scroll.contentView.scroll(to: NSPoint(x: visible.minX, y: clamped))
            scroll.reflectScrolledClipView(scroll.contentView)
        } else if restore.found && content > visible.height + 2 {
            restore.stable += 1
        }
        if restore.stable >= 6 {
            log("restore-done top=\(clamped)")
            restoring = nil; previous = nil
            return
        }
        restoring = restore
    }

    // MARK: Diagnostics

    private var logPath: String?
    private func log(_ line: String) {
        guard let logPath, let data = "\(Date().timeIntervalSince1970) \(line)\n".data(using: .utf8) else { return }
        if let handle = FileHandle(forWritingAtPath: logPath) { handle.seekToEndOfFile(); handle.write(data); handle.closeFile() }
        else { FileManager.default.createFile(atPath: logPath, contents: data) }
    }
}
#endif
