#if os(macOS)
import AppKit

/// A skill chip's press (settings-appearance-and-skill-chip). In T3 Code a skill chip is the
/// trigger of a ContextChipPopover (ComposerPromptEditorTiptap.tsx ComposerSkillNodeView,
/// contextChipParts.tsx; T3 Code 1e2ecbd975, MIT, see LICENSE-T3): a button named
/// "Skill <label>. Show details" that opens the skill's details above it. Here the chip is drawn
/// in the text view, so the press is caught on the chip and reported to the app (`t3.chip`),
/// which draws the popover in Contract at the chip's frame (composer-chip-popover.contract).
///
/// One press is open per editor; the app shows the newest of the composer's and the Settings
/// prompt sample's (`seq` across both). Like Base UI's Popover it closes on a second press of
/// its chip, Escape, a press elsewhere in the text, an edit that takes the chip away, and the
/// editor leaving the window; a press outside the text is the app's (its window counts it).
/// While it is open the frame follows the chip through scrolling and layout.
///
/// The press is the editor's, not the text's. In T3 Code the chip is
/// a selectable atom (`atom: true, selectable: true`), so ProseMirror answers a press on it with a
/// NodeSelection of the chip; the popover then takes the focus, Escape gives it back to the chip, and
/// typing replaces the chip (`$frontend-design now`, caret after "now", press the chip, Escape, "x":
/// "x now", over CDP). Here the chip is its hidden source text, so a press selects that text and the
/// text view never places a caret for it (it put one before the chip, and "x" went in front of it).
///
/// diff-cite-popover DC-1: a citation chip's comment button is the same kind of trigger (AssistantCitationChip.tsx's
/// PopoverTrigger "Add comment to citation" / "Edit citation comment"), and Cite opens its comment at once under the
/// cited text (`openCitation`). Its popover holds a draft the app keeps, so while it is open a press in the text (its
/// trigger's included) only takes the focus, and the popover's blur settles the draft in the app; the app's Save,
/// Cancel and Escape end it (`finishCitation`), rewriting or removing the chip. An edit in the prompt leaves it open
/// (a draft past 8,000 characters keeps it open while the prompt has the focus): it follows its chip, and closes when
/// the chip goes, as the reference's node view keeps its popover by its `citeKey`.
final class T3ComposerChipPress {
    /// Newer presses win across editors.
    private static var lastSeq = 0
    private(set) weak var styler: T3ComposerStyler?
    /// "composer" or "preview" (the Settings › Appearance prompt sample).
    var surface = "composer"
    /// The editor's owner (the composer's draft key) when the press was made.
    var owner: () -> String = { "" }
    /// Announces `t3.chip` (T3Module wires it to the module's topic).
    var onChange: () -> Void = {}
    private(set) var chip: T3ComposerChip?
    private(set) var seq = 0
    /// [x, top, width, height] in the window content's top-left space, in points.
    private(set) var frame: [Double] = []
    /// diff-cite-popover DC-1: a citation chip's comment. "chip" opens it at the chip's comment button
    /// (`side="top" align="end"`), "source" under the cited text after Cite (`side="bottom"`).
    private(set) var anchor = "chip"
    /// The comment button's frame (as `frame`), and the cited text's after Cite (the app measured it).
    private(set) var trigger: [Double] = []
    private(set) var source: [Double] = []
    /// After Cite, Cancel and Escape take the chip away again (ComposerPromptEditorTiptap's `removeOnCancel`).
    private(set) var removeOnCancel = false
    /// The open citation's source link, by which it follows its chip through edits around it (`validate`).
    private var link = ""
    private var observers: [NSObjectProtocol] = []
    private var elements: [Element] = []
    private var monitor: Any?

    init(styler: T3ComposerStyler) { self.styler = styler }

    /// The text view's presses, seen as the application sends them (a hand's click and the agent's tap
    /// both go through `NSApplication.sendEvent`, AgentMac.swift), before the text view places the caret.
    func attach() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            self?.mouseDown(event) == true ? nil : event
        }
    }

    /// A primary press that lands on the text view: on a skill chip it selects the chip and opens (or
    /// closes) its details, and the text view does not see it (true: the press is taken); anywhere else
    /// in the text it closes them (Base UI's outside press). A press the window gives to another view
    /// (the popover drawn over the text included) is not the text's.
    @discardableResult func mouseDown(_ event: NSEvent) -> Bool {
        guard let styler, let view = styler.view, let window = view.window, event.window === window,
              event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty,
              let root = window.contentView?.superview ?? window.contentView else { return false }
        let hit = root.hitTest(root.convert(event.locationInWindow, from: nil))
        guard hit?.isDescendant(of: view) == true else {
            // diff-cite-popover DC-1: a press anywhere else outside the citation's comment (the timeline's ground, which
            // takes no focus and no window `pointerdown`, included) is Base UI's outside press: the popover's textarea
            // gives the focus up, and its blur settles the draft in the app. A press in the popover is its own.
            if chip?.kind == "citation", !T3ComposerChipPress.inCitationComment(hit), window.firstResponder !== view { window.makeFirstResponder(nil) }
            return false
        }
        let point = view.convert(event.locationInWindow, from: nil)
        // diff-cite-popover DC-1: while a citation's comment is open, a press in the text is its outside press, and on its
        // own button the trigger's: the text view takes the focus from the popover's textarea, whose blur settles the draft
        // in the app (resolveAssistantCitationCommentDismissal), so nothing closes here. The trigger's press is taken; any
        // other is the text view's, which places the caret.
        if let open = chip, open.kind == "citation" {
            guard styler.citationAction(at: point) == open, !view.hasMarkedText() else {
                if window.firstResponder !== view { window.makeFirstResponder(view) }
                return false
            }
            select(open, in: view, window: window)
            return true
        }
        guard let chip = styler.citationAction(at: point) ?? styler.skillChip(at: point) else { close(); return false }
        // A composition still open is the text view's to commit (it does so on any press).
        let taken = !view.hasMarkedText()
        if taken { select(chip, in: view, window: window) }
        press(chip)
        return taken
    }

    /// ProseMirror's NodeSelection of the pressed chip: its whole source, in the editor that has the focus.
    private func select(_ chip: T3ComposerChip, in view: NSTextView, window: NSWindow) {
        if window.firstResponder !== view { window.makeFirstResponder(view) }
        guard NSMaxRange(chip.range) <= (view.string as NSString).length else { return }
        view.setSelectedRange(chip.range)
        styler?.noteSelection()
    }

    var isOpen: Bool { chip != nil }

    /// A press on `chip`: opens its details, or closes them when they are open on it (PopoverTrigger toggles).
    func press(_ chip: T3ComposerChip) {
        if let open = self.chip, open == chip {
            // A citation's trigger toggles through the app, which commits a changed draft first (its textarea's blur).
            if open.kind != "citation" { close() }
            return
        }
        open(chip, anchor: "chip", source: [], removeOnCancel: false)
    }

    private func open(_ chip: T3ComposerChip, anchor: String, source: [Double], removeOnCancel: Bool) {
        T3ComposerChipPress.lastSeq += 1
        seq = T3ComposerChipPress.lastSeq
        self.chip = chip
        self.anchor = anchor
        self.source = source
        self.removeOnCancel = removeOnCancel
        if chip.kind == "citation", let view = styler?.view, chip.end <= (view.string as NSString).length {
            link = (view.string as NSString).substring(with: chip.range)
        } else { link = "" }
        // A citation's comment takes the focus as it opens (PopoverPopup `initialFocus`: its textarea): the prompt gives it
        // up, so the popover's `autofocus` (which yields to a focused view, as HTML's does) lands there.
        if chip.kind == "citation", let window = styler?.view?.window, window.firstResponder === styler?.view { window.makeFirstResponder(nil) }
        follow()
        (frame, trigger) = measure() ?? ([], [])
        onChange()
    }

    /// Whether `view` is in the window's citation comment popover (citation-comment.contract, its node's testId
    /// "citation-comment", which Exact writes as the view's accessibility identifier).
    static func inCitationComment(_ view: NSView?) -> Bool {
        sequence(first: view, next: { $0?.superview }).contains { $0?.accessibilityIdentifier() == "citation-comment" }
    }

    /// Whether the window's focus is in the citation comment popover (its textarea, or a field editor working for it).
    static func focusInCitationComment(_ window: NSWindow) -> Bool {
        let responder = window.firstResponder as? NSView
        return inCitationComment(responder) || inCitationComment((responder as? NSTextView)?.delegate as? NSView)
    }

    /// Whether nothing the web would focus holds the window's focus, the reference's `document.activeElement ===
    /// document.body`: the window itself (a press on ground that takes no focus, which the outside press left there),
    /// or a view that is no key view (a paragraph takes the focus only to select its text, where the web's stays on the body).
    static func focusIsNowhere(_ window: NSWindow) -> Bool {
        guard let responder = window.firstResponder, responder !== window else { return true }
        return (responder as? NSView)?.canBecomeKeyView == false
    }

    /// After Cite (ChatComposer citeAssistantText's `citationCommentAnchor`, the editor's requestCitationComment): the
    /// comment of the citation chip `href` just inserted opens under the cited text (`source`, the window frame the app
    /// measured), and Cancel or Escape removes that chip again. The chip is the one ending nearest before the caret.
    @discardableResult func openCitation(href: String, source: [Double]) -> Bool {
        guard let styler, let view = styler.view else { return false }
        let text = view.string as NSString, caret = view.selectedRange().location, link = "[Assistant quote](\(href))"
        let matches = styler.chips.filter { $0.kind == "citation" && $0.end <= text.length && text.substring(with: $0.range) == link }
        guard let chip = matches.last(where: { $0.end <= caret }) ?? matches.first else { return false }
        open(chip, anchor: "source", source: source.count == 4 ? source : [], removeOnCancel: true)
        return true
    }

    /// The app's Save, Cancel, Escape or outside press's commit on a citation's comment (`seq` the one it showed): the
    /// chip's source becomes `text` ("" removes it, nil keeps it), the comment closes and the prompt takes the focus with
    /// the caret after the chip, as ComposerCitationNodeView's onSave, onRemove and onRestoreFocus leave it, when `focus`
    /// (the popover's own Save, Cancel, Escape) or, for an outside press (`focus` false), when the press left the focus
    /// nowhere (PopoverPopup's `finalFocus`: the body or the popup); a control the press focused, the prompt's text
    /// included, keeps it. `escape` is the window's Escape: in the comment's textarea it is the editor's Cancel (after
    /// Cite the chip goes); anywhere else it is Base UI's escape-key dismissal, which only closes it
    /// (resolveAssistantCitationCommentDismissal: "close") and keeps the chip.
    @discardableResult func finishCitation(seq: Int, text: String?, focus: Bool, escape: Bool = false) -> Bool {
        guard seq == self.seq, let chip, chip.kind == "citation", let styler, let view = styler.view else { return false }
        let window = view.window
        let inside = window.map(T3ComposerChipPress.focusInCitationComment) ?? false
        let text = escape && !inside ? nil : text
        let focus = (focus && (!escape || inside)) || inside || (window.map(T3ComposerChipPress.focusIsNowhere) ?? false)
        let string = view.string as NSString
        let current = chip.end <= string.length ? string.substring(with: chip.range) : nil
        close()
        guard let current else { return false }
        if focus, let window, window.firstResponder !== view { window.makeFirstResponder(view) }
        if let text, text != current {
            // An outside press's commit that left the focus on a control keeps the reader's caret, moved with the text after the chip.
            let kept = view.selectedRange(), delta = (text as NSString).length - chip.range.length
            guard styler.editor?.apply(chip.range, text, in: view) == true else { return false }
            if !focus {
                let location = kept.location >= chip.end ? kept.location + delta : min(kept.location, chip.start)
                view.setSelectedRange(NSRange(location: min(max(0, location), (view.string as NSString).length), length: 0))
            }
            return true
        }
        if focus { view.setSelectedRange(NSRange(location: chip.end, length: 0)) }
        return true
    }

    func close() {
        guard chip != nil else { return }
        chip = nil
        frame = []
        trigger = []
        source = []
        removeOnCancel = false
        link = ""
        unfollow()
        onChange()
    }

    /// The app's close (an outside press, View instructions): only the press it saw. A citation's comment closed by an
    /// outside press with nothing to commit gives the prompt the focus, the caret after the chip, when the press left the
    /// focus nowhere (PopoverPopup's `finalFocus` → onRestoreFocus), as `finishCitation` does.
    func close(seq: Int) {
        guard seq == self.seq, let chip else { return }
        guard chip.kind == "citation", let view = styler?.view, let window = view.window, chip.end <= (view.string as NSString).length,
              T3ComposerChipPress.focusIsNowhere(window) || T3ComposerChipPress.focusInCitationComment(window) else { close(); return }
        close()
        if window.firstResponder !== view { window.makeFirstResponder(view) }
        view.setSelectedRange(NSRange(location: chip.end, length: 0))
    }

    /// After a restyle: the open chip must still be one of the text's chips. A citation's comment follows its chip when an
    /// edit around it moves it (its source link, nearest the old place), and closes when the chip goes.
    func validate() {
        guard let chip, let styler else { return }
        if !styler.chips.contains(chip) {
            guard chip.kind == "citation", let view = styler.view, !link.isEmpty else { close(); return }
            let text = view.string as NSString
            let moved = styler.chips.filter { $0.kind == "citation" && $0.end <= text.length && text.substring(with: $0.range) == link }
                .min { abs($0.start - chip.start) < abs($1.start - chip.start) }
            guard let moved else { close(); return }
            self.chip = moved
        }
        remeasure()
    }

    func remeasure() {
        guard chip != nil, let next = measure(), next.0 != frame || next.1 != trigger else { return }
        (frame, trigger) = next
        onChange()
    }

    /// What the app reads (`editorChip`).
    var state: [String: Any] {
        guard let chip, let styler else { return ["seq": seq, "open": false, "surface": surface] }
        var value: [String: Any] = ["seq": seq, "open": true, "surface": surface, "owner": owner(), "kind": chip.kind, "name": chip.label,
                                    "label": styler.displayLabel(chip), "frame": frame]
        if chip.kind == "citation", let view = styler.view, chip.end <= (view.string as NSString).length {
            // The chip's source link (its href carries the comment), the popover's anchor and the outside presses.
            value["link"] = (view.string as NSString).substring(with: chip.range)
            value["anchor"] = anchor
            value["anchorFrame"] = anchor == "source" && !source.isEmpty ? source : trigger
            value["removeOnCancel"] = removeOnCancel
        }
        return value
    }

    /// The newest open press of `presses`, else the newest closed one's sequence.
    static func latest(_ presses: [T3ComposerChipPress]) -> [String: Any] {
        guard let newest = presses.filter(\.isOpen).max(by: { $0.seq < $1.seq }) else {
            return ["seq": presses.map(\.seq).max() ?? 0, "open": false]
        }
        return newest.state
    }

    // MARK: Geometry

    /// The chip's frame and, for a citation, its comment button's.
    private func measure() -> ([Double], [Double])? {
        guard let chip, let styler, let view = styler.view, let rect = styler.chipRect(chip),
              let content = view.window?.contentView else { return nil }
        let half = { (value: CGFloat) in (Double(value) * 2).rounded() / 2 }
        let place = { (rect: NSRect) -> [Double] in
            let box = view.convert(rect, to: content)
            let top = content.isFlipped ? box.minY : content.bounds.height - box.maxY
            return [half(box.minX), half(top), half(box.width), half(box.height)]
        }
        return (place(rect), styler.citationActionRect(chip).map(place) ?? [])
    }

    /// Re-measure while open when any clip view above the text scrolls or any view above it moves.
    private func follow() {
        unfollow()
        guard let view = styler?.view else { return }
        let center = NotificationCenter.default
        var current: NSView? = view
        while let node = current {
            node.postsFrameChangedNotifications = true
            observers.append(center.addObserver(forName: NSView.frameDidChangeNotification, object: node, queue: .main) { [weak self] _ in self?.remeasure() })
            if let clip = node as? NSClipView {
                clip.postsBoundsChangedNotifications = true
                observers.append(center.addObserver(forName: NSView.boundsDidChangeNotification, object: clip, queue: .main) { [weak self] _ in self?.remeasure() })
            }
            current = node.superview
        }
    }

    private func unfollow() {
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
    }

    // MARK: Accessibility (ContextChipPopover's trigger: a button "Skill <label>. Show details")

    /// One button element per skill chip and per citation's comment button, children of the text view.
    func refreshAccessibility() {
        guard let styler, let view = styler.view else { elements = []; return }
        // diff-cite-popover DC-1: a citation's comment button too (its PopoverTrigger).
        let skills = styler.chips.filter { $0.kind == "skill" || $0.kind == "citation" }
        if skills.isEmpty && elements.isEmpty { return }
        elements = skills.map { Element(press: self, chip: $0, view: view) }
        view.setAccessibilityChildren(elements.isEmpty ? nil : elements)
    }

    func detach() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        if let view = styler?.view, !elements.isEmpty { view.setAccessibilityChildren(nil) }
        elements = []
        close()
    }

    var accessibilityElements: [NSAccessibilityElement] { elements }

    final class Element: NSAccessibilityElement {
        /// NSAccessibilityElement does not adopt the NSAccessibilityElement protocol at run time, so a Swift
        /// reader of the text view's `accessibilityChildrenInNavigationOrder()` (typed by that protocol and
        /// filled from its children, as the agent's accessibility walk reads it) traps on a plain one in a
        /// checked build; this subclass adopts it.
        private static let adopted: Bool = objc_getProtocol("NSAccessibilityElement").map { class_addProtocol(Element.self, $0) } ?? false
        private weak var press: T3ComposerChipPress?
        private weak var view: NSTextView?
        let chip: T3ComposerChip

        init(press: T3ComposerChipPress, chip: T3ComposerChip, view: NSTextView) {
            _ = Element.adopted
            self.press = press
            self.chip = chip
            self.view = view
            super.init()
            setAccessibilityRole(.button)
            // An NSAccessibilityElement reports itself disabled unless told otherwise
            // (`isAccessibilityEnabled` is false by default), so Accessibility Inspector and VoiceOver read the
            // chip's button as dimmed; the reference's PopoverTrigger is an enabled button.
            setAccessibilityEnabled(true)
            setAccessibilityParent(view)
            setAccessibilityLabel(chip.kind == "citation" ? (chip.detail == "comment" ? "Edit citation comment" : "Add comment to citation")
                : "Skill \(press.styler?.displayLabel(chip) ?? chip.label). Show details")
        }

        override func accessibilityFrame() -> NSRect {
            guard let view, let window = view.window,
                  let rect = chip.kind == "citation" ? press?.styler?.citationActionRect(chip) : press?.styler?.chipRect(chip) else { return .zero }
            return window.convertToScreen(view.convert(rect, to: nil))
        }

        override func accessibilityPerformPress() -> Bool {
            guard let press else { return false }
            press.press(chip)
            return true
        }
    }

    deinit { unfollow(); if let monitor { NSEvent.removeMonitor(monitor) } }
}
#endif
