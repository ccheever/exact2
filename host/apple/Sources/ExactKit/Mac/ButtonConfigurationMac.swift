// @ref LLP 1069.011.001 D1–D17 and §6: NSButton as it is, including reported stand-ins.
#if os(macOS)
import AppKit

/// The same AppKit configuration is used for measurement and presentation.
enum ButtonConfigurationMac {
    // Weak keys retain each pristine control's value only while our authored
    // override is active. An absent row on an untouched control writes nothing.
    private static let pristineSizes = NSMapTable<NSButton, NSNumber>.weakToStrongObjects()
    static func weight(_ value: Int) -> NSFont.Weight {
        switch value {
        case ..<200: return .ultraLight
        case 200..<300: return .thin
        case 300..<400: return .light
        case 400..<500: return .regular
        case 500..<600: return .medium
        case 600..<700: return .semibold
        case 700..<800: return .bold
        case 800..<900: return .heavy
        default: return .black
        }
    }
    static func font(_ incoming: NSFont, rows: NodeStyle) -> NSFont {
        guard rows["font_size"] != nil || rows["font_weight"] != nil else { return incoming }
        let size = rows["font_size"]?.number.map { CGFloat($0) } ?? incoming.pointSize
        let value = rows["font_weight"]?.number.map(Int.init) ?? TextEngine.controlWeight(incoming)
        return TextEngine.cssWeight(NSFont.systemFont(ofSize: size, weight: weight(value)), weight: value, size: size)
    }
    static func color(_ value: BatchValue?, appearance: NSAppearance, accent: NSColor?) -> NSColor? {
        let dark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        let contrast = appearance.name == .accessibilityHighContrastAqua || appearance.name == .accessibilityHighContrastDarkAqua
        return value?.cgColor(dark: dark, contrast: contrast, tint: accent).map { NSColor(cgColor: $0) ?? .controlTextColor }
    }
    /// A numeric radius can exactly describe the capsule preset. AppKit has
    /// no public numeric radius for its rounded-rectangle preset to match.
    static func capsuleRadius(_ face: ButtonFace, size: NSSize) -> Bool {
        let names = ["top_left", "top_right", "bottom_right", "bottom_left"]
        let radii = names.compactMap { face.rows.button["border_radius_" + $0]?.number }
        guard radii.count == 4, let radius = radii.first, radii.allSatisfy({ $0 == radius }), size.width > 0, size.height > 0 else { return false }
        return radius >= Double(min(size.width, size.height) / 2)
    }
    static func corners(_ face: ButtonFace, to button: NSButton, size: NSSize) {
        guard #available(macOS 26.0, *), LinkedDesign.liquidGlass else { return }
        let radiusAuthored = face.rows.button.keys.contains { $0.hasPrefix("border_radius_") }
        // A radius wins over the named style, including an unsupported radius.
        button.borderShape = radiusAuthored
            ? (capsuleRadius(face, size: size) ? .capsule : .automatic)
            : (face.rows.button["control_corner_style"]?.string == "capsule" ? .capsule : .automatic)
    }
    @discardableResult
    static func apply(_ face: ButtonFace, to button: NSButton, appearance: NSAppearance, accent: NSColor?) -> (look: String, glass: Bool) {
        let rows = face.rows
        var look = ButtonFace.drawn(face.macos).name
        var glass = false
        if look == "glass" || look == "glass-accent" {
            if #available(macOS 26.0, *), LinkedDesign.liquidGlass { button.bezelStyle = .glass; glass = true }
            else { look = look == "glass" ? "push" : "push-accent" }
        }
        if !glass { button.bezelStyle = .push }
        button.appearance = appearance
        button.isBordered = look != "borderless"
        if rows.button["control_size"] != nil, pristineSizes.object(forKey: button) == nil {
            pristineSizes.setObject(NSNumber(value: button.controlSize.rawValue), forKey: button)
        }
        switch rows.button["control_size"]?.string {
        case "mini": button.controlSize = .mini
        case "small": button.controlSize = .small
        case "large": button.controlSize = .large
        case "medium": button.controlSize = .regular
        default:
            if let pristine = pristineSizes.object(forKey: button) {
                button.controlSize = NSControl.ControlSize(rawValue: pristine.uintValue)!
                pristineSizes.removeObject(forKey: button)
            }
        }
        let platformFont = NSFont.systemFont(ofSize: NSFont.systemFontSize(for: button.controlSize))
        let titleFont = font(platformFont, rows: rows.title)
        button.font = titleFont
        button.bezelColor = look.hasSuffix("-accent") ? (accent ?? .controlAccentColor) : nil
        let foreground = color(rows.title["text_color"], appearance: appearance, accent: accent)
        button.contentTintColor = foreground ?? (look == "borderless" ? (accent ?? .controlAccentColor) : nil)
        button.title = face.title ?? ""
        // Keep the author's settings. AppKit still substitutes disabled ink
        // at draw time; observation reports that platform limitation (§6).
        if let foreground {
            let paragraph = NSMutableParagraphStyle()
            paragraph.alignment = alignment(rows.title["text_align"]?.string, rtl: button.userInterfaceLayoutDirection == .rightToLeft)
            button.attributedTitle = NSAttributedString(string: face.title ?? "", attributes: [.font: titleFont, .foregroundColor: foreground, .paragraphStyle: paragraph])
        }
        let symbolFont = font(titleFont, rows: rows.symbol)
        let symbolWeight = rows.symbol["font_weight"]?.number.map(Int.init) ?? TextEngine.controlWeight(titleFont)
        button.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: symbolFont.pointSize, weight: weight(symbolWeight))
        var image = face.symbol.flatMap { NSImage(systemSymbolName: $0, accessibilityDescription: nil) }
        if let tint = color(rows.symbol["tint_color"], appearance: appearance, accent: accent), let source = image {
            // Palette colours belong to the symbol, independently of the label.
            let config = button.symbolConfiguration!.applying(NSImage.SymbolConfiguration(paletteColors: [tint]))
            image = source.withSymbolConfiguration(config)
            image?.isTemplate = false
        }
        button.image = image
        if image == nil { button.imagePosition = .noImage }
        else if face.title == nil { button.imagePosition = .imageOnly }
        else {
            switch face.placement ?? (face.leading ? "leading" : "trailing") {
            case "trailing": button.imagePosition = .imageTrailing
            case "top": button.imagePosition = .imageAbove
            case "bottom": button.imagePosition = .imageBelow
            default: button.imagePosition = .imageLeading
            }
        }
        // imagePosition rewrites the cell alignment; the authored alignment wins last.
        button.alignment = alignment(rows.title["text_align"]?.string, rtl: button.userInterfaceLayoutDirection == .rightToLeft)
        button.lineBreakMode = .byTruncatingTail
        corners(face, to: button, size: button.fittingSize)
        return (look, glass)
    }
    static func alignment(_ value: String?, rtl: Bool) -> NSTextAlignment {
        switch value {
        case "left": return .left
        case "right": return .right
        case "start": return rtl ? .right : .left
        case "end": return rtl ? .left : .right
        default: return .center
        }
    }
    static func observation(_ face: ButtonFace, button: NSButton) -> [String: Any] {
        let rows = face.rows
        func row(_ authored: Bool, _ drawn: Any, _ standIn: String? = nil) -> [String: Any] {
            var result: [String: Any] = ["authored": authored, "drawn": drawn]
            if let standIn { result["standIn"] = standIn }
            return result
        }
        var shape = "automatic"
        if #available(macOS 26.0, *) { shape = String(describing: button.borderShape) }
        let radiusAuthored = rows.button.keys.contains { $0.hasPrefix("border_radius_") }
        let size = button.alignmentRect(forFrame: button.frame).size
        let preset = capsuleRadius(face, size: size)
        let modern = { if #available(macOS 26.0, *) { return LinkedDesign.liquidGlass }; return false }()
        let corner = rows.button["control_corner_style"]?.string
        var result: [String: Any] = [
            "gap": row(rows.imageGap != nil, "NSButton image spacing", rows.imageGap == nil ? nil : "NSButton has no image gap field"),
            "subtitle": row(face.subtitle != nil, "title only", face.subtitle == nil ? nil : "NSButton has no subtitle field; subtitle omitted"),
            "font-size": row(rows.title["font_size"] != nil, button.font?.pointSize ?? 0),
            "font-weight": row(rows.title["font_weight"] != nil, button.font.map(TextEngine.controlWeight) ?? 400),
            "color": row(rows.title["text_color"] != nil, button.isEnabled ? (button.contentTintColor?.description ?? "platform") : NSColor.disabledControlTextColor.description,
                button.isEnabled || rows.title["text_color"] == nil ? nil : "AppKit substitutes disabledControlTextColor despite authored contentTintColor and attributed title"),
            "white-space": row(rows.title["white_space"] != nil, "nowrap", rows.title["white_space"]?.string == "nowrap" ? nil : "NSButton push titles do not wrap"),
            "line-clamp": row(rows.title["line_clamp"] != nil, 1, rows.title["line_clamp"]?.number == 1 ? nil : "NSButton push titles have one line"),
            "text-align": row(rows.title["text_align"] != nil, button.alignment.rawValue),
            "-exact-control-size": row(rows.button["control_size"] != nil, String(describing: button.controlSize)),
            "-exact-corner-style": row(corner != nil, shape, corner == nil || (corner == "capsule" && modern && !radiusAuthored) ? nil : "NSButton keeps its preset shape; numeric radius takes precedence"),
            "border-radius": row(radiusAuthored, shape, !radiusAuthored || (modern && preset) ? nil : "NSButton has only preset border shapes; numeric radius unavailable"),
            "symbol.font-size": row(rows.symbol["font_size"] != nil, rows.symbol["font_size"]?.number ?? Double(button.font?.pointSize ?? 0)),
            "symbol.font-weight": row(rows.symbol["font_weight"] != nil, rows.symbol["font_weight"]?.number ?? Double(button.font.map(TextEngine.controlWeight) ?? 400)),
            "symbol.-exact-tint-color": row(rows.symbol["tint_color"] != nil, button.image?.isTemplate == false ? "original" : "title",
                button.isEnabled || (rows.symbol["tint_color"] == nil && rows.title["text_color"] == nil) || button.image == nil ? nil : "AppKit dims the disabled image despite its authored colour")
        ]
        for side in ["top", "right", "bottom", "left"] {
            result["padding-" + side] = row(rows.button["padding_" + side] != nil, "NSButton default insets", rows.button["padding_" + side] == nil ? nil : "NSButton has no content inset field; authored padding ignored")
        }
        return result
    }
}
#endif
