// @ref LLP 1035.001 D10 — the same tab semantics use AppKit's segmented
// control on macOS; Contract remains the owner of selection and actions.
#if os(macOS)
import AppKit

private final class ExactSegmentedControl: NSSegmentedControl {
    let ownerID: UInt32
    init(ownerID: UInt32) {
        self.ownerID = ownerID
        super.init(frame: .zero)
        trackingMode = .selectOne
    }
    required init?(coder: NSCoder) { nil }
}

final class SegmentHost {
    unowned let presenter: Presenter
    private var controls: [UInt32: ExactSegmentedControl] = [:]
    private var hidden: [UInt32: Bool] = [:]
    private var members: [UInt32: [UInt32]] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    private func tabs(in owner: NodeView) -> [NodeView] {
        owner.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.kind == "button" && $0.props["accessibilityRole"] == "tab" && $0.handlers.contains("press")
        }
    }

    private func restore(owner id: UInt32) {
        for childID in members.removeValue(forKey: id) ?? [] {
            if let child = presenter.views[childID] { child.isHidden = hidden.removeValue(forKey: childID) ?? false }
            else { hidden.removeValue(forKey: childID) }
        }
        controls.removeValue(forKey: id)?.removeFromSuperview()
    }

    func sync() {
        let owners = presenter.views.values.filter { $0.props["accessibilityRole"] == "tablist" }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) where !live.contains(id) { restore(owner: id) }
        for owner in owners {
            let tabs = tabs(in: owner)
            guard tabs.count > 1 else { restore(owner: owner.id); continue }
            let ids = tabs.map(\.id)
            if members[owner.id] != ids {
                restore(owner: owner.id)
                members[owner.id] = ids
                for tab in tabs { hidden[tab.id] = tab.isHidden; tab.isHidden = true }
            } else {
                for tab in tabs { tab.isHidden = true }
            }
            let control = controls[owner.id] ?? {
                let value = ExactSegmentedControl(ownerID: owner.id)
                value.target = self
                value.action = #selector(changed(_:))
                value.autoresizingMask = [.width, .height]
                owner.addSubview(value)
                controls[owner.id] = value
                return value
            }()
            if control.superview !== owner { owner.addSubview(control) }
            control.frame = owner.contentBox()
            control.setAccessibilityLabel(owner.props["accessibilityLabel"])
            control.segmentCount = tabs.count
            for (index, tab) in tabs.enumerated() {
                control.setLabel(tab.props["accessibilityLabel"] ?? "", forSegment: index)
                control.setEnabled(!tab.disabled, forSegment: index)
            }
            control.selectedSegment = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? -1
            owner.addSubview(control, positioned: .above, relativeTo: nil)
        }
    }

    @objc private func changed(_ sender: ExactSegmentedControl) {
        guard let ids = members[sender.ownerID], ids.indices.contains(sender.selectedSegment),
              let tab = presenter.views[ids[sender.selectedSegment]], !tab.disabled else { sync(); return }
        presenter.press(tab.id)
    }

    func activate(_ node: NodeView) -> Bool? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }) else { return nil }
        guard controls[entry.key]?.window != nil, !node.disabled else { return false }
        presenter.press(node.id)
        return true
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }),
              let control = controls[entry.key], let segment = entry.value.firstIndex(of: node.id) else { return nil }
        return ["view": "NSSegmentedControl", "segment": segment,
                "selected": control.selectedSegment, "segments": control.segmentCount]
    }

    func reset() {
        for id in Array(controls.keys) { restore(owner: id) }
    }
}
#endif
