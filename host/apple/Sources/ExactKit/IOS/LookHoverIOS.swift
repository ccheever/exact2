#if os(iOS)
import UIKit

// A pointer's `:hover` look on iOS (LLP 1121 D8, stage 9 G3): a node with one
// shows it while UIKit's hover recognizer has the pointer (an iPad's, its
// Pencil's) over it, at once, as the Mac's mouse tracking does. Each such
// node has its own recognizer, recognizing beside every other, so an
// ancestor's look holds with its child's, as the web's `:hover` does. A
// `hover` handler's recognizer (NodeViewIOS.swift) is apart: it dispatches
// the innermost node's path once.
@MainActor
final class LookHover: NSObject, UIGestureRecognizerDelegate {
    static let shared = LookHover()
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
}

extension NodeView {
    /// Its look recognizer, while its style has a `:hover` look or a hover
    /// is held on it (whose release the pointer's crossing is).
    func lookHover(_ wanted: Bool) {
        let wanted = wanted || (HeldState.view === self && HeldState.state == "hover")
        if wanted, lookHoverRecognizer == nil {
            let g = UIHoverGestureRecognizer(target: self, action: #selector(lookHovering(_:)))
            g.delegate = LookHover.shared
            // It observes the pointer: it never holds or cancels a finger.
            g.delaysTouchesBegan = false
            g.delaysTouchesEnded = false
            g.cancelsTouchesInView = false
            addGestureRecognizer(g)
            lookHoverRecognizer = g
        } else if !wanted, let g = lookHoverRecognizer {
            // Unheard as it goes: UIKit cancels a recognizer it takes off,
            // which is no crossing (and this runs inside `presented`).
            g.removeTarget(nil, action: nil)
            removeGestureRecognizer(g)
            lookHoverRecognizer = nil
            pressPaint.hovered = false
        }
    }

    @objc func lookHovering(_ g: UIHoverGestureRecognizer) {
        switch g.state {
        case .began: HeldState.lookHovered(self, true); lookState(hovered: true)
        case .ended: HeldState.lookHovered(self, false); lookState(hovered: false)
        // Cancelled (the recognizer turned off, as inspect mode does to every
        // one): no crossing, so a hold stays; whether the pointer is over is
        // known again at its next begin.
        case .cancelled, .failed: HeldState.lookUnknown(self); lookState(hovered: false)
        default: break
        }
    }

    /// Whether it has its look recognizer for the hold: put on for a hover
    /// held on a node with no look, and off again after (`lookHover`).
    var lookHoverInstalled: Bool { lookHoverRecognizer != nil }
}
#endif
