// LLP 1077 §5: what Apple's platforms draw and feel that CSS has no name
// for, declared as rows (LLP 1001): an SF Symbol's rendering mode, palette,
// variable value and effects (D10–D12), and the feedback a press makes and
// `haptic()` plays (D14). The iOS-only ones (scroll edge, pointer hover,
// Smart Invert, numerals that roll) are `IOS/AffordancesIOS.swift`.
import ObjectiveC
#if os(iOS) || os(tvOS)
import UIKit
typealias SymbolImage = UIImage
typealias SymbolConfig = UIImage.SymbolConfiguration
#else
import AppKit
typealias SymbolImage = NSImage
typealias SymbolConfig = NSImage.SymbolConfiguration
#endif

extension NodeView {
    /// A page symbol takes its presented computed color (LLP 1081 stage 2).
    var symbolInk: PlatformColor? {
        #if os(macOS)
        return channels("text_color").map(TextEngine.color) ?? .textColor
        #else
        return channels("text_color").map(TextEngine.color) ?? .label
        #endif
    }
    /// The ink it is drawn in: its computed colour, or as a list cell's
    /// image (iOS's custom grouped row) its tint, which the sheet sets, as
    /// UIKit tints a cell's image (`GroupedListIOS.swift`'s standard cell).
    var drawnSymbolInk: PlatformColor? {
        drawsListSymbol ? channels("tint_color").map(TextEngine.color) ?? inheritedTint : symbolInk
    }
    /// Whether a colour row is the platform's accent (`AccentColor`, iOS's
    /// inherited `tintColor`) rather than a colour of its own. An unset
    /// `tint-color` is its initial value, `AccentColor`.
    package func followsTint(_ key: String) -> Bool {
        guard let row = style[key] else { return key == "tint_color" }
        guard case .object(let o) = row else { return false }
        return o["sys"]?.string?.hasPrefix("@tint") == true
    }
    /// The platform tint used by a native control’s symbol.
    var inheritedTint: PlatformColor {
        #if os(iOS) || os(tvOS)
        return inheritedInk
        #else
        return viewTint ?? .controlAccentColor
        #endif
    }

    /// Whether it is drawn as a list cell's image (iOS's custom grouped row).
    var drawsListSymbol: Bool {
        #if os(iOS) || os(tvOS)
        listSymbol
        #else
        false
        #endif
    }
    /// Only shape changes replace a symbol; paint is a view configuration.
    var symbolLookKey: String {
        [style["symbol_rendering"]?.string ?? "", "\(number("symbol_value", -1))", "\(drawsListSymbol)"].joined(separator: "|")
    }

    #if os(iOS) || os(tvOS)
    /// The point size whose large scale UIKit's list cell draws its image at
    /// at each accessibility text size: a star measured on iOS 27.0 under the
    /// system's text size (ink 30.33, 35, 34.33, 40.33 and 45.33 points wide,
    /// 23.67 at the default's 17), about 0.76 of the checkmark accessory's
    /// growth, stepping back at AX3 as it does.
    static let listSymbolPoints: [UIContentSizeCategory: CGFloat] = [
        .accessibilityMedium: 21.78, .accessibilityLarge: 25.14, .accessibilityExtraLarge: 24.66,
        .accessibilityExtraExtraLarge: 28.97, .accessibilityExtraExtraExtraLarge: 32.56,
    ]
    #endif
    /// Shape belongs to the cached image; paint can change without replacing it.
    func symbolShape(_ base: SymbolConfig) -> SymbolConfig {
        #if os(iOS) || os(tvOS)
        // At an accessibility size UIKit draws a list cell's image smaller
        // than the text's large scale (`listSymbolPoints`).
        guard drawsListSymbol else { return base }
        let sized = Self.listSymbolPoints[traitCollection.preferredContentSizeCategory].map { base.applying(SymbolConfig(pointSize: $0)) } ?? base
        return sized.applying(SymbolConfig(scale: .large))
        #else
        return base
        #endif
    }

    /// The symbol's configuration: its size and weight, then its rendering
    /// mode (D10) — hierarchical in the tint, a palette, or multicolor.
    /// `face`: as a native control's face draws it, without the page's ink.
    func symbolConfiguration(_ base: SymbolConfig, face: Bool = false) -> SymbolConfig {
        #if os(iOS) || os(tvOS)
        // Take a face's colours undimmed: UIKit keeps the image a control is given as made (TintDimmingIOS.swift)
        let tint = face ? undimmedInheritedTint : drawnSymbolInk ?? inheritedTint
        #else
        let tint = (face ? nil : drawnSymbolInk) ?? inheritedTint
        #endif
        let base = symbolShape(base)
        switch style["symbol_rendering"]?.string {
        case "hierarchical": return base.applying(SymbolConfig(hierarchicalColor: tint))
        case "multicolor": return base.applying(SymbolConfig.preferringMulticolor())
        case "palette":
            let colors = (style["symbol_palette"]?.array ?? []).compactMap { c -> PlatformColor? in
                #if os(iOS) || os(tvOS)
                if face { return c.channels(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: undimmedTint(for: c)).map(TextEngine.color) }
                return c.channels(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: c, key: "symbol_palette"))
                    .map { TextEngine.color(dimmedWithTint($0, key: "symbol_palette")) }
                #else
                return c.channels(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: c)).map(TextEngine.color)
                #endif
            }
            return colors.isEmpty ? base : base.applying(SymbolConfig(paletteColors: colors))
        default: return base
        }
    }

    /// A native control configures its own image in the control tint.
    var faceImage: SymbolImage? {
        guard props["symbolPath"] == nil, let name = props["symbolName"], !name.isEmpty else { return image }
        let points = number("font_size", PageFacts.defaultRootFontSize), weight = number("font_weight", 400)
        return points > 0 ? symbolImage(name, symbolConfiguration(SymbolMeasure.configuration(points: points, weight: weight), face: true)) : nil
    }

    /// A symbol by name in that configuration, at its variable value (D11)
    /// when it has one.
    func symbolImage(_ name: String, _ config: SymbolConfig) -> SymbolImage? {
        let value = number("symbol_value", -1)
        #if os(iOS) || os(tvOS)
        if value >= 0 { return UIImage(systemName: name, variableValue: Double(value), configuration: config) }
        return UIImage(systemName: name, withConfiguration: config)
        #else
        let image = value >= 0
            ? NSImage(systemSymbolName: name, variableValue: Double(value), accessibilityDescription: nil)
            : NSImage(systemSymbolName: name, accessibilityDescription: nil)
        return image?.withSymbolConfiguration(config)
        #endif
    }

    /// A new symbol on its view: with `-exact-symbol-effect: replace`, the
    /// platform's replace transition (D12); else at once.
    func showSymbol(_ image: SymbolImage?, on leaf: SymbolLeaf) {
        if style["symbol_effect"]?.string == "replace", let image, leaf.image != nil {
            leaf.setSymbolImage(image, contentTransition: .replace)
        } else {
            leaf.image = image
        }
    }

    /// A continuing effect plays while set; a discrete one plays each time
    /// `symbolEffectValue` changes (D12).
    func applySymbolEffect(_ leaf: SymbolLeaf) {
        let effect = style["symbol_effect"]?.string ?? "none"
        let state = SymbolEffectState.of(leaf)
        if state.effect != effect {
            leaf.removeAllSymbolEffects(options: .default, animated: false)
            state.effect = effect
            switch effect {
            case "pulse": leaf.addSymbolEffect(.pulse)
            case "variable-color": leaf.addSymbolEffect(.variableColor.iterative)
            case "scale": leaf.addSymbolEffect(.scale.up)
            case "breathe": if #available(iOS 18.0, macOS 15.0, tvOS 18.0, *) { leaf.addSymbolEffect(.breathe) }
            case "rotate": if #available(iOS 18.0, macOS 15.0, tvOS 18.0, *) { leaf.addSymbolEffect(.rotate) }
            default: break
            }
        }
        let value = props["symbolEffectValue"]
        defer { state.value = value }
        guard state.value != nil, value != state.value else { return }
        switch effect {
        case "bounce": leaf.addSymbolEffect(.bounce, options: .nonRepeating)
        case "wiggle": if #available(iOS 18.0, macOS 15.0, tvOS 18.0, *) { leaf.addSymbolEffect(.wiggle, options: .nonRepeating) }
        default: break
        }
    }

    /// `-exact-press-haptic` (D14): host-owned, at the press, as `-exact-press-scale`.
    func pressHaptic() {
        guard let kind = style["press_haptic"]?.string, kind != "none" else { return }
        Haptics.play(kind)
    }
}

#if os(iOS) || os(tvOS)
typealias SymbolLeaf = UIImageView
#else
typealias SymbolLeaf = NSImageView
#endif

/// Which effect a symbol view runs and the value its discrete one last
/// played on, kept on the view.
final class SymbolEffectState {
    var effect = "none"
    var value: String?
    private static var key = 0
    static func of(_ leaf: SymbolLeaf) -> SymbolEffectState {
        if let s = objc_getAssociatedObject(leaf, &key) as? SymbolEffectState { return s }
        let s = SymbolEffectState()
        objc_setAssociatedObject(leaf, &key, s, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return s
    }
}

/// The platform's haptics (LLP 1077 D14): `-exact-press-haptic`'s kinds and
/// `haptic()`'s, which adds success, warning and error. macOS plays on a
/// Force Touch trackpad.
enum Haptics {
    static func play(_ kind: String) {
        #if os(tvOS)
        // tvOS has no haptics.
        #elseif os(iOS)
        switch kind {
        case "selection": UISelectionFeedbackGenerator().selectionChanged()
        case "success": UINotificationFeedbackGenerator().notificationOccurred(.success)
        case "warning": UINotificationFeedbackGenerator().notificationOccurred(.warning)
        case "error": UINotificationFeedbackGenerator().notificationOccurred(.error)
        case "impact-light": UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case "impact-heavy": UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
        case "impact-soft": UIImpactFeedbackGenerator(style: .soft).impactOccurred()
        case "impact-rigid": UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
        case "impact-medium", "impact": UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        default: fputs("exact: haptic \(kind) is not one this platform plays\n", stderr)
        }
        #else
        let pattern: NSHapticFeedbackManager.FeedbackPattern = kind == "selection" ? .alignment : kind.hasPrefix("impact") ? .levelChange : .generic
        NSHapticFeedbackManager.defaultPerformer.perform(pattern, performanceTime: .now)
        #endif
    }
}
