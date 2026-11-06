#if os(iOS)
// GAP 005: iOS Contract text refuses user-select:text; delete this view after native selection lands.
// UIKit owns scrolling and whole-paragraph Copy; no synthetic measure loop.
import UIKit

// Primary365 leaves text at RN's 14 pt default; Expo fits a system-size symbol in a 16 pt view.
private final class T3InformationSourceButton: UIButton {
    let label = UILabel()
    private let symbol = UIImageView()
    private let symbolBox = UIView()
    override var isHighlighted: Bool { didSet { alpha = isHighlighted ? 0.6 : 1 } }
    override var intrinsicContentSize: CGSize {
        CGSize(width: label.intrinsicContentSize.width + 7 + 16, height: max(42, label.intrinsicContentSize.height + 14))
    }
    override init(frame: CGRect) {
        super.init(frame: frame)
        label.text = "Project source"; label.font = UIFont(name: "DMSans-Medium", size: 14) ?? .systemFont(ofSize: 14, weight: .medium)
        symbol.contentMode = .scaleAspectFit
        symbol.isAccessibilityElement = false; symbolBox.isUserInteractionEnabled = false; symbolBox.addSubview(symbol)
        [label, symbolBox].forEach { $0.translatesAutoresizingMaskIntoConstraints = false; $0.isAccessibilityElement = false; addSubview($0) }
        NSLayoutConstraint.activate([label.leadingAnchor.constraint(equalTo: leadingAnchor), label.centerYAnchor.constraint(equalTo: centerYAnchor),
            label.topAnchor.constraint(greaterThanOrEqualTo: topAnchor, constant: 7), label.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -7),
            symbolBox.leadingAnchor.constraint(equalTo: label.trailingAnchor, constant: 7), symbolBox.trailingAnchor.constraint(equalTo: trailingAnchor), symbolBox.centerYAnchor.constraint(equalTo: centerYAnchor),
            symbolBox.widthAnchor.constraint(equalToConstant: 16), symbolBox.heightAnchor.constraint(equalToConstant: 16), heightAnchor.constraint(greaterThanOrEqualToConstant: 42)])
        isAccessibilityElement = true; accessibilityTraits = .link; accessibilityLabel = "Project source, arrow.up.right"; accessibilityHint = "Opens the project website"
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func layoutSubviews() {
        super.layoutSubviews()
        // Expo assigns the image frame directly; Auto Layout instead sizes its SF Symbol alignment rect.
        symbol.frame = symbolBox.bounds
    }
    func setColor(_ tint: UIColor) {
        label.textColor = tint
        guard let image = UIImage(systemName: "arrow.up.right") else { return }
        symbol.image = image
        symbol.preferredSymbolConfiguration = UIImage.SymbolConfiguration(pointSize: UIFont.systemFontSize, weight: .semibold, scale: .unspecified).applying(UIImage.SymbolConfiguration.preferringMonochrome())
        symbol.image = image.withTintColor(tint, renderingMode: .alwaysOriginal); symbol.tintColor = tint
    }
}

final class T3MobileInformationNotice: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in let view = T3MobileInformationNotice(events: events); try view.setProps(props); return view }
    private let scroll = UIScrollView(), stack = UIStackView(), header = UIStackView(), title = UILabel(), metadata = UILabel(), source = T3InformationSourceButton(type: .custom), text = T3SelectableWorkText(frame: .zero), card = UIView()
    private var entry = "", alive = true
    override var view: UIView { scroll }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        scroll.showsVerticalScrollIndicator = false; scroll.contentInsetAdjustmentBehavior = .automatic
        stack.axis = .vertical; stack.spacing = 17.5; stack.translatesAutoresizingMaskIntoConstraints = false; scroll.addSubview(stack)
        title.numberOfLines = 0; metadata.numberOfLines = 0
        source.addAction(UIAction { [weak self] _ in guard let self, alive, !entry.isEmpty else { return }; events.change(entry) }, for: .touchUpInside)
        text.backgroundColor = .clear
        text.allowsCopy = { [weak self] in
            guard let self, alive, !entry.isEmpty, !(text.attributedText?.string.isEmpty ?? true),
                  scroll.window != nil, !scroll.isHidden, scroll.bounds.width > 0, scroll.bounds.height > 0,
                  !text.isHidden, text.bounds.width > 0, text.bounds.height > 0 else { return false }
            return UIApplication.shared.applicationState == .active
        }
        text.translatesAutoresizingMaskIntoConstraints = false; card.addSubview(text); card.layer.cornerRadius = 24; card.layer.cornerCurve = .continuous
        header.axis = .vertical; header.alignment = .leading; header.spacing = 7; header.isLayoutMarginsRelativeArrangement = true; header.layoutMargins = UIEdgeInsets(top: 0, left: 3.5, bottom: 0, right: 3.5)
        [title, metadata, source].forEach { header.addArrangedSubview($0) }; stack.addArrangedSubview(header); stack.addArrangedSubview(card)
        NSLayoutConstraint.activate([title.widthAnchor.constraint(equalTo: header.layoutMarginsGuide.widthAnchor), metadata.widthAnchor.constraint(equalTo: header.layoutMarginsGuide.widthAnchor),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 17.5), stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -17.5),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 14), stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -36), stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -35),
            text.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 14), text.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -14), text.topAnchor.constraint(equalTo: card.topAnchor, constant: 14), text.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14)])
    }
    override func setProps(_ props: [String: String]) throws {
        let nextEntry = props["entry-id"] ?? ""
        title.text = props["notice-name"] ?? ""; metadata.text = props["notice-metadata"] ?? ""
        let size = CGFloat(Double(props["notice-point-size"] ?? "14") ?? 14), line = CGFloat(Double(props["notice-line-height"] ?? "21") ?? 21)
        let titleSize = CGFloat(Double(props["notice-title-size"] ?? "26") ?? 26), titleLine = CGFloat(Double(props["notice-title-line-height"] ?? "32") ?? 32)
        let titleFont = UIFont(name: "DMSans-Bold", size: titleSize) ?? .systemFont(ofSize: titleSize, weight: .bold), metadataFont = UIFont(name: "DMSans-Regular", size: size) ?? .systemFont(ofSize: size)
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = line; paragraph.maximumLineHeight = line
        let titleParagraph = NSMutableParagraphStyle(); titleParagraph.minimumLineHeight = titleLine; titleParagraph.maximumLineHeight = titleLine
        title.attributedText = NSAttributedString(string: title.text ?? "", attributes: [.font: titleFont, .foregroundColor: color(props["notice-foreground"]), .paragraphStyle: titleParagraph])
        metadata.attributedText = NSAttributedString(string: metadata.text ?? "", attributes: [.font: metadataFont, .foregroundColor: color(props["notice-muted"]), .paragraphStyle: paragraph])
        let notice = NSAttributedString(string: props["notice-text"] ?? "", attributes: [.font: UIFont.systemFont(ofSize: size, weight: .regular), .foregroundColor: color(props["notice-foreground"]), .paragraphStyle: paragraph])
        if entry != nextEntry || text.attributedText?.isEqual(to: notice) != true { text.cancelMenu(); text.attributedText = notice }
        entry = nextEntry
        source.setColor(color(props["notice-link"]))
        scroll.backgroundColor = color(props["notice-sheet"]); card.backgroundColor = color(props["notice-card"]); source.isHidden = props["has-source"] != "true"
        scroll.contentInset.bottom = max(0, CGFloat(Double(props["notice-bottom"] ?? "36") ?? 36) - 36)
    }
    private func color(_ value: String?) -> UIColor { T3HomeChrome.color(value ?? "") }
    override func destroy() { alive = false; text.cancelMenu(); text.allowsCopy = nil }
}
#endif
