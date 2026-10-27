// @ref LLP 1123 — `-exact-background-extension="auto"` on an image: where
// the image meets a safe area (a split's sidebar or inspector, the iPhone
// Duo's side strip), UIKit's background extension view (iOS 26) fills that
// area with the image's mirrored, blurred edges, as SwiftUI's
// `backgroundExtensionEffect()` does. The image stays where it is laid out;
// the extension is drawn around it, in the nearest scroll view or route.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// Each image's extension view, out of `NodeView`'s stored properties,
    /// which every view pays for.
    private static let extensions = NSMapTable<NodeView, UIView>.weakToStrongObjects()

    /// The image's extension view placed, filled and sized for where the
    /// image is now; removed where it has nothing to fill.
    func syncBackgroundExtension() {
        let held = Self.extensions.object(forKey: self)
        guard #available(iOS 26.0, tvOS 26.0, *), kind == "image", props["backgroundExtension"] == "auto", !cssVisibilityHidden,
              window != nil, let image = raster?.image, let place = extensionPlace() else { return removeBackgroundExtension() }
        let view = held as? UIBackgroundExtensionView ?? {
            let view = UIBackgroundExtensionView()
            view.automaticallyPlacesContentView = false
            view.isUserInteractionEnabled = false
            view.accessibilityElementsHidden = true
            let content = UIView()
            content.clipsToBounds = true
            // The image's content box, which clips its picture, and the picture.
            let box = CALayer()
            box.masksToBounds = true
            box.addSublayer(CALayer())
            content.layer.addSublayer(box)
            content.layer.mask = CAShapeLayer()
            view.contentView = content
            let outside = CAShapeLayer()
            outside.fillRule = .evenOdd
            view.layer.mask = outside
            Self.extensions.setObject(view, forKey: self)
            return view
        }()
        if view.superview !== place.host || place.host.subviews.firstIndex(of: view) != place.host.subviews.firstIndex(of: place.below).map({ $0 - 1 }) {
            place.host.insertSubview(view, belowSubview: place.below)
        }
        view.frame = place.frame
        // Outside the image's subtree, the extension takes the opacity and the
        // hiding of the image and of each ancestor between it and the host.
        let between = sequence(first: self as UIView, next: \.superview).prefix { $0 !== place.host }
        view.alpha = between.reduce(1) { $0 * $1.alpha }
        view.isHidden = between.contains { $0.isHidden }
        guard let content = view.contentView, let box = content.layer.sublayers?.first, let picture = box.sublayers?.first,
              let corners = content.layer.mask as? CAShapeLayer, let outside = view.layer.mask as? CAShapeLayer else { return }
        content.frame = place.image.offsetBy(dx: -place.frame.minX, dy: -place.frame.minY)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        // The extension mirrors the image with its corners and draws only
        // outside the image's box, which the image draws: nothing of the copy
        // shows behind the image's corners or through the image as it fades.
        corners.path = roundedPath(in: content.bounds).cgPath
        let path = UIBezierPath(rect: view.bounds)
        path.append(UIBezierPath(rect: content.frame))
        outside.path = path.cgPath
        // The pixels as the image shows them (`applyImageLayer`, `draw(_:)`):
        // an animated image's frame now, fitted by its `object-fit` in its
        // content box, which clips them, and with a `-exact-tint-color` the
        // tint through their alpha (`RasterGeometry.draw`).
        let inner = contentBox(), fit = style["object_fit"]?.string ?? "fill"
        box.frame = inner
        picture.frame = RasterGeometry.rect(natural: image.naturalSize, content: inner, fit: fit).offsetBy(dx: -inner.minX, dy: -inner.minY)
        let pixels = AnimatedRasters.shared.frame(for: self) ?? image.image
        if let tint = channels("tint_color").map({ TextEngine.color($0).cgColor }) {
            let alpha = picture.mask ?? CALayer()
            alpha.frame = picture.bounds
            if (alpha.contents as AnyObject?) !== pixels { alpha.contents = pixels }
            picture.mask = alpha
            picture.contents = nil
            picture.backgroundColor = tint
        } else {
            picture.mask = nil
            picture.backgroundColor = nil
            if (picture.contents as AnyObject?) !== pixels { picture.contents = pixels }
        }
        CATransaction.commit()
    }

    /// The extension is a view beside the image, outside its subtree: it
    /// goes when the image leaves the window, is destroyed or is parked.
    func removeBackgroundExtension() {
        Self.extensions.object(forKey: self)?.removeFromSuperview()
        Self.extensions.removeObject(forKey: self)
    }

    /// Where the extension goes: the nearest scroll view, else the route's
    /// view, behind the image's ancestor there; its frame the image's, grown
    /// to the route's edges on each side where the image meets the route's
    /// safe area. Nil where it meets none.
    private func extensionPlace() -> (host: UIView, below: UIView, frame: CGRect, image: CGRect)? {
        let ancestors = Array(sequence(first: self as UIView, next: \.superview))
        guard let route = ancestors.first(where: { $0.next is RouteController }),
              let host = ancestors.first(where: { $0 is UIScrollView && $0.isDescendant(of: route) }) ?? Optional(route),
              let below = ancestors.first(where: { $0.superview === host }) else { return nil }
        let image = convert(bounds, to: host)
        let edges = route.convert(route.bounds, to: host)
        let safe = route.convert(route.bounds.inset(by: route.safeAreaInsets), to: host)
        var frame = image
        if abs(image.minX - safe.minX) < 1, safe.minX > edges.minX { frame.size.width += frame.minX - edges.minX; frame.origin.x = edges.minX }
        if abs(image.maxX - safe.maxX) < 1, safe.maxX < edges.maxX { frame.size.width = edges.maxX - frame.minX }
        if abs(image.minY - safe.minY) < 1, safe.minY > edges.minY { frame.size.height += frame.minY - edges.minY; frame.origin.y = edges.minY }
        if abs(image.maxY - safe.maxY) < 1, safe.maxY < edges.maxY { frame.size.height = edges.maxY - frame.minY }
        return frame == image ? nil : (host, below, frame, image)
    }
}

extension Presenter {
    /// `view`'s opacity or display changed: the extensions of the images in
    /// it follow, as they are outside the images' subtrees.
    func compositingChanged(_ view: UIView) {
        for image in carrying("backgroundExtension") where image.isDescendant(of: view) { image.syncBackgroundExtension() }
    }
}
#endif
