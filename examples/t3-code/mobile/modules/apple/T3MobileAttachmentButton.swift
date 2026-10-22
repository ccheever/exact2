#if os(iOS)
// ComposerAttachmentButton and the pinned menu UIButton, upstream365aa87982.
// @ref llp/1024-native-modules.rfc.md#design
import UIKit

private final class T3AttachmentMenuButton: UIButton {
    private var presented = false
    private var pending: (() -> Void)?
    override var isEnabled: Bool { didSet { updateAlpha() } }
    override var isHighlighted: Bool { didSet { updateAlpha() } }
    private func updateAlpha() { alpha = isEnabled ? (isHighlighted ? 0.7 : 1) : 0.5 }
    func assign(_ operation: @escaping () -> Void) {
        if presented { pending = operation } else { operation() }
    }
    private func flush() { let operation = pending; pending = nil; operation?() }
    override func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        if pending != nil { presented = false; flush() }
        let configuration = super.contextMenuInteraction(interaction, configurationForMenuAtLocation: location)
        presented = configuration != nil
        return configuration
    }
    override func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willDisplayMenuFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        super.contextMenuInteraction(interaction, willDisplayMenuFor: configuration, animator: animator)
        presented = true
    }
    override func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willEndFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        super.contextMenuInteraction(interaction, willEndFor: configuration, animator: animator)
        presented = false; flush()
    }
    func retire() { pending = nil; presented = false; menu = nil }
}

final class T3MobileAttachmentButton: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3MobileAttachmentButton(events: events); try instance.setProps(props); return instance
    }
    private struct Identity: Equatable {
        let owner: String, admission: String, mountId: String, visit: String, url: String
        var fields: [String: String] { ["owner": owner, "admission": admission, "mountId": mountId, "visit": visit, "url": url] }
    }
    private let button = T3AttachmentMenuButton(type: .system)
    private var identity: Identity?
    private var supportsFiles = false, alive = true
    override var view: UIView { button }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        button.accessibilityLabel = "Add attachment"
        button.setImage(UIImage(systemName: "plus", withConfiguration: UIImage.SymbolConfiguration(pointSize: 20, weight: .regular)), for: .normal)
        button.addAction(UIAction { [weak self] _ in
            guard let self, !supportsFiles, let identity else { return }
            choose("photos", captured: identity)
        }, for: .touchUpInside)
    }
    override func setProps(_ props: [String: String]) throws {
        guard let bytes = props["configuration"]?.data(using: .utf8),
              let value = try JSONSerialization.jsonObject(with: bytes) as? [String: Any],
              let owner = value["owner"] as? String, let admission = value["admission"] as? String,
              let mountId = value["mountId"] as? String, let visit = value["visit"] as? String, let url = value["url"] as? String,
              let enabled = value["enabled"] as? Bool, let files = value["supportsFiles"] as? Bool else {
            throw ExactNativeRefusal("Invalid composer attachment button.")
        }
        let next = Identity(owner: owner, admission: admission, mountId: mountId, visit: visit, url: url)
        identity = next; supportsFiles = files
        button.tintColor = try T3SymbolView.color(props["button-tint"] ?? "#27272a")
        button.isEnabled = enabled && !owner.isEmpty && !visit.isEmpty && !url.isEmpty && props["button-disabled"] != "true"
        let menu = files ? UIMenu(children: [("photos", "Photo Library", "photo"), ("files", "Choose Files", "folder")].map { source, title, symbol in
            UIAction(title: title, image: UIImage(systemName: symbol)) { [weak self] _ in self?.choose(source, captured: next) }
        }) : nil
        // Assigning a new menu during presentation dismisses UIKit's current menu.
        // Keep its native interaction stable through unrelated app observations.
        button.assign { [weak self] in
            guard let self, alive else { return }
            button.menu = menu; button.showsMenuAsPrimaryAction = files
        }
    }
    private func choose(_ source: String, captured: Identity) {
        guard alive, identity == captured, button.isEnabled, button.window != nil, !button.isHidden,
              button.bounds.width > 0, button.bounds.height > 0, source == "photos" || source == "files" && supportsFiles,
              let bytes = try? JSONSerialization.data(withJSONObject: captured.fields.merging(["source": source], uniquingKeysWith: { _, new in new })) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    override func destroy() { alive = false; identity = nil; button.isEnabled = false; button.retire() }
}
#endif
