#if os(iOS)
import UIKit
import QuartzCore

extension NodeView {
    func updateHeightDragGesture() {
        if presenter?.heightBindings[id]?.target != nil, heightRecognizer == nil {
            let pan = UIPanGestureRecognizer(target: self, action: #selector(heightDragging(_:)))
            pan.maximumNumberOfTouches = 1
            pan.delegate = self
            addGestureRecognizer(pan)
            heightRecognizer = pan
        } else if presenter?.heightBindings[id]?.target == nil, let pan = heightRecognizer {
            let previous = heightHold; heightHold = nil
            DispatchQueue.main.async { previous?.cancel() }
            removeGestureRecognizer(pan)
            heightRecognizer = nil
        }
    }
    @objc func heightDragging(_ pan: UIPanGestureRecognizer) {
        let translation = Double(pan.translation(in: window).y)
        let time = CACurrentMediaTime()
        switch pan.state {
        case .began:
            heightHold?.cancel()
            heightHold = HeightDragHold(self, time: time)
            // UIKit may deliver a coalesced drag entirely in its first sample.
            // Catch the current presentation, then apply that sample too.
            if let hold = heightHold, !hold.move(downward: translation, time: time) {
                hold.cancel(); heightHold = nil
            }
        case .changed:
            guard let hold = heightHold else { return }
            if !hold.move(downward: translation, time: time) {
                hold.cancel(); heightHold = nil
            }
        case .ended, .cancelled, .failed:
            let previous = heightHold; heightHold = nil
            previous?.finish(downward: translation, time: time, cancel: pan.state != .ended)
        default: break
        }
    }
}
#endif
