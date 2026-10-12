// @ref LLP 1116.007 D4 — `input type="number"` is a field with the platform's
// stepping control, where the web draws its spin buttons: on the Mac an
// `NSStepper` beside the text field, at the input's trailing edge, as a
// hand-built Mac settings pane sets a field and its stepper side by side. The
// field stays the editor (direct entry as before) and gives the stepper its
// width and a gap; the stepper steps the field's text by HTML's rules
// (`NumberStepping`: `step`, `min`, `max`) and says so as the web does,
// `input` and then `change` with the new text. `appearance="textfield"` leaves
// the bare field; an input of any other `type` never has one.
//
// The stepper's own value is only ever the field's, held to `min` and `max`;
// each step is computed from the field's text and the value put back,
// whatever AppKit did to its own.
#if os(macOS)
import AppKit

/// The stepper beside a number input: `NSStepper`, which keeps what it last showed.
final class FieldStepper: NSStepper {
    /// The field's value as last written: a change from it is a step up or down.
    var shown = 0.0
}

extension NodeView {
    /// Room kept between the field and the stepper.
    static let stepperGap: CGFloat = 4
    /// The least width the field keeps beside its stepper: with less room than
    /// this and the stepper, the field keeps priority and shows no stepper.
    static let stepperLeastField: CGFloat = 44

    /// Whether this input is one the platform steps: a native field of
    /// `type="number"` whose appearance is not `textfield`. Nothing is
    /// inferred from `min`, `max` or `inputmode`.
    var wantsStepper: Bool {
        field != nil && isNativeTextControl && props["type"]?.lowercased() == "number" && style["appearance"]?.string != "textfield"
    }

    /// What the field gives up at its trailing end for the stepper, 0 with none shown.
    var stepperRoom: CGFloat {
        guard let stepper, !stepper.isHidden else { return 0 }
        return stepper.fittingSize.width + NodeView.stepperGap
    }

    /// The field's text as it stands, the editor's while it is being edited.
    private var shownFieldText: String {
        (field?.currentEditor() as? NSTextView)?.string ?? field?.stringValue ?? ""
    }

    /// Makes, updates or removes the stepper as the props, style and field now
    /// ask: called from `applyProps` and `applyStyle`, and as the text changes.
    func updateNumberStepper() {
        guard wantsStepper else {
            if let old = stepper { old.removeFromSuperview(); stepper = nil; layoutField() }
            return
        }
        let made = stepper ?? {
            let s = FieldStepper()
            s.target = self
            s.action = #selector(stepperChanged(_:))
            s.autorepeat = true
            s.valueWraps = false
            addSubview(s)
            stepper = s
            return s
        }()
        let stepping = NumberStepping(props)
        // Bounds the author did not write are the web's: none. AppKit wants a
        // range, so a very wide one stands for it; a floor the ceiling does
        // not clear (one value only) keeps a step of room above it.
        let wide = 1e15
        let lo = stepping.min.map(NumberStepping.double) ?? -wide
        var hi = stepping.max.map(NumberStepping.double) ?? wide
        let step = NumberStepping.double(stepping.step)
        if hi <= lo { hi = lo + step }
        if made.minValue != lo { made.minValue = lo }
        if made.maxValue != hi { made.maxValue = hi }
        if made.increment != step { made.increment = step }
        made.shown = NumberStepping.double(stepping.shown(shownFieldText))
        if made.doubleValue != made.shown { made.doubleValue = made.shown }
        let enabled = !disabled && props["editable"] != "false"
        if made.isEnabled != enabled { made.isEnabled = enabled }
        if made.accessibilityLabel() != props["accessibilityLabel"] { made.setAccessibilityLabel(props["accessibilityLabel"]) }
        let identifier = props["testId"].map { "\($0)-stepper" }
        if made.accessibilityIdentifier() != identifier { made.setAccessibilityIdentifier(identifier) }
        layoutField()
    }

    /// The stepper at the input's trailing edge, centered in its box, the
    /// field narrower by its width and a gap (`layoutField` calls this after
    /// it has given the field its whole frame).
    func layoutStepper() {
        guard let stepper, let field else { return }
        let size = stepper.fittingSize
        let fits = field.frame.width - size.width - NodeView.stepperGap >= NodeView.stepperLeastField
        if stepper.isHidden == fits { stepper.isHidden = !fits }
        guard fits else { return }
        let rtl = style["direction"]?.string == "rtl"
        let room = size.width + NodeView.stepperGap
        var frame = field.frame
        if rtl { frame.origin.x += room }
        frame.size.width -= room
        field.frame = frame
        let place = NSRect(x: rtl ? bounds.minX : bounds.maxX - size.width, y: ((bounds.height - size.height) / 2).rounded(), width: size.width, height: size.height)
        if stepper.frame != place { stepper.frame = place }
    }

    /// A click of the up or the down arrow: the field's text steps one way,
    /// and the app hears an `input` and a `change` with it (HTML's stepping).
    @objc func stepperChanged(_ sender: NSStepper) {
        guard let stepper = sender as? FieldStepper else { return }
        let up = stepper.doubleValue > stepper.shown, moved = stepper.doubleValue != stepper.shown
        defer { updateNumberStepper() }
        guard moved, !disabled, props["editable"] != "false" else { return }
        step(up: up)
    }

    /// The text of the field one step up or down; false when there is none to take
    /// (the value is at the bound it steps toward).
    @discardableResult
    func step(up: Bool) -> Bool {
        guard let field, let next = NumberStepping(props).stepped(shownFieldText, up: up) else { return false }
        if let editor = field.currentEditor() as? NSTextView { writeValue(next, into: editor) } else { field.stringValue = next }
        toldText = next
        presenter?.typed(id, next, input: handlers.contains("input"))
        presenter?.commitEdit(id, next, change: handlers.contains("change"))
        return true
    }
}

// The agent's side (LLP 1012: every path is a place a pointer goes): `layout <id>`
// says where the stepper's arrows are in the node's own space, and `tap <id> at
// <x> <y>` over one is a real click on it, which AppKit's stepper answers.
extension NodeView {
    /// `native.stepper` of `layout <id>`: the stepper's box and the middle of
    /// each arrow, in this node's space (what `tap <id> at <x> <y>` takes),
    /// and what it is set to.
    var stepperObservation: [String: Any]? {
        guard let stepper else { return nil }
        let f = stepper.frame
        let point = { (x: CGFloat, y: CGFloat) in [Agent.r2(x), Agent.r2(y)] }
        // The up arrow is the top half of the control, the down arrow the bottom (the node is flipped).
        return ["frame": [f.minX, f.minY, f.width, f.height].map { Agent.r2($0) },
                "increment": point(f.midX, f.minY + f.height / 4), "decrement": point(f.midX, f.maxY - f.height / 4),
                "value": stepper.doubleValue, "min": stepper.minValue, "max": stepper.maxValue, "step": stepper.increment,
                "enabled": stepper.isEnabled, "shown": !stepper.isHidden]
    }
}
#endif
