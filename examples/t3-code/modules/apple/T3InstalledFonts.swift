// The installed catalog for Appearance's font picker. Font names remain runtime CSS families.
import AppKit

enum T3InstalledFonts {
    static func catalog() -> [[String: Any]] {
        let manager = NSFontManager.shared
        return manager.availableFontFamilies.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }.map { family in
            let font = manager.font(withFamily: family, traits: [], weight: 5, size: 14)
            let monospace = font.map { $0.isFixedPitch || manager.traits(of: $0).contains(.fixedPitchFontMask) } ?? false
            return ["family": family, "monospace": monospace]
        }
    }
}
