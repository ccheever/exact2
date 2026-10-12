// Native buttons on AppKit (LLP 1069.011): a `button appearance="auto"` is a
// `Control` of type `button`, and its control is AppKit's own `NSButton`,
// its look the `buttonStyles` row's macOS column (`borderless`, `push`,
// `push-accent`, `glass`, `glass-accent`). Its title and symbol are the
// node's face, read from the kernel (`exact_button_face`). The button takes
// the click and runs the node's activation (D4); it is the focus owner and
// relays focus and keys to its node (LLP 1104 D6), and a click on it leaves
// the first responder where it is (LLP 1115.000 D7.1). It is the one
// accessibility element. A glass look's button is the glass body the
// glass-group pass isolates (D9).
#if os(macOS)
import AppKit

/// A native button's cell: AppKit ignores the button, so the cell is the
/// accessibility element VoiceOver lands on. Its show menu (AXShowMenu) is
/// its node's, offered where the node offers it (X84): a `contextPopover`.
final class NativeButtonCellMac: NSButtonCell {
    private var owner: NodeView? { (controlView as? NativeButtonMac)?.owner }
    override func accessibilityPerformShowMenu() -> Bool { owner?.accessibilityPerformShowMenu() ?? false }
    override func isAccessibilitySelectorAllowed(_ selector: Selector) -> Bool {
        selector == #selector(accessibilityPerformShowMenu) ? owner?.isAccessibilitySelectorAllowed(selector) == true : super.isAccessibilitySelectorAllowed(selector)
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute.rawValue == "AXEnabled", owner?.ariaUnavailable == true { return false }
        return super.accessibilityAttributeValue(attribute)
    }
}

/// AppKit's button, as a native button's control.
final class NativeButtonMac: NSButton {
    override class var cellClass: AnyClass? { get { NativeButtonCellMac.self } set {} }
    weak var owner: NodeView?
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute.rawValue == "AXEnabled", owner?.ariaUnavailable == true { return false }
        return super.accessibilityAttributeValue(attribute)
    }
    struct Written: Equatable {
        var face: ButtonFace
        var accent: NSColor?
        var destructive: Bool
        var red: Bool
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
    // @ref LLP 1104 D6; LLP 1115.000 D7.2 — it takes the focus an action or
    // `autofocus` gives it whatever Keyboard Navigation says, and is a Tab
    // stop as that setting has an `NSButton` (`NodeView.tabStop`).
    override var acceptsFirstResponder: Bool {
        guard let owner else { return false }
        return !refusesFirstResponder && isEnabled && !owner.formDisabled && !owner.inert
            && !isHiddenOrHasHiddenAncestor && !owner.cssVisibilityHidden
    }
    override var canBecomeKeyView: Bool { acceptsFirstResponder && owner?.tabStop == true }
    /// A click gives the button the focus only where its author wrote a
    /// `tabindex` (LLP 1115.000 D7.1: what the author writes wins, as on iOS,
    /// where a touch focuses no other button), and not under `retainFocus`.
    /// AppKit's window focuses a clicked view that accepts first responder
    /// and needs its panel key, before the mouse-down; an `NSButton` answers
    /// false, which is why a click on one moves no focus.
    override var needsPanelToBecomeKey: Bool { owner.map { $0.explicitTabIndex != nil && !$0.retainsFocus } ?? false }
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
            owner.presenter?.tab(from: self, back: event.modifierFlags.contains(.shift), in: window)
            return
        }
        // Space presses the focused control. Return is the default button's
        // (LLP 1115.000 D7.3): a dialog's took it before it came here
        // (`DialogHost.key`), and a merely focused button does nothing with it.
        if name == " " {
            performClick(nil)
            return
        }
        if name == "Enter" { return }
        super.keyDown(with: event)
    }
    /// Return's press of a dialog's default button: AppKit's own click, its
    /// flash included. No press moves the focus (`activateNative`).
    func pressAsDefault() { performClick(nil) }
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard written?.interactive != false else { return nil }
        return super.hitTest(point)
    }
    // `pointerdown`/`pointerup` (LLP 1005 §Events): AppKit's tracking loop
    // takes the button's mouse events, so the node hears them here. The up
    // goes before the action the loop sends, DOM's order.
    override func mouseDown(with event: NSEvent) {
        // A Control-click that a `contextPopover` or `contextmenu` from its
        // node up answers is no click (X85).
        if contextClicked(event) { return }
        owner?.pointerPressed(event)
        // A click leaves the first responder where it is (LLP 1115.000
        // D7.1): AppKit's tracking must not take the editor beside it. The
        // button accepts first responder for Tab, `focus()` and `autofocus`.
        refusesFirstResponder = !needsPanelToBecomeKey
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
    /// one), as iOS resolves it. The focus stays where it is, for a click,
    /// for Space on the focused button and for Return's press of a default
    /// button alike (LLP 1115.000 D7.1): the action's own `focus()` is what
    /// moves it. Then `press` and the canvas's pointer return.
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
        guard let target, presenter.views[target.id] === target else { return }
        presenter.press(target.id)
        target.finishPointerPress()
    }
}

extension NodeView {
    /// Whether the author wrote the style row `key` on this node (it is false of a value the node only
    /// inherits and of a row the compiler implied), as iOS reads it.
    func wrote(_ key: String) -> Bool {
        guard case .array(let rows)? = style["written"] else { return false }
        return rows.contains(.string(key))
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
        guard let button = sender as? NativeButtonMac else { return }
        button.owner?.activateNative()
    }

    /// Its look, face, accent, enabled state and accessibility, written only
    /// when one of them changes.
    func configureNative(_ button: NativeButtonMac, _ owner: NodeView, accent: NSColor?) {
        let face = presenter.buttonFace?(owner.id) ?? ButtonFace()
        // A destructive button is the destructive role AppKit draws (`hasDestructiveAction`), in the platform's red as
        // a hand-built pane's is: the accent it only inherits leaves it red, as on iOS, and an accent written on the
        // button itself wins over the red (LLP 1116.008; LLP 1115: what the author writes wins).
        let destructive = owner.props["destructive"] == "true"
        let red = destructive && !owner.wrote("accent_color")
        let accent = red ? NSColor.systemRed : accent
        let written = NativeButtonMac.Written(
            face: face, accent: accent, destructive: destructive, red: red, enabled: !owner.disabled,
            label: owner.authoredLabel ?? face.title, testId: owner.props["testId"],
            selected: owner.props["accessibilitySelected"] == "true" || owner.checkedRole.map { $0.role == "radio" && $0.checked } == true,
            expanded: owner.props["accessibilityExpanded"],
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
        let (look, glass) = ButtonConfigurationMac.apply(face, to: button, appearance: owner.effectiveAppearance, accent: accent, destructive: destructive, red: red)
        button.isEnabled = written.enabled
        button.setAccessibilityLabel(written.label)
        button.setAccessibilityIdentifier(written.testId)
        if button.toolTip != written.toolTip { button.toolTip = written.toolTip }
        button.setAccessibilitySelected(written.selected)
        if let expanded = written.expanded { button.setAccessibilityExpanded(expanded == "true") }
        button.setAccessibilityToggle(written.pressed, else: .button)
        // A `radio` is AXRadioButton, its value whether it is checked (a radio group's segment kept as authored).
        if let checked = owner.checkedRole, checked.role == "radio" { button.setAccessibilityChecked(checked.role, checked.checked) }
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
