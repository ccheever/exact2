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
    /// A presenter's table (LLP 1121 stage 5: a session's own, not the
    /// process's), shared by the views it hangs off.
    private final class Table { var style: NodeStyle = [:] }
    private struct Owner { weak var view: PlatformView?; let table: Table }
    /// Each presenter's table by its root (and a sheet's view it mounts
    /// into); the one whose batch applies now; the last one to apply.
    nonisolated(unsafe) private static var owners: [ObjectIdentifier: Owner] = [:]
    nonisolated(unsafe) private static var applying: Table?
    nonisolated(unsafe) private static var last = Table()
    /// Moves when a table does, so a surface that cached a colour asks again.
    nonisolated(unsafe) package static var generation = 0

    private static func table(of root: PlatformView) -> Table {
        if let owner = owners[ObjectIdentifier(root)], owner.view === root { return owner.table }
        let table = Table()
        own(root, table)
        return table
    }

    /// `view` resolves by `table`; the owners whose view has gone, dropped.
    private static func own(_ view: PlatformView, _ table: Table) {
        owners = owners.filter { $0.value.view != nil }
        owners[ObjectIdentifier(view)] = Owner(view: view, table: table)
    }

    /// The boot's table (`{"op": "roles"}`), the last plan's replaced, for
    /// the presenter at `root` (none: the last to apply's); whether it changed.
    @discardableResult
    static func apply(_ batch: Batch, root: PlatformView? = nil) -> Bool {
        let table = root.map(table(of:)) ?? last
        last = table
        guard let op = batch.ops.last(where: { $0.op == .roles }), op.style != table.style else { return false }
        table.style = op.style
        generation += 1
        return true
    }

    /// `body` with `root`'s presenter the one a view outside its root (one
    /// not yet mounted) resolves by.
    static func within<T>(_ root: PlatformView, _ body: () -> T) -> T {
        let previous = applying
        applying = table(of: root)
        defer { applying = previous }
        return body()
    }

    /// `view`, a surface a presenter presents outside its root (a sheet's),
    /// resolves by the table of the presenter at `root` (none: the one
    /// applying now).
    static func adopt(_ view: PlatformView, from root: PlatformView?) {
        guard let table = root.map(table(of:)) ?? applying else { return }
        own(view, table)
    }

    /// The surfaces the presenter at `root` adopted (its sheets), alive.
    static func adopted(by root: PlatformView) -> [PlatformView] {
        guard let owner = owners[ObjectIdentifier(root)], owner.view === root else { return [] }
        return owners.values.compactMap { $0.table === owner.table && $0.view !== root ? $0.view : nil }
    }

    /// The table `view` resolves by: its presenter's, the nearest owner
    /// above it; else the applying presenter's; else the last to apply.
    private static func style(in view: PlatformView?) -> NodeStyle {
        var at = view
        while let v = at {
            if let owner = owners[ObjectIdentifier(v)], owner.view === v { return owner.table.style }
            at = v.superview
        }
        return (applying ?? last).style
    }

    /// The app's colour for `role` (the schema's name: `grouped-background`,
    /// `CanvasText`), dynamic over the appearance; `nil` where the theme
    /// leaves the role the platform's.
    package static func color(_ role: String, in view: PlatformView? = nil) -> PlatformColor? {
        guard let value = style(in: view)[role] else { return nil }
        let side = { (dark: Bool, contrast: Bool?, elevated: Bool) in
            TextEngine.color(value.textChannels(dark: dark, contrast: contrast, elevated: elevated) ?? [0, 0, 0, 0])
        }
        #if os(macOS)
        return NSColor(name: nil) { appearance in
            // Increase Contrast is the appearance's on the Mac.
            let match = appearance.bestMatch(from: [.aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua])
            let dark = match == .darkAqua || match == .accessibilityHighContrastDarkAqua
            return side(dark, match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua, false)
        }
        #elseif os(tvOS)
        return UIColor { traits in side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, false) }
        #else
        // The view's level and contrast too: a role it names resolves as a
        // sheet's (elevated) and under Increase Contrast.
        return UIColor { traits in side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, traits.userInterfaceLevel == .elevated) }
        #endif
    }

    /// `role`'s app colour, else the platform's own.
    package static func color(_ role: String, in view: PlatformView? = nil, or platform: PlatformColor) -> PlatformColor {
        color(role, in: view) ?? platform
    }
}

extension Presenter {
    /// The theme's `AccentColor` as the app's tint here, as a hand-built app
    /// sets its root's (LLP 1095 D4): rows that name the tint resolve by it,
    /// and an authored `accent-color` below still sets its subtree's.
    func applyThemeTint() {
        #if os(iOS) || os(tvOS)
        root.tintColor = ThemeRoles.color("AccentColor", in: root)
        // Its sheets sit outside the root, and its bars' titles were set
        // once: a live edit sets them again (LLP 1121 stage 5).
        for sheet in ThemeRoles.adopted(by: root) {
            sheet.tintColor = root.tintColor
            #if os(iOS)
            sheet.backgroundColor = ThemeRoles.color("background", in: sheet) ?? ThemeRoles.color("Canvas", in: sheet, or: .systemBackground)
            #endif
        }
        navigation.retheme()
        #else
        // The page's ground where the app sets none (LLP 1115 D2).
        viewport.backgroundColor = ThemeRoles.color("background", in: root) ?? ThemeRoles.color("Canvas", in: root, or: .windowBackgroundColor)
        // AppKit has no tint: what names it, here and in what the presenter
        // shows outside its root, resolves the theme's `AccentColor` again
        // (a live edit, LLP 1121 stage 5).
        for view in [root] + ThemeRoles.adopted(by: root) {
            for case let node as NodeView in view.subviews {
                node.retint(changed: true)
                node.accentChanged()
            }
        }
        #endif
    }
}
