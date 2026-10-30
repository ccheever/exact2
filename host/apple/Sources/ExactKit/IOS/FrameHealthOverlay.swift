#if os(iOS)
import UIKit
import CExact
import ObjectiveC

final class FrameHealthDragHandle: UIButton {
    var resetPosition: (() -> Void)?
    override func accessibilityActivate() -> Bool { resetPosition?(); return true }
}

/// Host UI outside the app's node tree, below sheets and alerts.
final class FrameHealthOverlay: UIView {
    fileprivate weak var health: FrameHealth?
    let card = UIView()
    let handle = FrameHealthDragHandle(type: .system)
    let close = UIButton(type: .system)
    let details = UIButton(type: .system)
    let values = UILabel()
    let chart = FrameHealthChart()
    private let legend = UILabel()
    private let coverage = UILabel()
    private var origin = CGPoint.zero
    private var placed = false
    private var dragging = false
    private var keyboard: CGRect?
    private var availableFrame = CGRect.zero
    private var minimumValuesHeight: NSLayoutConstraint!
    private var notifications: [NSObjectProtocol] = []
    private(set) var desiredVisible = false

    init(health: FrameHealth) {
        self.health = health
        super.init(frame: .zero)
        accessibilityIdentifier = "exact.host.frame-health-overlay"
        accessibilityViewIsModal = false
        backgroundColor = .clear
        isHidden = true
        card.backgroundColor = .secondarySystemBackground
        card.layer.cornerRadius = 16
        card.layer.cornerCurve = .continuous
        card.layer.borderWidth = 1 / traitCollection.displayScale
        card.layer.borderColor = UIColor.separator.cgColor
        card.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 0, leading: 12, bottom: 12, trailing: 12)
        addSubview(card)
        var grip = UIButton.Configuration.plain()
        grip.title = "Frame Health"
        grip.image = UIImage(systemName: "line.3.horizontal")
        grip.imagePadding = 8
        grip.titleLineBreakMode = .byWordWrapping
        grip.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 0, bottom: 8, trailing: 0)
        grip.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
            var attributes = incoming
            attributes.font = UIFont.preferredFont(forTextStyle: .subheadline)
            return attributes
        }
        handle.configuration = grip
        handle.titleLabel?.numberOfLines = 0
        handle.titleLabel?.adjustsFontForContentSizeCategory = true
        handle.contentHorizontalAlignment = .leading
        handle.accessibilityLabel = "Frame health position"
        handle.accessibilityHint = "Drag to move. Double-tap to reset. Accessibility actions also move the readout."
        handle.resetPosition = { [weak self] in self?.resetPosition() }
        handle.addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(dragReadout(_:))))
        handle.accessibilityCustomActions = [
            UIAccessibilityCustomAction(name: "Move up", target: self, selector: #selector(moveUp)),
            UIAccessibilityCustomAction(name: "Move down", target: self, selector: #selector(moveDown)),
            UIAccessibilityCustomAction(name: "Move left", target: self, selector: #selector(moveLeft)),
            UIAccessibilityCustomAction(name: "Move right", target: self, selector: #selector(moveRight)),
            UIAccessibilityCustomAction(name: "Reset position", target: self, selector: #selector(resetPosition)),
            UIAccessibilityCustomAction(name: "Hide frame health", target: self, selector: #selector(hideReadout))]
        close.setImage(UIImage(systemName: "xmark.circle.fill"), for: .normal)
        close.tintColor = .secondaryLabel
        close.accessibilityLabel = "Hide frame health"
        close.addAction(UIAction { [weak self] _ in self?.setVisible(false) }, for: .touchUpInside)
        details.setImage(UIImage(systemName: "info.circle"), for: .normal)
        details.accessibilityLabel = "Frame health details"
        details.widthAnchor.constraint(equalToConstant: 44).isActive = true
        details.heightAnchor.constraint(equalToConstant: 44).isActive = true
        details.addAction(UIAction { _ in DevMenu.showFrameHealthDetails() }, for: .touchUpInside)
        let heading = UIStackView(arrangedSubviews: [handle, details, close])
        heading.alignment = .center
        close.widthAnchor.constraint(equalToConstant: 44).isActive = true
        close.heightAnchor.constraint(equalToConstant: 44).isActive = true
        handle.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        values.numberOfLines = 4
        values.font = UIFontMetrics(forTextStyle: .footnote).scaledFont(for:
            .monospacedDigitSystemFont(ofSize: 13, weight: .regular))
        values.adjustsFontForContentSizeCategory = true
        minimumValuesHeight = values.heightAnchor.constraint(greaterThanOrEqualToConstant: ceil(values.font.lineHeight * 4))
        minimumValuesHeight.isActive = true
        values.textColor = .label
        coverage.text = "Measures callback and apply timing. Native-scroll deadlines are experimental. Presentation is unmeasured."
        coverage.font = .preferredFont(forTextStyle: .caption2)
        coverage.adjustsFontForContentSizeCategory = true
        coverage.textColor = .secondaryLabel
        coverage.numberOfLines = 0
        chart.heightAnchor.constraint(equalToConstant: 72).isActive = true
        legend.text = "30 s · blue average · orange slowest gap bound"
        legend.font = .preferredFont(forTextStyle: .caption2)
        legend.adjustsFontForContentSizeCategory = true
        legend.textColor = .secondaryLabel
        legend.numberOfLines = 0
        let content = UIStackView(arrangedSubviews: [heading, values, chart, legend, coverage])
        content.axis = .vertical
        content.spacing = 6
        content.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: card.layoutMarginsGuide.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: card.layoutMarginsGuide.trailingAnchor),
            content.topAnchor.constraint(equalTo: card.layoutMarginsGuide.topAnchor),
            content.bottomAnchor.constraint(equalTo: card.layoutMarginsGuide.bottomAnchor)])
        accessibilityElements = [handle, details, close, values, chart, legend, coverage]
        health.ended = { [weak self] in self?.detach() }
        let center = NotificationCenter.default
        for name in [UIApplication.didEnterBackgroundNotification, UIApplication.willEnterForegroundNotification] {
            notifications.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] notification in
                guard let self else { return }
                if notification.name == UIApplication.didEnterBackgroundNotification { self.health?.stop() }
                self.refresh()
            })
        }
        notifications.append(center.addObserver(forName: UIResponder.keyboardWillChangeFrameNotification, object: nil, queue: .main) { [weak self] notification in
            guard let self, let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect else { return }
            self.keyboard = self.window.map { self.convert($0.convert(frame, from: nil), from: $0) }
            self.setNeedsLayout()
        })
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    deinit { notifications.forEach(NotificationCenter.default.removeObserver) }

    fileprivate func detach() {
        setVisible(false)
        notifications.forEach(NotificationCenter.default.removeObserver)
        notifications.removeAll()
        removeFromSuperview()
    }

    func setVisible(_ visible: Bool) {
        if !visible { dragging = false }
        desiredVisible = visible
        isHidden = !visible
        health?.setHUDVisible(visible)
        health?.hudEffectiveVisible = visible && !card.isHidden
        if visible {
            health?.changed = { [weak self] in self?.refresh() }
            refresh()
            setNeedsLayout()
        } else {
            health?.changed = nil
        }
    }

    func refresh() {
        guard desiredVisible, !dragging, let health else { return }
        let source = health
        values.text = FrameHealthReadout.compact(source)
        chart.update(source)
        values.accessibilityLabel = values.text
        handle.accessibilityValue = [values.text, coverage.text].compactMap { $0 }.joined(separator: ". ")
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        var available = safeAreaLayoutGuide.layoutFrame.insetBy(dx: 8, dy: 8)
        if let keyboard, keyboard.intersects(available), keyboard.maxY >= available.maxY {
            available.size.height = max(0, keyboard.minY - 8 - available.minY)
        }
        let width = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
            ? max(0, available.width) : min(320, max(0, available.width))
        availableFrame = available
        coverage.isHidden = false
        chart.isHidden = false
        legend.isHidden = false
        values.isHidden = false
        minimumValuesHeight.isActive = true
        values.numberOfLines = 4
        minimumValuesHeight.constant = ceil(values.font.lineHeight * 4)
        var size = card.systemLayoutSizeFitting(CGSize(width: width, height: 0),
            withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        if size.height > available.height {
            chart.isHidden = true
            legend.isHidden = true
            coverage.isHidden = true
            values.numberOfLines = 1
            minimumValuesHeight.constant = values.font.lineHeight
            size = card.systemLayoutSizeFitting(CGSize(width: width, height: 0),
                withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        }
        if size.height > available.height {
            minimumValuesHeight.isActive = false
            values.isHidden = true
            size = card.systemLayoutSizeFitting(CGSize(width: width, height: 0),
                withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        }
        card.isHidden = size.height > available.height
        health?.hudEffectiveVisible = desiredVisible && !card.isHidden
        if !placed { card.frame.origin = CGPoint(x: available.minX, y: available.minY); placed = true }
        card.frame.size = CGSize(width: width, height: min(size.height, max(0, available.height)))
        card.frame.origin = clamped(card.frame.origin)
        health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
    }

    override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        card.layer.borderColor = UIColor.separator.cgColor
        setNeedsLayout()
    }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, !card.isHidden, alpha > 0.01 else { return nil }
        for control in [handle, details, close] {
            let local = control.convert(point, from: self)
            if control.point(inside: local, with: event) { return control.hitTest(local, with: event) }
        }
        return nil
    }

    @objc private func dragReadout(_ gesture: UIPanGestureRecognizer) {
        if gesture.state == .began { dragging = true; health?.hudDragging = true; origin = card.frame.origin }
        let delta = gesture.translation(in: self)
        UIView.performWithoutAnimation {
            card.frame.origin = clamped(CGPoint(x: origin.x + delta.x, y: origin.y + delta.y))
            health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
        }
        if gesture.state == .ended || gesture.state == .cancelled || gesture.state == .failed {
            dragging = false
            health?.hudDragging = false
            refresh()
        }
    }

    @objc @discardableResult func hideReadout() -> Bool { setVisible(false); return true }
    @objc @discardableResult func resetPosition() -> Bool { placed = false; setNeedsLayout(); return true }
    private func shift(_ x: CGFloat, _ y: CGFloat) -> Bool {
        card.frame.origin = clamped(CGPoint(x: card.frame.minX + x, y: card.frame.minY + y))
        health?.hudBounds = window.map { card.convert(card.bounds, to: $0) }
        return true
    }
    private func clamped(_ point: CGPoint) -> CGPoint {
        CGPoint(x: min(max(point.x, availableFrame.minX), max(availableFrame.minX, availableFrame.maxX - card.frame.width)),
            y: min(max(point.y, availableFrame.minY), max(availableFrame.minY, availableFrame.maxY - card.frame.height)))
    }
    @objc private func moveUp() -> Bool { shift(0, -44) }
    @objc private func moveDown() -> Bool { shift(0, 44) }
    @objc private func moveLeft() -> Bool { shift(-44, 0) }
    @objc private func moveRight() -> Bool { shift(44, 0) }
}

extension DevMenu {
    private static var overlayKey: UInt8 = 0

    static func frameHealthOverlay() -> FrameHealthOverlay? {
        guard let controller, let session, let container = frameHealthContainer, !(container is ExactView) else { return nil }
        if let existing = objc_getAssociatedObject(controller, &overlayKey) as? FrameHealthOverlay {
            if existing.health === session.frameHealth, existing.superview === container { return existing }
            existing.detach()
        }
        let overlay = FrameHealthOverlay(health: session.frameHealth)
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        container.addSubview(overlay)
        objc_setAssociatedObject(controller, &overlayKey, overlay, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return overlay
    }
}
#endif
