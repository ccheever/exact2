// The app's accent on UIKit (LLP 1095, "There is no accent role"): the
// root's `accent-color` is its window's tint, as an app's AccentColor asset
// is, so every view, sheet, alert and bar inherits it, and the accent Exact
// resolves itself (`SystemColor.appTint`) is it. With no root accent the
// window keeps its own, which UIKit gives the app's `AccentColor` asset
// (`NSAccentColorName`). A control still reads its own `accent-color`.
#if os(iOS) || os(tvOS)
import UIKit

final class AppAccent {
    private unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }
    /// The roots' accent row (its `key` names it) and the tint made of it;
    /// nil when no root says one, and the window has its own back.
    private var said: String?
    private var tint: UIColor?
    /// The window the accent tints, held weakly: the window holds the view
    /// that holds this session.
    private weak var window: UIWindow?
    /// Set while the accent lets its window go: the window's tint changing
    /// back restyles the roots, which must not take it again.
    private var releasing = false

    /// A new set of roots says its accent, the first root's that has one,
    /// before it joins the window: set after, UIKit tells every view in the
    /// tree its tint changed.
    func roots(_ roots: [NodeView]) {
        guard !releasing else { return }
        set(roots.lazy.compactMap(Self.own).first)
    }

    /// A new set of roots joins the presenter: its accent is said first, and
    /// a root's restyle while the others are still joining (its inherited
    /// tint changed) reads the whole set, not the part attached so far.
    func attach(_ set: [NodeView], _ add: ([NodeView]) -> Void) {
        roots(set)
        joining = set
        defer { joining = nil }
        add(set)
    }
    private var joining: [NodeView]?

    /// A root restyled: the roots say the accent again, by the same rule.
    func restyled(_ node: NodeView) {
        if let joining {
            if joining.contains(node) { roots(joining) }
            return
        }
        guard node.superview === presenter.root else { return }
        roots(presenter.root.subviews.compactMap { $0 as? NodeView })
    }

    /// The window the view is in takes the accent. A window the view left,
    /// or one with no accent said, has its own back.
    func apply() {
        let current = presenter.viewport.window
        if let window, window !== current || tint == nil {
            Ledger.leave(window, self)
            self.window = nil
        }
        guard let current, let tint else { return }
        window = current
        Ledger.take(current, self, tint)
    }

    /// The session ends: its window has its own tint back.
    func release() {
        releasing = true
        defer { releasing = false }
        (said, tint) = (nil, nil)
        apply()
    }

    /// A colour of the node's own: `AccentColor`, `auto` or none is the platform's.
    private static func own(_ node: NodeView) -> BatchValue? {
        guard let row = node.style["accent_color"], row.string != "auto", !node.followsTint("accent_color") else { return nil }
        return row
    }

    private func set(_ row: BatchValue?) {
        guard row?.key != said else { return }
        said = row?.key
        // Each appearance, contrast and level, in its own colour space and
        // unclipped (LLP 1100 D2), as UIKit's own dynamic colours answer the traits
        tint = row.map { row in
            let made = (0..<8).map { i in row.cgColor(dark: i & 4 != 0, contrast: i & 2 != 0, elevated: i & 1 != 0).map(UIColor.init(cgColor:)) }
            return UIColor { traits in
                #if os(iOS)
                let elevated = traits.userInterfaceLevel == .elevated
                #else
                let elevated = false // tvOS has no elevated level
                #endif
                let i = (traits.userInterfaceStyle == .dark ? 4 : 0) | (traits.accessibilityContrast == .high ? 2 : 0) | (elevated ? 1 : 0)
                return made[i] ?? made[i & 4] ?? made[0] ?? .systemBlue
            }
        }
        apply()
    }

    /// The accents of every session in one window (an app may host several):
    /// the window's own tint, kept once, and each session's, in the order
    /// they took the window. The latest still there tints it; the last to
    /// leave gives the window its own tint back.
    private final class Ledger {
        private static let ledgers = NSMapTable<UIWindow, Ledger>.weakToStrongObjects()
        private let own: UIColor?
        private var held: [(accent: ObjectIdentifier, tint: UIColor)] = []
        private init(_ own: UIColor?) { self.own = own }

        static func take(_ window: UIWindow, _ accent: AppAccent, _ tint: UIColor) {
            let ledger = ledgers.object(forKey: window) ?? Ledger(window.tintColor)
            ledgers.setObject(ledger, forKey: window)
            ledger.held.removeAll { $0.accent == ObjectIdentifier(accent) }
            ledger.held.append((ObjectIdentifier(accent), tint))
            if window.tintColor !== tint { window.tintColor = tint }
        }

        static func leave(_ window: UIWindow, _ accent: AppAccent) {
            guard let ledger = ledgers.object(forKey: window) else { return }
            ledger.held.removeAll { $0.accent == ObjectIdentifier(accent) }
            if ledger.held.isEmpty { ledgers.removeObject(forKey: window) }
            window.tintColor = ledger.held.last?.tint ?? ledger.own
        }
    }
}
#endif
