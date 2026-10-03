// LLP 1077 D13 on UIKit (`Vibrancy.swift` is the shared part). A blur's
// vibrancy draws only what is inside a vibrancy effect view within the
// blur's content view, so:
//
// - A blur material whose box clips its children (`overflow: hidden` on
//   both axes) hosts them in its effect view's content view, which clips
//   them the same way. One whose children may overflow keeps them beside
//   it, and its labels are their colours, not vibrant (declared in LLP 1001).
// - A paragraph in a system colour inside such a material draws its ink
//   (`TextRasterIOS.swift`) inside a vibrancy effect view of that colour's
//   style, which takes the ink's shape and draws it as UIKit draws the
//   label: blended with what the material blurs.
#if os(iOS) || os(tvOS)
import UIKit

/// A paragraph's vibrancy effect view: its ink is drawn in the content view.
final class VibrancyView: UIVisualEffectView {}

/// The container a material last held its children in.
private final class WeakView {
    weak var view: UIView?
    let hosts: Bool
    init(_ view: UIView, hosts: Bool) { self.view = view; self.hosts = hosts }
}

extension NodeView {
    /// A blur's own material, not glass and not a CSS backdrop.
    var materialBlurs: Bool { materialKind.map { !Materials.glass($0) && $0 != "backdrop" } ?? false }

    /// Whether this blur material hosts its children in its content view.
    var blurHostsChildren: Bool {
        guard materialBlurs, materialView != nil else { return false }
        let clips = { (v: String?) in v == "hidden" || v == "clip" }
        return clips(style["overflow_x"]?.string) && clips(style["overflow_y"]?.string)
    }

    /// The children back where `container` says, after a material, its
    /// clip or a scroll changed which view holds them, in their order. Flat
    /// leaves (`presenter.flats`) follow the container even when no view
    /// moves, and every descendant re-decides whether it is vibrant.
    func rehomeMaterialChildren() {
        let target = container
        let holders = [self, materialView?.contentView].compactMap { $0 }
        let strays = holders.filter { $0 !== target }.flatMap(\.subviews).compactMap { $0 as? NodeView }
        for child in strays { target.addSubview(child) }
        let last = objc_getAssociatedObject(self, &Self.containerSlot) as? WeakView
        let moved = !strays.isEmpty || (last != nil && last?.view !== target)
        // A glass group keeps `container` while the material starts or stops
        // hosting it, so the hosting itself is watched too.
        let hosts = blurHostsChildren
        let flipped = last != nil && last?.hosts != hosts
        objc_setAssociatedObject(self, &Self.containerSlot, WeakView(target, hosts: hosts), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        if moved { presenter?.flats.containerChanged(id) }
        if moved || flipped { vibrancyUnsettled = true }
    }

    /// After the material's views have all moved (the glass group last,
    /// `syncGlassGroup`), every descendant re-decides its vibrancy.
    func settleVibrancy() {
        guard vibrancyUnsettled else { return }
        vibrancyUnsettled = false
        syncDescendantVibrancy()
    }

    /// Every descendant paragraph and fill re-decides its vibrancy: a
    /// material around it came, went or changed what holds its children.
    func syncDescendantVibrancy() {
        var stack: [UIView] = [self]
        while let v = stack.popLast() {
            for s in v.subviews where !(s is VibrancyView) {
                (s as? NodeView)?.syncVibrancy()
                stack.append(s)
            }
        }
    }

    /// A plain fill in a system colour (a separator, a fill bar): no border,
    /// gradient or image to draw with it.
    var vibrantFill: Bool {
        !isParagraph && systemColor("background_color") != nil && style["background_image"] == nil
            && style["corner_shape"] == nil && (style["background_clip"]?.string ?? "border-box") == "border-box"
            && !["top", "right", "bottom", "left"].contains { number("border_width_" + $0, number("border_width")) > 0 }
    }

    /// The vibrancy view this paragraph draws its ink in — or a plain fill
    /// its colour in (the style gives the colour; the view only its shape)
    /// — kept in step with the colour and the material around it; nil when
    /// it draws plainly.
    @discardableResult
    func syncVibrancy() -> VibrancyView? {
        #if os(tvOS)
        // tvOS has no vibrancy styles; the paragraph draws plainly.
        return nil
        #else
        let style = (isParagraph ? systemColor("text_color") : vibrantFill ? systemColor("background_color") : nil)
            .flatMap(Self.vibrancyStyle)
        let blur = style == nil ? nil : enclosingBlur.flatMap { host -> UIBlurEffect? in
            // Only inside the blur's content view does vibrancy draw (else
            // UIKit draws it clear).
            guard host.blurHostsChildren, let material = host.materialView, isDescendant(of: material.contentView),
                  let effect = material.effect as? UIBlurEffect else { return nil }
            return effect
        }
        let ink = textRasterLayer
        guard let style, let blur, !isParagraph || vibrantInk else {
            if let v = vibrancyView {
                if let ink, ink.superlayer === v.contentView.layer { ink.removeFromSuperlayer(); insertBoxSublayer(ink) }
                v.removeFromSuperview()
                vibrancyView = nil
                // A fill paints its own background again.
                if !isParagraph { setNeedsDisplay() }
            }
            return nil
        }
        let v = vibrancyView ?? VibrancyView(effect: nil)
        let effect = UIVibrancyEffect(blurEffect: blur, style: style)
        if v.superview !== self {
            v.isUserInteractionEnabled = false
            v.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            insertSubview(v, at: 0)
            vibrancyView = v
            // A fill stops painting its own background (`applyBoxLayer`).
            if !isParagraph { setNeedsDisplay() }
            // A paragraph's text paints over its box (CSS): above its
            // border and fill layers; a vibrant fill is the box's back.
            if isParagraph, let top = [boxBorder, imageLayer, insetCaster, boxGradient, shadowCaster].compactMap({ $0 }).first(where: { $0.superlayer === layer }) {
                layer.insertSublayer(v.layer, above: top)
            }
        }
        if v.effect != effect { v.effect = effect }
        if v.frame != bounds { v.frame = bounds }
        if isParagraph {
            if let ink, ink.superlayer !== v.contentView.layer { v.contentView.layer.addSublayer(ink) }
        } else if v.contentView.backgroundColor != .white {
            v.contentView.backgroundColor = .white
        }
        return v
        #endif
    }

    /// Whether the paragraph's whole ink is its one system colour: no run
    /// in another colour, and no background, shadow or stroke drawn with it
    /// (the vibrancy view would take them all). Otherwise it draws its pair.
    var vibrantInk: Bool {
        let spec = paragraphSpec(), own = channels("text_color", dark: drawsDark)
        return spec.shadow == nil
            && spec.runs.allSatisfy { ($0.color == nil || $0.color == own) && $0.background == nil && $0.shadow == nil && $0.stroke == nil }
    }

    /// UIKit's vibrancy style for a system colour, by WebKit's name.
    #if !os(tvOS)
    static func vibrancyStyle(_ name: String) -> UIVibrancyEffectStyle? {
        switch name {
        case "-apple-system-label": .label
        case "-apple-system-secondary-label": .secondaryLabel
        case "-apple-system-tertiary-label": .tertiaryLabel
        case "-apple-system-quaternary-label": .quaternaryLabel
        case "-apple-system-separator": .separator
        case "-apple-system-fill": .fill
        default: nil
        }
    }
    #endif

    private static var vibrancySlot = 0
    private static var containerSlot = 0
    private static var unsettledSlot = 0
    private var vibrancyUnsettled: Bool {
        get { objc_getAssociatedObject(self, &Self.unsettledSlot) as? Bool ?? false }
        set { objc_setAssociatedObject(self, &Self.unsettledSlot, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
    var vibrancyView: VibrancyView? {
        get { objc_getAssociatedObject(self, &Self.vibrancySlot) as? VibrancyView }
        set { objc_setAssociatedObject(self, &Self.vibrancySlot, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}
#endif
