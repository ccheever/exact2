// @ref LLP 1095 D4 — the roles the app's theme replaces, for the surfaces
// the platform draws itself: a grouped list's ground, a bar's title, a
// sheet, a field. A row naming a role already crosses as what replaces it;
// these surfaces read no row, so they ask here, and keep the platform's
// colour where the theme leaves the role alone (and where the platform
// offers no supported way to colour a surface, it stays the platform's).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

package enum ThemeRoles {
    nonisolated(unsafe) private static var table: NodeStyle = [:]
    /// Moves when the table does, so a surface that cached a colour asks again.
    nonisolated(unsafe) package static var generation = 0

    /// The boot's table (`{"op": "roles"}`), the last plan's replaced;
    /// whether it changed.
    @discardableResult
    static func apply(_ batch: Batch) -> Bool {
        guard let op = batch.ops.last(where: { $0.op == .roles }), op.style != table else { return false }
        table = op.style
        generation += 1
        return true
    }

    /// The app's colour for `role` (the schema's name: `grouped-background`,
    /// `CanvasText`), dynamic over the appearance; `nil` where the theme
    /// leaves the role the platform's.
    package static func color(_ role: String) -> PlatformColor? {
        guard let value = table[role] else { return nil }
        let side = { (dark: Bool) in TextEngine.color(value.textChannels(dark: dark) ?? [0, 0, 0, 0]) }
        #if os(macOS)
        return NSColor(name: nil) { appearance in side(appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua) }
        #else
        return UIColor { traits in side(traits.userInterfaceStyle == .dark) }
        #endif
    }

    /// `role`'s app colour, else the platform's own.
    package static func color(_ role: String, or platform: PlatformColor) -> PlatformColor {
        color(role) ?? platform
    }
}

extension Presenter {
    /// The theme's `AccentColor` as the app's tint here, as a hand-built app
    /// sets its root's (LLP 1095 D4): rows that name the tint resolve by it,
    /// and an authored `accent-color` below still sets its subtree's.
    func applyThemeTint() {
        #if os(iOS) || os(tvOS)
        root.tintColor = ThemeRoles.color("AccentColor")
        #endif
    }
}
