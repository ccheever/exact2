// A paragraph's words laid out again while UIKit resizes their box cross-fade
// from their old lines to the new ones over the resize. The fade is Core
// Animation's: a `CATransition` on the box's lines blends the two states, so
// pixels the same in both stay as they are, and the render server runs it.
// Where the fade is comes from the lines' clock, which `TextBoxLayer` sets
// from UIKit's resize of the box: from where the fade is to where the resize
// ends, at the resize's pace, or held where a gesture holds the resize. It is
// set again only when UIKit adds or removes a resize. A paused, scrubbed,
// continued or reversed `UIViewPropertyAnimator` was seen to replace the
// animations it made with copies of the same values, re-timed: paused at the
// finger's fraction (speed 0, the offset that fraction of the duration), or
// running again (speed 1, or -1 counting back from its end), and to remove
// them when it finishes. So the main thread works when UIKit does, and no
// frame needs it.
#if os(iOS) || os(tvOS)
import UIKit

/// A paragraph's text box (`NodeView.makeTextBox`): it moves and resizes in
/// UIKit's animation as a `BoxPartLayer` does, and cross-fades its lines.
final class TextBoxLayer: BoxPartLayer {
    /// The ink and its HDR shadow's cast, at the box's origin, never moving
    /// in it (`NodeView.makeTextBox`).
    var lines: CALayer?
    /// The fade's delegate, which ends it when Core Animation finishes it:
    /// set while the lines' clock is the fade's, from new lines shown in a
    /// resize until the fade ends.
    private var finish: Finish?
    private var fading: Bool { finish != nil }
    /// While UIKit holds a resize paused (a gesture's), until it is removed.
    private var held: Hold?

    /// The fade's own time: it runs from 1 to 2 in the lines' time, which
    /// `set` maps to the resize. Not from 0, which Core Animation takes as
    /// the commit's time.
    private static let start: CFTimeInterval = 1

    /// New pixels in the lines. The same words (`shown`'s and `spec`'s text,
    /// whatever their lines, fit or style) while UIKit resizes the box fade
    /// from the old lines, or carry a fade under way on to the new ones;
    /// other words, or no resize, show at once, ending any fade.
    func show(from shown: Spec?, to spec: Spec) {
        guard let lines else { return }
        guard animationKeys()?.contains(where: { $0.hasPrefix("bounds") }) == true,
              let shown, shown.runs.lazy.flatMap(\.text.utf16).elementsEqual(spec.runs.lazy.flatMap(\.text.utf16)) else { return end() }
        // A fade under way blends its old lines with these
        if fading, lines.animation(forKey: kCATransition) != nil { return }
        end()
        // A numeral roll still running in the lines finishes at once: their clock is the fade's now
        lines.sublayers?.forEach { $0.removeAllAnimations() }
        // The old lines until the fade is aimed
        set(lines, progress: 0, rate: 0)
        let fade = CATransition()
        fade.type = .fade
        fade.beginTime = Self.start
        fade.duration = 1
        // The old lines until the box moves
        fade.fillMode = .backwards
        let finish = Finish(box: self)
        fade.delegate = finish
        lines.add(fade, forKey: kCATransition)
        self.finish = finish
        changed()
    }

    /// Where a gesture took hold of a paused resize: its key (UIKit's
    /// re-timed copies of it keep the key), its fraction then and the fade's
    /// progress then.
    private struct Hold {
        let key: String
        let fraction: CFTimeInterval
        let progress: CFTimeInterval

        /// The fade's progress with the resize at `fraction`: from where it
        /// was when the gesture took hold, as far toward the new lines as the
        /// resize is toward its end, or toward the old ones as it is toward
        /// its start.
        func progress(at fraction: CFTimeInterval) -> CFTimeInterval {
            if fraction < self.fraction { return progress * fraction / self.fraction }
            if fraction > self.fraction { return progress + (1 - progress) * (fraction - self.fraction) / (1 - self.fraction) }
            return progress
        }
    }

    /// A fade's delegate, holding its box weakly (an animation retains its
    /// delegate): when Core Animation finishes the fade, as a resize ends
    /// forward, the box's fade ends, unless the box ended it first (`end`).
    private final class Finish: NSObject, CAAnimationDelegate {
        weak var box: TextBoxLayer?
        init(box: TextBoxLayer) { self.box = box }
        func animationDidStop(_ anim: CAAnimation, finished: Bool) { box?.end() }
    }

    override func add(_ anim: CAAnimation, forKey key: String?) {
        super.add(anim, forKey: key)
        if fading, key?.hasPrefix("bounds") == true { changed() }
    }

    override func removeAnimation(forKey key: String) {
        super.removeAnimation(forKey: key)
        guard fading, key.hasPrefix("bounds") else { return }
        if held?.key == key { held = nil }
        changed()
    }

    override func removeAllAnimations() {
        super.removeAllAnimations()
        guard fading else { return }
        held = nil
        changed()
    }

    /// A change the fade is aimed from (new lines, or a resize added or
    /// removed), aimed once Core Animation has committed it, which fixes the
    /// begin times of the animations UIKit added: the commit's time for one
    /// begun at 0, or a time counted from it (UIKit was seen to add a 0.3 s
    /// delay as 0.3, and a continued animator's re-timed animation as
    /// -0.74). A mark added with the latest change, begun at 0, says when
    /// that commit has happened: its begin time is the commit's from then on.
    private func changed() {
        // Held where it is until it is aimed for the change, in the change's
        // own transaction: its clock was set for the resize as it was
        if let lines, lines.animation(forKey: kCATransition) != nil {
            set(lines, progress: Self.progress(lines, at: convertTime(CACurrentMediaTime(), from: nil)), rate: 0)
        }
        let mark = CABasicAnimation(keyPath: Self.mark)
        mark.speed = 0
        add(mark, forKey: Self.mark)
        awaitCommit()
    }

    private func awaitCommit() {
        guard !Self.unaimed.contains(self) else { return }
        if Self.unaimed.count == 0 {
            // Ordered last in the turn, after Core Animation's commit (its own
            // observer was seen at order 2000000); exit covers a turn that never waits
            let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue | CFRunLoopActivity.exit.rawValue, false, CFIndex.max) { _, _ in
                Self.committed()
            }
            CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
        }
        Self.unaimed.add(self)
    }

    private static let mark = "exactTextFadeCommit"
    /// The boxes whose latest change waits for its commit, held weakly; one
    /// observer is pending while any is here.
    private static let unaimed = NSHashTable<TextBoxLayer>.weakObjects()

    private static func committed() {
        let taken = unaimed.allObjects
        unaimed.removeAllObjects()
        var ready: [TextBoxLayer] = []
        for box in taken where box.fading {
            // Not committed yet (an open explicit transaction holds it, or a run
            // loop run inside another's callout was seen to leave it to that one): a later turn
            if box.animation(forKey: mark)?.beginTime == 0 { box.awaitCommit() } else { ready.append(box) }
        }
        guard !ready.isEmpty else { return }
        // The clocks in a transaction of their own, sent when it commits, as
        // the main thread may sleep through the resize. Opened inside one still
        // open (a callout's, which a run loop run inside it leaves uncommitted),
        // it commits with that one; a flush would send the callout's changes
        // before the callout ends.
        CATransaction.begin()
        for box in ready {
            box.removeAnimation(forKey: mark)
            box.aim()
        }
        CATransaction.commit()
    }

    /// The lines' clock set from the box's resize as committed. Held where a
    /// gesture holds the resize: from where the fade was when the gesture took
    /// hold, as far toward the new lines as the resize is toward its end, or
    /// toward the old ones as it is toward its start. Running: from where the
    /// fade is now to where the resize ends, at its pace, the new lines as it
    /// ends forward, the old ones as it ends back. With no resize running,
    /// the fade ends.
    private func aim() {
        guard fading, let lines, lines.animation(forKey: kCATransition) != nil, let found = resize() else { return end() }
        let (key, resize) = found
        let now = convertTime(CACurrentMediaTime(), from: nil)
        let progress = Self.progress(lines, at: now)
        if resize.speed == 0 {
            let fraction = min(1, max(0, resize.timeOffset / resize.duration))
            let hold = held.flatMap { $0.key == key ? $0 : nil } ?? Hold(key: key, fraction: fraction, progress: progress)
            held = hold
            set(lines, progress: hold.progress(at: fraction), rate: 0)
        } else {
            held = nil
            let (ends, target) = Self.end(of: resize)
            // A fade not begun waits for the box to move; one under way goes on from now
            let from = progress > 0 ? now : max(now, resize.beginTime)
            // The resize over: the fade with it
            guard ends > from else { return end() }
            let rate = (target - progress) / (ends - from)
            set(lines, progress: progress - (from - now) * rate, rate: rate)
        }
    }

    private func end() {
        guard fading, let lines else { return }
        held = nil
        finish?.box = nil
        finish = nil
        removeAnimation(forKey: Self.mark)
        lines.removeAnimation(forKey: kCATransition)
        lines.beginTime = 0
        lines.speed = 1
        lines.timeOffset = 0
    }

    /// The box's resize, as committed, and its key: one UIKit holds paused,
    /// else the one that ends last.
    private func resize() -> (key: String, resize: CAAnimation)? {
        var last: (key: String, resize: CAAnimation, end: CFTimeInterval)?
        for key in animationKeys() ?? [] where key.hasPrefix("bounds") {
            guard let resize = animation(forKey: key) else { continue }
            if resize.speed == 0 { return (key, resize) }
            let end = Self.end(of: resize).time
            if last.map({ $0.end < end }) ?? true { last = (key, resize, end) }
        }
        return last.map { ($0.key, $0.resize) }
    }

    /// When a running `resize` ends, and the fade's progress then: forward at
    /// its duration's end, with the new lines; back (speed -1) at its start,
    /// with the old ones.
    private static func end(of resize: CAAnimation) -> (time: CFTimeInterval, target: CFTimeInterval) {
        let speed = CFTimeInterval(resize.speed)
        return speed > 0 ? (resize.beginTime + (resize.duration - resize.timeOffset) / speed, 1)
                         : (resize.beginTime + (resize.duration + resize.timeOffset) / -speed, 0)
    }

    /// The lines' time: `progress` of the fade now (below 0 before it begins,
    /// where it shows the old lines), going on at `rate` a second of the
    /// box's time. Core Animation runs it from there.
    private func set(_ lines: CALayer, progress: CFTimeInterval, rate: CFTimeInterval) {
        lines.beginTime = convertTime(CACurrentMediaTime(), from: nil)
        lines.speed = Float(rate)
        lines.timeOffset = Self.start + progress
    }

    /// Where the fade is at `now` (the box's time), 0 to 1.
    private static func progress(_ lines: CALayer, at now: CFTimeInterval) -> CFTimeInterval {
        min(1, max(0, (now - lines.beginTime) * CFTimeInterval(lines.speed) + lines.timeOffset - start))
    }
}
#endif
