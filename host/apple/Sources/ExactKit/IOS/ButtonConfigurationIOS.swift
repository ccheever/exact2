// @ref LLP 1069.011.001 D1–D9, D14–D17: one configuration for fitting and presenting.
#if os(iOS) || os(tvOS)
import UIKit

/// No defaults of ours: dictionary membership is the kernel's authored mask.
enum ButtonConfigurationIOS {
    /// The title, subtitle or symbol font where its author wrote a size or a
    /// weight. A written size is the kernel's number and is drawn at it (LLP
    /// 1115.000 D12.1): an absolute `font-size` is the author's, the same on
    /// a button as on a `text` or a field, and what follows Dynamic Type
    /// (`rem`, `em` under the platform's control font, a text style) the
    /// kernel has resolved against the reader's size already. Unsaid, the
    /// font is UIKit's, which UIKit scales.
    static func font(_ incoming: UIFont, rows: NodeStyle) -> UIFont {
        let size = rows["font_size"]?.number.map { CGFloat($0) }
        let weight = rows["font_weight"]?.number.map(Int.init)
        guard size != nil || weight != nil else { return incoming }
        let targetSize = size ?? incoming.pointSize
        let cssWeight = weight ?? TextEngine.controlWeight(incoming)
        let platformWeight: UIFont.Weight
        switch cssWeight {
        case ..<200: platformWeight = .ultraLight
        case 200..<300: platformWeight = .thin
        case 300..<400: platformWeight = .light
        case 400..<500: platformWeight = .regular
        case 500..<600: platformWeight = .medium
        case 600..<700: platformWeight = .semibold
        case 700..<800: platformWeight = .bold
        case 800..<900: platformWeight = .heavy
        default: platformWeight = .black
        }
        return TextEngine.cssWeight(UIFont.systemFont(ofSize: targetSize, weight: platformWeight), weight: cssWeight, size: targetSize)
    }
    /// Whether a colour row is the platform's accent itself (`AccentColor`),
    /// which UIKit draws from the view's tint rather than as a colour of its
    /// own; one derived from it (`Highlight`, `@tint/0.2`) keeps its alpha
    /// as a colour of its own.
    static func followsAccent(_ value: BatchValue?) -> Bool {
        guard case .object(let o)? = value else { return false }
        return o["sys"]?.string == "@tint"
    }
    static func color(_ value: BatchValue?, traits: UITraitCollection, accent: UIColor?) -> UIColor? {
        value?.cgColor(dark: traits.userInterfaceStyle == .dark,
            contrast: traits.accessibilityContrast == .high, tint: accent).map { UIColor(cgColor: $0) }
    }
    /// Whether white would read under 3:1 on `fill` in these traits: its relative luminance is above 0.3.
    static func prefersDarkInk(on fill: UIColor, traits: UITraitCollection) -> Bool {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        guard fill.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a), a > 0.5 else { return false }
        let linear = { (v: CGFloat) in v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) > 0.3
    }
    /// UIKit's public system-spacing constraint, evaluated on the running OS
    /// once per font: solving it takes an Auto Layout pass. Main thread.
    private static var spacings: [UIFont: CGFloat] = [:]
    static func systemSpacing(font: UIFont) -> CGFloat {
        if let known = spacings[font] { return known }
        let spacing = solveSystemSpacing(font: font)
        spacings[font] = spacing
        return spacing
    }
    private static func solveSystemSpacing(font: UIFont) -> CGFloat {
        let parent = UIView(), before = UIImageView(), after = UILabel()
        after.font = font
        for view in [before, after] { view.translatesAutoresizingMaskIntoConstraints = false; parent.addSubview(view) }
        NSLayoutConstraint.activate([
            before.leadingAnchor.constraint(equalTo: parent.leadingAnchor), before.widthAnchor.constraint(equalToConstant: 0),
            after.leadingAnchor.constraint(equalToSystemSpacingAfter: before.trailingAnchor, multiplier: 1),
            after.trailingAnchor.constraint(equalTo: parent.trailingAnchor), after.widthAnchor.constraint(equalToConstant: 0),
            before.topAnchor.constraint(equalTo: parent.topAnchor), after.topAnchor.constraint(equalTo: parent.topAnchor),
            before.heightAnchor.constraint(equalToConstant: 0), after.heightAnchor.constraint(equalToConstant: 0),
            before.bottomAnchor.constraint(equalTo: parent.bottomAnchor)
        ])
        return parent.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize).width
    }
    static func uniformRadius(_ face: ButtonFace) -> CGFloat? {
        let names = ["top_left", "top_right", "bottom_right", "bottom_left"]
        let radii = names.map { face.rows.button["border_radius_" + $0]?.number ?? 0 }
        guard face.rows.button.keys.contains(where: { $0.hasPrefix("border_radius_") }),
              radii.allSatisfy({ $0 == radii[0] }) else { return nil }
        return CGFloat(radii[0])
    }
    static func observation(_ face: ButtonFace, button: UIButton) -> [String: Any] {
        let config = button.configuration
        func row(_ authored: Bool, _ drawn: Any) -> [String: Any] { ["authored": authored, "drawn": drawn] }
        let rows = face.rows
        var result: [String: Any] = [
            "gap": row(rows.imageGap != nil, config?.imagePadding ?? 0),
            "font-size": row(rows.title["font_size"] != nil, button.titleLabel?.font.pointSize ?? 0),
            "font-weight": row(rows.title["font_weight"] != nil, button.titleLabel.map { TextEngine.controlWeight($0.font) } ?? 400),
            "color": row(rows.title["text_color"] != nil, button.titleLabel?.textColor.description ?? "platform"),
            "white-space": row(rows.title["white_space"] != nil, button.titleLabel?.numberOfLines == 1 ? "nowrap" : "normal"),
            "line-clamp": row(rows.title["line_clamp"] != nil, button.titleLabel?.numberOfLines ?? 0),
            "text-align": row(rows.title["text_align"] != nil, button.contentHorizontalAlignment.rawValue),
            "-exact-control-size": row(rows.button["control_size"] != nil, String(describing: config?.buttonSize)),
            "-exact-corner-style": row(rows.button["control_corner_style"] != nil, String(describing: config?.cornerStyle)),
            "border-radius": row(rows.button.keys.contains { $0.hasPrefix("border_radius_") }, config?.background.cornerRadius ?? 0),
            "symbol.font-size": row(rows.symbol["font_size"] != nil, rows.symbol["font_size"]?.number ?? button.titleLabel.map { Double($0.font.pointSize) } ?? 0),
            "symbol.font-weight": row(rows.symbol["font_weight"] != nil, rows.symbol["font_weight"]?.number ?? 400),
            "symbol.-exact-tint-color": row(rows.symbol["tint_color"] != nil, config?.image?.renderingMode == .alwaysOriginal ? "original" : "title")
        ]
        if rows.button.keys.contains(where: { $0.hasPrefix("border_radius_") }), uniformRadius(face) == nil {
            result["border-radius"] = ["authored": true, "drawn": config?.background.cornerRadius ?? 0,
                "standIn": "UIButton.Configuration has one corner radius; non-uniform radii keep the platform shape"]
        }
        let inset = config?.contentInsets ?? .zero
        for (name, value) in [("top", inset.top), ("right", inset.trailing), ("bottom", inset.bottom), ("left", inset.leading)] {
            result["padding-" + name] = row(rows.button["padding_" + name] != nil, value)
        }
        return result
    }
    /// The styles whose fill is the tint, under white ink: a destructive one is red filled. Any other
    /// (plain, gray, tinted, bordered, borderless, glass) draws its ink, and a destructive one draws it red.
    static let filledStyles: Set<String> = ["filled", "borderedProminent", "prominentGlass", "prominentClearGlass"]

    /// A destructive button's ink: `systemRed`. The gray, bordered and tinted styles sit on a fill of their own,
    /// where `systemRed` reads 2.9:1 (on UIKit's gray) and 2.7:1 (on its red tint) in light, a hair under the 3:1
    /// `check` holds a control's label to (LLP 1116.003 D3); there the light red is deepened one notch (#f0302f
    /// for #ff383c), which no one tells apart, and dark and increased contrast stay `systemRed`'s own.
    static func destructiveInk(for style: String) -> UIColor {
        guard ["gray", "bordered", "tinted", "borderedTinted"].contains(style) else { return .systemRed }
        return UIColor { traits in
            traits.userInterfaceStyle == .light && traits.accessibilityContrast != .high
                ? UIColor(red: 240 / 255, green: 48 / 255, blue: 47 / 255, alpha: 1) : UIColor.systemRed.resolvedColor(with: traits)
        }
    }

    /// `destructive`: the platform's destructive role, its red (`accent` is then that red): on a filled style
    /// the tint is the fill and UIKit draws white ink; on the others the ink is red, unless the author painted it.
    /// `padding` (top, right, bottom, left) is the presented node's applied
    /// content box's (`NodeView.kernelPadding`), `nil` on a side its author
    /// left unsaid: what a sheet or a failed layout holds, values and sides
    /// both, where the face was resolved against the latest layout. Measuring
    /// has none and takes the face's.
    static func apply(_ face: ButtonFace, to button: UIButton, traits: UITraitCollection, accent: UIColor?, destructive: Bool = false,
                      padding: [CGFloat?]? = nil) {
        let rows = face.rows
        var (config, drawn, _) = ControlHost.configuration(face)
        config.title = face.title
        config.subtitle = face.subtitle
        config.image = face.symbol.flatMap { UIImage(systemName: $0) }
        switch face.placement ?? (face.leading ? "leading" : "trailing") {
        case "trailing": config.imagePlacement = .trailing
        case "top": config.imagePlacement = .top
        case "bottom": config.imagePlacement = .bottom
        default: config.imagePlacement = .leading
        }
        switch rows.button["control_size"]?.string {
        case "mini": config.buttonSize = .mini
        case "small": config.buttonSize = .small
        case "medium": config.buttonSize = .medium
        case "large": config.buttonSize = .large
        default: break
        }
        switch rows.button["control_corner_style"]?.string {
        case "dynamic": config.cornerStyle = .dynamic
        case "small": config.cornerStyle = .small
        case "medium": config.cornerStyle = .medium
        case "large": config.cornerStyle = .large
        case "capsule": config.cornerStyle = .capsule
        default: break
        }
        if let radius = uniformRadius(face) {
            config.cornerStyle = .fixed; config.background.cornerRadius = radius
        }
        // Physical CSS sides map to directional UIKit insets for the current layout direction.
        var insets = config.contentInsets
        let rtl = button.effectiveUserInterfaceLayoutDirection == .rightToLeft
        let sides = padding ?? ["top", "right", "bottom", "left"].map { rows.button["padding_" + $0]?.number.map { CGFloat($0) } }
        if let n = sides[0] { insets.top = n }
        if let n = sides[2] { insets.bottom = n }
        if let n = sides[3] { if rtl { insets.trailing = n } else { insets.leading = n } }
        if let n = sides[1] { if rtl { insets.leading = n } else { insets.trailing = n } }
        if sides.contains(where: { $0 != nil }) { config.contentInsets = insets }
        let foreground = color(rows.title["text_color"], traits: traits, accent: accent)
        if let foreground { config.baseForegroundColor = foreground }
        // LLP 1116.007 D3.10: the ink on an accent fill is chosen for contrast, as the web draws it (LLP 1116.003
        // D3.1): black once white would fall under 3:1 on the accent as this scheme resolves it (relative luminance
        // above 0.3). UIKit keeps white on a pale tint, which `check` finds at 1.4:1 on yellow and 2.1:1 on teal
        // (three round-9 builds); the system blue, red and the rest keep UIKit's white. An authored colour wins.
        else if destructive, !filledStyles.contains(drawn) {
            // UIKit's `role` draws nothing; the gray and bordered styles keep the label colour under a red tint.
            config.baseForegroundColor = destructiveInk(for: drawn)
        } else if (drawn == "filled" || drawn == "borderedProminent"), let accent, prefersDarkInk(on: accent, traits: traits) {
            config.baseForegroundColor = .black
        }
        let titleFontAuthored = rows.title["font_size"] != nil || rows.title["font_weight"] != nil
        // An authored colour survives UIKit's disabled-state foreground transform (D16). Enabled, it is the
        // configuration's foreground, which UIKit greys with the tint behind a modal (`tintAdjustmentMode`).
        if titleFontAuthored || foreground != nil {
            config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak button] incoming in
                var outgoing = incoming
                if titleFontAuthored, let f = incoming.uiKit.font { outgoing.uiKit.font = font(f, rows: rows.title) }
                if let foreground, button?.isEnabled == false { outgoing.uiKit.foregroundColor = foreground }
                return outgoing
            }
        }
        let subtitleColor = color(rows.subtitle["text_color"], traits: traits, accent: accent)
        let subtitleFontAuthored = rows.subtitle["font_size"] != nil || rows.subtitle["font_weight"] != nil
        if subtitleFontAuthored || subtitleColor != nil {
            config.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
                var outgoing = incoming
                if subtitleFontAuthored, let f = incoming.uiKit.font { outgoing.uiKit.font = font(f, rows: rows.subtitle) }
                if let subtitleColor { outgoing.uiKit.foregroundColor = subtitleColor }
                return outgoing
            }
        }
        let nowrap = rows.title["white_space"]?.string == "nowrap"
        let clamp = rows.title["line_clamp"]?.number.map(Int.init) ?? 0
        if nowrap { config.titleLineBreakMode = .byTruncatingTail }
        switch rows.title["text_align"]?.string {
        case "start": button.contentHorizontalAlignment = .leading; config.titleAlignment = .leading
        case "end": button.contentHorizontalAlignment = .trailing; config.titleAlignment = .trailing
        case "left": button.contentHorizontalAlignment = .left; config.titleAlignment = rtl ? .trailing : .leading
        case "right": button.contentHorizontalAlignment = .right; config.titleAlignment = rtl ? .leading : .trailing
        case "center": button.contentHorizontalAlignment = .center; config.titleAlignment = .center
        default: button.contentHorizontalAlignment = .center
        }
        button.tintColor = accent
        button.configuration = config
        // Read the real title font for this style/size, after UIKit resolved the configuration.
        let titleFont = button.titleLabel?.font ?? UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits)
        if rows.symbol["font_size"] != nil || rows.symbol["font_weight"] != nil {
            let symbolFont = font(titleFont, rows: rows.symbol)
            config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: symbolFont)
        } else { config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: titleFont) }
        config.imagePadding = rows.imageGap ?? systemSpacing(font: titleFont)
        // A symbol's colour is the configuration's, which UIKit greys with the whole button behind a modal
        // (`tintAdjustmentMode`), its own colour as much as the accent. A glass configuration draws a template
        // image in its own ink and ignores that, so there the symbol carries its colour, drawn again as the
        // tint changes (`NativeButtonIOS.tintColorDidChange`).
        let follows = followsAccent(rows.symbol["tint_color"])
        let resolved = color(rows.symbol["tint_color"], traits: traits, accent: accent), own = follows ? nil : resolved
        let native = button as? NativeButtonIOS
        native?.symbolInk = nil
        if follows || own != nil, let template = config.image {
            if !button.isEnabled {
                // Disabled, an authored colour keeps its look through UIKit's disabled one (D16)
                if let resolved { config.image = template.withTintColor(resolved, renderingMode: .alwaysOriginal) }
            } else if drawn.lowercased().hasSuffix("glass"), let native {
                let ink = NativeButtonIOS.Ink(template: template, color: own, alpha: (follows ? accent : own)?.cgColor.alpha)
                native.noteTint()
                native.symbolInk = ink
                config.image = ink.drawn(in: native)
            } else {
                config.imageColorTransformer = UIConfigurationColorTransformer { [weak button] _ in own ?? button?.tintColor ?? .tintColor }
            }
        }
        button.configuration = config
        // Only an authored clamp changes UIKit's line count. Configuration owns
        // wrapping and nowrap; replacing it restores the factory's default.
        if clamp > 0 && !nowrap {
            button.titleLabel?.numberOfLines = clamp
            button.titleLabel?.lineBreakMode = .byTruncatingTail
        }
    }
}
#endif
