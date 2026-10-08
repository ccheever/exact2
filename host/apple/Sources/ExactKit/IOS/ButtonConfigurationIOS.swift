// @ref LLP 1069.011.001 D1–D9, D14–D17: one configuration for fitting and presenting.
#if os(iOS) || os(tvOS)
import UIKit

/// No defaults of ours: dictionary membership is the kernel's authored mask.
enum ButtonConfigurationIOS {
    static func font(_ incoming: UIFont, rows: NodeStyle, traits: UITraitCollection) -> UIFont {
        let size = rows["font_size"]?.number.map { CGFloat($0) }
        let weight = rows["font_weight"]?.number.map(Int.init)
        guard size != nil || weight != nil else { return incoming }
        // UIKit already scaled its incoming font. Only an authored size goes through metrics.
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
        let base = TextEngine.cssWeight(UIFont.systemFont(ofSize: targetSize, weight: platformWeight), weight: cssWeight, size: targetSize)
        return size == nil || rows["font_size_resolved"]?.number == 1 ? base : UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traits)
    }
    static func color(_ value: BatchValue?, traits: UITraitCollection, accent: UIColor?) -> UIColor? {
        value?.cgColor(dark: traits.userInterfaceStyle == .dark,
            contrast: traits.accessibilityContrast == .high, tint: accent).map { UIColor(cgColor: $0) }
    }
    /// UIKit's public system-spacing constraint, evaluated on the running OS.
    static func systemSpacing(font: UIFont) -> CGFloat {
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
    static func apply(_ face: ButtonFace, to button: UIButton, traits: UITraitCollection, accent: UIColor?) {
        let rows = face.rows
        var (config, _, _) = ControlHost.configuration(face)
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
        if let n = rows.button["padding_top"]?.number { insets.top = CGFloat(n) }
        if let n = rows.button["padding_bottom"]?.number { insets.bottom = CGFloat(n) }
        if let n = rows.button["padding_left"]?.number { if rtl { insets.trailing = CGFloat(n) } else { insets.leading = CGFloat(n) } }
        if let n = rows.button["padding_right"]?.number { if rtl { insets.leading = CGFloat(n) } else { insets.trailing = CGFloat(n) } }
        if rows.button.keys.contains(where: { $0.hasPrefix("padding_") }) { config.contentInsets = insets }
        let foreground = color(rows.title["text_color"], traits: traits, accent: accent)
        if let foreground { config.baseForegroundColor = foreground }
        let titleFontAuthored = rows.title["font_size"] != nil || rows.title["font_weight"] != nil
        // An authored colour survives UIKit's disabled-state foreground transform (D16).
        if titleFontAuthored || foreground != nil {
            config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
                var outgoing = incoming
                if titleFontAuthored, let f = incoming.uiKit.font { outgoing.uiKit.font = font(f, rows: rows.title, traits: traits) }
                if let foreground { outgoing.uiKit.foregroundColor = foreground }
                return outgoing
            }
        }
        let subtitleColor = color(rows.subtitle["text_color"], traits: traits, accent: accent)
        let subtitleFontAuthored = rows.subtitle["font_size"] != nil || rows.subtitle["font_weight"] != nil
        if subtitleFontAuthored || subtitleColor != nil {
            config.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
                var outgoing = incoming
                if subtitleFontAuthored, let f = incoming.uiKit.font { outgoing.uiKit.font = font(f, rows: rows.subtitle, traits: traits) }
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
            let symbolFont = font(titleFont, rows: rows.symbol, traits: traits)
            config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: symbolFont)
        } else { config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: titleFont) }
        config.imagePadding = rows.imageGap ?? systemSpacing(font: titleFont)
        if let tint = color(rows.symbol["tint_color"], traits: traits, accent: accent) {
            config.image = config.image?.withTintColor(tint, renderingMode: .alwaysOriginal)
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
