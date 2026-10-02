// Native buttons on UIKit (LLP 1069.011): a `button appearance="auto"` is a
// `Control` of type `button`, and its control is UIKit's own `UIButton` with
// the `UIButton.Configuration` its `buttonStyles` row names. Its title and
// symbol are the node's face, read from the kernel (`exact_button_face`);
// its children are never views. The control takes the touch and runs the
// node's activation (D4); the node keeps keys and focus; the control is the
// one accessibility element. A glass style's control is the glass body the
// glass-group pass isolates (D9).
#if os(iOS)
import UIKit

/// UIKit's button, as a native button's control: never a UIKit focus item
/// (the node is the focus owner, D4).
final class NativeButtonIOS: UIButton {
    weak var owner: NodeView?
    /// What was last written, so a batch that changes nothing writes nothing.
    struct Written: Equatable {
        var face: ButtonFace
        var accent: UIColor?
        var enabled: Bool
        var label: String?
        var testId: String?
        var selected: Bool
        var expanded: String?
    }
    var written: Written?
    /// Whether it draws glass: a glass row on iOS 26 and later.
    var isGlass = false
    /// The configuration drawn, its name in the table's iOS column.
    var drawn = "bordered"
    override var canBecomeFocused: Bool { false }
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
    /// does, once: the press goes to this node or, without a handler, the
    /// nearest ancestor with one (refused at a disabled one); that node takes
    /// the focus unless a `retainFocus` ancestor keeps the editor's, else the
    /// editor loses it; then `press` and the canvas's pointer return.
    func activateNative() {
        guard let presenter, let target = activationTarget(at: convert(CGPoint(x: bounds.midX, y: bounds.midY), to: nil)),
              presenter.views[target.id] === target else { return }
        let retains = presenter.contextRetainsFocus(target) == true
        if target.canBecomeFirstResponder, !target.isFirstResponder, !retains { _ = target.becomeFirstResponder() }
        if !target.isFirstResponder && !retains { presenter.viewport.endEditing(true) }
        presenter.press(target.id)
        target.finishPointerPress()
    }
}

extension ControlHost {
    func makeNativeButton(_ node: NodeView) -> UIControl {
        let button = NativeButtonIOS(configuration: .bordered())
        button.owner = node
        button.addAction(UIAction { [weak button] _ in button?.owner?.activateNative() }, for: .primaryActionTriggered)
        return button
    }

    /// The `UIButton.Configuration` a face's row names, on this iOS.
    static func configuration(_ face: ButtonFace) -> (UIButton.Configuration, String, Bool) {
        var name = ButtonFace.drawn(face.ios).name
        if #unavailable(iOS 26.0) { name = ButtonFace.drawn(face.iosBefore26).name }
        if #available(iOS 26.0, *) {
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
        let face = presenter.buttonFace?(owner.id) ?? ButtonFace()
        let written = NativeButtonIOS.Written(
            face: face, accent: accent, enabled: !owner.disabled,
            label: owner.props["accessibilityLabel"] ?? face.title, testId: owner.props["testId"],
            selected: owner.props["accessibilitySelected"] == "true", expanded: owner.props["accessibilityExpanded"])
        guard button.written != written else { return }
        if !face.known, button.written?.face.style != face.style {
            presenter.session?.log("buttonStyle `\(face.style)` is not a button style; drawing bordered")
        }
        button.written = written
        var (config, drawn, glass) = Self.configuration(face)
        config.title = face.title
        config.image = face.symbol.flatMap { UIImage(systemName: $0) }
        config.imagePlacement = face.leading ? .leading : .trailing
        config.titleLineBreakMode = .byTruncatingTail
        button.configuration = config
        button.titleLabel?.numberOfLines = 1
        button.tintColor = accent
        button.isEnabled = written.enabled
        button.accessibilityLabel = written.label
        button.accessibilityIdentifier = written.testId
        if written.selected { button.accessibilityTraits.insert(.selected) } else { button.accessibilityTraits.remove(.selected) }
        if #available(iOS 18, *) {
            button.accessibilityExpandedStatus = written.expanded.map { $0 == "true" ? .expanded : .collapsed } ?? .unsupported
        }
        button.drawn = drawn
        if button.isGlass != glass {
            button.isGlass = glass
            owner.syncGlassSlot()
        }
    }

    /// What the agent's `layout` says of a native button (D10).
    func nativeObservation(_ button: NativeButtonIOS) -> [String: Any] {
        let face = button.written?.face ?? ButtonFace()
        return ["view": "UIButton", "style": face.style, "drawn": button.drawn, "title": face.title as Any,
                "symbol": face.symbol as Any, "enabled": button.isEnabled,
                "size": [Agent.r2(button.bounds.width), Agent.r2(button.bounds.height)]]
    }
}
#endif
