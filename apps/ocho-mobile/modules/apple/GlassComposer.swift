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
// A glass "+" beside it attaches photos and files (ComposerAttachments.swift):
// each uploads as it is picked, shows as a chip over the text, and its path
// on the session's machine follows the text when the message is sent.
//
// Props: `draft` (the saved draft, or "" once a message is sent), `prompt`
// (the placeholder), `editable` (`false` takes no typing), `sendable`
// (`false`: the session can't take a message; send is also dimmed while there
// is nothing to send or an upload is still going), `upload` (the session's
// upload route; no "+" without it) and `auth` (its bearer). Events: `message` with `h:<points>` when the height it wants
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
    private static let plus: CGFloat = 44

    private let host = ComposerHost()
    private let container: UIVisualEffectView
    private let field: UIVisualEffectView
    private let text = UITextView()
    private let placeholder = UILabel()
    private let button = UIButton(type: .system)
    private let attach = UIButton(type: .system)
    private let tray = AttachmentTray()
    private let picker = AttachmentPicker()
    private var attachments: [Attachment] = []
    private var uploadURL: URL?
    private var auth = ""
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
        // A sheet or menu over the app dims every tint beneath it, and inside
        // glass the dimming can outlast it: send stayed grey. It never dims.
        button.tintAdjustmentMode = .normal
        button.addAction(UIAction { [weak self] _ in self?.send() }, for: .primaryActionTriggered)

        var plus: UIButton.Configuration
        if #available(iOS 26.0, *) { plus = .glass() } else { plus = .bordered(); plus.cornerStyle = .capsule }
        plus.image = UIImage(systemName: "plus", withConfiguration: UIImage.SymbolConfiguration(textStyle: .body).applying(UIImage.SymbolConfiguration(weight: .semibold)))
        plus.contentInsets = .zero
        attach.configuration = plus
        attach.accessibilityLabel = "Attach a photo or file"
        attach.menu = picker.menu(from: host)
        attach.showsMenuAsPrimaryAction = true
        picker.picked = { [weak self] data, name, thumbnail in self?.add(data, name: name, thumbnail: thumbnail) }
        picker.failed = { [weak self] why in self?.addFailed(why) }
        tray.removed = { [weak self] id in self?.remove(id) }
        field.contentView.addSubview(tray)

        container.contentView.addSubview(attach)
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
        uploadURL = (props["upload"]).flatMap { $0.isEmpty ? nil : URL(string: $0) }
        auth = props["auth"] ?? ""
        attach.isHidden = uploadURL == nil
        attach.isEnabled = sendable
        refresh()
        layout()
    }

    private func refresh() {
        placeholder.isHidden = !text.text.isEmpty
        let typed = !text.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        let attached = attachments.contains { $0.path != nil }
        let busy = attachments.contains { $0.uploading }
        button.isEnabled = sendable && !busy && (typed || attached)
    }

    // MARK: Attachments

    private func add(_ data: Data, name: String, thumbnail: UIImage?) {
        guard let uploadURL else { return }
        let attachment = Attachment(name: name, thumbnail: thumbnail)
        attachments.append(attachment)
        changedAttachments()
        Uploader.upload(data, name: name, to: uploadURL, auth: auth) { [weak self] outcome in
            switch outcome {
            case .path(let path): attachment.state = .done(path)
            case .failed(let why): attachment.state = .failed(why)
            }
            self?.changedAttachments()
        }
    }

    private func addFailed(_ why: String) {
        let attachment = Attachment(name: why, thumbnail: nil)
        attachment.state = .failed(why)
        attachments.append(attachment)
        changedAttachments()
    }

    private func remove(_ id: UUID) {
        attachments.removeAll { $0.id == id }
        changedAttachments()
    }

    private func changedAttachments() {
        tray.show(attachments)
        refresh()
        layout()
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
        let typed = text.text.trimmingCharacters(in: .whitespacesAndNewlines)
        // The paths follow the text, one word each, as the desktop pastes them.
        let paths = attachments.compactMap(\.path).joined(separator: " ")
        let message = [typed, paths].filter { !$0.isEmpty }.joined(separator: "\n\n")
        guard sendable, !message.isEmpty, !attachments.contains(where: { $0.uploading }) else { return }
        text.text = ""
        heard = ""
        attachments.removeAll()
        changedAttachments()
        events.message("s:" + message)
    }

    /// The capsule fills the box; the text wraps beside the send button and
    /// scrolls past six lines. The height it needs goes to the app.
    private func layout() {
        let bounds = host.bounds
        guard bounds.width > 0 else { return }
        container.frame = bounds
        let offset = attach.isHidden ? 0 : Self.plus + 8
        attach.frame = CGRect(x: 0, y: bounds.height - Self.plus, width: Self.plus, height: Self.plus)
        field.frame = CGRect(x: offset, y: 0, width: max(1, bounds.width - offset), height: bounds.height)
        let radius = Self.minHeight / 2
        if #available(iOS 26.0, *) {
            field.cornerConfiguration = .corners(radius: .fixed(Double(radius)))
        } else {
            field.layer.cornerRadius = radius
            field.clipsToBounds = true
        }
        let left: CGFloat = 16
        let right = Self.inset * 2 + Self.send
        let width = max(1, field.bounds.width - left - right)
        let trayHeight = attachments.isEmpty ? 0 : AttachmentTray.height
        tray.isHidden = attachments.isEmpty
        tray.frame = CGRect(x: 10, y: 4, width: max(1, field.bounds.width - 20), height: trayHeight)
        let line = Self.font.lineHeight
        let wanted = ceil(text.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height)
        let capped = min(wanted, line * Self.maxLines)
        text.isScrollEnabled = wanted > capped
        let vertical = max(0, (Self.minHeight - line) / 2)
        let top = trayHeight + vertical
        text.frame = CGRect(x: left, y: top, width: width, height: max(line, bounds.height - top - vertical))
        placeholder.frame = CGRect(x: left, y: top, width: width, height: line)
        button.frame = CGRect(x: bounds.width - Self.inset - Self.send, y: bounds.height - Self.inset - Self.send, width: Self.send, height: Self.send)
        let height = max(Self.minHeight, capped + vertical * 2) + trayHeight
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
