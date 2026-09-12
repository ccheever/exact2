// @ref LLP 1035.001 D10 — standard tab semantics project to the platform's
// segmented control. Contract remains the state owner; UIKit owns the control.
#if os(iOS)
import UIKit

private final class ExactSegmentedControl: UISegmentedControl {
    let ownerID: UInt32
    var icons: [Int: (source: UIImage, size: CGSize, label: String)] = [:]
    init(ownerID: UInt32) {
        self.ownerID = ownerID
        super.init(items: [])
    }
    required init?(coder: NSCoder) { nil }
}

final class SegmentHost: NSObject, UIGestureRecognizerDelegate {
    unowned let presenter: Presenter
    private var controls: [UInt32: ExactSegmentedControl] = [:]
    private var hidden: [UInt32: Bool] = [:]
    private var members: [UInt32: [UInt32]] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter; super.init() }

    private func contextTab(_ gesture: UIGestureRecognizer) -> NodeView? {
        guard let control = gesture.view as? ExactSegmentedControl,
              let ids = members[control.ownerID], control.bounds.width > 0,
              let owner = presenter.views[control.ownerID], available(owner) else { return nil }
        let point = gesture.location(in: control)
        guard control.bounds.contains(point) else { return nil }
        var index = Int(point.x / control.bounds.width * CGFloat(ids.count))
        if control.effectiveUserInterfaceLayoutDirection == .rightToLeft { index = ids.count - 1 - index }
        guard ids.indices.contains(index), let tab = presenter.views[ids[index]],
              !tab.disabled, tab.handlers.contains("contextmenu") else { return nil }
        return tab
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        contextTab(gestureRecognizer) != nil
    }

    @objc private func longPressed(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, let tab = contextTab(gesture),
              let control = gesture.view as? ExactSegmentedControl else { return }
        // UIKit cancels the pending segment tap. Opening a context action must
        // not first navigate to that tab, nor commit selection when lifted.
        control.cancelTracking(with: nil)
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        presenter.contextmenu(tab.id)
    }

    private func tabs(in owner: NodeView) -> [NodeView] {
        owner.container.subviews.compactMap { $0 as? NodeView }.filter {
            $0.kind == "button" && $0.props["accessibilityRole"] == "tab" && $0.handlers.contains("press")
        }
    }

    private func available(_ owner: UIView) -> Bool {
        var ancestor: UIView? = owner
        while let view = ancestor {
            if view.isHidden || view.alpha <= 0.01 || (view as? NodeView)?.props["inert"] == "true" { return false }
            ancestor = view.superview
        }
        return owner.window != nil
    }

    /// An image-only authored tab stays image-only in UIKit. Its accessible
    /// name belongs to the segment image; it is not a visible fallback title.
    private func content(_ tab: NodeView, at index: Int, in control: ExactSegmentedControl) {
        let label = tab.props["accessibilityLabel"] ?? ""
        let children = tab.container.subviews.compactMap { $0 as? NodeView }
        if children.count == 1, let icon = children.first, icon.kind == "image" {
            guard let source = icon.image, icon.bounds.width > 0, icon.bounds.height > 0 else {
                control.setTitle(nil, forSegmentAt: index)
                return
            }
            let size = icon.bounds.size
            if let old = control.icons[index], old.source === source, old.size == size, old.label == label { return }
            let image = UIGraphicsImageRenderer(size: size).image { _ in
                let ratio = min(size.width / source.size.width, size.height / source.size.height)
                let fit = CGSize(width: source.size.width * ratio, height: source.size.height * ratio)
                source.draw(in: CGRect(x: (size.width - fit.width) / 2, y: (size.height - fit.height) / 2, width: fit.width, height: fit.height))
            }.withRenderingMode(.alwaysOriginal)
            image.accessibilityLabel = label
            control.setImage(image, forSegmentAt: index)
            control.icons[index] = (source, size, label)
        } else {
            control.icons.removeValue(forKey: index)
            if control.imageForSegment(at: index) != nil { control.setImage(nil, forSegmentAt: index) }
            if control.titleForSegment(at: index) != label { control.setTitle(label, forSegmentAt: index) }
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
                value.addTarget(self, action: #selector(changed(_:)), for: .valueChanged)
                let context = UILongPressGestureRecognizer(target: self, action: #selector(longPressed(_:)))
                context.delegate = self
                value.addGestureRecognizer(context)
                value.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                owner.addSubview(value)
                controls[owner.id] = value
                return value
            }()
            if control.superview !== owner { owner.addSubview(control) }
            let frame = owner.contentBox()
            if control.frame != frame { control.frame = frame }
            control.isEnabled = available(owner)
            control.accessibilityLabel = owner.props["accessibilityLabel"]
            if control.numberOfSegments != tabs.count {
                control.removeAllSegments()
                control.icons.removeAll()
                for index in tabs.indices { control.insertSegment(withTitle: tabs[index].props["accessibilityLabel"] ?? "", at: index, animated: false) }
            }
            for (index, tab) in tabs.enumerated() {
                content(tab, at: index, in: control)
                control.setEnabled(!tab.disabled, forSegmentAt: index)
            }
            let selected = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? UISegmentedControl.noSegment
            if control.selectedSegmentIndex != selected { control.selectedSegmentIndex = selected }
            owner.bringSubviewToFront(control)
        }
    }

    @objc private func changed(_ sender: ExactSegmentedControl) {
        guard let ids = members[sender.ownerID], ids.indices.contains(sender.selectedSegmentIndex),
              let tab = presenter.views[ids[sender.selectedSegmentIndex]], !tab.disabled,
              let owner = presenter.views[sender.ownerID], available(owner) else { sync(); return }
        presenter.press(tab.id)
    }

    /// Agent activation names the authored tab even though UIKit owns its pixels.
    func activate(_ node: NodeView) -> Bool? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }) else { return nil }
        guard let owner = presenter.views[entry.key], available(owner), !node.disabled else { return false }
        presenter.press(node.id)
        return true
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let entry = members.first(where: { $0.value.contains(node.id) }),
              let control = controls[entry.key], let segment = entry.value.firstIndex(of: node.id) else { return nil }
        return ["view": "UISegmentedControl", "segment": segment,
                "selected": control.selectedSegmentIndex, "segments": control.numberOfSegments]
    }

    func reset() {
        for id in Array(controls.keys) { restore(owner: id) }
    }
}
#endif
