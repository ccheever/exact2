// @ref LLP 1095 D3, D5 — a colour row that names a platform colour: a role's
// class colour property (`secondaryLabelColor`), a `-exact-platform-color()`'s, an
// asset catalogue colour (`named:Brand`), or the view's inherited tint
// (`@tint`, `@tint/0.2` with an alpha). A class colour is one of the
// documented ones (`SystemColorNames`), resolved against the appearance the
// view draws in and the system's Increased Contrast; any other name is the
// fallback pair, logged once.
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

package enum SystemColor {
    /// CSS's `CanvasText`, the initial `color` (LLP 1095
    /// stage 2): the platform's own dynamic text colour, so a row the
    /// presenter has no value for still follows appearance and contrast.
    #if os(iOS) || os(tvOS)
    package static let canvasText: UIColor = .label
    #else
    package static let canvasText: NSColor = .textColor
    #endif
    /// `canvasText`'s channels under an appearance, for a painter that takes
    /// channels; the role's pair where the platform has no answer.
    static func canvasTextChannels(dark: Bool, contrast: Bool? = nil) -> [Double] {
        #if os(iOS) || os(tvOS)
        let name = "labelColor"
        #else
        let name = "textColor"
        #endif
        let pair: [Double] = dark ? [255, 255, 255, 255] : [0, 0, 0, 255]
        return channels(name, dark: dark, contrast: contrast, fallback: pair) ?? pair
    }

    private static let lock = NSLock()
    nonisolated(unsafe) private static var resolved: [String: [Double]] = [:]
    nonisolated(unsafe) private static var refused: Set<String> = []
    /// The app's tint as last read on the main thread, for a read off it.
    nonisolated(unsafe) private static var lastTint: Native?
    nonisolated(unsafe) private static var resolutions = 0

    #if os(macOS)
    /// @ref LLP 1115.002 D5, LLP 1115.000 D3.7 — set, on the main thread,
    /// while a view in a table's selected row that AppKit draws emphasized
    /// resolves its ink (`NodeView.paragraphSpec`, TableInkMac.swift): a
    /// role that is ink resolves to the selected item's text.
    nonisolated(unsafe) static var selectedItem = false
    /// The roles that are ink, and the share of
    /// `alternateSelectedControlTextColor` a stock table draws each at in an
    /// emphasized selected row (measured on macOS 27.0.1, TableInkMac.swift).
    /// The accent and the link's colour, which AppKit leaves alone and which
    /// cannot be read on the accent, are the selected item's text whole.
    private static let selectedInk: [String: Double] = [
        "labelColor": 1, "textColor": 1, "controlTextColor": 1, "headerTextColor": 1, "linkColor": 1, "@tint": 1,
        "secondaryLabelColor": 0.92, "tertiaryLabelColor": 0.61, "quaternaryLabelColor": 0.39,
    ]
    #endif

    /// Whether Increased Contrast is on, as `prefers-contrast` reads it (an
    /// agent's `prefer contrast more` pins it, LLP 1095 D7).
    static var highContrast: Bool { DisplayPreferences.contrast == "more" }

    /// A system colour changed (macOS's accent, say): every resolution is
    /// stale, and anything keyed by `generation` is made again.
    static func invalidate() {
        lock.lock(); resolved.removeAll(); refused.removeAll(); resolutions += 1; lock.unlock()
    }
    /// Counts `invalidate`s, for a cache of something resolved from these.
    static var generation: Int { lock.lock(); defer { lock.unlock() }; return resolutions }

    /// The share of the tint a tint's name says (`@tint` 1, `@tint/0.2` 0.2)
    static func tintShare(_ name: String) -> Double {
        name.contains("/") ? Double(name.split(separator: "/").last ?? "") ?? 1 : 1
    }

    /// The four channels (0–255) `name` shows in `dark`, else `fallback`.
    /// `contrast` is the requesting view's Increased Contrast (the system's
    /// when nil); `elevated` is iOS's user-interface level: a sheet's or a
    /// popover's content, whose dark backgrounds are lighter. `tint` is the
    /// opaque tint in `dark`, read on main for a resolution made off it;
    /// `tintColor` is the requesting view's own (its window's tint, D8).
    static func channels(_ name: String, dark: Bool, contrast: Bool? = nil, elevated: Bool = false,
                         tint resolvedTint: [Double]? = nil, tintColor: PlatformColor? = nil, fallback: [Double]?) -> [Double]? {
        let contrast = contrast ?? highContrast
        #if os(macOS)
        // The grouped-card metric carries its identity until its window is known.
        if name == "exactGroupedCardColor" { return dark && elevated ? [255, 255, 255, 8] : fallback }
        if selectedItem, Thread.isMainThread, let share = selectedInk[name],
           var c = rgba(.alternateSelectedControlTextColor, dark: dark, contrast: contrast) { c[3] *= share; return c }
        #endif
        if name.hasPrefix("@tint") {
            let alpha = tintShare(name)
            if var c = resolvedTint, c.count == 4 { c[3] *= alpha; return c }
            if let tintColor, var c = rgba(tintColor, dark: dark, contrast: contrast, elevated: elevated) { c[3] *= alpha; return c }
            return tint(alpha: alpha, dark: dark, contrast: contrast, fallback: fallback)
        }
        let key = "\(name)|\(dark ? 1 : 0)|\(contrast ? 1 : 0)|\(elevated ? 1 : 0)"
        lock.lock()
        if let hit = resolved[key] { lock.unlock(); return hit }
        let known = !refused.contains(name), seen = resolutions
        lock.unlock()
        guard known, let color = lookup(name) else {
            lock.lock()
            if refused.insert(name).inserted { NSLog("Exact: no platform colour %@; its fallback is shown (LLP 1095 D3)", name) }
            lock.unlock()
            return fallback
        }
        // A colour that will not resolve now (no sRGB form) is not refused:
        // these traits may resolve it, and the next ones may.
        guard let rgba = rgba(color, dark: dark, contrast: contrast, elevated: elevated) else { return fallback }
        // Kept only if no `invalidate` came between: a resolution begun
        // before one must not refill the cache with the old colour.
        lock.lock(); if resolutions == seen { resolved[key] = rgba }; lock.unlock()
        return rgba
    }

    #if os(iOS) || os(tvOS)
    private typealias Native = UIColor
    #else
    private typealias Native = NSColor
    #endif

    /// The platform's own colour a row names (`{"sys": "systemTealColor"}`),
    /// which the platform resolves for each trait itself; nil for any other row.
    static func native(_ row: BatchValue?) -> PlatformColor? {
        guard case .object(let o)? = row, let name = o["sys"]?.string, !name.hasPrefix("@"), name != "exactGroupedCardColor" else { return nil }
        return lookup(name)
    }

    /// The colour a name looks up: an asset, or a documented class colour
    /// of `UIColor`/`NSColor` (`SystemColorNames`); nil for any other name.
    private static func lookup(_ name: String) -> Native? {
        if name.hasPrefix("named:") {
            let asset = String(name.dropFirst(6))
            #if os(iOS) || os(tvOS)
            return UIColor(named: asset)
            #else
            return NSColor(named: asset)
            #endif
        }
        return SystemColorNames.color(name)
    }

    /// The colour's sRGB channels under an appearance and contrast.
    private static func rgba(_ color: Native, dark: Bool, contrast: Bool, elevated: Bool = false) -> [Double]? {
        #if os(iOS) || os(tvOS)
        var traits = [UITraitCollection(userInterfaceStyle: dark ? .dark : .light),
                      UITraitCollection(accessibilityContrast: contrast ? .high : .normal)]
        #if os(iOS)
        traits.append(UITraitCollection(userInterfaceLevel: elevated ? .elevated : .base))
        #endif
        let c = color.resolvedColor(with: UITraitCollection(traitsFrom: traits))
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        guard c.getRed(&r, green: &g, blue: &b, alpha: &a) else { return nil }
        return [r, g, b, a].map { Double(min(max($0, 0), 1)) * 255 }
        #else
        let name: NSAppearance.Name = dark
            ? (contrast ? .accessibilityHighContrastDarkAqua : .darkAqua)
            : (contrast ? .accessibilityHighContrastAqua : .aqua)
        var out: [Double]?
        NSAppearance(named: name)?.performAsCurrentDrawingAppearance {
            guard let c = color.usingColorSpace(.sRGB) else { return }
            out = [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map { Double(min(max($0, 0), 1)) * 255 }
        }
        return out
        #endif
    }

    /// `AccentColor`: the app's tint (iOS) or the system accent (macOS),
    /// times an alpha (`Highlight` is the tint at 0.2 on iOS, LLP 1095 §6.1).
    private static func tint(alpha: Double, dark: Bool, contrast: Bool, fallback: [Double]?) -> [Double]? {
        var color: Native?
        if Thread.isMainThread {
            #if os(iOS) || os(tvOS)
            color = appTint ?? defaultTint
            #else
            color = NSColor.controlAccentColor
            #endif
            lock.lock(); lastTint = color; lock.unlock()
        } else {
            lock.lock(); color = lastTint; lock.unlock()
        }
        // The fallback already carries the alpha (`Highlight`'s is the tint's at 0.2).
        guard let color, var c = rgba(color, dark: dark, contrast: contrast) else { return fallback }
        c[3] *= alpha
        return c
    }

    #if os(iOS) || os(tvOS)
    /// The tint of a window that sets none: the app's `AccentColor` asset
    /// (`NSAccentColorName`) as UIKit applies it, else system blue. A window
    /// reads nil until one is set; any view resolves it. On main.
    static let defaultTint: UIColor = UIView().tintColor
    #endif

    /// The app's tint, one for every session (LLP 1095 D9): its key
    /// window's, else its first window's, among the app's own windows at
    /// the normal level (never the keyboard's, often key), the foreground
    /// scene's first; nil with no window. What the
    /// kernel resolves `AccentColor` by, never a view's own, so sessions
    /// under different tints never overwrite each other's report. On main.
    static var appTint: PlatformColor? {
        #if os(iOS) || os(tvOS)
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        let ordered = scenes.filter { $0.activationState == .foregroundActive } + scenes.filter { $0.activationState != .foregroundActive }
        let windows = ordered.flatMap(\.windows).filter { $0.windowLevel == .normal }
        guard let window = windows.first(where: \.isKeyWindow) ?? windows.first else { return nil }
        return window.tintColor ?? defaultTint
        #else
        return NSColor.controlAccentColor
        #endif
    }

    /// A tint's opaque channels, light then dark under the system's
    /// contrast, for `channels(tint:)` (LLP 1095 D9): read here, on main
    /// (`appTint` for the report), since the owner thread that reports
    /// colours can read no window.
    static func tintPair(_ color: PlatformColor?) -> [[Double]]? {
        let color = color ?? {
            #if os(iOS) || os(tvOS)
            return defaultTint
            #else
            return .controlAccentColor
            #endif
        }()
        let pair = [false, true].compactMap { rgba(color, dark: $0, contrast: highContrast) }
        return pair.count == 2 ? pair : nil
    }
}

#if os(iOS) || os(tvOS)
extension SystemColor {
    /// Whether a platform colour resolves differently under `b` than under
    /// `a`: by style, contrast and, on iOS, the level (tvOS has none).
    static func resolvesDifferently(_ a: UITraitCollection, _ b: UITraitCollection) -> Bool {
        #if os(iOS)
        if a.userInterfaceLevel != b.userInterfaceLevel { return true }
        #endif
        return a.userInterfaceStyle != b.userInterfaceStyle || a.accessibilityContrast != b.accessibilityContrast
    }
}
#endif

extension BatchValue {
    /// Whether the value names the view's tint (`AccentColor`, `Highlight`)
    /// anywhere: a colour, a palette's, a shadow's.
    var namesTint: Bool {
        switch self {
        case .object(let o): return o["sys"]?.string?.hasPrefix("@tint") == true || o.values.contains { $0.namesTint }
        case .array(let a): return a.contains { $0.namesTint }
        default: return false
        }
    }

    /// The share of the tint a colour is when it is the tint (`AccentColor` 1, `Highlight` 0.2);
    /// nil for any other value
    var tintAlpha: Double? {
        guard case .object(let o) = self, let name = o["sys"]?.string, name.hasPrefix("@tint") else { return nil }
        return SystemColor.tintShare(name)
    }
}

#if os(iOS) || os(tvOS)
extension BatchValue {
    /// The colour as a dynamic colour resolves it in `traits`, which carry the tint of the view UIKit
    /// draws it in
    package func channels(in traits: UITraitCollection, contrast: Bool?, elevated: Bool = false) -> [Double]? {
        channels(dark: traits.userInterfaceStyle == .dark, contrast: contrast, elevated: elevated,
                 tint: namesTint ? UIColor.tintColor.resolvedColor(with: traits) : nil)
    }
}

extension UIView.TintAdjustmentMode {
    /// A node's `-exact-tint-adjustment` (LLP 1077 D19) as UIKit's mode for its views, or a list row's
    /// cell: `normal` keeps the tint as it is behind a sheet or an alert; `auto`, or none, is
    /// `.automatic`, the superview's, as UIKit carries a mode down
    package init(_ row: BatchValue?) {
        switch row?.string {
        case "normal": self = .normal
        case "dimmed": self = .dimmed
        default: self = .automatic
        }
    }
}

extension SystemColor {
    nonisolated(unsafe) private static var tints: [Double: UIColor] = [:]
    /// The tint at `alpha` as UIKit draws it (`UIColor.tintColor`): resolved by the tint of the view
    /// it is drawn in, so it follows that tint and dims with it. One object per alpha, so writing it
    /// again changes nothing.
    static func tint(_ alpha: Double) -> UIColor {
        if alpha >= 1 { return .tintColor }
        lock.lock(); defer { lock.unlock() }
        if let made = tints[alpha] { return made }
        let made = UIColor.tintColor.withAlphaComponent(alpha)
        tints[alpha] = made
        return made
    }
}
#endif

extension NodeView {
    /// The tint `row` resolves `@tint` by: this view's inherited
    /// `tintColor` on iOS, so a second window's is its own (LLP 1095 D8);
    /// on macOS the nearest authored `accent-color` at or above it (LLP
    /// 1115.000 D3.4), nil where there is none: the system's accent.
    func ownTint(for row: BatchValue?) -> PlatformColor? {
        row?.namesTint == true ? viewTint : nil
    }
    var viewTint: PlatformColor? {
        #if os(iOS) || os(tvOS)
        return tintColor
        #else
        return nearestAccent
        #endif
    }
    #if os(iOS) || os(tvOS)
    /// A window or ancestor tint changed, or UIKit dimmed it or brought it back: the rows naming it
    /// apply again, as UIKit asks of a view that draws in its tint.
    package override func tintColorDidChange() {
        super.tintColorDidChange()
        tintPadBar()
        // An `svg`'s paints are resolved into its scene's layers.
        if kind == "svg" { presenter?.svg.reappear(id, dark: drawsDark, clock: presenter?.session?.clock, tint: viewTint) }
        guard style.values.contains(where: \.namesTint) || inlineText.contains(where: \.namesTint) else { return }
        paragraphOwner.invalidateText(); paragraphOwner.setNeedsDisplay()
        applyStyle(style)
        // The picture of a filtered box above it shows its colours
        presenter?.boxFilters.changed(self)
        // The canvas, which shows the first root's background
        if superview === presenter?.root { presenter?.paintCanvas() }
        presenter?.controls.tintChanged(self)
    }

    /// The node's tint rows, applied where they changed from `old`: its accent (`applyAccentTint`) and
    /// its `-exact-tint-adjustment`, UIKit's `tintAdjustmentMode` (LLP 1077 D19), which UIKit carries down
    func applyTint(from old: NodeStyle) {
        if old["accent_color"] != style["accent_color"] { applyAccentTint() }
        if old["tint_adjustment"] != style["tint_adjustment"] { tintAdjustmentMode = .init(style["tint_adjustment"]) }
    }

    /// An authored `accent-color` is its subtree's tint (LLP 1116.001 D2),
    /// as a hand-built UIKit app sets a view's `tintColor`: UIKit carries it
    /// down, and every row beneath that names the tint (`AccentColor`,
    /// `LinkText`, a symbol's default) resolves by it (`ownTint`). `auto`, or
    /// an accent that names the tint itself, leaves the inherited one.
    /// Called when the row changes; nothing else sets a node view's tint. The tint is made once
    /// (`authoredAccentTint`), in each appearance as the row resolves: one object for every view
    /// handed it, so writing it again changes nothing.
    func applyAccentTint() {
        // The colour reads the row as it is when UIKit resolves it, so a view holding it (a bar) follows
        // a new accent; not one that names the tint, nor one gone, which a view under one holding this
        // colour would resolve by this colour itself: the window's then, until the view holding it is
        // given its new tint, at its site's next write in the same batch
        authoredAccentTint = style["accent_color"]?.namesTint != true && channels("accent_color", dark: false) != nil ? UIColor { [weak self] traits in
            guard let self, let row = style["accent_color"], !row.namesTint else { return self?.window?.tintColor ?? .systemBlue }
            return row.channels(dark: traits.userInterfaceStyle == .dark, contrast: drawsHighContrast, elevated: drawsElevated).map { TextEngine.color($0) } ?? .systemBlue
        } : nil
        tintColor = authoredAccentTint
    }

    /// The nearest node at or above this one in the authored tree (`Presenter.authoredParent`: a
    /// route under its tabpanel, a sheet under its invoker) whose authored `accent-color` is its
    /// subtree's tint
    private var accentAuthor: NodeView? {
        sequence(first: self as UIView, next: { [weak presenter] in presenter?.authoredParent(of: $0) ?? $0.superview })
            .lazy.compactMap { $0 as? NodeView }.first { $0.authoredAccentTint != nil }
    }

    /// The tint a view UIKit draws for this node outside the node's views takes as its own (a bar, a
    /// bar item, a readout, a tab bar, a list row's switch, a route's view, a spinner), as
    /// `applyAccentTint` gives the node's views theirs: the nearest authored `accent-color` at or
    /// above it in the authored tree, unless that is the roots', which their window takes
    /// (`AppAccent`); nil where the node's tint is theirs, which such a view inherits.
    package var ownAccent: UIColor? { ownAccentAuthor?.authoredAccentTint }
    /// The node whose accent `ownAccent` is
    private var ownAccentAuthor: NodeView? {
        accentAuthor.flatMap { $0.style["accent_color"]?.key == presenter?.accent.said ? nil : $0 }
    }

    /// The tint this node draws in, as a value for what Exact resolves or bakes for UIKit to keep (a
    /// view's own tint made from it, a face baked into an image, a native button's configuration):
    /// its `tintColor`, but undimmed where UIKit dims it (the nearest authored `accent-color` at or
    /// above it, else the theme's `AccentColor`, else the session's viewport's as UIKit last gave it
    /// undimmed, else its window's, which UIKit never dims), and out of a window with only UIKit's
    /// default, the roots' accent, which the window will take (`AppAccent`), else the app's.
    var carriedTint: UIColor {
        if tintAdjustmentMode == .dimmed {
            return accentAuthor?.authoredAccentTint ?? ThemeRoles.color("AccentColor", in: self) ?? (presenter?.viewport as? Viewport)?.restingTint
                ?? window?.tintColor ?? tintColor
        }
        guard window == nil, tintColor == SystemColor.defaultTint else { return tintColor }
        return presenter?.accent.tint ?? SystemColor.appTint ?? tintColor
    }

    /// The colour of row `key` by `carriedTint`
    func carriedChannels(_ key: String, dark: Bool) -> [Double]? {
        style[key].flatMap { $0.channels(dark: dark, contrast: drawsHighContrast, elevated: drawsElevated, tint: $0.namesTint ? carriedTint : nil) }
    }

    /// The colour of row `key` by `carriedTint`, in its own space
    func carriedColor(_ key: String) -> UIColor? {
        style[key].flatMap { $0.cgColor(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: $0.namesTint ? carriedTint : nil) }.map { UIColor(cgColor: $0) }
    }

    /// The node's `accent-color` by `carriedTint`, for a native button's configuration, which draws
    /// colours made from its value (`ButtonConfigurationIOS`)
    var carriedAccent: UIColor? { carriedChannels("accent_color", dark: drawsDark).map { TextEngine.color($0) } }

    /// The colour of row `key` for UIKit to draw, nil where it is unset: where it names the tint
    /// (`AccentColor`, `Highlight`), the tint as UIKit draws it (`SystemColor.tint`), which follows
    /// the tint of the view it is drawn in and dims with it; else the colour as written.
    package func uiColor(_ key: String) -> UIColor? {
        guard let row = style[key] else { return nil }
        guard let share = row.tintAlpha else { return cgColor(key).map { UIColor(cgColor: $0) } }
        return SystemColor.tint(share)
    }

    /// `uiColor` of the colour this node's author wrote for row `key`; nil where the author wrote none
    func ownUIColor(_ key: String) -> UIColor? { ownColor(key) == nil ? nil : uiColor(key) }

    /// The node's `accent-color` for UIKit to draw, nil where it has none (or `auto`): the tint as
    /// UIKit draws it at the accent's share, since an authored accent is the node's tint
    /// (`applyAccentTint`)
    package func accentUIColor() -> UIColor? {
        guard let row = style["accent_color"], row.tintAlpha != nil || channels("accent_color", dark: false) != nil else { return nil }
        return SystemColor.tint(row.tintAlpha ?? 1)
    }

    /// The node's `accent-color` for a view in its views that draws in its tint (a progress bar, a
    /// checkbox): nil, so the view draws in the tint, which the accent is (`applyAccentTint`), but a
    /// share of the tint (`Highlight`) as UIKit's tint at that share, since UIKit follows no view's
    /// own tint made from the one above it
    var accentShare: UIColor? {
        style["accent_color"]?.tintAlpha.flatMap { $0 < 1 ? SystemColor.tint($0) : nil }
    }

    /// The colour of row `key` as a UIKit view's own tint, nil where it is unset or the tint itself,
    /// which the view inherits; else as written, the tint in it by `carriedTint`, since UIKit follows
    /// no view's own tint made from the one above it
    func ownTintColor(_ key: String) -> UIColor? {
        guard let row = style[key], row.tintAlpha != 1 else { return nil }
        return row.cgColor(dark: drawsDark, contrast: drawsHighContrast, elevated: drawsElevated, tint: row.namesTint ? carriedTint : nil).map { UIColor(cgColor: $0) }
    }

    /// The colour of row `key` as a dynamic colour resolves it in `traits`, the tint they carry included
    func channels(_ key: String, in traits: UITraitCollection) -> [Double]? {
        style[key]?.channels(in: traits, contrast: drawsHighContrast, elevated: drawsElevated)
    }
    #endif

    /// The Increased Contrast this view draws in (LLP 1095 D5): its own
    /// trait, so a subtree's override counts; nil before it has one, and
    /// on macOS, where contrast is the system's alone (AppKit's named
    /// high-contrast appearances resolve as plain Aqua).
    var drawsHighContrast: Bool? {
        #if os(iOS) || os(tvOS)
        switch traitCollection.accessibilityContrast {
        case .high: return true
        case .normal: return false
        default: return nil
        }
        #else
        return nil
        #endif
    }
    /// Whether this view draws at iOS's elevated level (a sheet, a popover).
    var drawsElevated: Bool {
        #if os(iOS)
        return traitCollection.userInterfaceLevel == .elevated
        #else
        return false
        #endif
    }
}
