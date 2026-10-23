#if os(macOS)
import AppKit

/// Authored toolbar ink uses a native button in the item's public view slot.
final class ToolbarInkMac {
    func button(_ item: NSToolbarItem, node: NodeView, face: ButtonFace?, painted: Bool, enabled: Bool, target: AnyObject, action: Selector) {
        let title = face?.rows.title["text_color"] ?? (node.wrote("text_color") ? node.style["text_color"] : nil)
        let symbol = face?.rows.symbol["tint_color"]
        let titleInk = title?.cgColor(dark: node.drawsDark).flatMap(NSColor.init(cgColor:))
        let symbolInk = symbol?.cgColor(dark: node.drawsDark).flatMap(NSColor.init(cgColor:))
        guard !painted && (titleInk != nil || symbolInk != nil) else {
            if item.view != nil {
                let title = item.title, image = item.image, target = item.target, action = item.action
                item.view = nil
                item.title = title; item.image = image; item.target = target; item.action = action
            }
            return
        }
        let button = (item.view as? NSButton) ?? NSButton(title: "", target: target, action: action)
        button.tag = Int(node.id)
        button.bezelStyle = .texturedRounded
        button.title = face?.title ?? item.title
        button.image = symbolInk.flatMap { ink in item.image.map { FaceSymbol.tinted($0, ink) } } ?? item.image
        button.imagePosition = button.title.isEmpty ? .imageOnly : button.image == nil ? .noImage : .imageLeading
        button.contentTintColor = titleInk
        button.attributedTitle = NSAttributedString(string: button.title,
            attributes: titleInk.map { [.foregroundColor: $0] } ?? [:])
        button.isEnabled = enabled
        button.setAccessibilityLabel(item.label)
        button.sizeToFit()
        item.view = button
    }
}
#endif
