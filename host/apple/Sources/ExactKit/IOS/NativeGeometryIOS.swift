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

    func reset() { fresh = true; boxes = []; modal = nil }

    func changed() {
        guard !pending else { return }
        pending = true
        DispatchQueue.main.async { [weak self] in
            guard let self, let session = presenter?.session, session.state == .ready else { self?.pending = false; return }
            session.whenIdle(bound: true, { [weak self, weak session] in
                guard let self, let session, session.state == .ready else { return }
                pending = false
                guard let presenter, !presenter.applying, !Owner.shared.isServingMain else { changed(); return }
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
