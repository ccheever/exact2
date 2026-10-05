// A symbol image on AppKit (`image "symbol:<role>"`, LLP 1011, 1035.004.000):
// the node stays an image leaf; an NSImageView in a clip draws the glyph and
// its tint. Moved out of NodeViewMac.swift (the 1,500-line cap).
#if os(macOS)
import AppKit

private final class SymbolClip: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension NodeView {
    // A symbol remains an image leaf; AppKit owns glyph rendering and tint.
    func clearSymbol() {
        symbolClip?.removeFromSuperview(); symbolClip = nil; symbolView = nil; symbolKey = nil; symbolFound = false
    }
    func updateSymbol() {
        guard kind == "image", let source = imageSource, source.hasPrefix("symbol:") else { return }
        updateRoleAccessibility()
        let name = props["symbolName"] ?? "", points = number("font_size", 16)
        let weights: [NSFont.Weight] = [.ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black]
        let index = min(8, max(0, Int((number("font_weight", 400) / 100).rounded()) - 1))
        let key = "\(source):\(name):\(points):\(index):\(symbolLookKey)"
        if symbolKey != key {
            symbolKey = key; loadGeneration += 1
            let generation = loadGeneration
            image = name.isEmpty ? nil : symbolImage(name, symbolConfiguration(NSImage.SymbolConfiguration(pointSize: points > 0 ? points : 1, weight: weights[index])))
            symbolFound = image != nil; if points <= 0 { image = nil }
            if name.isEmpty, !source.hasPrefix("symbol:sf/"), symbolRefusal != source { symbolRefusal = source; presenter?.session?.log("image \(source) refused: unknown symbol role") }
            if !name.isEmpty || source.hasPrefix("symbol:sf/") { symbolRefusal = nil }
            let leaf = symbolView ?? NSImageView()
            if symbolView == nil {
                let clip = SymbolClip(); clip.wantsLayer = true; clip.layer?.masksToBounds = true
                symbolClip = clip; symbolView = leaf; leaf.wantsLayer = true; clip.addSubview(leaf); addSubview(clip)
            }
            showSymbol(image, on: leaf); leaf.setAccessibilityElement(false)
            let size = image?.size ?? (points > 0 ? CGSize(width: points, height: points) : nil)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.loadGeneration == generation, let presenter = self.presenter,
                      presenter.views[self.id] === self else { return }
                presenter.intrinsic(self.id, size)
            }
        }
        symbolView?.contentTintColor = symbolTint ?? .controlAccentColor
        if let leaf = symbolView { applySymbolEffect(leaf) }
        layoutSymbol()
    }
    func layoutSymbol() {
        guard let leaf = symbolView, let clip = symbolClip else { return }
        let uniform = number("border_width")
        let content = bounds.insetBy(left: number("border_width_left", uniform) + number("padding_left"), top: number("border_width_top", uniform) + number("padding_top"), right: number("border_width_right", uniform) + number("padding_right"), bottom: number("border_width_bottom", uniform) + number("padding_bottom"))
        clip.frame = content; leaf.frame = clip.bounds
        switch style["object_fit"]?.string ?? "fill" {
        case "contain": leaf.imageScaling = .scaleProportionallyUpOrDown
        case "none": leaf.imageScaling = .scaleNone
        case "scale-down": leaf.imageScaling = .scaleProportionallyDown
        case "cover":
            let size = image?.size ?? .zero
            if size.width > 0 && size.height > 0 {
                let ratio = max(content.width / size.width, content.height / size.height)
                leaf.frame = CGRect(x: (content.width - size.width * ratio) / 2, y: (content.height - size.height * ratio) / 2, width: size.width * ratio, height: size.height * ratio)
            }
            leaf.imageScaling = .scaleAxesIndependently
        default: leaf.imageScaling = .scaleAxesIndependently
        }
        let path = roundedPath(in: bounds).cgPath
        var transform = CGAffineTransform(translationX: -content.minX, y: -content.minY)
        let mask = CAShapeLayer(); mask.path = path.copy(using: &transform); clip.layer?.mask = mask
    }
}
#endif
