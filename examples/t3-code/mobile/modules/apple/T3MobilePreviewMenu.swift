#if os(iOS)
// Pinned mobile browser/device menu presentation over captured stream targets.
// @ref llp/1107-t3-code-ios.rfc.md#architecture
import UIKit
final class T3MobilePreviewMenu: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let view = T3MobilePreviewMenu(events: events); try view.setProps(props); return view
    }
    private let button = UIButton(type: .system)
    private var owner = "", target = ""
    private var alive = true
    override var view: UIView { button }
    override init(events: ExactNativeEvents) { super.init(events: events); button.showsMenuAsPrimaryAction = true }
    override func setProps(_ props: [String: String]) throws {
        guard let bytes = props["configuration"]?.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: bytes) as? [String: Any] else { throw ExactNativeRefusal("Invalid preview menu.") }
        owner = config["owner"] as? String ?? ""; target = config["target"] as? String ?? ""
        button.setImage(UIImage(systemName: config["symbol"] as? String ?? "ellipsis"), for: .normal)
        button.accessibilityLabel = config["title"] as? String
        let rows = config["items"] as? [[String: Any]] ?? []
        button.isEnabled = !owner.isEmpty && !rows.isEmpty
        var children: [UIMenuElement] = [], groups: [String] = []
        for row in rows {
            let group = row["group"] as? String ?? ""
            if group.isEmpty { if let item = action(row) { children.append(item) } }
            else if !groups.contains(group) {
                groups.append(group)
                children.append(UIMenu(title: group, children: rows.filter { $0["group"] as? String == group }.compactMap(action)))
            }
        }
        button.menu = UIMenu(children: children)
    }
    private func action(_ row: [String: Any]) -> UIAction? {
        guard let title = row["title"] as? String, let operation = row["operation"] as? String else { return nil }
        let capturedOwner = owner, capturedTarget = target
        var attributes: UIMenuElement.Attributes = []
        if row["disabled"] as? Bool == true { attributes.insert(.disabled) }
        if row["destructive"] as? Bool == true { attributes.insert(.destructive) }
        let item = UIAction(title: title, image: UIImage(systemName: row["symbol"] as? String ?? ""), attributes: attributes,
            state: row["selected"] as? Bool == true ? .on : .off) { [weak self] _ in
            guard let self, alive, owner == capturedOwner, target == capturedTarget,
                  let bytes = try? JSONSerialization.data(withJSONObject: ["owner": capturedOwner, "target": capturedTarget,
                    "operation": operation, "value": row["value"] as? String ?? ""]) else { return }
            events.change(String(decoding: bytes, as: UTF8.self))
        }
        item.subtitle = row["subtitle"] as? String
        return item
    }
    override func destroy() { alive = false; button.menu = nil }
}
#endif
