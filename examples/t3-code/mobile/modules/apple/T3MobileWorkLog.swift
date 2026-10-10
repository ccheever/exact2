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
        if props["detail-kind"] == "command" {
            let result = T3CommandWorkDetail(owner: self, events: events); try result.setProps(props); return result
        }
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
    private var menuRevision: UInt = 0
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
        let revision = menuRevision
        return UIMenu(children: [UIAction(title: "Copy") { [weak self] _ in self?.copyParagraph(revision: revision) }])
    }
    private func copyParagraph(revision: UInt) {
        guard revision == menuRevision, allowsCopy?() == true, let attributedText else { return }
        var item: [String: Any] = [UTType.utf8PlainText.identifier: attributedText.string]
        if let data = try? attributedText.data(from: NSRange(location: 0, length: attributedText.length),
            documentAttributes: [.documentType: NSAttributedString.DocumentType.rtfd]) { item[UTType.flatRTFD.identifier] = data }
        UIPasteboard.general.items = [item]
    }
    func cancelMenu() { menuRevision &+= 1; menu.dismissMenu(); resignFirstResponder(); hold.isEnabled = false; hold.isEnabled = true }
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

// Pinned ThreadWorkLogRow renders input/output as separate selectable paragraphs;
// the exit annotation is ordinary text. The Contract owns the 210pt scroll cap.
private final class T3CommandDetailScroll: UIScrollView {
    let input = T3SelectableWorkText(frame: .zero), output = T3SelectableWorkText(frame: .zero), result = UILabel()
    var preferredWidth: CGFloat = 1
    override init(frame: CGRect) {
        super.init(frame: frame)
        for label in [input, output, result] { label.numberOfLines = 0; addSubview(label) }
        result.isAccessibilityElement = true; result.accessibilityTraits = .staticText
        backgroundColor = .clear; showsVerticalScrollIndicator = true; isDirectionalLockEnabled = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    private func textHeight(_ label: UILabel, width: CGFloat) -> CGFloat {
        guard let text = label.attributedText, !text.string.isEmpty, width > 0 else { return 0 }
        let container = NSTextContainer(size: CGSize(width: width, height: .greatestFiniteMagnitude))
        container.lineFragmentPadding = 0; container.lineBreakMode = .byClipping
        let layout = NSLayoutManager(); layout.usesFontLeading = false; layout.addTextContainer(container)
        let storage = NSTextStorage(attributedString: text); storage.addLayoutManager(layout)
        layout.ensureLayout(for: container)
        let scale = max(1, window?.screen.scale ?? traitCollection.displayScale)
        return ceil((layout.usedRect(for: container).height + 0.001) * scale) / scale
    }
    private func arrange(width: CGFloat, assign: Bool) -> CGSize {
        let textWidth = max(0, width - 8), inputHeight = textHeight(input, width: textWidth)
        var y: CGFloat = 0
        for (index, label) in [input, output, result].enumerated() {
            let height = textHeight(label, width: textWidth)
            if height > 0 && (index == 1 && inputHeight > 0 || index == 2) { y += 5.25 }
            if assign { label.frame = CGRect(x: 0, y: y, width: textWidth, height: height) }
            y += height
        }
        if assign { contentSize = CGSize(width: width, height: y) }
        return CGSize(width: width, height: y)
    }
    var preferredSize: CGSize { arrange(width: preferredWidth, assign: false) }
    override func layoutSubviews() { super.layoutSubviews(); _ = arrange(width: bounds.width, assign: true) }
}

private final class T3CommandWorkDetail: ExactNativeInstance {
    private let scroll = T3CommandDetailScroll(frame: .zero)
    private weak var owner: T3MobileWorkLog?
    private var config: T3WorkRowConfiguration?
    private var inputText = "", outputText = "", alive = true
    override var view: UIView { scroll }
    init(owner: T3MobileWorkLog, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        for label in [scroll.input, scroll.output] { label.allowsCopy = { [weak self] in self?.canCopy() == true } }
    }
    private func canCopy() -> Bool {
        guard alive, let config, scroll.window != nil, !scroll.isHidden,
              scroll.bounds.width > 0, scroll.bounds.height > 0 else { return false }
        return owner?.allows(config.routeKey) == true
    }
    private func attributed(_ text: String, color: UIColor) -> NSAttributedString {
        let font = UIFont(name: "Menlo", size: 12) ?? .monospacedSystemFont(ofSize: 12, weight: .regular)
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = 18; paragraph.maximumLineHeight = 18
        return NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color,
            .paragraphStyle: paragraph, .baselineOffset: (18 - font.lineHeight) / 2])
    }
    override func setProps(_ props: [String: String]) throws {
        let next = try T3WorkRowConfiguration.read(props)
        guard let width = Double(props["preferred-width"] ?? ""), width.isFinite, width > 0 else {
            throw ExactNativeRefusal("Command detail requires a positive feed width.")
        }
        let input = props["detail-text"] ?? "", output = props["detail-output"] ?? ""
        let foreground = try T3SymbolView.color(props["detail-foreground"] ?? "#27272a")
        let muted = try T3SymbolView.color(props["detail-muted"] ?? "#6f6f79")
        let resultColor = try T3SymbolView.color(props["detail-result-foreground"] ?? "#c10007")
        let scopeChanged = config?.owner != next.owner
        if scopeChanged || inputText != input { scroll.input.cancelMenu() }
        if scopeChanged || outputText != output { scroll.output.cancelMenu() }
        if scopeChanged { scroll.setContentOffset(.zero, animated: false) }
        config = next; inputText = input; outputText = output; scroll.preferredWidth = CGFloat(width)
        scroll.input.attributedText = attributed(input, color: foreground)
        scroll.output.attributedText = attributed(output, color: muted)
        scroll.result.attributedText = attributed(props["detail-result"] ?? "", color: resultColor)
        scroll.input.accessibilityIdentifier = "thread-work-input-\(next.id)"
        scroll.output.accessibilityIdentifier = "thread-work-output-\(next.id)"
        scroll.result.accessibilityIdentifier = "thread-work-result-\(next.id)"
        scroll.setNeedsLayout(); events.intrinsicSize(scroll.preferredSize)
    }
    override func destroy() {
        alive = false; config = nil
        for label in [scroll.input, scroll.output] { label.cancelMenu(); label.allowsCopy = nil; label.attributedText = nil }
        scroll.result.attributedText = nil; inputText = ""; outputText = ""
    }
}
#endif
