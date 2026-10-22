#if os(macOS)
import AppKit

/// Authored toolbar ink uses a native button in the item's public view slot.
final class ToolbarInkMac {
    func button(_ item: NSToolbarItem, node: NodeView, face: ButtonFace?, target: AnyObject, action: Selector) {
        let title = face?.rows.title["text_color"] ?? (node.wrote("text_color") ? node.style["text_color"] : nil)
        let symbol = face?.rows.symbol["tint_color"]
        guard let value = symbol ?? title,
              let cg = value.cgColor(dark: node.drawsDark) else {
            if item.view != nil {
                let title = item.title, image = item.image, target = item.target, action = item.action
                item.view = nil
                item.title = title; item.image = image; item.target = target; item.action = action
            }
            return
        }
        let ink = NSColor(cgColor: cg)
        let button = (item.view as? NSButton) ?? NSButton(title: "", target: target, action: action)
        button.tag = Int(node.id)
        button.bezelStyle = .texturedRounded
        button.title = face?.title ?? item.title
        button.image = item.image
        button.imagePosition = button.title.isEmpty ? .imageOnly : button.image == nil ? .noImage : .imageLeading
        button.contentTintColor = ink
        if let ink, !button.title.isEmpty {
            button.attributedTitle = NSAttributedString(string: button.title, attributes: [.foregroundColor: ink])
        }
        button.isEnabled = !node.disabled && !node.inert
        button.setAccessibilityLabel(item.label)
        button.sizeToFit()
        item.view = button
    }
}
#endif
