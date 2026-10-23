#if os(macOS)
import AppKit
import CoreText

/// The bounded bar face UIKit's ChipFace draws: text, symbols and sized dots.
/// Read styles because a projected header has no laid-out page geometry.
struct MacBarFace {
    private let parts: [(CGSize, CGFloat, (CGRect) -> Void)]
    private let gap: CGFloat

    init?(_ node: NodeView, readout: Bool = false) {
        guard !node.isNativeButton else { return nil }
        var parts: [(CGSize, CGFloat, (CGRect) -> Void)] = []
        var dotted = false, typed = false, valid = true, gap: CGFloat?
        func children(_ node: NodeView) -> [NodeView] {
            node.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
        }
        func ink(_ node: NodeView, _ key: String, authored: Bool) -> NSColor {
            guard authored, let light = node.channels(key, dark: false), let dark = node.channels(key, dark: true) else { return .labelColor }
            return NSColor(name: nil) { appearance in
                TextEngine.color(appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light)
            }
        }
        func visit(_ child: NodeView, fonted: Bool, inked: Bool, alpha: CGFloat) {
            let fonted = fonted || ["font_family", "font_size", "font_weight"].contains(where: child.wrote)
            let inked = inked || child.wrote("text_color")
            if child.isParagraph {
                let text = child.accessibleText
                guard !text.isEmpty else { return }
                typed = typed || fonted
                var font = fonted ? child.text?.font(size: child.number("font_size", PageFacts.defaultRootFontSize),
                    weight: Int(child.number("font_weight", 400)), family: Int(child.number("font_family")),
                    italic: child.style["font_style"]?.string == "italic") ?? NSFont.systemFont(ofSize: child.number("font_size", 13))
                    : NSFont.systemFont(ofSize: NSFont.systemFontSize, weight: DisplayPreferences.boldText ? .semibold : .regular)
                if Int(child.number("font_variant_numeric")) & 1 != 0 {
                    let settings = [[kCTFontOpenTypeFeatureTag: "tnum", kCTFontOpenTypeFeatureValue: 1]] as CFArray
                    let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontFeatureSettingsAttribute: settings] as CFDictionary)
                    font = CTFontCreateCopyWithAttributes(font as CTFont, font.pointSize, nil, descriptor) as NSFont
                }
                let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: ink(child, "text_color", authored: inked)]
                let size = (text as NSString).size(withAttributes: attrs)
                parts.append((CGSize(width: ceil(size.width), height: ceil(size.height)), alpha,
                    { rect in (text as NSString).draw(at: rect.origin, withAttributes: attrs) }))
            } else if child.kind == "image", let symbol = child.faceSymbol {
                let color = ink(child, "tint_color", authored: inked || child.wrote("tint_color"))
                let image = FaceSymbol.image(symbol, points: child.number("font_size", 13))
                parts.append((image?.size ?? .zero, alpha, { rect in
                    if let image { FaceSymbol.tinted(image, color).draw(in: rect) }
                }))
            } else if children(child).isEmpty, child.kind == "view", child.channels("background_color") != nil,
                      let width = child.style["width"]?.number, let height = child.style["height"]?.number, width > 0, height > 0 {
                dotted = true
                let fill = ink(child, "background_color", authored: true)
                let size = CGSize(width: width, height: height)
                let corners = child.cornerSizes(in: CGRect(origin: .zero, size: size))
                parts.append((size, alpha, { rect in
                    guard let context = NSGraphicsContext.current?.cgContext else { return }
                    context.addPath(BorderPaint.roundedRect(rect, corners, shape: nil))
                    context.setFillColor(fill.cgColor); context.fillPath()
                }))
            } else if child.kind == "view", !child.pressable, child.props["popover"] == nil {
                walk(child, fonted: fonted, inked: inked, alpha: alpha)
            } else { valid = false }
        }
        func walk(_ parent: NodeView, fonted: Bool, inked: Bool, alpha: CGFloat) {
            let kids = children(parent)
            if kids.count > 1, gap == nil, let value = parent.style["column_gap"]?.number { gap = CGFloat(value) }
            for child in kids { visit(child, fonted: fonted, inked: inked, alpha: alpha * child.alphaValue) }
        }
        if readout { visit(node, fonted: false, inked: false, alpha: node.alphaValue) }
        else { walk(node, fonted: ["font_family", "font_size", "font_weight"].contains(where: node.wrote), inked: node.wrote("text_color"), alpha: 1) }
        guard valid, !parts.isEmpty, readout || dotted || typed else { return nil }
        self.parts = parts; self.gap = gap ?? 0
    }

    var image: NSImage {
        let height = parts.map { $0.0.height }.max() ?? 0
        let width = parts.map { $0.0.width }.reduce(0, +) + gap * CGFloat(max(0, parts.count - 1))
        return NSImage(size: CGSize(width: max(1, width), height: max(1, height)), flipped: true) { _ in
            var x: CGFloat = 0
            for (size, alpha, paint) in parts {
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current?.cgContext.setAlpha(alpha)
                paint(CGRect(x: x, y: (height - size.height) / 2, width: size.width, height: size.height))
                NSGraphicsContext.restoreGraphicsState()
                x += size.width + self.gap
            }
            return true
        }
    }
}
#endif
