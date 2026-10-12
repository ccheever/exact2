// @ref LLP 1116.007 D4 — `input type="number"` is a field with the platform's
// stepping control, where the web draws its spin buttons: a `UIStepper` at the
// field's trailing edge, inside the input's box, the text between the field's
// leading edge and the stepper. The field stays the editor (direct entry, the
// decimal pad); the stepper steps its text by HTML's rules (`NumberStepping`:
// `step`, `min`, `max`) and says so as the web does, `input` and then `change`
// with the new text. `appearance="textfield"` leaves the bare field; an input
// of any other `type` never has one, whatever its bounds say.
//
// The stepper's own value is only ever the field's, held to `min` and `max`,
// so UIKit dims its minus at the floor and its plus at the top as it does a
// hand-built stepper's; each step is computed from the field's text and the
// value put back, whatever UIKit did to its own.
#if os(iOS)
import UIKit

/// The stepper in a number input: `UIStepper`, which keeps what it last showed.
final class FieldStepper: UIStepper {
    /// The field's value as last written: a change from it is a step up or down.
    var shown = 0.0
}

extension NodeView {
    /// Room kept between the text and the stepper.
    static let stepperGap: CGFloat = 8
    /// The least text a field keeps beside its stepper (about three digits at
    /// the body size): with less room than this and the stepper, the field
    /// keeps priority and shows no stepper rather than a field it squeezes out.
    static let stepperLeastText: CGFloat = 28

    var stepper: FieldStepper? { get { extras?.stepper } set { if newValue != nil || extras != nil { more.stepper = newValue } } }

    /// Whether this input is one the platform steps: a native field of
    /// `type="number"` whose appearance is not `textfield`. Nothing is
    /// inferred from `min`, `max` or `inputmode`.
    var wantsStepper: Bool {
        field != nil && isNativeTextControl && props["type"]?.lowercased() == "number" && style["appearance"]?.string != "textfield"
    }

    /// Whether the stepper has the room to show: the field's own width less
    /// the stepper and its gap still holds `stepperLeastText`.
    private func stepperFits(_ stepper: UIStepper) -> Bool {
        let room = nativeFieldContent?.width ?? bounds.width
        return room - stepper.intrinsicContentSize.width - NodeView.stepperGap >= NodeView.stepperLeastText
    }

    /// What the text gives up at its trailing edge for the stepper, 0 with none shown.
    var stepperRoom: CGFloat {
        guard let stepper, !stepper.isHidden else { return 0 }
        return stepper.intrinsicContentSize.width + NodeView.stepperGap
    }

    /// Makes, updates or removes the stepper as the props, style and field
    /// now ask: called from `applyProps` and `applyStyle`, and as the field's
    /// text changes.
    func updateNumberStepper() {
        guard wantsStepper, let field else {
            if let old = stepper { old.removeFromSuperview(); stepper = nil; field?.setNeedsLayout() }
            return
        }
        let made = stepper ?? {
            let s = FieldStepper()
            s.addTarget(self, action: #selector(stepperChanged(_:)), for: .valueChanged)
            addSubview(s)
            stepper = s
            return s
        }()
        let stepping = NumberStepping(props)
        // Bounds the author did not write are the web's: none. UIKit wants a
        // range, so a very wide one stands for it; a floor the ceiling does
        // not clear (one value only) keeps a step of room above it.
        let wide = 1e15
        let lo = stepping.min.map(NumberStepping.double) ?? -wide
        var hi = stepping.max.map(NumberStepping.double) ?? wide
        let step = NumberStepping.double(stepping.step)
        if hi <= lo { hi = lo + step }
        // Never an invalid pair in between: the new ceiling first when it clears the old floor.
        // UIKit's setters are not free, even to the same value (`assign`).
        if hi > made.minimumValue { assign(made, \.maximumValue, hi); assign(made, \.minimumValue, lo) } else { assign(made, \.minimumValue, lo); assign(made, \.maximumValue, hi) }
        assign(made, \.stepValue, step)
        assign(made, \.autorepeat, true)
        assign(made, \.wraps, false)
        made.shown = NumberStepping.double(stepping.shown(field.text ?? ""))
        assign(made, \.value, made.shown)
        assign(made, \.isEnabled, !disabled && props["editable"] != "false")
        assign(made, \.accessibilityLabel, props["accessibilityLabel"])
        assign(made, \.accessibilityIdentifier, props["testId"].map { "\($0)-stepper" })
        layoutStepper()
    }

    /// The stepper at the trailing edge, centered in the input's box: its
    /// trailing edge where the text's ends (the field's own padding, or the
    /// cell's margin), and the text's room less its width and a gap.
    func layoutStepper() {
        guard let stepper, let field else { return }
        let size = stepper.intrinsicContentSize
        let fits = stepperFits(stepper)
        if stepper.isHidden == fits { stepper.isHidden = !fits; field.setNeedsLayout() }
        let rect = nativeFieldContent ?? bounds
        let rtl = style["direction"]?.string == "rtl"
        let x = rtl ? rect.minX : rect.maxX - size.width
        let frame = CGRect(x: x, y: ((bounds.height - size.height) / 2).rounded(.toNearestOrAwayFromZero), width: size.width, height: size.height)
        if stepper.frame != frame { stepper.frame = frame; field.setNeedsLayout() }
        if stepper.superview === self, subviews.last !== stepper { bringSubviewToFront(stepper) }
    }

    /// A tap of the plus or the minus: the field's text steps one way, and
    /// the app hears an `input` and a `change` with it (HTML's stepping).
    @objc func stepperChanged(_ sender: UIStepper) {
        guard let stepper = sender as? FieldStepper, let field else { return }
        defer { updateNumberStepper() }
        // UIKit says so only when its value moved; one that did not (a half
        // at its limit) steps nothing.
        guard stepper.value != stepper.shown, !disabled, props["editable"] != "false" else { return }
        step(field, up: stepper.value > stepper.shown)
    }

    /// The text of `field` one step up or down; false when there is none to take
    /// (the value is at the bound it steps toward).
    @discardableResult
    func step(_ field: UITextField, up: Bool) -> Bool {
        guard let next = NumberStepping(props).stepped(field.text ?? "", up: up) else { return false }
        field.text = next
        toldText = next
        presenter?.typed(id, next, input: handlers.contains("input"))
        presenter?.commitEdit(id, next, change: handlers.contains("change"))
        return true
    }
}

// The agent's side (LLP 1012: every path is a touch at a place): `layout <id>`
// says where the stepper's halves are in the node's own space, and a `tap <id>
// at <x> <y>` over one steps it as a finger on that half does.
extension NodeView {
    /// `native.stepper` of `layout <id>`: the stepper's box and the middle of
    /// each half, in this node's space (what `tap <id> at <x> <y>` takes),
    /// and what it is set to.
    var stepperObservation: [String: Any]? {
        guard let stepper else { return nil }
        let f = stepper.frame
        let point = { (x: CGFloat, y: CGFloat) in [Agent.r2(x), Agent.r2(y)] }
        return ["frame": [f.minX, f.minY, f.width, f.height].map { Agent.r2($0) },
                "decrement": point(f.minX + f.width / 4, f.midY), "increment": point(f.maxX - f.width / 4, f.midY),
                "value": stepper.value, "min": stepper.minimumValue, "max": stepper.maximumValue, "step": stepper.stepValue,
                "enabled": stepper.isEnabled, "shown": !stepper.isHidden]
    }
}

extension Agent {
    /// A plain press at a point of `node` that a finger would land on its
    /// stepper: the half it is on steps the field, as a touch up inside it
    /// does (`stepperChanged`). Nil when the point is not on one.
    func steppedTap(_ node: NodeView, hit: UIView, at point: CGPoint, tapped at: [Double]) -> [String: Any]? {
        var up: UIView? = hit
        while let view = up, !(view is FieldStepper) { up = view.superview }
        guard let stepper = up as? FieldStepper, stepper.superview === node else { return nil }
        let half = stepper.convert(point, from: nil).x > stepper.bounds.midX ? "increment" : "decrement"
        guard stepper.isEnabled, !stepper.isHidden, !node.disabled, !node.inert else { return ["error": "tap #\(node.id): its stepper is disabled"] }
        let before = node.field?.text ?? ""
        stepper.value = min(max(stepper.value + (half == "increment" ? stepper.stepValue : -stepper.stepValue), stepper.minimumValue), stepper.maximumValue)
        stepper.sendActions(for: .valueChanged)
        let after = node.field?.text ?? ""
        return ["tapped": Int(node.id), "at": at, "stepper": half, "stepped": after != before, "value": after, "delivery": "host-activation", "native": "stepper"]
    }
}
#elseif os(tvOS)
import UIKit

// tvOS has no stepper.
extension NodeView {
    var stepper: UIView? { nil }
    func updateNumberStepper() {}
    func layoutStepper() {}
    var stepperRoom: CGFloat { 0 }
}
#endif
