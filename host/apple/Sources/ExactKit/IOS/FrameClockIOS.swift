// One display link for the app's life (iOS, tvOS); its users come and go.
// @ref LLP 1009 D4 (frames only while something moves); LLP 1061 D4 (rate).
#if os(iOS) || os(tvOS)
import UIKit

/// The app's one `CADisplayLink`. A user asks for frames with the rate it
/// needs and gives them up when it is done; the link is made once and paused
/// while no one asks, so an idle app gets no callbacks. Users: the session's
/// motion, timers and canvases (`Frames`), a list's smooth correction
/// (`OffsetDriver`), a navigation transition's reveal, the scroll pump's
/// owed work, deferred heavy leaves, live SVG filters. Press feedback eases
/// on the render server and needs none. A development session's
/// `FrameSampler` (LLP 1079) keeps its own link: it measures the cadence
/// the display delivers, which must not be the clock's own callbacks or
/// change with the rate the clock asks for.
/// It asks for the highest rate any user is asking for. Users tick in the
/// order they gave (`Order`), the session's frame first.
final class FrameClock: NSObject {
    static let shared = FrameClock()

    /// A correction moves the port before the reveal and the pump read it.
    enum Order: Int { case session, offsetDriver, navigationReveal, scroll, heavyLeaves, svgFilter }

    private struct User {
        weak var owner: AnyObject?
        var order: Order
        var rate: CAFrameRateRange
        var tick: (CADisplayLink) -> Void
    }
    private var users: [ObjectIdentifier: User] = [:]
    private(set) var link: CADisplayLink?
    /// Links made over the clock's life: one.
    private(set) var linksMade = 0

    var running: Bool { link.map { !$0.isPaused } ?? false }
    func wants(_ owner: AnyObject) -> Bool { users[ObjectIdentifier(owner)]?.owner != nil }

    /// Main thread only. `owner` takes frames at `rate` until `drop` or its
    /// release (a released owner is let go at the next frame, so an owner
    /// that may die off the main thread need not drop itself); asking again
    /// only changes its rate or tick.
    func want(_ owner: AnyObject, _ order: Order, rate: CAFrameRateRange = .default, tick: @escaping (CADisplayLink) -> Void) {
        users[ObjectIdentifier(owner)] = User(owner: owner, order: order, rate: rate, tick: tick)
        update()
    }
    func drop(_ owner: AnyObject) {
        guard users.removeValue(forKey: ObjectIdentifier(owner)) != nil else { return }
        update()
    }

    /// What something moving on screen asks for: the display's full rate,
    /// never under 60 (`screen` nil: the app's fastest).
    static func full(on screen: UIScreen?) -> CAFrameRateRange {
        let fastest = screen?.maximumFramesPerSecond
            ?? UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.screen.maximumFramesPerSecond }.max() ?? 60
        let m = Float(fastest)
        return CAFrameRateRange(minimum: min(60, m), maximum: m, preferred: m)
    }

    /// The highest of the users' ranges; `.default` when none asks for one.
    /// `.default` is a timer's (LLP 1061 D4): it never raises the rate, so
    /// motion asks with `full(on:)`.
    var rate: CAFrameRateRange {
        users.values.reduce(CAFrameRateRange.default) { a, u in
            let b = u.rate
            if b == .default { return a }
            if a == .default { return b }
            return CAFrameRateRange(minimum: max(a.minimum, b.minimum), maximum: max(a.maximum, b.maximum), preferred: max(a.preferred ?? 0, b.preferred ?? 0))
        }
    }

    private func update() {
        users = users.filter { $0.value.owner != nil }
        guard !users.isEmpty else { link?.isPaused = true; return }
        let l = link ?? make()
        let r = rate
        if l.preferredFrameRateRange != r { l.preferredFrameRateRange = r }
        l.isPaused = false
    }
    deinit { link?.invalidate() }

    private func make() -> CADisplayLink {
        let l = CADisplayLink(target: Target(self), selector: #selector(Target.tick(_:)))
        l.isPaused = true
        l.add(to: .main, forMode: .common)
        link = l; linksMade += 1
        return l
    }

    /// One frame: each user in order. A user may drop itself or another, or
    /// ask anew, during it; one dropped before its turn does not tick.
    func fire(_ link: CADisplayLink) {
        let order = users.sorted { ($0.value.order.rawValue, $0.key.hashValue) < ($1.value.order.rawValue, $1.key.hashValue) }.map(\.key)
        for key in order {
            guard let user = users[key], user.owner != nil else { continue }
            user.tick(link)
        }
        update()
    }
}

private final class Target: NSObject {
    weak var clock: FrameClock?
    init(_ clock: FrameClock) { self.clock = clock }
    @objc func tick(_ link: CADisplayLink) { clock?.fire(link) }
}
#endif
