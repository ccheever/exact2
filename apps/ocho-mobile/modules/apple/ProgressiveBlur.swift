// `<progressive-blur>` on iOS: a blur that is strongest at one edge and fades
// to nothing at the other, under a scrolling header, the way the system's own
// bars blend into content. Props: `strength` (the strongest blur, points),
// `progress` (0…1, scaling the strength, so a header can fade it in as the list
// scrolls without fading the view's alpha, which breaks a visual effect),
// `start` (where the fade begins, as a fraction of the height), and
// `direction` (`top`, blurred at the top, or `bottom`). No events.
//
// Core Animation's variable blur filter, as nativemail uses it (adapted from
// nikstar/VariableBlur, MIT): a private filter, set on the backdrop layer of a
// UIVisualEffectView, masked by a gradient image.
import Foundation

#if os(iOS)
import CoreImage.CIFilterBuiltins
import QuartzCore
import UIKit

final class ProgressiveBlur: ExactNativeInstance {
    private let host = PassThroughView()
    private var blur: VariableBlurView?
    private var radius: CGFloat = 8
    private var progress: CGFloat = 1
    private var start: CGFloat = 0
    private var top = true

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        host.isUserInteractionEnabled = false
        host.clipsToBounds = true
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { host }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        let radius = CGFloat(Double(props["strength"] ?? "") ?? 8)
        let progress = CGFloat(min(1, max(0, Double(props["progress"] ?? "") ?? 1)))
        let start = CGFloat(Double(props["start"] ?? "") ?? 0)
        let top = props["edge"] != "bottom"
        // The mask changes only with its shape; the strength, every scroll.
        if blur == nil || start != self.start || top != self.top {
            self.start = start
            self.top = top
            blur?.removeFromSuperview()
            let next = VariableBlurView(start: start, blurredTop: top)
            next.frame = host.bounds
            next.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            next.isUserInteractionEnabled = false
            host.addSubview(next)
            blur = next
        }
        self.radius = radius
        self.progress = progress
        blur?.setRadius(radius * progress)
    }
}

/// Never a touch target: what is under the blur gets the touch.
private final class PassThroughView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

private final class VariableBlurView: UIVisualEffectView {
    private var filter: NSObject?

    init(start: CGFloat, blurredTop: Bool) {
        super.init(effect: UIBlurEffect(style: .regular))
        // CAFilter and filterWithType:, spelled backwards as nativemail does.
        guard let filterClass = NSClassFromString(String("retliFAC".reversed())) as? NSObject.Type,
              let variable = filterClass.perform(NSSelectorFromString(String(":epyThtiWretlif".reversed())), with: "variableBlur")?
                .takeUnretainedValue() as? NSObject else { return }
        variable.setValue(VariableBlurView.mask(start: start, blurredTop: blurredTop), forKey: "inputMaskImage")
        variable.setValue(true, forKey: "inputNormalizeEdges")
        filter = variable
        // The stock tint and highlight views would draw a hard-edged slab.
        for view in subviews.dropFirst() { view.alpha = 0 }
    }

    required init?(coder: NSCoder) { nil }

    func setRadius(_ radius: CGFloat) {
        guard let filter, let backdrop = subviews.first?.layer else { return }
        filter.setValue(radius, forKey: "inputRadius")
        backdrop.filters = radius > 0 ? [filter] : []
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        guard let window, let backdrop = subviews.first?.layer else { return }
        backdrop.setValue(window.traitCollection.displayScale, forKey: "scale")
    }

    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        // Calling super brings the stock subviews back, and their hard edge.
    }

    private static func mask(start: CGFloat, blurredTop: Bool) -> CGImage? {
        let height: CGFloat = 100
        let gradient = CIFilter.linearGradient()
        gradient.color0 = CIColor.black
        gradient.color1 = CIColor.clear
        // Core Image's y grows upward: black (full blur) at the blurred edge.
        gradient.point0 = CGPoint(x: 0, y: blurredTop ? height : 0)
        gradient.point1 = CGPoint(x: 0, y: blurredTop ? start * height : height - start * height)
        guard let image = gradient.outputImage else { return nil }
        return CIContext().createCGImage(image, from: CGRect(x: 0, y: 0, width: 100, height: height))
    }
}
#endif
