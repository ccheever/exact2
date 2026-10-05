// `<glass-composer>` on iOS: the message field as the system draws one, in
// real Liquid Glass. A glass container holds an interactive glass capsule with
// a native text view in it, and UIKit's prominent glass send button beside
// the text, so the two blend as the system's own glass shapes do. It grows
// with its text up to six lines and says how tall it wants to be.
//
// Typing stays here: each keystroke reaching the app made it re-render the
// whole conversation (14 ms a key, median, on an M-series simulator), so the
// app hears the text only when it matters.
//
// Props: `draft` (the saved draft, or "" once a message is sent), `prompt`
// (the placeholder), `editable` (`false` takes no typing), `sendable`
// (`false`: the session can't take a message; send is also dimmed while the
// text is empty). Events: `message` with `h:<points>` when the height it wants
// changes and `s:<text>` when send is pressed (the field clears itself);
// `change` with the text when editing ends, to keep the draft.
import Foundation

#if os(iOS)
import UIKit

final class GlassComposer: ExactNativeInstance {
    private static let font = UIFont.preferredFont(forTextStyle: .body)
    private static let minHeight: CGFloat = 44
    private static let maxLines: CGFloat = 6
    private static let send: CGFloat = 34
    private static let inset: CGFloat = 5

    private let host = ComposerHost()
    private let container: UIVisualEffectView
    private let field: UIVisualEffectView
    private let text = UITextView()
    private let placeholder = UILabel()
    private let button = UIButton(type: .system)
    /// What this view last told the app, so the app's echo of it is no edit.
    private var heard = ""
    private var reported: CGFloat = 0
    private var sendable = false
    private lazy var delegate = Delegate(self)

    init(props: [String: String], events: ExactNativeEvents) {
        if #available(iOS 26.0, *) {
            let glass = UIGlassEffect(style: .regular)
            glass.isInteractive = true
            field = UIVisualEffectView(effect: glass)
            let group = UIGlassContainerEffect()
            group.spacing = 8
            container = UIVisualEffectView(effect: group)
        } else {
            field = UIVisualEffectView(effect: UIBlurEffect(style: .systemThinMaterial))
            container = UIVisualEffectView(effect: nil)
        }
        super.init(events: events)

        text.font = Self.font
        text.adjustsFontForContentSizeCategory = true
        text.backgroundColor = .clear
        text.textColor = .label
        text.isScrollEnabled = false
        text.textContainerInset = UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
        text.textContainer.lineFragmentPadding = 0
        text.delegate = delegate
        placeholder.font = Self.font
        placeholder.adjustsFontForContentSizeCategory = true
        placeholder.textColor = .placeholderText
        placeholder.isUserInteractionEnabled = false
        field.contentView.addSubview(text)
        field.contentView.addSubview(placeholder)

        var config: UIButton.Configuration
        if #available(iOS 26.0, *) { config = .prominentGlass() } else { config = .borderedProminent(); config.cornerStyle = .capsule }
        config.image = UIImage(systemName: "arrow.up", withConfiguration: UIImage.SymbolConfiguration(textStyle: .subheadline).applying(UIImage.SymbolConfiguration(weight: .bold)))
        config.contentInsets = .zero
        button.configuration = config
        button.accessibilityLabel = "Send"
        button.addAction(UIAction { [weak self] _ in self?.send() }, for: .primaryActionTriggered)

        container.contentView.addSubview(field)
        container.contentView.addSubview(button)
        host.addSubview(container)
        host.laidOut = { [weak self] in self?.layout() }
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { host }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        let value = props["draft"] ?? ""
        // The app's draft wins when it isn't this view's own echo: a sent
        // message clears it, a retry puts text back.
        if value != heard, value != text.text {
            text.text = value
            heard = value
            layout()
        }
        placeholder.text = props["prompt"] ?? "Message"
        text.isEditable = props["editable"] != "false"
        text.accessibilityLabel = props["prompt"] ?? "Message"
        sendable = props["sendable"] == "true"
        refresh()
    }

    private func refresh() {
        placeholder.isHidden = !text.text.isEmpty
        button.isEnabled = sendable && !text.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    fileprivate func edited() {
        refresh()
        layout()
    }

    /// Editing ended (the field lost focus): the app keeps the draft.
    fileprivate func ended() {
        guard text.text != heard else { return }
        heard = text.text
        events.change(text.text)
    }

    private func send() {
        let message = text.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard sendable, !message.isEmpty else { return }
        text.text = ""
        heard = ""
        refresh()
        layout()
        events.message("s:" + message)
    }

    /// The capsule fills the box; the text wraps beside the send button and
    /// scrolls past six lines. The height it needs goes to the app.
    private func layout() {
        let bounds = host.bounds
        guard bounds.width > 0 else { return }
        container.frame = bounds
        field.frame = bounds
        let radius = Self.minHeight / 2
        if #available(iOS 26.0, *) {
            field.cornerConfiguration = .corners(radius: .fixed(Double(radius)))
        } else {
            field.layer.cornerRadius = radius
            field.clipsToBounds = true
        }
        let left: CGFloat = 16
        let right = Self.inset * 2 + Self.send
        let width = max(1, bounds.width - left - right)
        let line = Self.font.lineHeight
        let wanted = ceil(text.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height)
        let capped = min(wanted, line * Self.maxLines)
        text.isScrollEnabled = wanted > capped
        let vertical = max(0, (Self.minHeight - line) / 2)
        text.frame = CGRect(x: left, y: vertical, width: width, height: max(line, bounds.height - vertical * 2))
        placeholder.frame = CGRect(x: left, y: vertical, width: width, height: line)
        button.frame = CGRect(x: bounds.width - Self.inset - Self.send, y: bounds.height - Self.inset - Self.send, width: Self.send, height: Self.send)
        let height = max(Self.minHeight, capped + vertical * 2)
        if abs(height - reported) >= 0.5 {
            reported = height
            events.message(String(format: "h:%.0f", height))
        }
    }

    private final class Delegate: NSObject, UITextViewDelegate {
        weak var owner: GlassComposer?
        init(_ owner: GlassComposer) { self.owner = owner }
        func textViewDidChange(_ textView: UITextView) { owner?.edited() }
        func textViewDidEndEditing(_ textView: UITextView) { owner?.ended() }
    }
}

/// Lays the composer out whenever the app resizes its box.
private final class ComposerHost: UIView {
    var laidOut: (() -> Void)?
    override func layoutSubviews() {
        super.layoutSubviews()
        laidOut?()
    }
}
#endif
