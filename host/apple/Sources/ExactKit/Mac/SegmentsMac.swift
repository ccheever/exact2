// @ref LLP 1035.001 D10 — the same tab semantics use AppKit's segmented
// control on macOS; Contract remains the owner of selection and actions.
// The control fills the tablist's content box and reports its native height
// to layout as the tablist's automatic minimum, as on iOS (LLP 1059 D2a).
// @ref LLP 1116.004 D5.2 — a radio group of radios is the same control.
#if os(macOS)
import AppKit

private final class SegmentAccessibility: NSAccessibilityElement {
    weak var control: ExactSegmentedControl?
    let index: Int
    init(_ control: ExactSegmentedControl, _ index: Int) {
        self.control = control
        self.index = index
        super.init()
    }
    private var native: NSAccessibilityElement? { control?.nativeSegment(at: index) }
    private var enabled: Bool {
        guard let control, index < control.segmentCount else { return false }
        return control.isEnabled(forSegment: index)
            && !(control.accessibilityDisabledSegments.indices.contains(index) && control.accessibilityDisabledSegments[index])
    }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] { native?.accessibilityAttributeNames() ?? [] }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute == .enabled { return enabled }
        if attribute == .parent { return control }
        return native?.accessibilityAttributeValue(attribute)
    }
    override func accessibilityActionNames() -> [NSAccessibility.Action] { native?.accessibilityActionNames() ?? [] }
    override func accessibilityPerformAction(_ action: NSAccessibility.Action) { native?.accessibilityPerformAction(action) }
    override func accessibilityRole() -> NSAccessibility.Role? { native?.accessibilityRole() }
    override func accessibilitySubrole() -> NSAccessibility.Subrole? { native?.accessibilitySubrole() }
    override func accessibilityLabel() -> String? { native?.accessibilityLabel() }
    override func accessibilityHelp() -> String? { native?.accessibilityHelp() }
    override func accessibilityValue() -> Any? { native?.accessibilityValue() }
    override func accessibilityFrame() -> NSRect { native?.accessibilityFrame() ?? .zero }
    override func isAccessibilitySelected() -> Bool { native?.isAccessibilitySelected() ?? false }
    override func isAccessibilityEnabled() -> Bool { enabled }
    override func accessibilityParent() -> Any? { control }
    override func accessibilityPerformPress() -> Bool { native?.accessibilityPerformPress() ?? false }
}

private final class ExactSegmentedControl: NSSegmentedControl {
    let ownerID: UInt32
    var accessibilityDisabledSegments: [Bool] = []
    private var segmentElements: [SegmentAccessibility] = []
    private var exposingNativeChildren = false
    init(ownerID: UInt32) {
        self.ownerID = ownerID
        super.init(frame: .zero)
        trackingMode = .selectOne
    }
    required init?(coder: NSCoder) { nil }
    // As the other native controls (X85): a Control-click a `contextmenu`
    // above answers never starts this control's tracking, so no segment is
    // chosen. AppKit's own right-click route does not fire in every window.
    override func mouseDown(with event: NSEvent) { if !contextClicked(event) { super.mouseDown(with: event) } }
    func nativeSegment(at index: Int) -> NSAccessibilityElement? {
        exposingNativeChildren = true
        defer { exposingNativeChildren = false }
        guard let cell = super.accessibilityChildren()?.first as? NSObject,
              let segments = cell.accessibilityAttributeValue(.children) as? [NSAccessibilityElement],
              segments.indices.contains(index) else { return nil }
        return segments[index]
    }
    override func accessibilityChildren() -> [Any]? {
        if exposingNativeChildren { return super.accessibilityChildren() }
        if segmentElements.count != segmentCount { segmentElements = (0..<segmentCount).map { SegmentAccessibility(self, $0) } }
        return segmentElements
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if exposingNativeChildren { return super.accessibilityAttributeValue(attribute) }
        if attribute == .children { return accessibilityChildren() }
        return super.accessibilityAttributeValue(attribute)
    }
    override func accessibilityRole() -> NSAccessibility.Role? { .radioGroup }
}

final class SegmentHost {
    unowned let presenter: Presenter
    private var controls: [UInt32: ExactSegmentedControl] = [:]
    /// The segmented control a tablist or radio group projects to, if any:
    /// a hatched node's platform object (LLP 1075.003.000 §3.2).
    func control(of id: UInt32) -> NSSegmentedControl? { controls[id] }
    private var hidden: [UInt32: Bool] = [:]
    private var members: [UInt32: [UInt32]] = [:]
    /// What each tablist last reported: its control's size, until the
    /// projection ends.
    private var sizes: [UInt32: CGSize] = [:]
    /// Reports for after the batch being applied, as controls' and images'
    /// are; the latest per tablist reaches the runner.
    private struct Report { weak var view: NodeView?; let generation: Int; let size: CGSize? }
    private var reports: [Report] = []
    /// LLP 1080.001 D3: the control standing in for a tablist; its hidden tabs.
    func inspectionOwns(_ view: NSView) -> Bool { controls.values.contains { $0 === view } }
    func hides(_ node: NodeView) -> Bool { members.values.contains { $0.contains(node.id) } }
    /// The last projection decision journaled per tablist, so each is said once.
    private var decisions: [UInt32: String] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Journal why a tablist or radio group is or is not a segmented control, once per change.
    private func decide(_ owner: NodeView, _ decision: String) {
        guard decisions[owner.id] != decision else { return }
        decisions[owner.id] = decision
        presenter.session?.log("\(owner.props["accessibilityRole"] ?? "tablist") #\(owner.id): \(decision)")
    }

    /// A tablist's pressable tabs, or a radio group's pressable radios.
    private func tabs(in owner: NodeView) -> [NodeView] {
        let role = owner.props["accessibilityRole"] == "radiogroup" ? "radio" : "tab"
        return owner.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.isButton && $0.props["accessibilityRole"] == role && $0.handlers.contains("press")
        }
    }
    /// The chosen member: a tab `aria-selected`, a radio `aria-checked`.
    private static func chosen(_ tab: NodeView) -> Bool {
        tab.props[tab.props["accessibilityRole"] == "radio" ? "accessibilityChecked" : "accessibilitySelected"] == "true"
    }

    private func restore(owner id: UInt32) {
        for childID in members.removeValue(forKey: id) ?? [] {
            if let child = presenter.views[childID] { child.isHidden = hidden.removeValue(forKey: childID) ?? false }
            else { hidden.removeValue(forKey: childID) }
        }
        controls.removeValue(forKey: id)?.removeFromSuperview()
        if sizes.removeValue(forKey: id) != nil, let owner = presenter.views[id] { report(owner, nil) }
    }

    /// The control's own size, at any width of the box: its height does not
    /// follow that width. Its width is the narrowest it can be, the
    /// tablist's automatic minimum width (LLP 1116.004 D2): AppKit's control
    /// does not shrink its segments, so its natural width. It fills the
    /// content box, so a border-box minimum also holds the padding and
    /// border around it.
    private func measure(_ owner: NodeView, _ control: NSSegmentedControl) {
        let natural = control.intrinsicContentSize
        var size = CGSize(width: max(natural.width, 1), height: natural.height)
        if owner.style["box_sizing"]?.string == "border-box" {
            let border = owner.number("border_width")
            size.height += owner.number("border_width_top", border) + owner.padding("top")
                + owner.number("border_width_bottom", border) + owner.padding("bottom")
        }
        guard size.width.isFinite, size.height.isFinite, size.height > 0, sizes[owner.id] != size else { return }
        sizes[owner.id] = size
        report(owner, size)
    }

    private func report(_ owner: NodeView, _ size: CGSize?) {
        if reports.isEmpty { DispatchQueue.main.async { [weak self] in self?.publish() } }
        reports.append(Report(view: owner, generation: owner.loadGeneration, size: size))
    }

    /// A tablist destroyed or reset before then reports nothing.
    private func publish() {
        var latest: [UInt32: CGSize?] = [:], order: [UInt32] = []
        for entry in reports {
            guard let view = entry.view, view.loadGeneration == entry.generation, presenter.views[view.id] === view else { continue }
            if latest.updateValue(entry.size, forKey: view.id) == nil { order.append(view.id) }
        }
        reports = []
        if !order.isEmpty { presenter.onIntrinsic?(order.map { ($0, latest[$0]!) }) }
    }

    func sync() {
        // @ref LLP 1039 D6 — only explicit vertical tablists opt out; ignore invalid ARIA values.
        // The root tablist the window's sidebar shows is no segmented control (LLP 1115.002 D3),
        // nor a header's that the window's toolbar shows as its own group (D2).
        let owners = (presenter.carrying("role:tablist") + presenter.carrying("role:radiogroup")).filter {
            $0.props["accessibilityOrientation"] != "vertical" && !presenter.sidebar.adopts($0) && !presenter.toolbar.adopts($0) &&
                !($0.props["accessibilityRole"] == "tablist" && $0.wrote("appearance") && $0.style["appearance"]?.string == "none")
        }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) where !live.contains(id) { restore(owner: id) }
        for id in Array(decisions.keys) where !live.contains(id) { decisions.removeValue(forKey: id) }
        for owner in owners {
            let tabs = tabs(in: owner)
            // Authored tabs a segment cannot show stay as authored (LLP 1035.001 D10);
            // AppKit gives a title segment no label of its own, so a title its
            // `aria-label` disagrees with is one (iOS labels it, LLP 1116 D6).
            let unshown = tabs.first { tab in
                guard let face = tab.segmentFace else { return true }
                if case .title(let title) = face { return title != tab.accessibleName }
                return false
            }
            if tabs.count <= 1 || unshown != nil {
                restore(owner: owner.id)
                let member = owner.props["accessibilityRole"] == "radiogroup" ? "radio" : "tab"
                decide(owner, unshown.map { "kept as authored: \(member) #\($0.id) is not one image or its label alone, which a segment cannot show" } ?? "kept as authored: fewer than two pressable \(member)s")
                continue
            }
            decide(owner, "projected to NSSegmentedControl (\(tabs.count) segments)")
            let ids = tabs.map(\.id)
            if members[owner.id] != ids {
                restore(owner: owner.id)
                members[owner.id] = ids
                for tab in tabs { hidden[tab.id] = tab.hiddenByHost; tab.isHidden = true }
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
            control.accessibilityDisabledSegments = tabs.map(\.ariaUnavailable)
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
            control.selectedSegment = tabs.firstIndex(where: Self.chosen) ?? -1
            owner.addSubview(control, positioned: .above, relativeTo: nil)
            measure(owner, control)
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
