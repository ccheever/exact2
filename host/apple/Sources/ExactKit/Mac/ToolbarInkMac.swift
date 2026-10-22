#if os(macOS)
import AppKit

/// AppKit's standard title and toolbar item have no public ink property.
/// Authored ink uses native controls through the toolbar's public view slot.
final class ToolbarInkMac {
    private let titleItem = NSToolbarItem(itemIdentifier: .init("exact.authored-title"))
    private let label = NSTextField(labelWithString: "")
    private var titleVisibility: NSWindow.TitleVisibility?

    func title(_ node: NodeView?, window: NSWindow, text: String) -> NSToolbarItem? {
        guard let node, node.wrote("text_color"), let ink = node.cgColor("text_color") else {
            reset(window)
            return nil
        }
        if titleVisibility == nil { titleVisibility = window.titleVisibility }
        window.titleVisibility = .hidden
        label.stringValue = text
        label.font = .titleBarFont(ofSize: 0)
        label.textColor = NSColor(cgColor: ink)
        label.lineBreakMode = .byTruncatingTail
        label.sizeToFit()
        titleItem.label = text
        titleItem.view = label
        return titleItem
    }

    func reset(_ window: NSWindow?) {
        if let titleVisibility { window?.titleVisibility = titleVisibility }
        titleVisibility = nil
    }

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
