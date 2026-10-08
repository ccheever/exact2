// `<menu-picker>` on iOS: a choice made from UIKit's own pull-down menu.
// Three looks, as `kind` says:
//   - "inline" (the default): an SF Symbol, the choice, and ⇕, as the
//     pickers over a composer;
//   - "row": the setting's name at the leading edge and the choice with ⇕ at
//     the trailing, as a settings sheet's row;
//   - "link": `primary` then `secondary` (dimmer) and ›, centered; pressing
//     it reports `change("press")` instead of opening a menu.
// Props: `symbol` and its `tint` ("#rrggbb", or "label"), or `mark` (a
// coding harness: its logo, ProviderMarks.swift); `primary`, `secondary`,
// `kind`, `enabled`, and `menu`, a JSON list of `{"id", "title",
// "selected"}`, each with an optional `subtitle` (the menu's smaller second
// line), and `symbol`, `tint` or `mark`. Events: `message` with the
// chosen item's id; `change` ("press") for a link.
import Foundation

#if os(iOS)
import UIKit

final class MenuPicker: ExactNativeInstance {
    private struct Item: Decodable {
        let id: String
        let title: String
        var selected: Bool?
        var symbol: String?
        var tint: String?
        var mark: String?
        var subtitle: String?
    }

    private var items: [Item] = []

    /// `tint` as a colour: "#rrggbb", else the text colour.
    static func color(_ tint: String?) -> UIColor {
        guard let tint, tint.hasPrefix("#"), tint.count == 7, let value = UInt32(tint.dropFirst(), radix: 16) else {
            return .label
        }
        return UIColor(red: CGFloat((value >> 16) & 0xff) / 255, green: CGFloat((value >> 8) & 0xff) / 255,
                       blue: CGFloat(value & 0xff) / 255, alpha: 1)
    }

    /// An SF Symbol drawn in its tint, kept so (a menu would recolour it).
    static func mark(_ symbol: String?, _ tint: String?) -> UIImage? {
        guard let symbol, !symbol.isEmpty else { return nil }
        return UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(textStyle: .body))?
            .withTintColor(color(tint), renderingMode: .alwaysOriginal)
    }

    private let button = UIButton(type: .system)
    private var applied: [String: String] = [:]

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        button.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            if self.applied["kind"] == "link" {
                self.events.change("press")
            }
        }, for: .primaryActionTriggered)
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { button }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        guard props != applied else { return }
        applied = props
        let kind = props["kind"] ?? "inline"
        let primary = props["primary"] ?? ""
        let secondary = props["secondary"] ?? ""
        var config = UIButton.Configuration.plain()
        config.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 0, bottom: 6, trailing: 0)
        let body = UIFont.preferredFont(forTextStyle: .body)
        switch kind {
        case "row":
            // The name on the leading edge; the choice and ⇕ trail it.
            var title = AttributedString(primary)
            title.font = body
            title.foregroundColor = UIColor.label
            config.attributedTitle = title
            var value = AttributedString(secondary)
            value.font = body
            value.foregroundColor = UIColor.secondaryLabel
            button.configuration = config
            trailingValue(value)
            button.contentHorizontalAlignment = .leading
        case "link":
            var title = AttributedString(primary)
            title.font = UIFont.preferredFont(forTextStyle: .headline)
            title.foregroundColor = UIColor.label
            if !secondary.isEmpty {
                var rest = AttributedString(" " + secondary)
                rest.font = UIFont.preferredFont(forTextStyle: .headline)
                rest.foregroundColor = UIColor.secondaryLabel
                title += rest
            }
            config.attributedTitle = title
            config.image = UIImage(systemName: "chevron.right", withConfiguration: UIImage.SymbolConfiguration(textStyle: .subheadline, scale: .small).applying(UIImage.SymbolConfiguration(weight: .semibold)))
            config.imagePlacement = .trailing
            config.imagePadding = 6
            config.baseForegroundColor = .secondaryLabel
            button.configuration = config
            button.contentHorizontalAlignment = .center
            trailingValue(nil)
        default:
            var title = AttributedString(primary)
            title.font = body
            title.foregroundColor = UIColor.label
            config.attributedTitle = title
            if let image = ProviderMarks.image(props["mark"]) ?? Self.mark(props["symbol"], props["tint"]) {
                config.image = image
                config.imagePadding = 12
            }
            config.baseForegroundColor = .label
            // ⇕ after the title, as the system's pull-down buttons draw it.
            config.indicator = .popup
            button.configuration = config
            button.contentHorizontalAlignment = .leading
            trailingValue(nil)
        }
        button.isEnabled = props["enabled"] != "false"
        button.accessibilityLabel = [primary, secondary].filter { !$0.isEmpty }.joined(separator: ", ")
        items = (props["menu"]?.data(using: .utf8)).flatMap { try? JSONDecoder().decode([Item].self, from: $0) } ?? []
        if kind != "link", !items.isEmpty {
            button.menu = UIMenu(children: items.map { item in
                UIAction(title: item.title, subtitle: item.subtitle, image: ProviderMarks.image(item.mark) ?? Self.mark(item.symbol, item.tint), state: item.selected == true ? .on : .off) { [weak self] _ in
                    self?.events.message(item.id)
                }
            })
            button.showsMenuAsPrimaryAction = true
            button.changesSelectionAsPrimaryAction = false
        } else {
            button.menu = nil
            button.showsMenuAsPrimaryAction = false
        }
    }

    /// The choice at a row's trailing edge (nil: none), with ⇕.
    private var valueLabel: UILabel?
    private var valueIcon: UIImageView?
    private func trailingValue(_ value: AttributedString?) {
        guard let value else {
            valueLabel?.removeFromSuperview()
            valueIcon?.removeFromSuperview()
            valueLabel = nil
            valueIcon = nil
            return
        }
        let label = valueLabel ?? UILabel()
        let icon = valueIcon ?? UIImageView(image: UIImage(systemName: "chevron.up.chevron.down", withConfiguration: UIImage.SymbolConfiguration(textStyle: .footnote).applying(UIImage.SymbolConfiguration(weight: .semibold))))
        label.attributedText = NSAttributedString(value)
        icon.tintColor = .secondaryLabel
        if valueLabel == nil {
            for view in [label, icon] {
                view.translatesAutoresizingMaskIntoConstraints = false
                view.isUserInteractionEnabled = false
                button.addSubview(view)
            }
            NSLayoutConstraint.activate([
                icon.trailingAnchor.constraint(equalTo: button.trailingAnchor),
                icon.centerYAnchor.constraint(equalTo: button.centerYAnchor),
                label.trailingAnchor.constraint(equalTo: icon.leadingAnchor, constant: -6),
                label.centerYAnchor.constraint(equalTo: button.centerYAnchor),
            ])
        }
        valueLabel = label
        valueIcon = icon
    }
}
#endif
