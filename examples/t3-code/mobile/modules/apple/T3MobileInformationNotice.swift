#if os(iOS)
// GAP 005: iOS Contract text refuses user-select:text; delete this view after native selection lands.
// Complete UIKit scroll ownership keeps full notice selection native; no synthetic measure loop.
import UIKit
final class T3MobileInformationNotice: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in let view = T3MobileInformationNotice(events: events); try view.setProps(props); return view }
    private let scroll = UIScrollView(), stack = UIStackView(), header = UIStackView(), title = UILabel(), metadata = UILabel(), source = UIButton(type: .system), text = UITextView(), card = UIView()
    private var entry = "", alive = true
    override var view: UIView { scroll }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        scroll.showsVerticalScrollIndicator = false; scroll.contentInsetAdjustmentBehavior = .automatic
        stack.axis = .vertical; stack.spacing = 17.5; stack.translatesAutoresizingMaskIntoConstraints = false; scroll.addSubview(stack)
        title.numberOfLines = 0; metadata.numberOfLines = 0
        source.setTitle("Project source ↗", for: .normal); source.contentHorizontalAlignment = .leading; source.accessibilityTraits = .link
        source.addAction(UIAction { [weak self] _ in guard let self, alive, !entry.isEmpty else { return }; events.change(entry) }, for: .touchUpInside)
        source.heightAnchor.constraint(greaterThanOrEqualToConstant: 42).isActive = true
        text.isEditable = false; text.isSelectable = true; text.isScrollEnabled = false; text.backgroundColor = .clear; text.textContainerInset = .zero; text.textContainer.lineFragmentPadding = 0
        text.translatesAutoresizingMaskIntoConstraints = false; card.addSubview(text); card.layer.cornerRadius = 24
        header.axis = .vertical; header.spacing = 7; header.isLayoutMarginsRelativeArrangement = true; header.layoutMargins = UIEdgeInsets(top: 0, left: 3.5, bottom: 0, right: 3.5)
        [title, metadata, source].forEach { header.addArrangedSubview($0) }; stack.addArrangedSubview(header); stack.addArrangedSubview(card)
        NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 17.5), stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -17.5),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 14), stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -36), stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -35),
            text.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 14), text.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -14), text.topAnchor.constraint(equalTo: card.topAnchor, constant: 14), text.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14)])
    }
    override func setProps(_ props: [String: String]) throws {
        entry = props["entry-id"] ?? ""; title.text = props["notice-name"] ?? ""; metadata.text = props["notice-metadata"] ?? ""; text.text = props["notice-text"] ?? ""
        let size = CGFloat(Double(props["notice-point-size"] ?? "14") ?? 14), line = CGFloat(Double(props["notice-line-height"] ?? "21") ?? 21)
        let titleSize = CGFloat(Double(props["notice-title-size"] ?? "21") ?? 21)
        title.font = UIFont(name: "DMSans-Bold", size: titleSize) ?? .systemFont(ofSize: titleSize, weight: .bold); metadata.font = UIFont(name: "DMSans-Regular", size: size) ?? .systemFont(ofSize: size); source.titleLabel?.font = UIFont(name: "DMSans-Medium", size: size) ?? .systemFont(ofSize: size, weight: .medium)
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = line; paragraph.maximumLineHeight = line
        text.attributedText = NSAttributedString(string: text.text, attributes: [.font: UIFont.monospacedSystemFont(ofSize: size, weight: .regular), .foregroundColor: color(props["notice-foreground"]), .paragraphStyle: paragraph])
        title.textColor = color(props["notice-foreground"]); metadata.textColor = color(props["notice-muted"]); source.setTitleColor(color(props["notice-link"]), for: .normal)
        scroll.backgroundColor = color(props["notice-sheet"]); card.backgroundColor = color(props["notice-card"]); source.isHidden = props["has-source"] != "true"
        scroll.contentInset.bottom = max(0, CGFloat(Double(props["notice-bottom"] ?? "36") ?? 36) - 36)
    }
    private func color(_ value: String?) -> UIColor {
        guard let value, value.hasPrefix("#"), let n = UInt64(value.dropFirst(), radix: 16) else { return .label }
        let rgb = value.count == 9 ? n >> 8 : n; return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: value.count == 9 ? CGFloat(n & 255) / 255 : 1)
    }
    override func destroy() { alive = false; text.resignFirstResponder() }
}
#endif
