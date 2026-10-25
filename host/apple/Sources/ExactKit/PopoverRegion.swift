// A painted popover sized in its position-area option's region (LLP 1021
// D2), as Chrome sizes the box: the iOS lift's and the Mac's painted
// popovers, each placed by `PositionArea.place` with the kernel measuring
// every option (`Runtime.measureBlock`) and told the region it chose.
import CoreGraphics
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    /// Where painted popover `pop` goes, against `anchor` in `bounds`: each
    /// option measured in its region by the kernel, what the kernel laid out
    /// (`laid`) standing in where it can't measure; the chosen region is told
    /// as its block (`told` keeps what was).
    func placePainted(_ pop: NodeView, laid: CGSize, anchor: CGRect, scroll: CGPoint, pageScroll: CGPoint, overflow: PositionArea.Overflow,
                      history: inout PositionArea.History, told: inout HostBlock?, in bounds: CGRect) -> PositionArea.Placement {
        let rtl = documentDirection == "rtl", id = pop.id
        let placed = PositionArea.painted(pop, rtl: rtl), margins = PositionArea.margins(of: pop)
        // An authored `none` is no area: the viewport its top layer implies,
        // and D2's default placement held inside it, as before regions.
        if placed.area == "none" {
            tellRegion(id, nil, told: &told)
            let origin = PositionArea.origin("none", anchor: anchor, size: laid, margins: margins, fallbacks: placed.fallbacks,
                                             overflow: overflow, rtl: rtl, history: &history, in: bounds)
            return PositionArea.Placement(origin: origin, size: laid, region: bounds)
        }
        let at = PositionArea.place(placed.area, anchor: anchor, margins: margins, fallbacks: placed.fallbacks,
                                    overflow: overflow, rtl: rtl, scroll: scroll, pageScroll: pageScroll, history: &history, in: bounds) { region in
            self.session?.runtime.measureBlock(id, in: .region(region)) ?? PositionArea.Measured(size: laid, margins: margins)
        }
        tellRegion(id, .region(at.region.size), told: &told)
        return at
    }

    /// How far the scrollers between `anchor` and `layer` are scrolled,
    /// summed, as a scroll down is positive: what CSS remembers of an
    /// anchor's scroll (`PositionArea.place`). A scroller that holds the
    /// layer further down moves both, so it is not counted; one the layer is
    /// mounted in directly keeps the layer over its bounds as it scrolls
    /// (iOS's `PopoverLayer.mount`), so the anchor moves past it and it is.
    func anchorScroll(_ anchor: PlatformView, outside layer: PlatformView) -> CGPoint {
        var sum = CGPoint.zero, at = anchor.superview
        while let view = at {
            #if os(macOS)
            if let clip = view as? NSClipView, !layer.isDescendant(of: clip) || layer.superview === clip {
                sum.x += clip.bounds.origin.x
                sum.y += clip.isFlipped ? clip.bounds.origin.y : -clip.bounds.origin.y
            }
            #else
            if let scroller = view as? UIScrollView, !layer.isDescendant(of: scroller) || layer.superview === scroller {
                sum.x += scroller.contentOffset.x
                sum.y += scroller.contentOffset.y
            }
            #endif
            at = view.superview
        }
        return sum
    }

    /// The kernel told the region a painted popover is sized in (nil: back to
    /// the viewport its top layer implies): after the batch being applied, or
    /// on the next turn when none is, since a placement runs inside the menus'
    /// layout, which the batch's own layout would enter again. The latest
    /// for a popover is what is told, once, however the waits interleave (a
    /// close's next turn and a reopening's batch end), and only to the view
    /// it was for: not to another the id names by then.
    func tellRegion(_ id: UInt32, _ block: HostBlock?, told: inout HostBlock?) {
        guard told != block else { return }
        told = block
        let view = views[id].map(ObjectIdentifier.init)
        let waiting = regionsUntold.updateValue((view, block), forKey: id) != nil
        if waiting { return }
        let tell = { [weak self] in
            guard let self, let pending = self.regionsUntold.removeValue(forKey: id) else { return }
            guard let session = self.session, let now = self.views[id], ObjectIdentifier(now) == pending.view else { return }
            session.applyUnlessEmpty(session.runtime.blocks([(id, pending.block)]))
        }
        #if os(macOS)
        let inBatch = isApplying
        #else
        let inBatch = applying
        #endif
        if inBatch { afterBatch(tell) } else { DispatchQueue.main.async(execute: tell) }
    }
}
