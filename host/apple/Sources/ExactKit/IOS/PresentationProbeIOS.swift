#if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
import Metal
import QuartzCore
import UIKit

/// A recording's presentation probe: transparent Metal layers of one point
/// in the session's window, one of which presents a drawable just before
/// each UI update it has one ready for commits, so its `presentedTime` says
/// when the composite holding that update reached the screen: late or never
/// when the render server missed its frame. The drawable is cleared and its
/// clear completed off the main thread, and it presents on its own, not with
/// the transaction (`presentsWithTransaction`), which made every commit wait
/// on Metal: on an iPhone Duo simulator a commit took 1.5 ms with it and
/// 0.12 ms without, and presenting this way takes 0.03 ms. A commit past its
/// deadline can reach the screen after the drawable; `overrun` sees that. Plain layers, not views, they take no touches, are no
/// accessibility elements and change no layout or safe area. On the
/// simulator, whose Metal has no `presentedTime` or `addPresentedHandler`
/// (MTLDrawable.h in its SDKs), it presents but hears nothing, and its
/// frames wait until it goes.
final class PresentationProbe {
    /// Two layers: with one, the probe heard from about seven in ten frames
    /// of a 120 Hz scroll on an iPhone, as its three drawables, each held
    /// until a later one of the layer's reaches the screen, were often all
    /// in use when an update came
    private let layers = [CAMetalLayer(), CAMetalLayer()]
    private let commands: MTLCommandQueue
    private let fetching = DispatchQueue(label: "exact.presentation-probe")
    private let lock = NSLock()
    /// Each layer's drawable fetched and cleared for the next update, and
    /// whether a fetch is under way; the reports waiting for the main
    /// thread: each drawable's number (`presentations` when it was
    /// presented) and when it was shown (0: never).
    private var ready: [CAMetalDrawable?] = [nil, nil], fetchRunning = [false, false]
    private var reports: [(drawable: Int, presented: CFTimeInterval)] = []
    /// The drawables it presented.
    private(set) var presentations = 0

    init?(in window: UIWindow) {
        guard let device = MTLCreateSystemDefaultDevice(), let commands = device.makeCommandQueue() else { return nil }
        self.commands = commands
        for layer in layers {
            layer.device = device
            layer.pixelFormat = .bgra8Unorm
            layer.framebufferOnly = true
            layer.isOpaque = false
            layer.allowsNextDrawableTimeout = true
            layer.drawableSize = CGSize(width: 1, height: 1)
            layer.frame = CGRect(x: 0, y: 0, width: 1, height: 1)
            // Above the window's content, which could otherwise cover it from the compositor
            layer.zPosition = 1_000_000
            window.layer.addSublayer(layer)
        }
        for i in layers.indices { fetch(i) }
    }

    var isInstalled: Bool { layers[0].superlayer != nil }

    /// How many of its layers have a drawable ready for the next update.
    var readyCount: Int { lock.withLock { ready.filter { $0 != nil }.count } }

    func isIn(_ window: UIWindow) -> Bool { layers[0].superlayer === window.layer }

    /// Take the layers out of their window, which the probe's owner does
    /// before it lets the probe go: a fetch under way can hold the probe
    /// until it returns.
    func remove() { for layer in layers { layer.removeFromSuperlayer() } }

    /// Fetch and clear the next drawable off the main thread, where
    /// `nextDrawable` can wait for frames while every drawable is queued or
    /// on screen (UIKit's advice in UIUpdateActionPhase.h), until its clear
    /// is complete, so the render server waits on no GPU work of the probe's.
    private func fetch(_ i: Int) {
        guard lock.withLock({ () -> Bool in
            guard !fetchRunning[i], ready[i] == nil else { return false }
            fetchRunning[i] = true
            return true
        }) else { return }
        fetching.async { [layer = layers[i], commands, weak self] in
            let drawable = layer.nextDrawable().flatMap { Self.cleared($0, by: commands) }
            guard let self else { return }
            self.lock.withLock { self.ready[i] = drawable; self.fetchRunning[i] = false }
        }
    }

    private static func cleared(_ drawable: CAMetalDrawable, by commands: MTLCommandQueue) -> CAMetalDrawable? {
        guard let buffer = commands.makeCommandBuffer() else { return nil }
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = drawable.texture
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        pass.colorAttachments[0].storeAction = .store
        buffer.makeRenderCommandEncoder(descriptor: pass)?.endEncoding()
        buffer.commit()
        buffer.waitUntilCompleted()
        return drawable
    }

    /// Present a drawable as the UI update now committing begins its commit,
    /// the first layer's when both have one; the number its report will
    /// carry, or nil when no drawable was ready (the main thread never waits
    /// for one).
    func present() -> Int? {
        defer { for i in layers.indices { fetch(i) } }
        guard let drawable = lock.withLock({ () -> CAMetalDrawable? in
            guard let i = ready.firstIndex(where: { $0 != nil }) else { return nil }
            defer { ready[i] = nil }
            return ready[i]
        }) else { return nil }
        presentations += 1
        let number = presentations
        #if !targetEnvironment(simulator)
        drawable.addPresentedHandler { [weak self] shown in self?.heard(number, at: shown.presentedTime) }
        #endif
        drawable.present()
        return number
    }

    /// Metal's report that drawable `number` was shown at `presented`, or
    /// never (0), from any thread. On the simulator only a test stands in
    /// for it.
    func heard(_ number: Int, at presented: CFTimeInterval) {
        lock.withLock { reports.append((number, presented)) }
    }

    /// The presentations reported since the last call.
    func take() -> [(drawable: Int, presented: CFTimeInterval)] {
        lock.withLock { defer { reports = [] }; return reports }
    }
}
#endif
