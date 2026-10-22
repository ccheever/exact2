// @ref LLP 1035.001, the zoom transition on a push: a route pushed with a
// `navigationSource` arrives by UIKit's zoom from the source element in the
// route beneath, as a full-screen presentation does (ModalIOS `present`),
// and lands on its own element with the source's id (an image, as drawn).
// iOS 18's `preferredTransition` applies to a push as to a presentation
// (UIKit, "UIViewController.Transition.zoom"); the swipe back is the
// zoom's own, which `transitionEnded` dispatches as any interactive pop.
#if os(iOS)
import UIKit

extension NavigationHost {
    /// Give `top`, the route about to be pushed over `beneath`, its zoom
    /// transition when it names a `navigationSource`; nothing otherwise, and
    /// nothing before iOS 18.
    func prepareZoomPush(_ top: RouteController, beneath: RouteController?) {
        guard #available(iOS 18.0, *), let beneath, top.node.props["navigationSource"] != nil,
              top.preferredTransition == nil else { return }
        let route = top.node, preceding = beneath.node
        let options = UIViewController.Transition.ZoomOptions()
        let landing = ZoomLanding()
        // The zoom's own swipe back pops the route (`transitionEnded`
        // dispatches it); it begins only where an edge swipe may.
        options.interactiveDismissShouldBegin = { [weak self] context in
            guard let self, context.willBegin else { return false }
            return landing.dismissal(begins: back != nil && !changing && !presenter.modals.inTransition)
        }
        top.appeared = landing.appeared
        let resolve: (UIView, NodeView?) -> CGRect? = { [weak self, weak route] zoomed, sourceView in
            guard let self, let route, let name = route.props["navigationSource"],
                  let target = presenter.views.values.filter({
                      $0.props["id"] == name && $0.window != nil && $0.isDescendant(of: route)
                  }).min(by: { $0.id < $1.id }) else { return nil }
            let source = sourceView.flatMap(ModalHost.zoomImage)
            return ModalHost.zoomAlignment(target: target, fallbackNatural: source?.raster?.image.naturalSize, in: zoomed)
        }
        options.alignmentRectProvider = { context in
            landing.answer(context.zoomedViewController.viewIfLoaded.flatMap { resolve($0, context.sourceView as? NodeView) })
        }
        top.preferredTransition = .zoom(options: options, sourceViewProvider: { [weak self, weak route, weak preceding] _ in
            guard let self, let route, let preceding, presenter.views[preceding.id] === preceding,
                  let name = route.props["navigationSource"] else { return nil }
            return presenter.views.values.filter {
                $0.props["id"] == name && $0.window != nil && ($0 === preceding || $0.isDescendant(of: preceding))
            }.min(by: { $0.id < $1.id })
        })
    }
}
#endif
