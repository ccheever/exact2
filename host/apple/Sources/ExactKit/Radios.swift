// `input type="radio"` (x2apps survey #2), projected as the checkbox is:
// AppKit's radio button, and on UIKit (which has none) a circle drawn as
// Safari iOS draws one (`ExactRadio`). Exact owns the group, never the
// platform: AppKit groups radio buttons that share a superview and an action,
// and these live one per node, so the group is HTML's — every radio of one
// non-empty `name` in the window, in tree order (`exact_radio_group`, the
// kernel's `radio_group`). A click checks at once and unchecks the rest,
// fires `input` then `change` with the radio's value, and then the whole
// group shows its bound `checked` again (LLP 1069.001 D4's snap-back).
import Foundation
#if canImport(AppKit)
import AppKit
#else
import UIKit
#endif

/// A radio's group as the kernel reads it (`exact_radio_group`): its
/// radios in tree order, itself included, and the enabled radio each arrow
/// moves the check to.
struct RadioGroup: Equatable {
    var group: [UInt32] = []
    var next: UInt32?
    var previous: UInt32?

    init(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return }
        group = (o["group"] as? [Int] ?? []).map { UInt32($0) }
        next = (o["next"] as? Int).map { UInt32($0) }
        previous = (o["previous"] as? Int).map { UInt32($0) }
    }
}

extension NodeView {
    /// An `input type="radio"`: a control node HTML lets the person focus.
    var isRadio: Bool { kind == "control" && props["type"] == "radio" }
    /// HTML's one Tab stop per radio group: its checked radio, else (none
    /// checked) its first enabled one.
    var radioTabStop: Bool {
        guard isRadio, !formDisabled, let controls = presenter?.controls else { return false }
        let members = controls.radioMembers(id)
        if members.contains(where: controls.radioShows) { return controls.radioShows(id) }
        return members.first { presenter?.views[$0]?.formDisabled == false } == id
    }
}

extension ControlHost {
    /// The radios of `id`'s group, itself alone when the kernel has none.
    func radioMembers(_ id: UInt32) -> [UInt32] {
        let group = radioGroup?(id).group ?? []
        return group.isEmpty ? [id] : group
    }

    /// Whether radio `id` shows checked: its control's state, else its bound one.
    func radioShows(_ id: UInt32) -> Bool {
        if let control = controls[id] { return radioOn(control) }
        return presenter.views[id]?.props["checked"] == "true"
    }

    /// A click on radio `id`, the host's own and the agent's alike: an
    /// unchecked enabled radio is checked and the rest of its group unchecked
    /// at once, then `input` and `change` carry its value (HTML's, `on` when
    /// it has none); a checked one fires nothing. True when it was checked.
    @discardableResult
    func checkRadio(_ id: UInt32) -> Bool {
        guard let node = presenter.views[id], node.isRadio, !node.formDisabled, !node.inert, !radioShows(id) else { return false }
        for other in radioMembers(id) where other != id { showRadio(other, false) }
        showRadio(id, true)
        presenter.controlValue(id, node.props["value"] ?? "on", input: true, change: true)
        snapRadios()
        return true
    }

    /// After the action's commit every bound radio shows its `checked`
    /// again: an action that wrote nothing unchecks the clicked radio and
    /// rechecks the one before. A radio with no `checked` keeps its own.
    func snapRadios() {
        for (id, kind) in kinds where kind == "radio" {
            if let on = presenter.views[id]?.props["checked"] { showRadio(id, on == "true") }
        }
    }

    /// A key's default action at a focused radio, after its `key` handlers
    /// (none prevented it): ArrowDown and ArrowRight move the focus and the
    /// check to the next enabled radio of the group in tree order, ArrowUp
    /// and ArrowLeft to the previous, wrapping; Space checks it. True when
    /// the key was the radio's.
    func radioKey(_ node: NodeView, _ name: String, held: String) -> Bool {
        guard node.isRadio, !node.formDisabled, !node.inert, held.isEmpty || held == "Shift+" else { return false }
        switch name {
        case " ":
            checkRadio(node.id)
            return true
        case "ArrowDown", "ArrowRight", "ArrowUp", "ArrowLeft":
            let group = radioGroup?(node.id)
            let forward = name == "ArrowDown" || name == "ArrowRight"
            guard let to = (forward ? group?.next : group?.previous).flatMap({ presenter.views[$0] }) else { return true }
            focusRadio(to, keyboard: true)
            checkRadio(to.id)
            return true
        default:
            return false
        }
    }

    /// The agent's `type <radio> true`: checked as a click checks it. A
    /// radio is never typed `false`: HTML unchecks one only by checking
    /// another of its group.
    func typeRadio(_ node: NodeView, _ value: String) -> [String: Any] {
        guard value == "true" else {
            return ["error": value == "false" ? "radio \(node.id) cannot be typed false: a radio is unchecked by checking another of its group"
                : "radio \(node.id) takes true, not \"\(value)\""]
        }
        if !radioShows(node.id), activate(node) != true { return ["error": "control #\(node.id) is disabled, inert or not shown"] }
        return ["typed": Int(node.id), "checked": radioShows(node.id), "delivery": "host-activation", "native": "control"]
    }
}
