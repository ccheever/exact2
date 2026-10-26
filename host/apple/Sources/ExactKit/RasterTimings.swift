// @ref LLP 1145 D2 — when each image arrived, and which arrived late. The
// loader stamps each view's image at the stage boundaries it passes; the
// frame sampler marks the first frame its box showed with no pixels. An image
// that showed blank is counted when its pixels land or its row retires, so a
// fling's late images survive the rows that scrolled past. Development builds
// only; a production bake keeps nothing.
import Foundation
import QuartzCore

final class RasterTimings {
    /// One view's image, seconds on the media clock.
    struct Stamps {
        var asked: Double
        var admitted: Double?
        /// The header was read and the decode asked for: a remote image's
        /// bytes have arrived.
        var requested: Double?
        var decoded: Double?
        var delivered: Double?
        /// The first frame its box showed on screen without its pixels.
        var blankFrom: Double?
        /// What it was waiting on then.
        var stage: String?
    }

    /// The stage an image is in: waiting to be admitted, downloading (and its
    /// header read), waiting for or in its decode, or decoded and waiting for
    /// the main thread to deliver it.
    static func stage(_ s: Stamps) -> String {
        s.admitted == nil ? "admission" : s.requested == nil ? "download" : s.decoded == nil ? "decode" : "delivery"
    }

    private(set) var live: [UInt32: Stamps] = [:]
    /// Late images across retirements: how long each showed blank (ms), the
    /// stage it waited on, and its lead (ms from asked to first showing).
    private var blankMs: [Double] = [], leadMs: [Double] = []
    private var byStage: [String: Int] = [:]
    private(set) var late = 0
    static let kept = 512
    var clock: () -> Double = CACurrentMediaTime

    func asked(_ view: UInt32) {
        guard HatchDiagnostics.available else { return }
        live[view] = Stamps(asked: clock())
    }
    func admitted(_ view: UInt32) { stamp(view) { if $0.admitted == nil { $0.admitted = self.clock() } } }
    func requested(_ view: UInt32) { stamp(view) { if $0.requested == nil { $0.requested = self.clock() } } }
    func decoded(_ view: UInt32) { stamp(view) { if $0.decoded == nil { $0.decoded = self.clock() } } }
    /// Its pixels are on its layer: counted if it showed blank, and let go of.
    func delivered(_ view: UInt32) {
        guard var s = live.removeValue(forKey: view), s.delivered == nil else { return }
        s.delivered = clock()
        close(s)
    }
    /// A replaced runner's views are gone.
    func reset() { live.removeAll() }
    /// The row retired: a blank box that never got its pixels counts too.
    func retired(_ view: UInt32) {
        guard var s = live.removeValue(forKey: view) else { return }
        if s.delivered == nil, s.blankFrom != nil { s.delivered = clock(); close(s) }
    }
    /// A presented frame: each image whose box shows and has no pixels yet
    /// starts its blank time here, if it had not already.
    func showing(_ blank: [UInt32]) {
        let now = clock()
        for view in blank {
            stamp(view) { s in
                guard s.blankFrom == nil, s.delivered == nil else { return }
                s.blankFrom = now; s.stage = Self.stage(s)
            }
        }
    }

    private func stamp(_ view: UInt32, _ change: (inout Stamps) -> Void) {
        guard var s = live[view] else { return }
        change(&s)
        live[view] = s
    }

    private func close(_ s: Stamps) {
        guard let from = s.blankFrom, let to = s.delivered else { return }
        late += 1
        byStage[s.stage ?? "download", default: 0] += 1
        blankMs.append((to - from) * 1000); leadMs.append((from - s.asked) * 1000)
        if blankMs.count > Self.kept { blankMs.removeFirst(); leadMs.removeFirst() }
    }

    /// For `state.raster`: the late images so far, and one live image's stamps
    /// as ms since it was asked for.
    var summary: [String: Any] {
        let q = { (xs: [Double], f: Double) -> Any in
            let s = xs.sorted()
            return s.isEmpty ? NSNull() : (s[min(s.count - 1, Int(f * Double(s.count)))] * 10).rounded() / 10
        }
        return ["late": late, "blankMs": ["p50": q(blankMs, 0.5), "p95": q(blankMs, 0.95), "max": blankMs.max() ?? NSNull()] as [String: Any],
                "leadMs": ["p50": q(leadMs, 0.5), "p5": q(leadMs, 0.05)] as [String: Any], "waitingOn": byStage]
    }
    func stamps(_ view: UInt32) -> [String: Any]? {
        guard let s = live[view] else { return nil }
        let ms = { (t: Double?) -> Any in t.map { (($0 - s.asked) * 10000).rounded() / 10 } ?? NSNull() }
        return ["admitted": ms(s.admitted), "requested": ms(s.requested), "decoded": ms(s.decoded), "delivered": ms(s.delivered),
                "blankFrom": ms(s.blankFrom), "stage": Self.stage(s)]
    }
}
