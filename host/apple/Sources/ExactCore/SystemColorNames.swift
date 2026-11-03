// @ref LLP 1095 D3 — the platform colours a row may name: the documented
// class colours of `UIColor` and `NSColor` that are not deprecated, by their
// Objective-C names (`systemTealColor`), each where its SDK declares it.
// Apple's lists: UIKit's "UI element colors" and "Standard colors"
// (https://developer.apple.com/documentation/uikit/ui-element-colors,
// https://developer.apple.com/documentation/uikit/standard-colors) and
// AppKit's NSColor "UI element colors" and "Standard colors"
// (https://developer.apple.com/documentation/appkit/nscolor). A name off the
// list is no colour, as an unknown one is: the row's fallback shows. A colour
// a later OS adds is added here.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

package enum SystemColorNames {
    /// The documented class colour `name` names on this platform, else nil.
    package static func color(_ name: String) -> PlatformColor? {
        if let fixed = fixed(name) { return fixed }
        #if os(macOS)
        return appKit(name)
        #elseif os(watchOS)
        return nil
        #else
        return uiKit(name)
        #endif
    }

    /// The fixed colours, on every platform.
    private static func fixed(_ name: String) -> PlatformColor? {
        switch name {
        case "blackColor": .black
        case "blueColor": .blue
        case "brownColor": .brown
        case "clearColor": .clear
        case "cyanColor": .cyan
        case "darkGrayColor": .darkGray
        case "grayColor": .gray
        case "greenColor": .green
        case "lightGrayColor": .lightGray
        case "magentaColor": .magenta
        case "orangeColor": .orange
        case "purpleColor": .purple
        case "redColor": .red
        case "whiteColor": .white
        case "yellowColor": .yellow
        default: nil
        }
    }

    #if os(iOS) || os(tvOS) || os(visionOS)
    private static func uiKit(_ name: String) -> UIColor? {
        switch name {
        case "systemRedColor": .systemRed
        case "systemOrangeColor": .systemOrange
        case "systemYellowColor": .systemYellow
        case "systemGreenColor": .systemGreen
        case "systemMintColor": .systemMint
        case "systemTealColor": .systemTeal
        case "systemCyanColor": .systemCyan
        case "systemBlueColor": .systemBlue
        case "systemIndigoColor": .systemIndigo
        case "systemPurpleColor": .systemPurple
        case "systemPinkColor": .systemPink
        case "systemBrownColor": .systemBrown
        case "systemGrayColor": .systemGray
        case "tintColor": .tintColor
        case "labelColor": .label
        case "secondaryLabelColor": .secondaryLabel
        case "tertiaryLabelColor": .tertiaryLabel
        case "quaternaryLabelColor": .quaternaryLabel
        case "linkColor": .link
        case "placeholderTextColor": .placeholderText
        case "separatorColor": .separator
        case "opaqueSeparatorColor": .opaqueSeparator
        default: iOSOnly(name)
        }
    }

    /// What UIKit declares on iOS and not on tvOS.
    private static func iOSOnly(_ name: String) -> UIColor? {
        #if os(tvOS)
        return nil
        #else
        switch name {
        case "systemGray2Color": return .systemGray2
        case "systemGray3Color": return .systemGray3
        case "systemGray4Color": return .systemGray4
        case "systemGray5Color": return .systemGray5
        case "systemGray6Color": return .systemGray6
        case "systemBackgroundColor": return .systemBackground
        case "secondarySystemBackgroundColor": return .secondarySystemBackground
        case "tertiarySystemBackgroundColor": return .tertiarySystemBackground
        case "systemGroupedBackgroundColor": return .systemGroupedBackground
        case "secondarySystemGroupedBackgroundColor": return .secondarySystemGroupedBackground
        case "tertiarySystemGroupedBackgroundColor": return .tertiarySystemGroupedBackground
        case "systemFillColor": return .systemFill
        case "secondarySystemFillColor": return .secondarySystemFill
        case "tertiarySystemFillColor": return .tertiarySystemFill
        case "quaternarySystemFillColor": return .quaternarySystemFill
        case "lightTextColor": return .lightText
        case "darkTextColor": return .darkText
        default: return nil
        }
        #endif
    }
    #endif

    #if os(macOS)
    private static func appKit(_ name: String) -> NSColor? {
        switch name {
        case "systemRedColor": .systemRed
        case "systemOrangeColor": .systemOrange
        case "systemYellowColor": .systemYellow
        case "systemGreenColor": .systemGreen
        case "systemMintColor": .systemMint
        case "systemTealColor": .systemTeal
        case "systemCyanColor": .systemCyan
        case "systemBlueColor": .systemBlue
        case "systemIndigoColor": .systemIndigo
        case "systemPurpleColor": .systemPurple
        case "systemPinkColor": .systemPink
        case "systemBrownColor": .systemBrown
        case "systemGrayColor": .systemGray
        case "labelColor": .labelColor
        case "secondaryLabelColor": .secondaryLabelColor
        case "tertiaryLabelColor": .tertiaryLabelColor
        case "quaternaryLabelColor": .quaternaryLabelColor
        case "quinaryLabelColor": .quinaryLabelColor
        case "linkColor": .linkColor
        case "placeholderTextColor": .placeholderTextColor
        case "windowFrameTextColor": .windowFrameTextColor
        case "selectedMenuItemTextColor": .selectedMenuItemTextColor
        case "alternateSelectedControlTextColor": .alternateSelectedControlTextColor
        case "headerTextColor": .headerTextColor
        case "separatorColor": .separatorColor
        case "gridColor": .gridColor
        case "windowBackgroundColor": .windowBackgroundColor
        case "underPageBackgroundColor": .underPageBackgroundColor
        case "controlBackgroundColor": .controlBackgroundColor
        case "selectedContentBackgroundColor": .selectedContentBackgroundColor
        case "unemphasizedSelectedContentBackgroundColor": .unemphasizedSelectedContentBackgroundColor
        case "findHighlightColor": .findHighlightColor
        case "textColor": .textColor
        case "textBackgroundColor": .textBackgroundColor
        case "textInsertionPointColor": .textInsertionPointColor
        case "selectedTextColor": .selectedTextColor
        case "selectedTextBackgroundColor": .selectedTextBackgroundColor
        case "unemphasizedSelectedTextBackgroundColor": .unemphasizedSelectedTextBackgroundColor
        case "unemphasizedSelectedTextColor": .unemphasizedSelectedTextColor
        case "controlColor": .controlColor
        case "controlTextColor": .controlTextColor
        case "selectedControlColor": .selectedControlColor
        case "selectedControlTextColor": .selectedControlTextColor
        case "disabledControlTextColor": .disabledControlTextColor
        case "keyboardFocusIndicatorColor": .keyboardFocusIndicatorColor
        case "scrubberTexturedBackgroundColor": .scrubberTexturedBackground
        case "systemFillColor": .systemFill
        case "secondarySystemFillColor": .secondarySystemFill
        case "tertiarySystemFillColor": .tertiarySystemFill
        case "quaternarySystemFillColor": .quaternarySystemFill
        case "quinarySystemFillColor": .quinarySystemFill
        case "controlAccentColor": .controlAccentColor
        case "highlightColor": .highlightColor
        case "shadowColor": .shadowColor
        default: nil
        }
    }
    #endif
}
