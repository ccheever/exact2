#if os(iOS) || os(tvOS)
import UIKit

/// The layout regions UIKit lays out beside the safe area (LLP 1123 D10),
/// what `env(-exact-<region>-inset-<edge>)` resolves to: each region of the
/// view controller's view that holds the session, as an inset per edge from
/// the layout viewport. UIKit is the source: its margins are a controller's
/// root view's (the system minimum), its corner adaptation is iOS 26's
/// `UIView.LayoutRegion`, and a region below iOS 26 is the one without it.
enum LayoutRegionsIOS {
    /// Twenty insets, five regions of top, right, bottom, left, in
    /// `Region`'s order (`kernel/src/style/env.rs`): the regions of the root
    /// view of `view`'s controller, from `frame`, the layout viewport in
    /// `view`'s coordinates, as UIKit lays them out (inside the safe area by
    /// default, outside it where `insetsLayoutMarginsFromSafeArea` is off),
    /// none less than zero.
    static func insets(of view: UIView, viewport frame: CGRect) -> [CGFloat] {
        let root = controllerView(of: view)
        func local(_ r: CGRect) -> CGRect { root.convert(r, to: view) }
        // Each region and its corner-adapted pair (left and right from the
        // horizontal adaptation, top and bottom from the vertical one), all
        // from iOS 26's regions, whose readable content can differ from
        // the older guide's; below iOS 26, the guides, without adaptation
        let regions: [(CGRect, (CGRect, CGRect))]
        if #available(iOS 26.0, tvOS 26.0, *) {
            func rect(_ region: UIView.LayoutRegion) -> CGRect { local(root.bounds.inset(by: root.edgeInsets(for: region))) }
            func pair(_ region: (UIView.LayoutRegion.AdaptivityAxis?) -> UIView.LayoutRegion) -> (CGRect, (CGRect, CGRect)) {
                (rect(region(nil)), (rect(region(.horizontal)), rect(region(.vertical))))
            }
            regions = [pair { .safeArea(cornerAdaptation: $0) }, pair { .margins(cornerAdaptation: $0) },
                       pair { .readableContent(cornerAdaptation: $0) }]
        } else {
            regions = [root.safeAreaLayoutGuide, root.layoutMarginsGuide, root.readableContentGuide].map {
                let r = local($0.layoutFrame)
                return (r, (r, r))
            }
        }
        func corners(_ pair: (CGRect, CGRect)) -> [CGFloat] {
            let h = edges(of: pair.0, from: frame), v = edges(of: pair.1, from: frame)
            return [v[0], h[1], v[2], h[3]]
        }
        // Region order: safe area with corners, margins, margins with
        // corners, readable content, readable content with corners
        return corners(regions[0].1)
            + edges(of: regions[1].0, from: frame) + corners(regions[1].1)
            + edges(of: regions[2].0, from: frame) + corners(regions[2].1)
    }

    /// How far `rect` lies inside `frame` at each edge, top, right, bottom,
    /// left, and none less than zero.
    private static func edges(of rect: CGRect, from frame: CGRect) -> [CGFloat] {
        [max(0, rect.minY - frame.minY), max(0, frame.maxX - rect.maxX),
         max(0, frame.maxY - rect.maxY), max(0, rect.minX - frame.minX)]
    }

    /// The root view of the nearest view controller that holds `view`: the
    /// view UIKit gives a screen's margins, which a plain subview does not
    /// have (8 points, not the system minimum). `view` when it has none.
    private static func controllerView(of view: UIView) -> UIView {
        var responder: UIResponder? = view
        while let r = responder {
            if let controller = r as? UIViewController, let root = controller.viewIfLoaded { return root }
            responder = r.next
        }
        return view
    }
}
#endif
