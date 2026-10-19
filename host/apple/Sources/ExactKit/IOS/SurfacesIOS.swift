#if os(iOS) || os(tvOS)
import UIKit

struct SessionSurface {
    enum Kind: Equatable { case document, route, dialog, popover }
    let view: UIView
    let label: String
    let kind: Kind
    var owned: [UIView] = []
}

extension Presenter {
    /// The session's view, viewport and root (nested trees occur only once),
    /// route mounts, dialog sheets and popover layers outside those trees. Live by default;
    /// inspection, editing and motion also need presentations still retiring.
    /// Roots are session-owned, never a window or another session's container.
    func surfaces(includingRetiring: Bool = false) -> [SessionSurface] {
        let candidates: [(UIView?, String)] = [(session?.view, "ExactView"), (viewport, "viewport"), (root, "document")]
        let own: [(UIView, String)] = candidates.compactMap { view, label in view.map { ($0, label) } }
        let documents = own.filter { candidate in
            !own.contains { other in candidate.0 !== other.0 && candidate.0.isDescendant(of: other.0) }
        }.map { SessionSurface(view: $0.0, label: $0.1, kind: .document) }
        return documents + modals.surfaces(includingRetiring: includingRetiring)
            + menus.sheets.surfaces(includingRetiring: includingRetiring)
            + menus.sheetLayers.map { SessionSurface(view: $0, label: "popover over a sheet", kind: .popover) }
    }

    /// The views of the surfaces beside the document.
    func presentations(includingRetiring: Bool = false) -> [UIView] {
        surfaces(includingRetiring: includingRetiring).filter { $0.kind != .document }.map(\.view)
    }

    /// The innermost surface containing a view; the document's scrolling
    /// port is still `viewport`, even when its root is the session's view.
    func surface(containing view: UIView, includingRetiring: Bool = true) -> SessionSurface? {
        surfaces(includingRetiring: includingRetiring).last { view === $0.view || view.isDescendant(of: $0.view) }
    }

    /// UIKit's accessibility hiding rule applies across presentation-container
    /// siblings. Include those wrappers for AX only, without entering a window
    /// or enclosing the session's own view. The AX walker deduplicates objects.
    var accessibilitySurfaces: [UIView] {
        let all = surfaces(includingRetiring: true)
        return all.map { surface in
            guard surface.kind == .route else { return surface.view }
            var view = surface.view
            while let parent = view.superview, !(parent is UIWindow),
                  !all.contains(where: { $0.kind == .document && $0.view.isDescendant(of: parent) }) { view = parent }
            return view
        }
    }
}

/// A lifted popover belongs to its sheet even though UIKit places its layer
/// beside the sheet's controller view, outside that controller's responders.
enum ViewPresentation {
    static func controller(for view: UIView, fallbackToWindow: Bool = false) -> UIViewController? {
        if let sheet = PopoverLayer.containing(view)?.sheet { return sheet }
        var responder: UIResponder? = view
        while let current = responder {
            if let controller = current as? UIViewController { return controller }
            responder = current.next
        }
        return fallbackToWindow ? view.window?.rootViewController : nil
    }

    /// The controllers on the responder chain from the owner, nearest first:
    /// its containment ancestors, then those presenting it.
    static func controllers(for view: UIView) -> [UIViewController] {
        guard let controller = controller(for: view) else { return [] }
        return sequence(first: controller as UIResponder, next: { $0.next }).compactMap { $0 as? UIViewController }
    }

    /// The owner, followed through presentations that are not being dismissed;
    /// nil out of a window, where UIKit refuses to present and calls back nothing.
    static func presenter(for view: UIView) -> UIViewController? {
        guard view.window != nil else { return nil }
        var owner = controller(for: view, fallbackToWindow: true)
        while let presented = owner?.presentedViewController, !presented.isBeingDismissed { owner = presented }
        return owner?.viewIfLoaded?.window == nil ? nil : owner
    }

    /// A flight stays in its presentation's paint plane. Above a sheet that
    /// is the popover layer, not the clipped card or the window around it.
    static func root(of view: UIView) -> UIView? {
        if let layer = PopoverLayer.containing(view), layer.sheet != nil { return layer }
        return controllers(for: view).first {
            $0.parent == nil || $0.parent is UINavigationController || $0.parent is UITabBarController
        }?.viewIfLoaded ?? view.window
    }
}
#endif
