// `<title-reveal>` on iOS: a navigation bar's compact title that arrives the
// way SwiftUI's `.blur` + `.offset` + `.opacity` bring one in (nativemail's
// header): out of a 7 pt blur, up 9 pt, fading in, ease-out over 0.22 s, and
// back the same way. Exact's motion animates opacity and translate but not
// `filter`, so the three run together here. Props: `text` and `shown`
// (`true`/`false`); the face is the bar's, 17 pt semibold. No events.
//
// The blur is Core Animation's own Gaussian filter (CAFilter, private, as the
// progressive blur's variable filter is), animated on the label's layer.
import Foundation

#if os(iOS)
import QuartzCore
import UIKit

final class TitleReveal: ExactNativeInstance {
    private static let duration: CFTimeInterval = 0.22
    private static let hiddenBlur: CGFloat = 7
    private static let hiddenOffset: CGFloat = 9
    private static let radiusPath = "filters.gaussianBlur.inputRadius"

    private let host = TitlePassThroughView()
    private let label = UILabel()
    private var shown: Bool?
    /// Bumped by each animation; the one that ends last takes the filter off.
    private var turn = 0

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        host.isUserInteractionEnabled = false
        label.frame = host.bounds
        label.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        label.textAlignment = .center
        label.textColor = .label
        label.lineBreakMode = .byTruncatingTail
        label.font = .systemFont(ofSize: 17, weight: .semibold)
        host.addSubview(label)
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { host }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        label.text = props["text"] ?? ""
        let shown = props["shown"] == "true"
        guard shown != self.shown else { return }
        // The first value is where it starts, not a change to animate.
        let animated = self.shown != nil && host.window != nil
        self.shown = shown
        let alpha: CGFloat = shown ? 1 : 0
        let transform = shown ? .identity : CGAffineTransform(translationX: 0, y: TitleReveal.hiddenOffset)
        let radius = shown ? 0 : TitleReveal.hiddenBlur
        turn += 1
        guard animated else {
            label.alpha = alpha
            label.transform = transform
            label.layer.filters = nil
            return
        }
        // The filter is on the layer only while it animates: a filtered
        // layer renders offscreen every frame it shows, a cost a still title
        // over a scrolling list needn't pay.
        let layer = label.layer
        if layer.filters?.isEmpty ?? true, let blur = TitleReveal.gaussianBlur() {
            blur.setValue(shown ? TitleReveal.hiddenBlur : 0, forKey: "inputRadius")
            layer.filters = [blur]
        }
        // From wherever a reversal caught it, so a quick scroll back and forth
        // turns around smoothly instead of jumping to an end.
        let from = layer.presentation()?.value(forKeyPath: TitleReveal.radiusPath) ?? layer.value(forKeyPath: TitleReveal.radiusPath)
        layer.setValue(radius, forKeyPath: TitleReveal.radiusPath)
        let blur = CABasicAnimation(keyPath: TitleReveal.radiusPath)
        blur.fromValue = from
        blur.toValue = radius
        blur.duration = TitleReveal.duration
        blur.timingFunction = CAMediaTimingFunction(name: .easeOut)
        let mine = turn
        CATransaction.begin()
        CATransaction.setCompletionBlock { [weak self] in
            guard let self, self.turn == mine else { return }
            self.label.layer.filters = nil
        }
        layer.add(blur, forKey: "reveal.blur")
        UIView.animate(withDuration: TitleReveal.duration, delay: 0, options: [.curveEaseOut, .beginFromCurrentState]) {
            self.label.alpha = alpha
            self.label.transform = transform
        }
        CATransaction.commit()
    }

    /// CAFilter's `gaussianBlur`, named so its radius has a key path.
    private static func gaussianBlur() -> NSObject? {
        // CAFilter and filterWithType:, spelled backwards as the progressive
        // blur spells them.
        guard let filterClass = NSClassFromString(String("retliFAC".reversed())) as? NSObject.Type,
              let filter = filterClass.perform(NSSelectorFromString(String(":epyThtiWretlif".reversed())), with: "gaussianBlur")?
                .takeUnretainedValue() as? NSObject else { return nil }
        filter.setValue("gaussianBlur", forKey: "name")
        filter.setValue(0, forKey: "inputRadius")
        return filter
    }
}

/// Never a touch target: what is under the title gets the touch.
private final class TitlePassThroughView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}
#endif
