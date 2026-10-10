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
/// realinput-1010e-followups RE-3: the press is the editor's, not the text's. In T3 Code the chip is
/// a selectable atom (`atom: true, selectable: true`), so ProseMirror answers a press on it with a
/// NodeSelection of the chip; the popover then takes the focus, Escape gives it back to the chip, and
/// typing replaces the chip (`$frontend-design now`, caret after "now", press the chip, Escape, "x":
/// "x now", over CDP). Here the chip is its hidden source text, so a press selects that text and the
/// text view never places a caret for it (it put one before the chip, and "x" went in front of it).
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
              let root = window.contentView?.superview ?? window.contentView,
              root.hitTest(root.convert(event.locationInWindow, from: nil))?.isDescendant(of: view) == true else { return false }
        guard let chip = styler.skillChip(at: view.convert(event.locationInWindow, from: nil)) else { close(); return false }
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
        if let open = self.chip, open == chip { close(); return }
        T3ComposerChipPress.lastSeq += 1
        seq = T3ComposerChipPress.lastSeq
        self.chip = chip
        follow()
        frame = measure() ?? []
        onChange()
    }

    func close() {
        guard chip != nil else { return }
        chip = nil
        frame = []
        unfollow()
        onChange()
    }

    /// The app's close (an outside press, View instructions): only the press it saw.
    func close(seq: Int) { if seq == self.seq { close() } }

    /// After a restyle: the open chip must still be one of the text's chips.
    func validate() {
        guard let chip, let styler else { return }
        if !styler.chips.contains(chip) { close(); return }
        remeasure()
    }

    func remeasure() {
        guard chip != nil, let next = measure(), next != frame else { return }
        frame = next
        onChange()
    }

    /// What the app reads (`editorChip`).
    var state: [String: Any] {
        guard let chip, let styler else { return ["seq": seq, "open": false, "surface": surface] }
        return ["seq": seq, "open": true, "surface": surface, "owner": owner(), "kind": chip.kind, "name": chip.label,
                "label": styler.displayLabel(chip), "frame": frame]
    }

    /// The newest open press of `presses`, else the newest closed one's sequence.
    static func latest(_ presses: [T3ComposerChipPress]) -> [String: Any] {
        guard let newest = presses.filter(\.isOpen).max(by: { $0.seq < $1.seq }) else {
            return ["seq": presses.map(\.seq).max() ?? 0, "open": false]
        }
        return newest.state
    }

    // MARK: Geometry

    private func measure() -> [Double]? {
        guard let chip, let styler, let view = styler.view, let rect = styler.chipRect(chip),
              let content = view.window?.contentView else { return nil }
        let box = view.convert(rect, to: content)
        let top = content.isFlipped ? box.minY : content.bounds.height - box.maxY
        let half = { (value: CGFloat) in (Double(value) * 2).rounded() / 2 }
        return [half(box.minX), half(top), half(box.width), half(box.height)]
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

    /// One button element per skill chip, children of the text view.
    func refreshAccessibility() {
        guard let styler, let view = styler.view else { elements = []; return }
        let skills = styler.chips.filter { $0.kind == "skill" }
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
            // realinput-1010c-fixes RC-4: an NSAccessibilityElement reports itself disabled unless told otherwise
            // (`isAccessibilityEnabled` is false by default), so Accessibility Inspector and VoiceOver read the
            // chip's button as dimmed; the reference's PopoverTrigger is an enabled button.
            setAccessibilityEnabled(true)
            setAccessibilityParent(view)
            setAccessibilityLabel("Skill \(press.styler?.displayLabel(chip) ?? chip.label). Show details")
        }

        override func accessibilityFrame() -> NSRect {
            guard let view, let window = view.window, let rect = press?.styler?.chipRect(chip) else { return .zero }
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
