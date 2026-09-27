// @ref LLP 1011.000 — the players: one per image view showing an animated
// GIF or WebP, all driven by one main-thread timer set for the earliest
// frame change among those on screen. A view off screen, or one that let its
// image go (a recycled row), costs nothing: no timer fires for it and no
// frame is decoded. Under the agent's clock (LLP 1012) nothing runs on its
// own: `evaluate` after each batch and each `clock` shows the frame the
// clock says, decoding it there and then, so a screenshot is exact.
import CoreGraphics
import Foundation
import QuartzCore
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class AnimatedRasters {
    static let shared = AnimatedRasters()

    /// Frames a player keeps: every frame when they all fit in this, else
    /// only the one showing and the next.
    static let keepAll = 4 * 1024 * 1024

    private final class Decoder {
        let animation: RasterAnimation
        lazy var frames: AnimatedFrames? = animation.frames()
        init(_ animation: RasterAnimation) { self.animation = animation }
    }
    private final class Player {
        weak var view: NodeView?
        let image: RasterImage
        let animation: RasterAnimation
        let decoder: Decoder
        let agent: Bool
        var clock: AnimationClock
        var shown = 0
        /// The frame showing, nil for the first (the image's own pixels).
        var frame: CGImage?
        var cache: [Int: CGImage] = [:]
        var inflight: Set<Int> = []
        var wanted: Int?
        init(view: NodeView, image: RasterImage, animation: RasterAnimation, agent: Bool) {
            self.view = view; self.image = image; self.animation = animation; self.agent = agent
            decoder = Decoder(animation); clock = AnimationClock(animation.schedule)
        }
        var keepsAll: Bool { animation.frameBytes * animation.count <= AnimatedRasters.keepAll }
    }

    private var players: [ObjectIdentifier: Player] = [:]
    private let queue = DispatchQueue(label: "exact.animated-images", qos: .userInitiated)
    private var timer: DispatchSourceTimer?
    private var timerDue: Double?
    private var queued = false
    /// Frames decoded, for `diagnostics`.
    private(set) var decoded = 0

    /// The view took a new raster: an animated one gets a player.
    func attach(_ view: NodeView) {
        let key = ObjectIdentifier(view)
        guard let image = view.raster?.image, let animation = image.animation else { players.removeValue(forKey: key); return }
        if players[key]?.image === image { return }
        players[key] = Player(view: view, image: image, animation: animation, agent: view.presenter?.session?.clock != nil)
        #if os(iOS)
        // Its scroll, layout and window notices are the ones video uses.
        if view.presenter?.videoVisibility == nil { view.presenter?.videoVisibility = VideoVisibilityHost() }
        #endif
        poke()
    }

    /// The frame `view` shows now, or nil for its raster's own pixels.
    func frame(for view: NodeView) -> CGImage? {
        guard let p = players[ObjectIdentifier(view)], p.image === view.raster?.image else { return nil }
        return p.frame
    }

    /// Something moved: look again at the next turn.
    func poke() {
        guard !players.isEmpty, !queued else { return }
        queued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            queued = false
            evaluate()
        }
    }

    /// Every player to the frame its clock says, if its view is on screen.
    func evaluate() {
        var earliest: Double?
        for (key, p) in players {
            guard let view = p.view, view.raster?.image === p.image else { players.removeValue(forKey: key); continue }
            let agentClock = view.presenter?.session?.clock
            let now = agentClock ?? CACurrentMediaTime() * 1000
            guard VideoVisibilityHost.fraction(view) > 0 else { p.clock.hide(at: now); continue }
            show(p, p.clock.show(at: now), sync: p.agent || agentClock != nil)
            if agentClock == nil, let next = p.clock.next(after: now) { earliest = min(earliest ?? next, next) }
        }
        schedule(earliest)
    }

    private func schedule(_ due: Double?) {
        guard let due else { timer?.cancel(); timer = nil; timerDue = nil; return }
        if timer != nil, timerDue == due { return }
        timer?.cancel()
        let t = DispatchSource.makeTimerSource(queue: .main)
        t.schedule(deadline: .now() + max(0, (due - CACurrentMediaTime() * 1000) / 1000), leeway: .milliseconds(1))
        t.setEventHandler { [weak self] in
            guard let self else { return }
            timer = nil; timerDue = nil
            evaluate()
        }
        timer = t; timerDue = due
        t.resume()
    }

    private func show(_ p: Player, _ index: Int, sync: Bool) {
        if index == p.shown, index == 0 || p.frame != nil { return }
        if index == 0 { put(p, 0, nil); return }
        if let frame = p.cache[index] { put(p, index, frame); return }
        if sync {
            let decoder = p.decoder
            let frame = queue.sync { autoreleasepool { decoder.frames?.frame(index) } }
            decoded += 1
            if let frame { p.cache[index] = frame; put(p, index, frame) }
            return
        }
        p.wanted = index
        decode(p, index)
    }

    /// The frame on screen, and the next one decoded ahead of its time.
    private func put(_ p: Player, _ index: Int, _ frame: CGImage?) {
        p.shown = index; p.frame = frame; p.wanted = nil
        if !p.keepsAll { p.cache = p.cache.filter { $0.key == index || $0.key == (index + 1) % p.animation.count } }
        present(p)
        let next = (index + 1) % p.animation.count
        if !p.agent, next != 0, p.cache[next] == nil { decode(p, next) }
    }

    private func decode(_ p: Player, _ index: Int) {
        guard !p.inflight.contains(index) else { return }
        p.inflight.insert(index)
        let decoder = p.decoder
        queue.async { [weak self, weak p] in
            let frame = autoreleasepool { decoder.frames?.frame(index) }
            DispatchQueue.main.async {
                guard let self, let p else { return }
                self.decoded += 1
                p.inflight.remove(index)
                guard let frame else { return }
                if p.keepsAll || p.wanted == index || (p.shown + 1) % p.animation.count == index { p.cache[index] = frame }
                if p.wanted == index { self.put(p, index, frame) }
            }
        }
    }

    private func present(_ p: Player) {
        guard let view = p.view else { return }
        #if os(iOS)
        if let layer = view.imageLayer {
            CATransaction.begin(); CATransaction.setDisableActions(true)
            layer.contents = p.frame ?? p.image.image
            CATransaction.commit()
        } else {
            view.setNeedsDisplay()
        }
        #else
        view.needsDisplay = true
        #endif
    }

    /// Players and frames, for the agent's `state` and tests.
    var diagnostics: [String: Any] {
        ["players": players.count,
         "showing": players.values.filter { $0.view.map { VideoVisibilityHost.fraction($0) > 0 } ?? false }.count,
         "cachedFrames": players.values.reduce(0) { $0 + $1.cache.count },
         "decodedFrames": decoded,
         "timer": timer != nil]
    }
}
