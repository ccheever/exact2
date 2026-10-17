// @ref llp/1109.005-composer-and-transcript.decision.md#failure-work-row-copy-and-selection
// Pinned365aa87982 ThreadWorkLogRow's prominent provider/usage failure branch.
#if os(iOS)
import UIKit

private final class T3WorkFailureSurface: UIControl, UIGestureRecognizerDelegate {
    let icon = UIImageView(), label = UILabel(), copied = UILabel(), timestamp = UILabel()
    let message = T3SelectableWorkText(frame: .zero), retry = UIButton(type: .system)
    var held: (() -> Void)?
    var subtle = UIColor.clear
    var preferredWidth: CGFloat = 1
    private var holding = false
    override var isHighlighted: Bool { didSet { backgroundColor = isHighlighted ? subtle : .clear } }
    override init(frame: CGRect) {
        super.init(frame: frame)
        for child in [icon, label, copied, timestamp, message, retry] { addSubview(child) }
        for child in [icon, label, copied, timestamp] {
            child.isUserInteractionEnabled = false; child.isAccessibilityElement = false; child.accessibilityElementsHidden = true
        }
        isAccessibilityElement = true; accessibilityTraits = []; layer.cornerRadius = 7
        label.numberOfLines = 0; label.lineBreakMode = .byWordWrapping
        label.font = UIFont(name: "DMSans-Medium", size: 14) ?? .systemFont(ofSize: 14, weight: .medium)
        copied.font = UIFont(name: "DMSans-Medium", size: 11) ?? .systemFont(ofSize: 11, weight: .medium)
        copied.text = "Copied"; copied.isHidden = true
        timestamp.font = UIFont(name: "DMSans-Regular", size: 13) ?? .systemFont(ofSize: 13)
        icon.contentMode = .center
        retry.accessibilityLabel = "Retry workspace preparation"
        var appearance = UIButton.Configuration.plain()
        appearance.title = "Retry"
        appearance.image = UIImage(systemName: "arrow.clockwise", withConfiguration: UIImage.SymbolConfiguration(pointSize: 13, weight: .regular))
        appearance.imagePadding = 5.25
        appearance.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 14, bottom: 0, trailing: 14)
        appearance.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { value in
            var result = value; result.font = UIFont(name: "DMSans-Medium", size: 14) ?? .systemFont(ofSize: 14, weight: .medium); return result
        }
        retry.configuration = appearance; retry.layer.cornerRadius = 19.25; retry.layer.borderWidth = 1
        addAction(UIAction { [weak self] _ in self?.isHighlighted = true }, for: .touchDown)
        addAction(UIAction { [weak self] _ in
            guard let self, !self.holding else { return }; self.isHighlighted = false
        }, for: [.touchUpInside, .touchUpOutside, .touchCancel])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    @objc func longPressed(_ gesture: UILongPressGestureRecognizer) {
        switch gesture.state {
        case .began: holding = true; isHighlighted = true; held?()
        case .ended, .cancelled, .failed: holding = false; isHighlighted = false
        default: break
        }
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // The source's selectable paragraph owns its own Copy menu; Retry owns a separate press.
        guard let target = touch.view else { return true }
        return !target.isDescendant(of: message) && !target.isDescendant(of: retry)
    }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool { bounds.insetBy(dx: -4, dy: -4).contains(point) }
    private func textHeight(_ text: UILabel, width: CGFloat) -> CGFloat {
        guard let value = text.attributedText, width > 0 else { return 0 }
        // Match pinned RN's RCTTextLayoutManager, including its baseline offset and pixel headroom.
        let container = NSTextContainer(size: CGSize(width: width, height: .greatestFiniteMagnitude))
        container.lineFragmentPadding = 0; container.lineBreakMode = .byClipping
        let manager = NSLayoutManager(); manager.usesFontLeading = false; manager.addTextContainer(container)
        let storage = NSTextStorage(attributedString: value); storage.addLayoutManager(manager)
        manager.ensureLayout(for: container)
        let height = manager.usedRect(for: container).height
        let scale = max(1, window?.screen.scale ?? traitCollection.displayScale)
        return ceil((height + 0.001) * scale) / scale
    }
    @discardableResult private func arrange(width: CGFloat, assign: Bool) -> CGSize {
        let timeWidth = ceil(timestamp.intrinsicContentSize.width)
        let copiedWidth = copied.isHidden ? 0 : ceil(copied.intrinsicContentSize.width) + 3.5
        let timeX = max(28, width - 1.75 - timeWidth)
        let copiedX = timeX - 5.25 - copiedWidth
        let labelWidth = max(0, (copied.isHidden ? timeX : copiedX) - 28 - 5.25)
        let labelHeight = textHeight(label, width: labelWidth), headerHeight = max(21, labelHeight)
        let timeHeight = textHeight(timestamp, width: timeWidth)
        let copiedHeight = textHeight(copied, width: max(1, copiedWidth))
        let messageWidth = max(0, width - 28.25), messageHeight = message.isHidden ? 0 : textHeight(message, width: messageWidth)
        let retrySize = retry.sizeThatFits(CGSize(width: max(0, width - 28.25), height: 38.5))
        let retryY = 3.5 + headerHeight + messageHeight + 7
        if assign {
            icon.frame = CGRect(x: 1.75, y: 3.5 + (headerHeight - 21) / 2, width: 21, height: 21)
            label.frame = CGRect(x: 28, y: 3.5 + (headerHeight - labelHeight) / 2, width: labelWidth, height: labelHeight)
            timestamp.frame = CGRect(x: timeX, y: 3.5 + (headerHeight - timeHeight) / 2, width: timeWidth, height: timeHeight)
            copied.frame = CGRect(x: copiedX, y: 3.5 + (headerHeight - copiedHeight) / 2, width: copiedWidth, height: copiedHeight)
            message.frame = CGRect(x: 26.25, y: 3.5 + headerHeight, width: messageWidth, height: messageHeight)
            retry.frame = CGRect(x: 26.25, y: retryY, width: min(retrySize.width, max(0, width - 28.25)), height: 38.5)
        }
        return CGSize(width: width, height: 7 + headerHeight + messageHeight + (retry.isHidden ? 0 : 45.5))
    }
    // The app supplies its settled feed width. Preferred size is independent of the assigned frame.
    var preferredSize: CGSize { arrange(width: preferredWidth, assign: false) }
    override func layoutSubviews() { super.layoutSubviews(); arrange(width: bounds.width, assign: true) }
}

final class T3WorkFailure: ExactNativeInstance, T3CopiedWorkRow {
    private let surface = T3WorkFailureSurface(frame: .zero)
    private weak var owner: T3MobileWorkLog?
    private var config: T3WorkRowConfiguration?
    private var hold: UILongPressGestureRecognizer!
    private var feedback: Timer?
    private var alive = true, retryAllowed = false
    private var messageText = ""
    override var view: UIView { surface }
    init(owner: T3MobileWorkLog, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        surface.held = { [weak self] in self?.copyRow() }
        hold = UILongPressGestureRecognizer(target: surface, action: #selector(T3WorkFailureSurface.longPressed(_:)))
        hold.minimumPressDuration = 0.5; hold.cancelsTouchesInView = true; hold.delegate = surface; surface.addGestureRecognizer(hold)
        surface.message.allowsCopy = { [weak self] in self?.current() != nil }
        surface.retry.addAction(UIAction { [weak self] _ in
            guard let self, self.current() != nil, self.retryAllowed, self.surface.retry.isEnabled else { return }
            self.events.press()
        }, for: .touchUpInside)
    }
    private func attributed(_ text: String, font: UIFont, color: UIColor, lineHeight: CGFloat) -> NSAttributedString {
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = lineHeight; paragraph.maximumLineHeight = lineHeight
        return NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color, .paragraphStyle: paragraph,
            .baselineOffset: (lineHeight - font.lineHeight) / 2])
    }
    override func setProps(_ props: [String: String]) throws {
        let next = try T3WorkRowConfiguration.read(props)
        guard let width = Double(props["preferred-width"] ?? ""), width.isFinite, width > 0 else {
            throw ExactNativeRefusal("Failure work row requires a positive feed width.")
        }
        let message = props["failure-message"] ?? "", warning = props["failure-warning"] == "true"
        let iconColor = try T3SymbolView.color(props["failure-icon"] ?? "#c10007")
        let foreground = try T3SymbolView.color(props["failure-foreground"] ?? "#27272a")
        let border = try T3SymbolView.color(props["failure-border"] ?? "#e4e4e7")
        let subtle = try T3SymbolView.color(props["failure-subtle"] ?? "#f4f4f5")
        let copiedColor = try T3SymbolView.color(next.copiedColor)
        let rose = try T3SymbolView.color(next.copiedColor == "#00d492" ? "#ff637e" : "#ec003f")
        let scopeChanged = config?.owner != next.owner
        if scopeChanged || config?.copyText != next.copyText { hold.isEnabled = false; hold.isEnabled = true }
        if scopeChanged { resetCopied() }
        if scopeChanged || messageText != message { surface.message.cancelMenu() }
        config = next; messageText = message; surface.preferredWidth = CGFloat(width)
        surface.label.attributedText = attributed(next.label, font: surface.label.font, color: warning ? iconColor : rose, lineHeight: 19)
        surface.message.attributedText = attributed(message, font: UIFont(name: "DMSans-Regular", size: 14) ?? .systemFont(ofSize: 14), color: foreground, lineHeight: 19)
        surface.message.isHidden = warning || message.isEmpty
        surface.message.accessibilityIdentifier = "thread-work-message-\(next.id)"
        // Pinned source's text-foreground-subtle has no generated token; RN keeps its black default.
        surface.timestamp.attributedText = attributed(props["failure-time"] ?? "", font: surface.timestamp.font, color: .black, lineHeight: 17)
        surface.icon.image = UIImage(systemName: "exclamationmark.circle", withConfiguration: UIImage.SymbolConfiguration(pointSize: 14, weight: .medium))
        surface.icon.tintColor = iconColor; surface.subtle = subtle
        surface.copied.attributedText = attributed("Copied", font: surface.copied.font, color: copiedColor, lineHeight: 14)
        surface.retry.isHidden = (props["retry-run"] ?? "").isEmpty
        retryAllowed = !surface.retry.isHidden && props["retry-disabled"] != "true"
        surface.retry.isEnabled = retryAllowed; surface.retry.alpha = retryAllowed ? 1 : 0.5
        surface.retry.configuration?.baseForegroundColor = foreground; surface.retry.layer.borderColor = border.cgColor
        surface.accessibilityLabel = warning ? next.label : "\(next.label): \(message)"
        surface.accessibilityHint = "Long press to copy."; surface.accessibilityIdentifier = "thread-work-\(next.id)"
        surface.isEnabled = alive && !next.routeKey.isEmpty; surface.setNeedsLayout()
        events.intrinsicSize(surface.preferredSize)
    }
    private func current() -> T3WorkRowConfiguration? {
        guard alive, let config, surface.isEnabled, surface.window != nil, !surface.isHidden,
              surface.bounds.width > 0, surface.bounds.height > 0, owner?.allows(config.routeKey) == true else { return nil }
        return config
    }
    private func copyRow() {
        guard hold.state == .began, let config = current() else { return }
        UIPasteboard.general.string = config.copyText; UISelectionFeedbackGenerator().selectionChanged()
        owner?.didCopy(self, key: config.routeKey)
        let expires = Date(timeIntervalSinceNow: 1.2)
        surface.copied.isHidden = false; surface.setNeedsLayout(); events.intrinsicSize(surface.preferredSize)
        feedback?.invalidate()
        let timer = Timer(fire: expires, interval: 0, repeats: false) { [weak self] _ in self?.resetCopied() }
        feedback = timer; RunLoop.main.add(timer, forMode: .common)
    }
    func resetCopied() {
        feedback?.invalidate(); feedback = nil; surface.copied.isHidden = true; surface.setNeedsLayout()
        if alive { events.intrinsicSize(surface.preferredSize) }
        if let config { owner?.forget(self, key: config.routeKey) }
    }
    override func destroy() {
        alive = false; resetCopied(); config = nil; hold.isEnabled = false; surface.isEnabled = false
        surface.held = nil; surface.message.cancelMenu(); surface.message.allowsCopy = nil; surface.message.text = ""
    }
}
#endif
