// @ref llp/1109.005-composer-and-transcript.decision.md#fork-work-row-copy-and-selection
// T3 Code365aa87982 ThreadWorkLogRow, WorkLogPressable and onCopyWorkRow.
#if os(iOS)
import UIKit
import UniformTypeIdentifiers

struct T3WorkRowConfiguration: Decodable, Equatable {
    let id: String; let owner: String; let routeKey: String; let label: String; let copyText: String
    let expanded: Bool; let expandable: Bool; let symbol: String; let copiedColor: String
    static func read(_ props: [String: String]) throws -> Self {
        guard let bytes = props["configuration"]?.data(using: .utf8),
              let result = try? JSONDecoder().decode(Self.self, from: bytes),
              !result.id.isEmpty, !result.owner.isEmpty, !result.label.isEmpty, !result.copyText.isEmpty else {
            throw ExactNativeRefusal("Invalid work row configuration.")
        }
        return result
    }
}

protocol T3CopiedWorkRow: AnyObject { func resetCopied() }

final class T3MobileWorkLog {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class RowRef { weak var value: (any T3CopiedWorkRow)?; init(_ value: any T3CopiedWorkRow) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var copied: [String: RowRef] = [:]
    private var alive = true
    func configure(_ route: ExactRoute) { if alive { routes[route.key] = RouteRef(route) } }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        copied[route.key]?.value?.resetCopied(); routes.removeValue(forKey: route.key)
    }
    func makeRow(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The work log session has ended.") }
        let result = T3WorkRow(owner: self, events: events); try result.setProps(props); return result
    }
    func makeFailure(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The work log session has ended.") }
        let result = T3WorkFailure(owner: self, events: events); try result.setProps(props); return result
    }
    func makeDetail(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The work log session has ended.") }
        let result = T3WorkDetail(owner: self, events: events); try result.setProps(props); return result
    }
    func allows(_ key: String) -> Bool {
        guard alive, UIApplication.shared.applicationState == .active, let route = routes[key]?.value,
              route.isLive, route.controller.navigationController?.topViewController === route.controller,
              route.controller.viewIfLoaded?.window != nil, route.controller.presentedViewController == nil else { return false }
        return true
    }
    func didCopy(_ row: any T3CopiedWorkRow, key: String) {
        copied[key]?.value?.resetCopied(); copied[key] = RowRef(row)
    }
    func forget(_ row: any T3CopiedWorkRow, key: String) { if copied[key]?.value === row { copied.removeValue(forKey: key) } }
    func destroy() {
        alive = false
        let rows = copied.values.compactMap(\.value); copied.removeAll()
        for row in rows { row.resetCopied() }
        routes.removeAll()
    }
}

private final class T3WorkButton: UIButton {
    let icon = UIImageView(), label = UILabel(), copied = UILabel(), chevron = UIImageView()
    var longPress: (() -> Void)?
    var subtle = UIColor.clear
    @objc func held(_ gesture: UILongPressGestureRecognizer) { if gesture.state == .began { longPress?() } }
    override var isHighlighted: Bool { didSet { backgroundColor = isHighlighted ? subtle : .clear } }
    override init(frame: CGRect) {
        super.init(frame: frame)
        for child in [icon, label, copied, chevron] {
            child.isUserInteractionEnabled = false; child.isAccessibilityElement = false; child.accessibilityElementsHidden = true; addSubview(child)
        }
        label.font = UIFont(name: "DMSans-Regular", size: 14) ?? .systemFont(ofSize: 14)
        label.numberOfLines = 1; label.lineBreakMode = .byTruncatingTail
        copied.font = UIFont(name: "DMSans-Medium", size: 11) ?? .systemFont(ofSize: 11, weight: .medium)
        copied.text = "Copied"; copied.isHidden = true
        icon.contentMode = .center; chevron.contentMode = .center
        layer.cornerRadius = 7; isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool { bounds.insetBy(dx: -4, dy: -4).contains(point) }
    override func layoutSubviews() {
        super.layoutSubviews()
        let copiedWidth = copied.isHidden ? 0 : ceil(copied.intrinsicContentSize.width) + 3.5
        icon.frame = CGRect(x: 1.75, y: (bounds.height - 21) / 2, width: 21, height: 21)
        chevron.frame = CGRect(x: bounds.width - 15.75, y: (bounds.height - 14) / 2, width: 14, height: 14)
        copied.frame = CGRect(x: chevron.frame.minX - copiedWidth - 1, y: (bounds.height - 14) / 2, width: copiedWidth, height: 14)
        let labelEnd = copied.isHidden ? chevron.frame.minX : copied.frame.minX
        label.frame = CGRect(x: 28, y: (bounds.height - 19) / 2, width: max(0, labelEnd - 28 - 5.25), height: 19)
    }
}

private final class T3WorkRow: ExactNativeInstance, T3CopiedWorkRow {
    private let button = T3WorkButton(frame: .zero)
    private weak var owner: T3MobileWorkLog?
    private var config: T3WorkRowConfiguration?
    private var hold: UILongPressGestureRecognizer!
    private var feedback: Timer?
    private var alive = true
    override var view: UIView { button }
    init(owner: T3MobileWorkLog, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        button.addAction(UIAction { [weak self] _ in self?.press() }, for: .touchUpInside)
        button.longPress = { [weak self] in self?.copyRow() }
        hold = UILongPressGestureRecognizer(target: button, action: #selector(T3WorkButton.held(_:)))
        hold.minimumPressDuration = 0.5; hold.cancelsTouchesInView = true; button.addGestureRecognizer(hold)
    }
    override func setProps(_ props: [String: String]) throws {
        let next = try T3WorkRowConfiguration.read(props)
        let muted = try T3SymbolView.color(props["row-muted"] ?? "#6f6f79")
        let iconColor = try T3SymbolView.color(props["row-icon"] ?? "#71717b")
        let subtle = try T3SymbolView.color(props["row-subtle"] ?? "#f4f4f5")
        let copiedColor = try T3SymbolView.color(next.copiedColor)
        if config?.owner != next.owner || config?.copyText != next.copyText { hold.isEnabled = false; hold.isEnabled = true }
        if config?.owner != next.owner { resetCopied() }
        config = next
        button.label.text = next.label; button.label.textColor = muted
        button.icon.image = UIImage(systemName: next.symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: 14, weight: .medium))
        button.icon.tintColor = iconColor
        button.chevron.image = UIImage(systemName: next.expanded ? "chevron.up" : "chevron.down",
            withConfiguration: UIImage.SymbolConfiguration(pointSize: 11, weight: .regular))
        button.chevron.tintColor = iconColor; button.chevron.isHidden = !next.expandable
        button.subtle = subtle; button.copied.textColor = copiedColor
        button.accessibilityLabel = next.label; button.accessibilityIdentifier = "thread-work-\(next.id)"
        button.accessibilityHint = "Long press to copy."
        if #available(iOS 18, *) { button.accessibilityExpandedStatus = next.expandable ? (next.expanded ? .expanded : .collapsed) : .unsupported }
        button.isEnabled = alive && !next.routeKey.isEmpty; button.setNeedsLayout()
    }
    private func current() -> T3WorkRowConfiguration? {
        guard alive, let config, button.isEnabled, button.window != nil, !button.isHidden,
              button.bounds.width > 0, button.bounds.height > 0, owner?.allows(config.routeKey) == true else { return nil }
        return config
    }
    private func press() { if current() != nil { events.press() } }
    private func copyRow() {
        guard hold.state == .began, let config = current() else { return }
        UIPasteboard.general.string = config.copyText
        UISelectionFeedbackGenerator().selectionChanged()
        owner?.didCopy(self, key: config.routeKey)
        button.copied.isHidden = false; button.setNeedsLayout()
        feedback?.invalidate()
        let timer = Timer(timeInterval: 1.2, repeats: false) { [weak self] _ in self?.resetCopied() }
        feedback = timer; RunLoop.main.add(timer, forMode: .common)
    }
    func resetCopied() {
        feedback?.invalidate(); feedback = nil; button.copied.isHidden = true; button.setNeedsLayout()
        if let config { owner?.forget(self, key: config.routeKey) }
    }
    override func destroy() {
        resetCopied(); alive = false; config = nil; hold.isEnabled = false; button.isEnabled = false; button.longPress = nil
    }
}

// RN0.88.0-rc.3 RCTParagraphComponentView: selectable Text offers Copy of the
// whole attributed paragraph, with UTF-8 and RTFD clipboard representations.
final class T3SelectableWorkText: UILabel, UIEditMenuInteractionDelegate {
    var allowsCopy: (() -> Bool)?
    private var menu: UIEditMenuInteraction!
    private var hold: UILongPressGestureRecognizer!
    override init(frame: CGRect) {
        super.init(frame: frame)
        numberOfLines = 0; lineBreakMode = .byWordWrapping; isUserInteractionEnabled = true; isAccessibilityElement = true
        menu = UIEditMenuInteraction(delegate: self); addInteraction(menu)
        hold = UILongPressGestureRecognizer(target: self, action: #selector(held(_:))); addGestureRecognizer(hold)
        accessibilityTraits = .staticText
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var canBecomeFirstResponder: Bool { allowsCopy?() == true }
    @objc private func held(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, allowsCopy?() == true else { return }
        menu.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: gesture.location(in: self)))
    }
    func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration,
        suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard allowsCopy?() == true else { return nil }
        return UIMenu(children: [UIAction(title: "Copy") { [weak self] _ in self?.copyParagraph() }])
    }
    private func copyParagraph() {
        guard allowsCopy?() == true, let attributedText else { return }
        var item: [String: Any] = [UTType.utf8PlainText.identifier: attributedText.string]
        if let data = try? attributedText.data(from: NSRange(location: 0, length: attributedText.length),
            documentAttributes: [.documentType: NSAttributedString.DocumentType.rtfd]) { item[UTType.flatRTFD.identifier] = data }
        UIPasteboard.general.items = [item]
    }
    func cancelMenu() { menu.dismissMenu(); resignFirstResponder(); hold.isEnabled = false; hold.isEnabled = true }
}

private final class T3WorkDetailScroll: UIScrollView {
    let text = T3SelectableWorkText(frame: .zero)
    override init(frame: CGRect) {
        super.init(frame: frame); addSubview(text); backgroundColor = .clear
        showsVerticalScrollIndicator = true; isDirectionalLockEnabled = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func layoutSubviews() {
        super.layoutSubviews()
        let width = max(0, bounds.width - 8)
        let height = ceil(text.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height)
        text.frame = CGRect(x: 0, y: 0, width: width, height: height)
        contentSize = CGSize(width: bounds.width, height: height)
    }
}

final class T3WorkDetail: ExactNativeInstance {
    private let scroll = T3WorkDetailScroll(frame: .zero)
    private weak var owner: T3MobileWorkLog?
    private var config: T3WorkRowConfiguration?
    private var content = "", alive = true
    override var view: UIView { scroll }
    init(owner: T3MobileWorkLog, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        scroll.text.allowsCopy = { [weak self] in
            guard let self, self.alive, let config = self.config, self.scroll.window != nil,
                  !self.scroll.isHidden, self.scroll.bounds.width > 0, self.scroll.bounds.height > 0 else { return false }
            return self.owner?.allows(config.routeKey) == true
        }
    }
    override func setProps(_ props: [String: String]) throws {
        let nextConfig = try T3WorkRowConfiguration.read(props), next = props["detail-text"] ?? ""
        let color = try T3SymbolView.color(props["detail-muted"] ?? "#6f6f79")
        let scopeChanged = config?.owner != nextConfig.owner
        if scopeChanged || content != next {
            scroll.text.cancelMenu()
            let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = 18; paragraph.maximumLineHeight = 18
            scroll.text.attributedText = NSAttributedString(string: next, attributes: [.font: UIFont(name: "Menlo", size: 12) ?? .monospacedSystemFont(ofSize: 12, weight: .regular),
                .foregroundColor: color, .paragraphStyle: paragraph])
            if scopeChanged { scroll.setContentOffset(.zero, animated: false) }
            content = next; scroll.setNeedsLayout()
        } else { scroll.text.textColor = color }
        config = nextConfig; scroll.text.accessibilityIdentifier = "thread-work-detail-\(nextConfig.id)"; scroll.text.accessibilityTraits = .staticText
    }
    override func destroy() {
        alive = false; scroll.text.cancelMenu(); scroll.text.allowsCopy = nil; scroll.text.text = ""; config = nil; content = ""
    }
}
#endif
