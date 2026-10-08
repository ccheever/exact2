// `<glass-button>` on iOS: UIKit's own Liquid Glass button, not a box with
// glass behind it. `UIButton.Configuration.glass()` (`.prominentGlass()` when
// `prominent` is `true`) brings the system's whole control: the glass that
// swells and lights under a finger, the press that cancels when the finger
// slides off, the highlight, the accessibility, and a menu attached the native
// way. Props: `symbol` (an SF Symbol name), `said` (what VoiceOver says),
// `prominent`, `enabled` (`false` dims it and takes no touches), and `menu`, a
// JSON list of `{"id", "title", "destructive"}` that the button opens instead
// of being pressed. Events: `change` ("press") when pressed, `message` (the
// item's id) when a menu item is chosen. `token`, when set, is what `change`
// carries instead of "press".
import Foundation

#if os(iOS)
import UIKit

final class GlassButton: ExactNativeInstance {
    private struct Item: Decodable {
        let id: String
        let title: String
        var destructive: Bool?
    }

    private let button = UIButton(type: .system)
    private var applied: [String: String] = [:]
    /// What a press reports (`token`), so a row's button can say which row.
    private var token = "press"

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        button.addAction(UIAction { [weak self] _ in self?.events.change(self?.token ?? "press") }, for: .primaryActionTriggered)
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { button }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        guard props != applied else { return }
        applied = props
        token = props["token"] ?? "press"
        var config: UIButton.Configuration
        if #available(iOS 26.0, *), props["prominent"] != "true" {
            config = .glass()
        } else if props["prominent"] == "true" {
            // A filled blue circle, as the composer's send.
            config = .borderedProminent()
            config.cornerStyle = .capsule
        } else {
            config = props["prominent"] == "true" ? .borderedProminent() : .bordered()
            config.cornerStyle = .capsule
        }
        let prominent = props["prominent"] == "true"
        config.image = UIImage(systemName: props["symbol"] ?? "circle",
                               withConfiguration: UIImage.SymbolConfiguration(textStyle: prominent ? .subheadline : .body, scale: .medium)
                                   .applying(UIImage.SymbolConfiguration(weight: .semibold)))
        config.contentInsets = .zero
        button.configuration = config
        button.accessibilityLabel = props["said"]
        button.isEnabled = props["enabled"] != "false"
        if let json = props["menu"], let data = json.data(using: .utf8),
           let items = try? JSONDecoder().decode([Item].self, from: data), !items.isEmpty {
            button.menu = UIMenu(children: items.map { item in
                UIAction(title: item.title, attributes: item.destructive == true ? .destructive : []) { [weak self] _ in
                    self?.events.message(item.id)
                }
            })
            button.showsMenuAsPrimaryAction = true
        } else {
            button.menu = nil
            button.showsMenuAsPrimaryAction = false
        }
    }
}
#endif
