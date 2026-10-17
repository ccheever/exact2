// Native buttons on UIKit (LLP 1069.011): a `button appearance="auto"` is a
// `Control` of type `button`, and its control is UIKit's own `UIButton` with
// the `UIButton.Configuration` its `buttonStyles` row names, or with no
// `buttonStyle` written the platform's own (`ButtonFace.unsaid`, LLP 1115.000
// D15: plain on iOS). Its title and
// symbol are the node's face, read from the kernel (`exact_button_face`);
// its children are never views. The control takes the touch and runs the
// node's activation (D4); the node keeps keys and focus; the control is the
// one accessibility element. A glass style's control is the glass body the
// glass-group pass isolates (D9).
#if os(iOS) || os(tvOS)
import UIKit

/// UIKit's button, as a native button's control: never a UIKit focus item
/// (the node is the focus owner, D4).
final class NativeButtonIOS: UIButton {
    weak var owner: NodeView?
    /// What was last written, so a batch that changes nothing writes nothing.
    struct Written: Equatable {
        var face: ButtonFace
        var accent: UIColor?
        var destructive: Bool
        var enabled: Bool
        var label: String?
        var testId: String?
        var selected: Bool
        var expanded: String?
        var pressed: String?
        var traits: FieldChromeCache.Traits
        var interactive: Bool
        /// The applied padding (`NodeView.kernelPadding`, shared, not copied)
        var padding: [CGFloat?]
    }
    /// A native button sent no padding: every side UIKit's
    static let unsaid: [CGFloat?] = [nil, nil, nil, nil]
    var written: Written?
    /// Its natural size, kept for what it was measured with: UIKit lays the
    /// configuration out again for each ask, and every batch asks.
    private var measured: (written: Written?, traits: [AnyHashable], size: CGSize)?
    var naturalSize: CGSize {
        let t = traitCollection
        let traits: [AnyHashable] = [t.preferredContentSizeCategory, t.legibilityWeight.rawValue, t.displayScale]
        if let measured, measured.written == written, measured.traits == traits { return measured.size }
        let s = intrinsicContentSize
        let size = CGSize(width: ceil(s.width), height: ceil(s.height))
        measured = (written, traits, size)
        return size
    }
    /// Whether it draws glass: a glass row on iOS 26 and later.
    var isGlass = false
    /// A glass button's coloured symbol: its template, its own colour (nil when it is drawn in the tint),
    /// and the alpha the face gives it (its own colour's, or an authored accent's; nil for an inherited tint)
    struct Ink {
        let template: UIImage
        let color: UIColor?
        let alpha: CGFloat?
        /// Drawn in its colour, grey while the tint is dimmed behind a modal, as UIKit greys a button,
        /// keeping its alpha: the face's, else an inherited tint's as last seen undimmed
        func drawn(in button: NativeButtonIOS) -> UIImage {
            let tint: UIColor = button.tintColor
            let ink = button.tintAdjustmentMode == .dimmed ? tint.withAlphaComponent(alpha ?? button.tintAlpha) : color ?? tint
            return template.withTintColor(ink, renderingMode: .alwaysOriginal)
        }
    }
    var symbolInk: Ink?
    /// An inherited tint's alpha while it is not dimmed: UIKit's dimmed tint is a grey that keeps none
    /// of it, which a symbol drawn in the tint keeps behind a modal (one that changes while dimmed cannot
    /// be read until the tint is restored)
    private(set) var tintAlpha: CGFloat = 1
    func noteTint() {
        if tintAdjustmentMode != .dimmed { tintAlpha = tintColor.cgColor.alpha }
    }
    override func tintColorDidChange() {
        super.tintColorDidChange()
        noteTint()
        guard let symbolInk, var config = configuration else { return }
        config.image = symbolInk.drawn(in: self)
        configuration = config
    }
    /// The configuration drawn, its name in the table's iOS column.
    var drawn = ButtonFace.unsaid
    override var canBecomeFocused: Bool {
        #if os(tvOS)
        return isEnabled && isUserInteractionEnabled && owner?.inert != true && owner?.cssVisibilityHidden != true
            && (owner?.explicitTabIndex ?? 0) >= 0
        #else
        return false
        #endif
    }
    #if os(tvOS)
    override func didUpdateFocus(in context: UIFocusUpdateContext, with coordinator: UIFocusAnimationCoordinator) {
        super.didUpdateFocus(in: context, with: coordinator)
        guard let owner, let presenter = owner.presenter else { return }
        if context.nextFocusedItem === self {
            presenter.focusKey = owner.props["testId"]
            presenter.focusGuides.focused(owner)
            presenter.focus(owner.id)
        }
        if context.previouslyFocusedItem === self { presenter.blur(owner.id) }
    }
    #endif
}

extension NodeView {
    /// A `button appearance="auto"` (LLP 1069.011 D3).
    var isNativeButton: Bool { kind == "control" && props["type"] == "button" }

    /// Where a native control sits: the glass slot's content when the
    /// glass-group pass made one (D9), else the node.
    var controlMount: UIView { glassSlot?.contentView ?? self }

    /// Its face's title, as its control shows it.
    var nativeTitle: String? { (presenter?.controls.controls[id] as? NativeButtonIOS)?.written?.face.title }

    /// The native glass button this node shows, the glass body the
    /// glass-group pass isolates (D9); nil for anything else.
    var nativeGlassBody: UIView? {
        guard isNativeButton, let b = presenter?.controls.controls[id] as? NativeButtonIOS, b.isGlass else { return nil }
        return b
    }

    /// D4: the control's primary action is what a custom button's touch-up
    /// does, once: the target is this node or, without a handler, the
    /// nearest ancestor with one whose box holds the touch (refused at a
    /// disabled one), resolved first; then the press's focus
    /// (`focusForPress`), and `press` when there is a target.
    func activateNative() {
        guard let presenter, !disabled, !inert else { return }
        let target = activationTarget(at: convert(CGPoint(x: bounds.midX, y: bounds.midY), to: nil))
        let bubbled = target.map { presenter.pressBox(of: $0, from: self) !== $0 } ?? false
        let path = target.map { PressPath(from: self, to: $0, presenter: presenter) }, heldTarget = target.map(HeldNode.init)
        let lifetimes = bubbled ? [] : presenter.pressLifetimes(from: self, to: target)
        let handoff = focusForPress(target: target)
        defer { handoff?.dispatched() }
        guard !bubbled || path?.isLive(in: presenter) == true,
              lifetimes.allSatisfy({ $0() }), let target = heldTarget?.live(in: presenter), target.takesPress(bubbled: bubbled) else { return }
        presenter.press(target.id, bubbled: bubbled)
    }
}

extension ControlHost {
    func makeNativeButton(_ node: NodeView) -> UIControl {
        // Its face is read and applied before it shows; until then it is the
        // button nobody styled.
        let button = NativeButtonIOS(configuration: Self.configuration(ButtonFace()).0)
        button.owner = node
        // The node is not an element. UIKit leaves this default false until
        // assistive technology loads its runtime, so expose the control's
        // identity and explicit name from its first frame as on a bare button.
        button.isAccessibilityElement = true
        button.accessibilityTraits.insert(.button)
        button.addAction(UIAction { [weak button] _ in button?.owner?.activateNative() }, for: .primaryActionTriggered)
        return button
    }

    /// The `UIButton.Configuration` a face's row names, on this iOS.
    static func configuration(_ face: ButtonFace) -> (UIButton.Configuration, String, Bool) {
        // Before iOS 26, or in an app that keeps the design before it
        // (`LinkedDesign`), the table's earlier column: UIKit draws a glass
        // configuration there as a bordered button.
        var current = false
        if #available(iOS 26.0, tvOS 26.0, *) { current = LinkedDesign.liquidGlass }
        let name = ButtonFace.drawn(current ? face.ios : face.iosBefore26).name
        if #available(iOS 26.0, tvOS 26.0, *) {
            switch name {
            case "glass": return (.glass(), name, true)
            case "prominentGlass": return (.prominentGlass(), name, true)
            case "clearGlass": return (.clearGlass(), name, true)
            case "prominentClearGlass": return (.prominentClearGlass(), name, true)
            default: break
            }
        }
        switch name {
        case "plain": return (.plain(), name, false)
        case "gray": return (.gray(), name, false)
        case "tinted": return (.tinted(), name, false)
        case "filled": return (.filled(), name, false)
        case "borderless": return (.borderless(), name, false)
        case "borderedTinted": return (.borderedTinted(), name, false)
        case "borderedProminent": return (.borderedProminent(), name, false)
        default: return (.bordered(), "bordered", false)
        }
    }

    /// Its configuration, face, accent, enabled state and accessibility,
    /// written only when one of them changes (a rewrite each batch would
    /// restart UIKit's own animations).
    func configureNative(_ button: NativeButtonIOS, _ owner: NodeView, accent: UIColor?) {
        let face = self.face(owner.id)
        // A destructive button draws in the platform's destructive red, as a hand-built screen's does (SwiftUI's
        // `Button(role: .destructive)`, UIKit's red tint): red ink, or a red fill under white ink on a filled
        // style (LLP 1116.008, team-kanban: "Delete is not styled as destructive"). The accent it only inherits
        // leaves it red, as it leaves a swipe action's; an accent written on the button itself, and a colour
        // written on its title, win (LLP 1115: what the author writes wins).
        let destructive = owner.props["destructive"] == "true" && !owner.wrote("accent_color")
        let accent = destructive ? UIColor.systemRed : Self.platformAccent(owner, accent, in: button.traitCollection)
        let written = NativeButtonIOS.Written(
            face: face, accent: accent, destructive: destructive, enabled: !owner.disabled,
            label: owner.authoredLabel ?? face.title, testId: owner.props["testId"],
            // A checked radio is selected, as Safari's VoiceOver reads one (`setAccessibilityChecked`).
            selected: owner.props["accessibilitySelected"] == "true" || owner.checkedRole.map { $0.role == "radio" && $0.checked } == true,
            expanded: owner.props["accessibilityExpanded"],
            pressed: owner.pressedState, traits: .init(button.traitCollection),
            interactive: owner.style["pointer_events"]?.string != "none",
            padding: owner.kernelPadding ?? NativeButtonIOS.unsaid)
        guard button.written != written else { return }
        if !face.known, button.written?.face.style != face.style {
            presenter.session?.log("buttonStyle `\(face.style ?? "")` is not a button style; drawing bordered")
        }
        button.written = written
        let (_, drawn, glass) = Self.configuration(face)
        button.isEnabled = written.enabled
        button.isUserInteractionEnabled = written.interactive
        ButtonConfigurationIOS.apply(face, to: button, traits: button.traitCollection, accent: accent, destructive: destructive,
                                     padding: written.padding)
        button.accessibilityLabel = written.label
        button.accessibilityIdentifier = written.testId
        if written.selected { button.accessibilityTraits.insert(.selected) } else { button.accessibilityTraits.remove(.selected) }
        button.setAccessibilityToggle(written.pressed)
        if #available(iOS 18, tvOS 18, *) {
            button.accessibilityExpandedStatus = written.expanded.map { $0 == "true" ? .expanded : .collapsed } ?? .unsupported
        }
        button.drawn = drawn
        if button.isGlass != glass {
            button.isGlass = glass
            owner.syncGlassSlot()
        }
    }

    /// A system accent (`-exact-system-teal`) as UIKit's own dynamic colour,
    /// where it resolves in the button's traits as the node draws it (a
    /// subtree's forced scheme keeps the node's resolution): under Increase
    /// Contrast a filled style draws UIKit's system colour at its
    /// high-contrast value, and darkens a fixed colour once more, so the
    /// resolved value read darker than a hand-built button (LLP 1116.019 D3).
    static func platformAccent(_ owner: NodeView, _ resolved: UIColor?, in traits: UITraitCollection) -> UIColor? {
        guard let resolved, let system = SystemColor.native(owner.style["accent_color"]) else { return resolved }
        var a: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0), b = a
        guard system.resolvedColor(with: traits).getRed(&a.0, green: &a.1, blue: &a.2, alpha: &a.3),
              resolved.getRed(&b.0, green: &b.1, blue: &b.2, alpha: &b.3) else { return resolved }
        let near = abs(a.0 - b.0) < 0.01 && abs(a.1 - b.1) < 0.01 && abs(a.2 - b.2) < 0.01 && abs(a.3 - b.3) < 0.01
        return near ? system : resolved
    }

    /// What the agent's `layout` says of a native button (D10).
    func nativeObservation(_ button: NativeButtonIOS) -> [String: Any] {
        let face = button.written?.face ?? ButtonFace()
        return ["view": "UIButton", "style": face.style as Any, "drawn": button.drawn, "title": face.title as Any,
                "symbol": face.symbol as Any, "enabled": button.isEnabled,
                "subtitle": face.subtitle as Any, "imagePlacement": face.placement ?? (face.leading ? "leading" : "trailing"),
                "pointerEvents": button.isUserInteractionEnabled ? "auto" : "none",
                "rows": ButtonConfigurationIOS.observation(face, button: button),
                "size": [Agent.r2(button.bounds.width), Agent.r2(button.bounds.height)]]
    }
}
#endif
