#if os(iOS) || os(tvOS)
import UIKit

/// Publishes native presentation movement independently of runner input.
final class NativeGeometry {
    private weak var presenter: Presenter?
    private var pending = false
    private var fresh = true
    private struct Box: Equatable {
        let id: UInt32
        let frame: CGRect
        let clip: CGRect?
    }
    private var boxes: [Box] = []
    private var modal: Int?
    init(_ presenter: Presenter) { self.presenter = presenter }

    func reset() {
        FrameClock.shared.drop(self)
        pending = false; fresh = true; boxes = []; modal = nil
    }

    /// Native containers animate above authored nodes. Their presentation layers
    /// carry the current position; authored transforms remain the runner's.
    static func convert(_ rect: CGRect, from source: UIView, to destination: UIView) -> CGRect {
        guard animates(source.layer) || animates(destination.layer),
              let from = source.layer.presentation(), let to = destination.layer.presentation() else {
            return source.convert(rect, to: destination)
        }
        return from.convert(rect, to: to)
    }

    /// A native clip changes size as well as position during UIKit animations.
    static func bounds(of source: UIView, in destination: UIView) -> CGRect {
        guard animates(source.layer) || animates(destination.layer),
              let from = source.layer.presentation(), let to = destination.layer.presentation() else {
            return source.convert(source.bounds, to: destination)
        }
        return from.convert(from.bounds, to: to)
    }

    private static func animates(_ layer: CALayer) -> Bool {
        var at: CALayer? = layer
        while let current = at {
            if !(current.delegate is NodeView), !(current.animationKeys() ?? []).isEmpty { return true }
            at = current.superlayer
        }
        return false
    }

    private func followAnimations() {
        guard let presenter else { FrameClock.shared.drop(self); return }
        let routes = presenter.navigation.controllers.values.compactMap(\.viewIfLoaded)
        let surfaces = presenter.presentations(includingRetiring: true)
        let moving = (routes + surfaces).contains { $0.window != nil && Self.animates($0.layer) }
        guard moving else { FrameClock.shared.drop(self); return }
        FrameClock.shared.want(self, .navigationReveal, rate: FrameClock.full(on: presenter.viewport.window?.screen)) { [weak self] _ in
            guard let self, !pending, let presenter = self.presenter else { return }
            presenter.owners.publish()
            changed()
        }
    }

    func changed() {
        guard !pending else { return }
        pending = true
        DispatchQueue.main.async { [weak self] in
            guard let self, let session = presenter?.session, session.state == .ready else {
                if let self { pending = false; FrameClock.shared.drop(self) }
                return
            }
            session.whenIdle(bound: true, { [weak self, weak session] in
                guard let self else { return }
                pending = false
                guard let session, session.state == .ready else { FrameClock.shared.drop(self); return }
                guard let presenter, !presenter.applying, !Owner.shared.isServingMain, !session.runtime.held else { changed(); return } // (held: after the reset, its caches and frame clock untouched: ResetGate)
                followAnimations()
                let shown = presenter.openTopLayer
                let now = shown.frames.map { Box(id: $0.0, frame: $0.1, clip: $0.2) }
                guard fresh || boxes != now || modal != shown.modal else { return }
                fresh = false; boxes = now; modal = shown.modal
                // Runtime.on samples the top layer before the pump; its emit wakes intersections.
                session.applyUnlessEmpty(session.runtime.pump(now: session.now()))
            }, cancelled: { [weak self] in self?.pending = false })
        }
    }
}
#endif
