#if os(iOS) || os(tvOS)
import ExactCore
import UIKit

/// Greys every colour with a hue on the face of a control Exact draws while UIKit dims tints (a
/// sheet or an alert is up), as UIKit greys a tint of that colour. UIKit greys only a control's
/// tint-coloured parts, but such a face paints most of its colours itself (a badge's circle, a
/// tile's fill), so the face dims as one; neutral colours (label text) stay, as UIKit leaves them.
/// A control UIKit draws dims as UIKit dims it. Outside controls only links in the tint dim, by
/// UIKit's dimmed tint, as a text view's do: a status's symbol keeps its colour beside its words.
@MainActor
enum TintDimming {
    private static var greys: [[Double]: Double] = [:]
    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!, gray = CGColorSpace(name: CGColorSpace.extendedGray)!
    /// The rows Exact hands UIKit as tints, which UIKit dims itself and keeps once the dimming ends
    static let tints: Set<String> = ["accent_color", "caret_color"]
    /// The share of a colour's alpha UIKit's dimmed tint keeps (TintDimmingSweepIOSTests). Exact's grey keeps the
    /// colour's own alpha instead, as a native button's dimmed symbol keeps its alpha.
    static let uikitDimmedAlpha = 0.8

    /// The grey UIKit gives a tint of this colour while it dims tints, in 0...255 sRGB channels, at
    /// the colour's own alpha. UIKit's dimmed tint is exactly the colour converted to grey by Core
    /// Graphics (to `extendedGray`: D50 luminance on sRGB's curve, from the colour's own space;
    /// TintDimmingSweepIOSTests), so it is converted here, touching no view: this runs inside UIKit's
    /// tint traversal and Core Animation's commit, where making a view or dimming one again faults,
    /// and a view kept for it answered every colour with the first one's grey. A colour whose sRGB
    /// channels lie within 24 of each other counts as neutral and stays. A colour in its own space
    /// (`textChannels`' nine numbers) greys from that space, as UIKit greys a display-p3 colour; a
    /// grey brighter than sRGB's white comes back as nine numbers in extended sRGB, unclipped.
    static func grey(_ c: [Double]) -> [Double] {
        guard c.count >= 4, (c[0...2].max() ?? 0) - (c[0...2].min() ?? 0) > 24 else { return c }
        let own = c.count == 9 ? ownSpace(c[4]) : nil
        let key = own == nil ? Array(c[0...3]) : Array(c[4...8])
        let level = greys[key] ?? {
            // In its extended space, as `TextEngine.color` paints it, untagged: UIKit's grey ignores a colour's headroom tag
            let colour = own.flatMap { CGColor(colorSpace: $0, components: c[5...8].map { CGFloat($0) }) }
                ?? CGColor(colorSpace: srgb, components: c[0...3].map { CGFloat($0 / 255) })
            return colour.flatMap { level($0, key) }
        }()
        guard let level else { return c }
        return level <= 255 ? [level, level, level, c[3]] : [255, 255, 255, c[3], 0, level / 255, level / 255, level / 255, c[3] / 255]
    }

    /// `grey` of a colour in any space, from that space; a neutral one exactly as it came, in its own
    /// space and range (an HDR white), and a grey past sRGB's white in extended sRGB
    static func grey(_ c: CGColor) -> CGColor {
        // Judged neutral or not untagged too: Core Graphics tone-maps a tagged colour on the way to sRGB
        guard let rgb = untagged(c).converted(to: srgb, intent: .defaultIntent, options: nil),
              let parts = rgb.components, parts.count == 4 else { return c }
        let channels = parts.map { Double($0) * 255 }
        guard (channels[0...2].max() ?? 0) - (channels[0...2].min() ?? 0) > 24,
              let level = level(untagged(c), cgKey(c)) else { return c }
        if level <= 255 { return SRGBColors.color(level, level, level, channels[3]) }
        // Past white: in extended sRGB, tagged with its headroom as the painters tag an HDR colour
        return CGColor(colorSpace: CGColorSpace(name: CGColorSpace.extendedSRGB)!, components: [level / 255, level / 255, level / 255, CGFloat(channels[3] / 255)])
            .map(ColorRange.tagged) ?? c
    }

    /// The space `textChannels` names by its code, extended as `TextEngine.color` draws it: sRGB (or
    /// a profile's colour, as extended sRGB), display-p3, linear sRGB
    private static func ownSpace(_ code: Double) -> CGColorSpace? {
        [0: CGColorSpace.extendedSRGB, 1: CGColorSpace.extendedDisplayP3, 2: CGColorSpace.extendedLinearSRGB][Int(code)].flatMap { CGColorSpace(name: $0) }
    }

    /// The grey level (0...255, past 255 for a colour past sRGB's white) of `colour`, kept under `key`
    /// (none: not kept)
    private static func level(_ colour: CGColor, _ key: [Double]?) -> Double? {
        if let key, let kept = greys[key] { return kept }
        guard let g = colour.converted(to: gray, intent: .defaultIntent, options: nil)?.components?.first else { return nil }
        // Bound the cache as `SRGBColors` bounds its own: a colour transition on a dimmed face asks for a new grey each frame
        if let key {
            if greys.count >= 512 { greys.removeAll() }
            greys[key] = Double(g) * 255
        }
        return Double(g) * 255
    }

    /// The colour without its headroom tag (`ColorRange.tagged`'s): UIKit's dimmed tint ignores the tag
    /// (TintDimmingSweepIOSTests), where Core Graphics' grey of a tagged colour tone-maps it
    private static func untagged(_ c: CGColor) -> CGColor {
        guard let space = c.colorSpace, let parts = c.components else { return c }
        return CGColor(colorSpace: space, components: parts) ?? c
    }

    /// A `CGColor`'s cache key: a marker no channel takes, its space's place among the named spaces a
    /// colour here comes in, then its components (its grey ignores a headroom tag, so the key does);
    /// none for a colour in another space, which is converted every time
    private static func cgKey(_ c: CGColor) -> [Double]? {
        guard let name = c.colorSpace?.name as String?, let space = cgSpaces.firstIndex(of: name) else { return nil }
        return [-1e9, Double(space)] + (c.components ?? []).map { Double($0) }
    }
    private static let cgSpaces = [CGColorSpace.sRGB, CGColorSpace.extendedSRGB, CGColorSpace.displayP3, CGColorSpace.extendedDisplayP3,
                                   CGColorSpace.linearSRGB, CGColorSpace.extendedLinearSRGB].map { $0 as String }
}

/// A view a session's nodes are mounted in: the viewport, the `DocumentMount` native navigation
/// holds the document fixed in, a route's or a modal's `PresentationMount`, a dialog sheet's view,
/// a `PopoverLayer`, or a `FlightLayer`. Its tint undimmed, its embedder's or its window's, is what
/// a status outside a control keeps while UIKit dims tints (`NodeView.restingTint`), as it is now:
/// an embedder's tint changed while dimmed, or one the mount joined while already dimmed, included.
protocol RestsTint: UIView {
    var restingTint: UIColor { get }
}

extension Viewport: RestsTint {}

/// A hidden view UIKit leaves undimmed (`tintAdjustmentMode = .normal`): its `tintColor` is the tint
/// its superview inherits, undimmed, whatever dims it, as UIKit resolves a view's tint from the
/// nearest one set and its own adjustment
final class TintProbe: UIView {
    override init(frame: CGRect) {
        super.init(frame: frame)
        isHidden = true; isUserInteractionEnabled = false; tintAdjustmentMode = .normal
    }
    required init?(coder: NSCoder) { nil }
}

/// A view nodes are mounted in whose tint undimmed a `TintProbe` reads (`RestsTint`)
package class RestingTintView: UIView, RestsTint {
    private let probe = TintProbe()
    var restingTint: UIColor { probe.tintColor }
    package override init(frame: CGRect) { super.init(frame: frame); addSubview(probe) }
    package required init?(coder: NSCoder) { nil }
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
    /// control Exact draws. A control UIKit draws is left to UIKit: a native control (`appearance`
    /// other than `none`) and a native field or text area (`isNativeTextControl`).
    var dimsItsPaint: Bool {
        tintAdjustmentMode == .dimmed && !(kind == "control" && style["appearance"]?.string != "none") && !isNativeTextControl && insideControl
    }

    /// The tint this node paints with: UIKit's, which it dims, inside a control; outside one the
    /// tint as it rests
    var paintedTint: UIColor {
        tintAdjustmentMode == .dimmed && !insideControl ? restingTint : tintColor
    }

    /// The tint as it rests, since UIKit's `tintColor` is dimmed here, from the sources it comes
    /// from: the nearest authored `accent-color` (`authoredAccentTint`), else the theme's `AccentColor`
    /// (`applyThemeTint`), else the undimmed tint of the nearest view the node is mounted in (`RestsTint`)
    var restingTint: UIColor {
        var current: UIView? = self, port: (any RestsTint)?
        while let view = current {
            if let node = view as? NodeView, let accent = node.authoredAccentTint { return accent }
            if port == nil { port = view as? RestsTint }
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
        dimsItsPaint ? UIColor(cgColor: dimmedWithTint(restingTint.resolvedColor(with: traitCollection).cgColor, key: "tint_color")) : paintedTint
    }

    /// The colour of row `key` as this node paints it now: greyed while it dims its paint
    /// (`dimsItsPaint`), except a tint Exact hands UIKit (`TintDimming.tints`)
    func dimmedWithTint(_ c: [Double], key: String) -> [Double] { greys(key) ? TintDimming.grey(c) : c }

    func dimmedWithTint(_ c: CGColor, key: String) -> CGColor { greys(key) ? TintDimming.grey(c) : c }

    private func greys(_ key: String) -> Bool { !TintDimming.tints.contains(key) && dimsItsPaint }

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

    /// The tint row `row` resolves `@tint` by for a colour Exact hands UIKit (`undimmedChannels`)
    func undimmedTint(for row: BatchValue) -> UIColor? {
        row.namesTint ? undimmedInheritedTint : nil
    }

    /// Inks the symbol view in its node's ink and keeps UIKit from dimming it, as the ink is already the
    /// node's, dimmed or not
    func inkSymbolView() {
        symbolView?.tintColor = drawnSymbolInk
        symbolView?.tintAdjustmentMode = .normal
    }

    /// The tint this node inherits, undimmed: what a face UIKit draws takes (`symbolConfiguration`)
    var undimmedInheritedTint: UIColor { tintAdjustmentMode == .dimmed ? restingTint : tintColor }

    /// The paragraph's runs as it paints them now: every run greyed while it dims its paint
    func dimmedRuns(_ runs: inout [Run], strut: inout Run) {
        guard dimsItsPaint else { return }
        for i in runs.indices { runs[i].greyColors() }
        strut.greyColors()
    }

    /// The tint a link's run resolves `@tint` by while UIKit dims tints outside a control: UIKit's own,
    /// dimmed, as a text view's link in the tint dims, where the paragraph's runs resolve the tint as
    /// it rests (`tint`, nil where nothing in the paragraph names it). A colour that does not name the
    /// tint does not change. Nil where no link's run differs from the paragraph's.
    func linkTint(_ tint: PlatformColor?) -> PlatformColor? {
        tint != nil && tintAdjustmentMode == .dimmed && !dimsItsPaint ? tintColor : nil
    }

    /// Paints this node again as a tint change paints it: its rows that name the tint, its colours
    /// on a control's face, its flat boxes, the filtered pictures above it and its background tracks
    func repaintWithTint() {
        paragraphOwner.invalidateText(); paragraphOwner.setNeedsDisplay()
        applyStyle(style)
        presenter?.flats.tintChanged(id)
        presenter?.boxFilters.changed(self)
        presenter?.svg.refreshBoxAnimations(id, clock: presenter?.session?.clock)
    }

    /// Paints this node and the nodes beneath it again while UIKit dims tints, after what makes them
    /// a control's face changed with no tint change for UIKit to report: a role or a link of a node
    /// at or above them (`controlMayHaveChanged`), or a move into or out of a control
    func repaintFacesWhileDimmed() {
        guard tintAdjustmentMode == .dimmed else { return }
        func walk(_ view: UIView) {
            (view as? NodeView)?.repaintWithTint()
            for child in view.subviews { walk(child) }
        }
        walk(self)
    }

    /// Notes whether this node is a control now (`isControl`), as its props changed, and paints the
    /// nodes beneath it again while UIKit dims tints where that changed
    func controlMayHaveChanged() {
        guard let presenter else { return }
        let now = isControl
        guard presenter.controlNodes.contains(id) != now else { return }
        if now { presenter.controlNodes.insert(id) } else { presenter.controlNodes.remove(id) }
        repaintFacesWhileDimmed()
    }

    /// A box's background track's colour as this node paints it now: greyed while it dims its paint
    var boxTrackGrey: ((CGColor) -> CGColor)? {
        guard dimsItsPaint else { return nil }
        return { (color: CGColor) in TintDimming.grey(color) }
    }
}

extension Run {
    /// Each colour the run paints, greyed (`TintDimming.grey`): its ink, background, lines, shadow
    /// (after its offset and blur) and stroke (after its width)
    @MainActor mutating func greyColors() {
        let grey = { (c: [Double]) in TintDimming.grey(c) }
        color = color.map(grey)
        background = background.map(grey)
        underlineColor = underlineColor.map(grey)
        strikeColor = strikeColor.map(grey)
        shadow = shadow.map { Array($0.prefix(3)) + grey(Array($0.dropFirst(3))) }
        stroke = stroke.map { Array($0.prefix(1)) + grey(Array($0.dropFirst(1))) }
    }
}
#endif
