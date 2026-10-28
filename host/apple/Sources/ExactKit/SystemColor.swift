// @ref LLP 1095 D3, D5 — a colour row that names a platform colour: a role's
// class colour property (`secondaryLabelColor`), a `-exact-platform-color()`'s, an
// asset catalogue colour (`named:Brand`), or the view's inherited tint
// (`@tint`, `@tint/0.2` with an alpha). Resolved by name at runtime, so a
// colour an OS adds works with no Exact change, against the appearance the
// view draws in and the system's Increased Contrast; anything the OS lacks
// or that is not a colour is the fallback pair, logged once.
import ObjectiveC
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
            let alpha = name.contains("/") ? Double(name.split(separator: "/").last ?? "") ?? 1 : 1
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

    /// The colour a name looks up: an asset, or a zero-argument class method
    /// of `UIColor`/`NSColor` that returns a colour. The name is already
    /// checked (`roles.rs`'s `native_name_ok`), so no private or allocating
    /// selector is reachable.
    private static func lookup(_ name: String) -> Native? {
        if name.hasPrefix("named:") {
            let asset = String(name.dropFirst(6))
            #if os(iOS) || os(tvOS)
            return UIColor(named: asset)
            #else
            return NSColor(named: asset)
            #endif
        }
        guard name.hasSuffix("Color"), !name.hasPrefix("_") else { return nil }
        let selector = NSSelectorFromString(name)
        guard let method = class_getClassMethod(Native.self, selector), method_getNumberOfArguments(method) == 2 else { return nil }
        // An object return (D3): `perform` would read a number or a struct as one.
        let returns = method_copyReturnType(method)
        defer { free(returns) }
        guard String(cString: returns) == "@" else { return nil }
        let value = (Native.self as AnyObject).perform(selector)?.takeUnretainedValue()
        return value as? Native
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
}

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
        return paintedTint
        #else
        return nearestAccent
        #endif
    }
    #if os(iOS) || os(tvOS)
    /// A window or ancestor tint changed: the rows naming it apply again.
    package override func tintColorDidChange() {
        super.tintColorDidChange()
        tintPadBar()
        // An `svg`'s paints are resolved into its scene's layers.
        if kind == "svg" { presenter?.svg.reappear(id, dark: drawsDark, clock: presenter?.session?.clock, tint: viewTint) }
        // A symbol counts: hierarchical and palette glyphs bake the tint in. In a control, every
        // colour greys with the tint (TintDimmingIOS.swift), and so do the boxes drawn flat in it and
        // the pictures of the filtered boxes above it, which copy its paint.
        guard style.values.contains(where: \.namesTint) || inlineText.contains(where: \.namesTint) || symbolView != nil || insideControl else { return }
        paragraphOwner.invalidateText(); paragraphOwner.setNeedsDisplay()
        applyStyle(style)
        if insideControl { presenter?.flats.tintChanged(id); presenter?.boxFilters.changed(self) }
        presenter?.controls.tintChanged(self)
    }

    /// An authored `accent-color` is its subtree's tint (LLP 1116.001 D2),
    /// as a hand-built UIKit app sets a view's `tintColor`: UIKit carries it
    /// down, and every row beneath that names the tint (`AccentColor`,
    /// `LinkText`, a symbol's default) resolves by it (`ownTint`). `auto`, or
    /// an accent that names the tint itself, leaves the inherited one.
    /// Called when the row changes; nothing else sets a node view's tint.
    func applyAccentTint() {
        guard style["accent_color"]?.namesTint != true, channels("accent_color", dark: false) != nil else {
            tintColor = nil
            return
        }
        tintColor = UIColor { [weak self] traits in
            self?.channels("accent_color", dark: traits.userInterfaceStyle == .dark).map { TextEngine.color($0) } ?? .systemBlue
        }
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
