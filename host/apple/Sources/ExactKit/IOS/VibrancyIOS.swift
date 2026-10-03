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
#if os(iOS)
import UIKit

/// A paragraph's vibrancy effect view: its ink is drawn in the content view.
final class VibrancyView: UIVisualEffectView {}

extension NodeView {
    /// A blur's own material, not glass and not a CSS backdrop.
    var materialBlurs: Bool { materialKind.map { !Materials.glass($0) && $0 != "backdrop" } ?? false }

    /// Whether this blur material hosts its children in its content view.
    var blurHostsChildren: Bool {
        guard materialBlurs, materialView != nil else { return false }
        let clips = { (v: String?) in v == "hidden" || v == "clip" }
        return clips(style["overflow_x"]?.string) && clips(style["overflow_y"]?.string)
    }

    /// The children back where `container` says, after a material or its
    /// clip changed which view holds them, in their order.
    func rehomeMaterialChildren() {
        let target = container
        let holders = [self, materialView?.contentView].compactMap { $0 }
        let strays = holders.filter { $0 !== target }.flatMap(\.subviews).compactMap { $0 as? NodeView }
        guard !strays.isEmpty else { return }
        for child in strays { target.addSubview(child) }
        presenter?.flats.containerChanged(id)
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
        let style = (isParagraph ? systemColor("text_color") : vibrantFill ? systemColor("background_color") : nil)
            .flatMap(Self.vibrancyStyle)
        let blur = style == nil ? nil : enclosingBlur.flatMap { host -> UIBlurEffect? in
            // Only inside the blur's content view does vibrancy draw.
            guard host.blurHostsChildren, let effect = host.materialView?.effect as? UIBlurEffect else { return nil }
            return effect
        }
        let ink = textRasterLayer
        guard let style, let blur else {
            if let v = vibrancyView {
                if let ink, ink.superlayer === v.contentView.layer { ink.removeFromSuperlayer(); insertBoxSublayer(ink) }
                v.removeFromSuperview()
                vibrancyView = nil
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
        }
        if v.effect != effect { v.effect = effect }
        if v.frame != bounds { v.frame = bounds }
        if isParagraph {
            if let ink, ink.superlayer !== v.contentView.layer { v.contentView.layer.addSublayer(ink) }
        } else if v.contentView.backgroundColor != .white {
            v.contentView.backgroundColor = .white
        }
        return v
    }

    /// UIKit's vibrancy style for a system colour, by WebKit's name.
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

    private static var vibrancySlot = 0
    var vibrancyView: VibrancyView? {
        get { objc_getAssociatedObject(self, &Self.vibrancySlot) as? VibrancyView }
        set { objc_setAssociatedObject(self, &Self.vibrancySlot, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}
#endif
