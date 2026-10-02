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
    /// The last projection decision journaled per tablist, so each is said once.
    private var decisions: [UInt32: String] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Journal why a tablist is or is not a segmented control, once per change.
    private func decide(_ owner: NodeView, _ decision: String) {
        guard decisions[owner.id] != decision else { return }
        decisions[owner.id] = decision
        presenter.session?.log("tablist #\(owner.id): \(decision)")
    }

    private func tabs(in owner: NodeView) -> [NodeView] {
        owner.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.isButton && $0.props["accessibilityRole"] == "tab" && $0.handlers.contains("press")
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
        // @ref LLP 1039 D6 — only explicit vertical tablists opt out; ignore invalid ARIA values.
        let owners = presenter.carrying("role:tablist").filter {
            $0.props["accessibilityOrientation"] != "vertical"
        }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) where !live.contains(id) { restore(owner: id) }
        for id in Array(decisions.keys) where !live.contains(id) { decisions.removeValue(forKey: id) }
        for owner in owners {
            let tabs = tabs(in: owner)
            // Authored tabs a segment cannot show stay as authored (LLP 1035.001 D10).
            let unshown = tabs.first { $0.segmentFace == nil }
            if tabs.count <= 1 || unshown != nil {
                restore(owner: owner.id)
                decide(owner, unshown.map { "kept as authored: tab #\($0.id) is not one image or its label alone, which a segment cannot show" } ?? "kept as authored: fewer than two pressable tabs")
                continue
            }
            decide(owner, "projected to NSSegmentedControl (\(tabs.count) segments)")
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
                if case .image(let icon)? = tab.segmentFace {
                    // Keep the native accessibility description even with no glyph.
                    let image = (icon.image?.copy() as? NSImage) ?? NSImage(size: NSSize(width: 1, height: 1))
                    if icon.bounds.width > 0, icon.bounds.height > 0 { image.size = icon.bounds.size }
                    image.accessibilityDescription = tab.accessibleName
                    control.setImage(image, forSegment: index)
                    control.setImageScaling(.scaleProportionallyDown, forSegment: index)
                    control.setLabel("", forSegment: index)
                    control.setToolTip(tab.accessibleName, forSegment: index)
                } else if case .symbol(let name)? = tab.segmentFace {
                    // A native tab's symbol, carrying its label (LLP 1069.011.000 D4).
                    let image = NSImage(systemSymbolName: name, accessibilityDescription: tab.accessibleName)
                        ?? NSImage(size: NSSize(width: 1, height: 1))
                    image.accessibilityDescription = tab.accessibleName
                    control.setImage(image, forSegment: index)
                    control.setLabel("", forSegment: index)
                    control.setToolTip(tab.accessibleName, forSegment: index)
                } else {
                    control.setImage(nil, forSegment: index)
                    control.setLabel(tab.accessibleName, forSegment: index)
                }
                control.setEnabled(!tab.disabled && !tab.inert, forSegment: index)
            }
            control.selectedSegment = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? -1
            owner.addSubview(control, positioned: .above, relativeTo: nil)
        }
    }

    @objc private func changed(_ sender: ExactSegmentedControl) {
        guard let ids = members[sender.ownerID], ids.indices.contains(sender.selectedSegment),
              let tab = presenter.views[ids[sender.selectedSegment]], !tab.disabled, !tab.inert, !sender.isHiddenOrHasHiddenAncestor else { sync(); return }
        presenter.press(tab.id)
    }

    func activate(_ node: NodeView) -> Bool? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }) else { return nil }
        // @ref LLP 1038 D6 — logical tabs are hidden by this projection;
        // their native control and route ancestors decide availability.
        guard let control = controls[entry.key], control.window != nil,
              !control.isHiddenOrHasHiddenAncestor, !node.disabled, !node.inert else { return false }
        presenter.press(node.id)
        return true
    }

    /// Whether a tab this projection hides is shown through its segment:
    /// `nil` for a view that is not one of its tabs.
    func shown(_ node: NodeView) -> Bool? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }) else { return nil }
        guard let control = controls[entry.key] else { return false }
        return control.window != nil && !control.isHiddenOrHasHiddenAncestor
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
