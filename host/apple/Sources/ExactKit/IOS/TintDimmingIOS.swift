#if os(iOS) || os(tvOS)
import ExactCore
import UIKit

/// Greys every colour with a hue on the face of a control Exact draws while UIKit dims tints (a
/// sheet or an alert is up), as UIKit greys a tint of that colour. UIKit greys only a control's
/// tint-coloured parts, but such a face paints most of its colours itself (a badge's circle, a
/// tile's fill), so the face dims as one; neutral colours (label text) stay, as UIKit leaves them.
/// A control UIKit draws dims as UIKit dims it. Outside controls only links dim, as UIKit's do: a
/// status's symbol keeps its colour beside its words.
@MainActor
enum TintDimming {
    private static let probe = UIView()
    private static var greys: [[Int]: [Double]] = [:]
    /// The rows Exact hands UIKit as tints, which UIKit dims itself and keeps once the dimming ends
    static let tints: Set<String> = ["accent_color", "caret_color"]

    /// The grey UIKit gives a tint of this colour while it dims tints, in 0...255 sRGB channels, at
    /// the colour's own alpha (as a native button's dimmed symbol keeps its alpha), or at UIKit's
    /// (`uikitAlpha`, as a text view's link has it). A colour whose channels lie within 24 of each
    /// other counts as neutral and stays. A colour in its own space (`textChannels`' nine numbers)
    /// greys from its sRGB channels.
    static func grey(_ c: [Double], uikitAlpha: Bool = false) -> [Double] {
        guard c.count >= 4, (c[0...2].max() ?? 0) - (c[0...2].min() ?? 0) > 24 else { return c }
        let key = c[0...3].map { Int($0.rounded()) }
        let dimmed = greys[key] ?? {
            probe.tintColor = UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
            probe.tintAdjustmentMode = .dimmed
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            guard probe.tintColor.getRed(&r, green: &g, blue: &b, alpha: &a) else { return nil }
            let out = [Double(r), Double(g), Double(b), Double(a)].map { $0 * 255 }
            greys[key] = out
            return out
        }()
        guard let dimmed else { return c }
        return uikitAlpha ? dimmed : Array(dimmed[0...2]) + [c[3]]
    }
}

extension NodeView {
    /// This node or one above it is a control
    var insideControl: Bool {
        var current: UIView? = self
        while let view = current {
            if let node = view as? NodeView, node.isControl { return true }
            current = view.superview
        }
        return false
    }

    /// Whether this node's colours grey now: UIKit dims tints and the node is on the face of a
    /// control Exact draws. A control UIKit draws (`appearance` other than `none`) is left to UIKit.
    /// A node that becomes or stops being a control while UIKit dims tints, by its role or by moving
    /// into or out of one, does not repaint the nodes beneath it, which keep their earlier paint
    /// until they next paint.
    var dimsItsPaint: Bool {
        tintAdjustmentMode == .dimmed && !(kind == "control" && style["appearance"]?.string != "none") && insideControl
    }

    /// The tint this node paints with: UIKit's, which it dims, inside a control; outside one the
    /// tint as it rests
    var paintedTint: UIColor {
        tintAdjustmentMode == .dimmed && !insideControl ? restingTint : tintColor
    }

    /// The tint as it rests, since UIKit's `tintColor` is dimmed here, from the sources it comes
    /// from: the nearest authored `accent-color` (`applyAccentTint`), else the theme's `AccentColor`
    /// (`applyThemeTint`), else what the viewport last had undimmed (its embedder's or window's)
    var restingTint: UIColor {
        var current: UIView? = self, port: Viewport?
        while let view = current {
            // An ancestor's accent in this node's appearance, as its dynamic tint resolves here (`applyAccentTint`)
            if let node = view as? NodeView, node.style["accent_color"]?.namesTint != true,
               let accent = node.channels("accent_color", dark: drawsDark) { return TextEngine.color(accent) }
            if port == nil { port = view as? Viewport }
            current = view.superview
        }
        return ThemeRoles.color("AccentColor", in: self) ?? port?.restingTint ?? window?.tintColor ?? tintColor
    }

    /// The tint row `key` resolves `@tint` by (`ownTint`), but while UIKit dims tints the tint as it
    /// rests: on a face Exact draws, which then greys the colour at its own alpha (UIKit's dimmed
    /// tint has its own), and for a tint Exact hands UIKit (`TintDimming.tints`), which UIKit dims
    /// and shows again when the dimming ends
    func ownTint(for row: BatchValue?, key: String) -> UIColor? {
        guard row?.namesTint == true else { return nil }
        return tintAdjustmentMode == .dimmed && (TintDimming.tints.contains(key) || dimsItsPaint) ? restingTint : viewTint
    }

    /// The tint a symbol inherits as this node paints it (`inheritedTint`): on a face Exact draws
    /// while UIKit dims tints, the tint as it rests greyed at its own alpha; else `paintedTint`
    var inheritedInk: UIColor {
        dimsItsPaint ? UIColor(cgColor: dimmedWithTint(restingTint.cgColor, key: "tint_color")) : paintedTint
    }

    /// The colour of row `key` as this node paints it now: greyed while it dims its paint
    /// (`dimsItsPaint`), except a tint Exact hands UIKit (`TintDimming.tints`)
    func dimmedWithTint(_ c: [Double], key: String) -> [Double] {
        TintDimming.tints.contains(key) || !dimsItsPaint ? c : TintDimming.grey(c)
    }

    func dimmedWithTint(_ c: CGColor, key: String) -> CGColor {
        guard !TintDimming.tints.contains(key), dimsItsPaint,
              let rgb = c.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil),
              let parts = rgb.components, parts.count == 4 else { return c }
        let channels = parts.map { Double($0) * 255 }, grey = TintDimming.grey(channels)
        // Keep a neutral colour as written, in its own space and range (an HDR white)
        return grey == channels ? c : SRGBColors.color(grey[0], grey[1], grey[2], grey[3])
    }

    /// The colour of row `key` as written, never greyed: what Exact bakes into an image UIKit draws
    /// (a bar item's face), which UIKit leaves undimmed and which stays as baked once the dimming ends
    func undimmedChannels(_ key: String, dark: Bool) -> [Double]? {
        style[key].flatMap { $0.channels(dark: dark, contrast: drawsHighContrast, elevated: drawsElevated, tint: undimmedTint(for: $0)) }
    }

    /// The colour of row `key` as written, in its own space: what Exact hands UIKit to draw with (a
    /// tab bar's or an alert's tint, a title's colour), which UIKit dims itself and keeps once the
    /// dimming ends
    func undimmedCGColor(_ key: String) -> CGColor? {
        style[key].flatMap { $0.cgColor(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: undimmedTint(for: $0)) }
    }

    private func undimmedTint(for row: BatchValue) -> UIColor? {
        row.namesTint ? (tintAdjustmentMode == .dimmed ? restingTint : viewTint) : nil
    }

    /// The paragraph's runs as it paints them now: every run greyed while it dims its paint, and
    /// outside a control a link's where the paragraph names the tint (`tinted`), as UIKit greys a
    /// link in the tint. A run keeps no record of where its colour came from, so a link in a fixed
    /// colour greys too when another run in its paragraph names the tint.
    func dimmedRuns(_ runs: inout [Run], strut: inout Run, tinted: Bool) {
        guard tintAdjustmentMode == .dimmed else { return }
        let all = dimsItsPaint
        for i in runs.indices {
            if all { runs[i].greyColors() } else if tinted, !runs[i].href.isEmpty { runs[i].greyColors(uikitAlpha: true) }
        }
        if all { strut.greyColors() }
    }
}

extension Run {
    /// Each colour the run paints, greyed (`TintDimming.grey`): its ink, background, lines, shadow
    /// (after its offset and blur) and stroke (after its width)
    @MainActor mutating func greyColors(uikitAlpha: Bool = false) {
        let grey = { (c: [Double]) in TintDimming.grey(c, uikitAlpha: uikitAlpha) }
        color = color.map(grey)
        background = background.map(grey)
        underlineColor = underlineColor.map(grey)
        strikeColor = strikeColor.map(grey)
        shadow = shadow.map { Array($0.prefix(3)) + grey(Array($0.dropFirst(3))) }
        stroke = stroke.map { Array($0.prefix(1)) + grey(Array($0.dropFirst(1))) }
    }
}
#endif
