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
    /// The symbol's own tint, or `nil` to follow the platform's accent
    /// (LLP 1095 stage 2): the initial `-exact-tint-color` is `AccentColor`, which the
    /// platform keeps dynamic (iOS inherits the hierarchy's `tintColor`, a
    /// window or app tint included; macOS has `controlAccentColor`), so it is
    /// never resolved to channels here.
    var symbolTint: PlatformColor? {
        guard let row = style["tint_color"] else { return nil }
        if case .object(let o) = row, o["sys"]?.string == "@tint" { return nil }
        return channels("tint_color").map(TextEngine.color)
    }
    /// The accent a `nil` `symbolTint` follows, for an API that needs a colour.
    var inheritedTint: PlatformColor {
        #if os(iOS) || os(tvOS)
        return tintColor
        #else
        return .controlAccentColor
        #endif
    }

    /// What the symbol's look reads from its style, for the image's key: a
    /// change makes it again. Its colours are resolved into the
    /// configuration, so every trait they resolve by is here too, and the
    /// system colours' generation (LLP 1095 D5).
    var symbolLookKey: String {
        [style["symbol_rendering"]?.string ?? "", "\(style["symbol_palette"] ?? .null)",
         "\(number("symbol_value", -1))", "\(style["tint_color"] ?? .null)", "\(drawsDark)",
         "\(drawsHighContrast ?? SystemColor.highContrast)", "\(drawsElevated)", "\(SystemColor.generation)", bakedTintKey].joined(separator: "|")
    }
    /// The inherited tint, when the look bakes it into the image: a
    /// hierarchical glyph without its own tint or tinted by it (`Highlight`
    /// too), or a palette naming it.
    private var bakedTintKey: String {
        let mode = style["symbol_rendering"]?.string
        let bakes = (mode == "hierarchical" && (symbolTint == nil || style["tint_color"]?.namesTint == true))
            || (mode == "palette" && style["symbol_palette"]?.namesTint == true)
        guard bakes, let tint = viewTint else { return "" }
        return "\(SystemColor.channels("@tint", dark: drawsDark, tintColor: tint, fallback: nil) ?? [])"
    }

    /// The symbol's configuration: its size and weight, then its rendering
    /// mode (D10) — hierarchical in the tint, a palette, or multicolor.
    func symbolConfiguration(_ base: SymbolConfig) -> SymbolConfig {
        let tint = symbolTint ?? inheritedTint
        switch style["symbol_rendering"]?.string {
        case "hierarchical": return base.applying(SymbolConfig(hierarchicalColor: tint))
        case "multicolor": return base.applying(SymbolConfig.preferringMulticolor())
        case "palette":
            let colors = (style["symbol_palette"]?.array ?? []).compactMap { c -> PlatformColor? in
                c.channels(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: ownTint(for: c)).map(TextEngine.color)
            }
            return colors.isEmpty ? base : base.applying(SymbolConfig(paletteColors: colors))
        default: return base
        }
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
