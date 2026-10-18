import Foundation
#if canImport(UIKit) && !os(watchOS)
import UIKit
#endif

/// The `exact_set_preferences` wire bits (runner/src/viewport.rs). Hosts read
/// their own environment; this value encodes it without finding an app or screen.
public struct Preferences: OptionSet, Sendable {
    public let rawValue: UInt32
    public init(rawValue: UInt32) { self.rawValue = rawValue }
    public static let reducedMotion = Self(rawValue: 1 << 0)
    public static let reducedTransparency = Self(rawValue: 1 << 1)
    public static let contrastMore = Self(rawValue: 1 << 2)
    public static let contrastLess = Self(rawValue: 1 << 3)
    public static let dark = Self(rawValue: 1 << 4)
    public static let coarsePointer = Self(rawValue: 1 << 5)
    public static let noPointer = Self(rawValue: 1 << 6)
    public static let noHover = Self(rawValue: 1 << 7)
    public static let p3 = Self(rawValue: 1 << 8)
    public static let rec2020 = Self(rawValue: 2 << 8)
    public static let highDynamicRange = Self(rawValue: 1 << 10)
    public static let boldText = Self(rawValue: 1 << 14)

    public enum DisplayMode: UInt32, Sendable {
        case standalone, browser, fullscreen, minimalUI, windowControlsOverlay, pictureInPicture, tabbed
    }
    public static func displayMode(_ mode: DisplayMode) -> Self { Self(rawValue: mode.rawValue << 11) }
    public static var platformInput: Self {
        #if os(tvOS)
        [.noPointer, .noHover]
        #elseif os(iOS) || os(watchOS)
        [.coarsePointer, .noHover]
        #else
        []
        #endif
    }
    public init(reducedMotion: Bool, reducedTransparency: Bool, contrast: String,
                dark: Bool, input: Self = .platformInput, gamut: String,
                highDynamicRange: Bool, displayMode: DisplayMode = .standalone, boldText: Bool) {
        self = input.union(Self.displayMode(displayMode))
        if reducedMotion { insert(.reducedMotion) }
        if reducedTransparency { insert(.reducedTransparency) }
        if contrast == "more" || contrast == "custom" { insert(.contrastMore) }
        if contrast == "less" || contrast == "custom" { insert(.contrastLess) }
        if dark { insert(.dark) }
        if gamut == "p3" { insert(.p3) }
        if gamut == "rec2020" { insert(.rec2020) }
        if highDynamicRange { insert(.highDynamicRange) }
        if boldText { insert(.boldText) }
    }
}

#if canImport(UIKit) && !os(watchOS)
extension Preferences {
    /// The extension's supplied traits and accessibility settings. No UIScreen
    /// lookup: pass the destination traits, including their display scale.
    public init(traits: UITraitCollection,
                reducedMotion: Bool = UIAccessibility.isReduceMotionEnabled,
                reducedTransparency: Bool = UIAccessibility.isReduceTransparencyEnabled,
                increasedContrast: Bool = UIAccessibility.isDarkerSystemColorsEnabled,
                boldText: Bool = UIAccessibility.isBoldTextEnabled,
                highDynamicRange: Bool = false, displayMode: DisplayMode = .standalone) {
        self.init(reducedMotion: reducedMotion, reducedTransparency: reducedTransparency,
                  contrast: increasedContrast ? "more" : "no-preference", dark: traits.userInterfaceStyle == .dark,
                  gamut: traits.displayGamut == .P3 ? "p3" : "srgb", highDynamicRange: highDynamicRange,
                  displayMode: displayMode, boldText: boldText)
    }
}
extension SessionFacts {
    public init(traits: UITraitCollection, rootFontSize: Double = Double(defaultRootFontSize),
                textResidencyBytes: Int = 2 * 1024 * 1024) {
        self.init(displayScale: traits.displayScale, preferences: Preferences(traits: traits),
                  rootFontSize: rootFontSize, textResidencyBytes: textResidencyBytes)
    }
}
#endif
