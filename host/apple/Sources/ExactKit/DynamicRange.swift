// @ref LLP 1100 D8, D9 — what a display can show and what a node allows.
// Exact asks a layer for a dynamic range and tags what it hands Core
// Animation; it never tone-maps itself.
import QuartzCore
#if canImport(UIKit)
import UIKit
typealias PlatformView = UIView
#else
import AppKit
typealias PlatformView = NSView
#endif

enum DisplayRange {
    /// The agent's pinned display headroom (`prefer dynamic-range`); `nil`
    /// reads the live display. Under the agent it starts SDR (LLP 1069.007 D2).
    nonisolated(unsafe) static var pinned: Double? = ExactEnv.agentMode ? 1 : nil

    /// The display's potential headroom over SDR white, not its current one.
    static func headroom(_ view: PlatformView) -> Double {
        if let pinned { return pinned }
        #if canImport(UIKit)
        return Double((view.window?.screen ?? UIScreen.main).potentialEDRHeadroom)
        #else
        return Double((view.window?.screen ?? NSScreen.main)?.maximumPotentialExtendedDynamicRangeColorComponentValue ?? 1)
        #endif
    }

    /// Whether a node's HDR picture decodes as HDR. HDR suppression by the OS
    /// is ignored: it is transient, and the OS already limits the layer.
    static func showsHDR(_ view: PlatformView, limit: String?) -> Bool {
        limit != "standard" && headroom(view) > 1
    }
}

/// A colour's exposure over SDR white, as UIKit's `linearExposure` /
/// `contentHeadroom` model it (LLP 1100 D2, D8). A colour in an extended or
/// PQ/HLG space is tagged with its own peak, so an untagged PQ white doesn't
/// draw at Core Graphics' implied 1000 nits.
enum ColorRange {
    /// A colour's peak over SDR white, at least 1; 0 for a standard space.
    static func headroom(_ color: CGColor?) -> Float {
        guard let color, let space = color.colorSpace,
              CGColorSpaceUsesExtendedRange(space) || CGColorSpaceUsesITUR_2100TF(space) else { return 0 }
        // Measured in linear Rec. 2020, not read off the colour: Core Graphics
        // answers 1000 nits for any untagged PQ/HLG colour.
        guard let linear = CGColorSpace(name: CGColorSpace.extendedLinearITUR_2020),
              let c = color.converted(to: linear, intent: .relativeColorimetric, options: nil)?.components, c.count == 4
        else { return 1 }
        return max(1, Float(max(c[0], c[1], c[2])))
    }

    static func isHDR(_ color: CGColor?) -> Bool { headroom(color) > 1 }

    /// The colour tagged with its headroom (`CGColor(headroom:)`), components unchanged.
    static func tagged(_ color: CGColor) -> CGColor {
        guard #available(iOS 26, macOS 26, tvOS 26, *), let space = color.colorSpace, space.model == .rgb,
              let c = color.components, c.count == 4, case let h = headroom(color), h >= 1,
              let tagged = CGColor(headroom: h, colorSpace: space, red: c[0], green: c[1], blue: c[2], alpha: c[3]) else { return color }
        return tagged
    }
}

/// `dynamic-range-limit` as a layer's range.
@available(iOS 26, macOS 26, tvOS 26, *)
func layerRange(_ limit: String?) -> CALayer.DynamicRange {
    limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
}

extension CALayer {
    private func setColorRange(hdr: Bool, limit: String?) {
        if #available(iOS 26, macOS 26, *) {
            let range: CALayer.DynamicRange = hdr ? layerRange(limit) : .standard
            if preferredDynamicRange != range { preferredDynamicRange = range }
        } else {
            let wants = hdr && limit != "standard"
            if wantsExtendedDynamicRangeContent != wants { wantsExtendedDynamicRangeContent = wants }
        }
    }

    /// The colours this layer paints itself.
    private var ownColors: [CGColor?] {
        var colors = [backgroundColor, borderWidth > 0 ? borderColor : nil, shadowOpacity > 0 ? shadowColor : nil]
        if let shape = self as? CAShapeLayer { colors += [shape.fillColor, shape.strokeColor] }
        return colors
    }

    /// An HDR colour this layer paints asks for what `limit` allows; `deep`
    /// does the same for every sublayer (a shadow caster, an SVG scene).
    func applyColorRange(limit: String?, deep: Bool = false) {
        setColorRange(hdr: ownColors.contains(where: ColorRange.isHDR), limit: limit)
        if deep { sublayers?.forEach { $0.applyColorRange(limit: limit, deep: true) } }
    }

    /// A view's own drawing in an HDR colour gets a half-float backing store.
    /// Answers whether the format changed, so the view draws again.
    func applyDrawnRange(headroom h: Float, limit: String?) -> Bool {
        let hdr = h > 1
        let format: CALayerContentsFormat = hdr ? .RGBA16Float : .RGBA8Uint
        // Only a store this made half float goes back to eight bits.
        let changed = contentsFormat != format && (hdr || contentsFormat == .RGBA16Float)
        if changed { contentsFormat = format }
        if hdr || changed { applyDynamicRange(hdr: hdr, headroom: h, limit: limit) }
        return changed
    }

    /// Where a text raster's headroom is kept, so a later limit can re-ask.
    static let textHeadroomKey = "exactTextHeadroom"

    /// A text raster's layer: HDR ink, or an HDR `text-shadow` it casts.
    func applyTextRange(headroom: Float? = nil, limit: String?) {
        if let headroom { setValue(NSNumber(value: headroom), forKey: Self.textHeadroomKey) }
        let ink = (value(forKey: Self.textHeadroomKey) as? NSNumber)?.floatValue ?? 0
        let h = max(ink, shadowOpacity > 0 ? ColorRange.headroom(shadowColor) : 0)
        applyDynamicRange(hdr: h > 1, headroom: h, limit: limit)
    }

    /// An HDR bitmap's layer asks for what `limit` allows, with its headroom.
    /// Anything else stays standard: an EDR layer costs the compositor even
    /// when its content is SDR.
    func applyDynamicRange(hdr: Bool, headroom: Float, limit: String?) {
        if #available(iOS 26, macOS 26, *) {
            let range: CALayer.DynamicRange = hdr ? layerRange(limit) : .standard
            if preferredDynamicRange != range { preferredDynamicRange = range }
            let tag = CGFloat(hdr ? headroom : 0)
            if contentsHeadroom != tag { contentsHeadroom = tag }
        } else {
            // No "constrained" before 26: it draws as `no-limit`.
            let wants = hdr && limit != "standard"
            if wantsExtendedDynamicRangeContent != wants { wantsExtendedDynamicRangeContent = wants }
        }
    }

    /// What the agent reports of a layer's range (LLP 1100 D12).
    var dynamicRangeFacts: [String: Any] {
        if #available(iOS 26, macOS 26, *) {
            let names: [CALayer.DynamicRange: String] = [.standard: "standard", .constrainedHigh: "constrained", .high: "high", .automatic: "automatic"]
            return ["dynamicRange": names[preferredDynamicRange] ?? "\(preferredDynamicRange)", "contentsHeadroom": contentsHeadroom]
        }
        return ["dynamicRange": wantsExtendedDynamicRangeContent ? "high" : "standard"]
    }
}

extension NodeView {
    /// A changed `dynamic-range-limit` re-asks this node's layers and re-plans
    /// its HDR picture if the decode no longer matches.
    func syncDynamicRange(from old: NodeStyle) {
        guard old["dynamic_range_limit"] != style["dynamic_range_limit"] else { return }
        applyColorRanges()
        guard kind == "image" else { return }
        presenter?.session?.rasters.rangeChanged(self)
        if let bitmap = raster?.image {
            imageLayer?.applyDynamicRange(hdr: bitmap.isHDR, headroom: bitmap.headroom, limit: style["dynamic_range_limit"]?.string)
        }
    }

    /// Every layer this node paints an HDR colour on asks for what its
    /// `dynamic-range-limit` allows. An untagged colour past white would draw
    /// at whatever headroom other layers raised the display to.
    func applyColorRanges(drawing: Bool = false) {
        let limit = style["dynamic_range_limit"]?.string
        #if os(iOS) || os(tvOS)
        let own: CALayer? = layer
        let fills: [CALayer?] = [boxBorder]
        let ink = textRasterLayer
        #else
        let own = layer
        let fills: [CALayer?] = [boxBorder, boxFill]
        let ink = textRasterOverflowLayer
        #endif
        own?.applyColorRange(limit: limit)
        if own?.applyDrawnRange(headroom: drawnHeadroom(drawing: drawing), limit: limit) == true {
            #if os(iOS) || os(tvOS)
            setNeedsDisplay()
            #else
            needsDisplay = true
            #endif
        }
        for l in fills { l?.applyColorRange(limit: limit) }
        for l in [shadowCaster as CALayer?, insetCaster] { l?.applyColorRange(limit: limit, deep: true) }
        ink?.applyTextRange(limit: limit)
        for l in own?.sublayers ?? [] where l.name == SvgHost.rootName { l.applyColorRange(limit: limit, deep: true) }
    }

    /// The peak of what this view draws itself (a drawn box, a paragraph
    /// drawn rather than rastered), or 0.
    func drawnHeadroom(drawing: Bool) -> Float {
        #if os(iOS) || os(tvOS)
        let (box, text) = (boxDrawn, isParagraph && textRaster == nil)
        #else
        let (box, text) = (boxNeedsDraw, isParagraph && !rastersText)
        #endif
        var h: Float = 0
        if box {
            for key in ["background_color", "border_color_top", "border_color_right", "border_color_bottom", "border_color_left"] {
                h = max(h, ColorRange.headroom(cgColor(key)))
            }
        }
        // Outside a draw only an already-made spec counts, so a paragraph's spec stays lazy.
        if text, let spec = drawing ? paragraphSpec() : cachedTextSpec { h = max(h, spec.headroom) }
        return h
    }

    /// Before a paragraph draws: its backing store in the format its colours need.
    func syncDrawnRange() {
        guard isParagraph else { return }
        applyColorRanges(drawing: true)
    }
}

extension Spec {
    /// The peak over SDR white of the paragraph's colours.
    var headroom: Float {
        var colors: [[Double]?] = [color, shadow.map { Array($0[3...]) }]
        for r in runs {
            colors += [r.color, r.background, r.shadow.map { Array($0[3...]) }, r.stroke.map { Array($0[1...]) }]
        }
        return colors.reduce(Float(0)) { h, c in
            guard let c, c.count == 9 else { return h }
            return max(h, ColorRange.headroom(TextEngine.color(c).cgColor))
        }
    }
}

extension GpuModule {
    /// Before a frame, an HDR GPU surface's headroom (the display's, or 1
    /// where the limit or display rules HDR out) and its layer's range
    /// (LLP 1100 D12b). wgpu already asked for EDR when it configured the target.
    func syncDynamicRange(_ id: UInt32, view: NodeView, layer: CALayer?) {
        guard let headroom, highDynamicRange?(id) == 1 else { return }
        let limit = view.style["dynamic_range_limit"]?.string
        headroom(id, DisplayRange.showsHDR(view, limit: limit) ? Float(DisplayRange.headroom(view)) : 1)
        if #available(iOS 26, macOS 26, *), let layer {
            let range = layerRange(limit)
            if layer.preferredDynamicRange != range { layer.preferredDynamicRange = range }
        }
    }
}
