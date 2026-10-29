// A symbol image on AppKit (`image "symbol:<role>"`, LLP 1011, 1035.004.000):
// the node stays an image leaf; an NSImageView in a clip draws the glyph and
// its tint; a template path's own paths are filled instead (IconShapeMac.swift).
// Moved out of NodeViewMac.swift (the 1,500-line cap).
#if os(macOS)
import AppKit
import ExactCore

private final class SymbolClip: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension NodeView {
    // A symbol remains an image leaf; AppKit owns glyph rendering and tint.
    func clearSymbol() {
        symbolClip?.removeFromSuperview(); symbolClip = nil; symbolView = nil; symbolShape = nil; symbolKey = nil; symbolFound = false
    }
    func updateSymbol() {
        guard kind == "image", let source = imageSource, source.hasPrefix("symbol:") else { return }
        updateRoleAccessibility()
        let name = props["symbolName"] ?? "", points = number("font_size", PageFacts.defaultRootFontSize), weight = number("font_weight", 400)
        let key = "\(source):\(name):\(points):\(weight):\(symbolLookKey)" + (props["symbolFillRule"].map { ":\($0)" } ?? "")
        if symbolKey != key {
            symbolKey = key; loadGeneration += 1
            let generation = loadGeneration
            // An icon with no SF Symbol draws its own path (D10, stage 9 A).
            image = name.isEmpty ? iconPathImage(side: points) : symbolImage(name, SymbolMeasure.configuration(points: points, weight: weight))
            symbolFound = image != nil; if points <= 0 { image = nil }
            if name.isEmpty, props["symbolPath"] == nil, !source.hasPrefix("symbol:sf/"), source != "symbol:", symbolRefusal != source { symbolRefusal = source; presenter?.session?.log("image \(RasterRetry.withoutQuery(source)) refused: unknown symbol role") }
            if !name.isEmpty || source.hasPrefix("symbol:sf/") || source == "symbol:" { symbolRefusal = nil }
            let clip = symbolClip ?? SymbolClip()
            if symbolClip == nil { clip.wantsLayer = true; clip.layer?.masksToBounds = true; symbolClip = clip; addSubview(clip) }
            // A template path is filled as its paths at the frame (stage 14 C1,
            // IconShapeMac.swift); an SF Symbol or a bitmap is an image view's.
            // One leaf in the clip: a source that changes kind swaps it.
            if let commands = props["symbolPath"], name.isEmpty {
                symbolView?.removeFromSuperview(); symbolView = nil
                let leaf = symbolShape ?? IconShapeView()
                if symbolShape == nil { symbolShape = leaf; clip.addSubview(leaf) }
                leaf.show(image == nil ? [] : IconPath.paths(commands), rule: IconPath.rule(props["symbolFillRule"])); leaf.setAccessibilityElement(false)
            } else {
                symbolShape?.removeFromSuperview(); symbolShape = nil
                let leaf = symbolView ?? NSImageView()
                if symbolView == nil { symbolView = leaf; leaf.wantsLayer = true; clip.addSubview(leaf) }
                showSymbol(image, on: leaf); leaf.setAccessibilityElement(false)
            }
            // The symbol's size when the kernel's measurer could not say
            presenter?.queueIntrinsicSize(self, generation: generation, SymbolMeasure.size(name, points: points, weight: weight))
        }
        symbolView?.contentTintColor = symbolInk
        symbolView?.symbolConfiguration = symbolConfiguration(SymbolMeasure.configuration(points: points, weight: weight))
        // A colour that comes or goes while its row is selected (TableInkMac.swift).
        symbolView?.cell?.backgroundStyle = symbolBackground(inEmphasizedRow)
        if let leaf = symbolView { applySymbolEffect(leaf) }
        tintIconShape()
        layoutSymbol()
    }
    func layoutSymbol() {
        guard let clip = symbolClip else { return }
        let content = contentBox()
        clip.frame = content
        if let shape = symbolShape { shape.frame = clip.bounds; placeIconShape(shape); maskSymbol(clip, content); return }
        guard let leaf = symbolView else { return }
        leaf.frame = clip.bounds
        switch style["object_fit"]?.string ?? "fill" {
        // `fill` too: a symbol keeps its aspect, as an SVG does (NodeViewIOS `layoutSymbol`).
        case "contain", "fill": leaf.imageScaling = .scaleProportionallyUpOrDown
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
        maskSymbol(clip, content)
    }
    /// The clip takes the border box's rounded corners.
    private func maskSymbol(_ clip: NSView, _ content: CGRect) {
        let path = roundedPath(in: bounds).cgPath
        var transform = CGAffineTransform(translationX: -content.minX, y: -content.minY)
        let mask = CAShapeLayer(); mask.path = path.copy(using: &transform); clip.layer?.mask = mask
    }
}
#endif
