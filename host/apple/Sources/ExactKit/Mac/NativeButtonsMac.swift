// Native buttons on AppKit (LLP 1069.011): a `button appearance="auto"` is a
// `Control` of type `button`, and its control is AppKit's own `NSButton`,
// its look the `buttonStyles` row's macOS column (`borderless`, `push`,
// `push-accent`, `glass`, `glass-accent`). Its title and symbol are the
// node's face, read from the kernel (`exact_button_face`). The button takes
// the click and runs the node's activation (D4); it is the focus owner and
// relays focus and keys to its node (LLP 1104 D6). It is the one accessibility element. A
// glass look's button is the glass body the glass-group pass isolates (D9).
#if os(macOS)
import AppKit

/// AppKit's button, as a native button's control.
final class NativeButtonMac: NSButton {
    weak var owner: NodeView?
    struct Written: Equatable {
        var face: ButtonFace
        var accent: NSColor?
        var enabled: Bool
        var label: String?
        var testId: String?
        var selected: Bool
        var expanded: String?
        var pressed: String?
        var appearance: String
        var interactive: Bool
        var toolTip: String?
    }
    var written: Written?
    /// Whether it draws glass: the glass bezel, in the macOS 26 design.
    var isGlass = false
    /// The look drawn, its name in the table's macOS column.
    var drawn = "push"
    /// The whole grouped row remains the authored slot; the native face
    /// begins 16 points inside it, matching the synchronous fitting answer.
    @discardableResult func layout(in box: CGRect) -> CGRect {
        let leading: CGFloat = owner?.props["groupedRowSeparator"] != nil ? 16 : 0
        let content = CGRect(x: box.minX + leading, y: box.minY,
                             width: max(0, box.width - leading), height: box.height)
        let frame = frame(forAlignmentRect: content)
        if self.frame != frame { self.frame = frame }
        return content
    }

    // @ref LLP 1104 D6 — independent of AppKit's Keyboard navigation setting.
    override var acceptsFirstResponder: Bool {
        guard let owner else { return false }
        return !refusesFirstResponder && isEnabled && !owner.formDisabled && !owner.inert
            && !isHiddenOrHasHiddenAncestor && !owner.cssVisibilityHidden
    }
    override var canBecomeKeyView: Bool { acceptsFirstResponder && owner?.tabbable == true }
    override func becomeFirstResponder() -> Bool {
        guard acceptsFirstResponder else { return false }
        let ok = super.becomeFirstResponder()
        if ok { owner?.focusEntered() }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { owner?.focusLeft() }
        return ok
    }
    override func keyDown(with event: NSEvent) {
        guard acceptsFirstResponder, let owner else { return }
        let name = NodeView.keyName(event)
        if name == "Tab", event.modifierFlags.intersection([.command, .control, .option]).isEmpty, let window {
            owner.presenter?.flushKeyViewLoop()
            if event.modifierFlags.contains(.shift) { window.selectPreviousKeyView(self) }
            else { window.selectNextKeyView(self) }
            return
        }
        if name == " " || name == "Enter" {
            performClick(nil)
            return
        }
        super.keyDown(with: event)
    }
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard written?.interactive != false else { return nil }
        return super.hitTest(point)
    }
    // `pointerdown`/`pointerup` (LLP 1005 §Events): AppKit's tracking loop
    // takes the button's mouse events, so the node hears them here. The up
    // goes before the action the loop sends, DOM's order.
    override func mouseDown(with event: NSEvent) {
        owner?.pointerPressed(event)
        // AppKit's tracking must not steal a retainFocus ancestor's editor.
        refusesFirstResponder = owner?.retainsFocus == true
        defer { refusesFirstResponder = false }
        super.mouseDown(with: event)
        owner?.pointerReleased(nil)
    }
    override func sendAction(_ action: Selector?, to target: Any?) -> Bool {
        owner?.pointerReleased(nil)
        return super.sendAction(action, to: target)
    }
}

extension NodeView {
    /// A `button appearance="auto"` (LLP 1069.011 D3).
    var isNativeButton: Bool { kind == "control" && props["type"] == "button" }

    /// Where a native control sits: the isolation container's content when
    /// the glass-group pass made one (D9), else the node.
    var controlMount: NSView {
        if #available(macOS 26.0, *), let isolation = glassIsolation as? NSGlassEffectContainerView, let content = isolation.contentView {
            return content
        }
        return self
    }

    /// Its face's title, as its control shows it.
    var nativeTitle: String? { (presenter?.controls.controls[id] as? NativeButtonMac)?.written?.face.title }

    /// The native glass button this node shows, the glass body the
    /// glass-group pass isolates (D9); nil for anything else.
    var nativeGlassBody: NSView? {
        guard isNativeButton, let b = presenter?.controls.controls[id] as? NativeButtonMac, b.isGlass else { return nil }
        return b
    }

    /// D4: the button's action is what a custom button's click does, once.
    /// The target is this node or, without a handler, the nearest ancestor
    /// with one whose box holds the click (refused at a disabled or inert
    /// one), as iOS resolves it. The clicked button keeps its single focus
    /// owner while the action bubbles, unless a `retainFocus` ancestor
    /// preserves the previous owner. Then `press` and the canvas's pointer return.
    func activateNative() {
        guard let presenter, !disabled, !inert else { return }
        let click = convert(NSPoint(x: bounds.midX, y: bounds.midY), to: nil)
        var target: NodeView?
        var at: NSView? = self
        while let view = at {
            if let node = view as? NodeView {
                if node.disabled || node.inert { break }
                // Its own command is a press as its handler is (a close row in a dialog).
                if node.pressable {
                    if node.bounds.contains(node.convert(click, from: nil)) { target = node }
                    break
                }
            }
            at = view.superview
        }
        // Keep the innermost eligible owner's focus while the press bubbles.
        // A handler on an ancestor must not become a second stop for this button.
        if !retainsFocus {
            let responder = presenter.keyView(of: self)
            if responder.acceptsFirstResponder { window?.makeFirstResponder(responder) }
            else if target != nil { window?.makeFirstResponder(nil) }
        }
        guard let target, presenter.views[target.id] === target else { return }
        presenter.press(target.id)
        target.finishPointerPress()
    }
}

extension ControlHost {
    func makeNativeButton(_ node: NodeView) -> NSControl {
        let button = NativeButtonMac(title: "", target: nil, action: nil)
        button.owner = node
        button.refusesFirstResponder = false
        button.focusRingType = .default
        return button
    }

    @objc func nativePressed(_ sender: NSControl) {
        (sender as? NativeButtonMac)?.owner?.activateNative()
    }

    /// Its look, face, accent, enabled state and accessibility, written only
    /// when one of them changes.
    func configureNative(_ button: NativeButtonMac, _ owner: NodeView, accent: NSColor?) {
        let face = presenter.buttonFace?(owner.id) ?? ButtonFace()
        let written = NativeButtonMac.Written(
            face: face, accent: accent, enabled: !owner.disabled,
            label: owner.authoredLabel ?? face.title, testId: owner.props["testId"],
            selected: owner.props["accessibilitySelected"] == "true", expanded: owner.props["accessibilityExpanded"],
            pressed: owner.pressedState, appearance: owner.effectiveAppearance.name.rawValue,
            interactive: owner.style["pointer_events"]?.string != "none",
            // HTML's `title`, else an icon-only button's label: AppKit's help
            // tag, as a toolbar item's label is (LLP 1115 wave 1).
            toolTip: owner.props["title"] ?? (face.title == nil ? owner.authoredLabel : nil))
        guard button.written != written else { return }
        if !face.known, button.written?.face.style != face.style {
            presenter.session?.log("buttonStyle `\(face.style)` is not a button style; drawing bordered")
        }
        button.written = written
        let (look, glass) = ButtonConfigurationMac.apply(face, to: button, appearance: owner.effectiveAppearance, accent: accent)
        button.isEnabled = written.enabled
        button.setAccessibilityLabel(written.label)
        button.setAccessibilityIdentifier(written.testId)
        if button.toolTip != written.toolTip { button.toolTip = written.toolTip }
        button.setAccessibilitySelected(written.selected)
        if let expanded = written.expanded { button.setAccessibilityExpanded(expanded == "true") }
        button.setAccessibilityToggle(written.pressed, else: .button)
        button.drawn = look
        if button.isGlass != glass {
            button.isGlass = glass
            owner.syncGlassSlot()
        }
    }

    /// What the agent's `layout` says of a native button (D10).
    func nativeObservation(_ button: NativeButtonMac) -> [String: Any] {
        let face = button.written?.face ?? ButtonFace()
        let rows = ButtonConfigurationMac.observation(face, button: button)
        var standIns = rows.compactMapValues { ($0 as? [String: Any])?["standIn"] as? String }
        let mapped = ButtonFace.drawn(face.macos)
        if mapped.standIn || mapped.name != button.drawn {
            standIns["buttonStyle"] = "AppKit draws \(button.drawn) for \(face.style)"
        }
        if !button.isEnabled, button.written?.accent != nil, button.bezelColor != nil {
            standIns["accent-color"] = "AppKit applies its disabled bezel tint despite the authored accent"
        }
        return ["view": "NSButton", "style": face.style, "drawn": button.drawn, "title": face.title as Any,
                "symbol": face.symbol as Any, "enabled": button.isEnabled,
                "rows": rows, "standIns": standIns, "imagePosition": button.imagePosition.rawValue,
                "pointerEvents": button.written?.interactive == false ? "none" : "auto",
                "size": [Agent.r2(button.frame.width), Agent.r2(button.frame.height)]]
    }
}
#endif
