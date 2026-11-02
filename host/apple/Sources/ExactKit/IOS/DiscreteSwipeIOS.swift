#if os(iOS) || os(tvOS)
import UIKit

// LLP 1057.001 §8 on UIKit: `swipe` is UIKit's own discrete swipe, one
// `UISwipeGestureRecognizer` per direction the node names (one given several
// directions does not say which it saw), taking the node's `touch-count` of
// touches. A new direction or count makes new recognizers, which fails a
// pending contact's swipe; the old ones dispatch nothing after they go. tvOS
// makes none (§8.2), so this compiles there and never runs.
final class DiscreteSwipe: UISwipeGestureRecognizer {
    /// The direction, as `SwipeEvent` names it.
    let word: String
    /// The `touch-count` it was made for; omitted, UIKit's own one.
    let touches: Int
    /// What the contact landed on, recorded as it went down (`admitsSwipe`).
    weak var hit: UIView?
    /// The editor the contact went down in: a change of its selection since
    /// `began` refuses the swipe (§8.4, rule 3).
    weak var editor: NodeView?
    /// When the contact went down, on `UITouch`'s clock.
    var began: TimeInterval = 0
    /// Whether the contact went down within the left 20 points, where Back
    /// begins, for a swipe to the right (rule 1).
    var atBackEdge = false

    /// A unit vector in its direction, which a pan's admission reads as its
    /// velocity.
    let travel: CGPoint

    /// Each direction's word, as `SwipeEvent` names it, with UIKit's
    /// direction and its unit vector.
    static let directions: [String: (Direction, CGPoint)] = [
        "left": (.left, CGPoint(x: -1, y: 0)), "right": (.right, CGPoint(x: 1, y: 0)),
        "up": (.up, CGPoint(x: 0, y: -1)), "down": (.down, CGPoint(x: 0, y: 1)),
    ]

    /// Nil for a word that names no direction.
    init?(_ node: NodeView, word: String, touches: Int) {
        guard let (direction, travel) = Self.directions[word] else { return nil }
        self.word = word
        self.touches = touches
        self.travel = travel
        super.init(target: node, action: #selector(NodeView.swiped(_:)))
        self.direction = direction
        #if os(iOS)
        numberOfTouchesRequired = touches
        #endif
    }
    override func reset() {
        super.reset()
        hit = nil
        editor = nil
        atBackEdge = false
    }
}

extension NodeView {
    /// The recognizers this node's `swipe`, `swipe-direction` and
    /// `touch-count` ask for, made again when the directions change or the
    /// count does (an omitted count is one).
    func updateDiscreteSwipes() {
        #if os(iOS)
        // Lowering and the runner give valid words and counts only; a word
        // that names no direction would make nothing
        let words = handlers.contains("swipe") ? (props["swipeDirection"] ?? "").split(separator: " ").map(String.init).filter { DiscreteSwipe.directions[$0] != nil } : []
        let touches = props["touchCount"].flatMap(Int.init) ?? 1
        let current = discreteSwipes
        if current.map(\.word) == words, current.allSatisfy({ $0.touches == touches }) { return }
        for gesture in current { removeGestureRecognizer(gesture) }
        discreteSwipes = words.compactMap { word in
            guard let gesture = DiscreteSwipe(self, word: word, touches: touches) else { return nil }
            gesture.delegate = self
            addGestureRecognizer(gesture)
            return gesture
        }
        #endif
    }
    @objc func swiped(_ gesture: DiscreteSwipe) {
        // Only the current recognizers of the node's current view speak for it (§8.5)
        guard presenter?.views[id] === self, discreteSwipes.contains(where: { $0 === gesture }) else {
            presenter?.session?.log("swipe \(gesture.word) dropped: its recognizer is no longer node \(id)'s")
            return
        }
        presenter?.swipe(id, gesture.word)
    }
    /// Whether a contact is this swipe's: a direct touch passes a text
    /// editor, and a pointer's or pencil's stays the editor's, whose drag
    /// selects; a `press` node between the hit and the node keeps it (rule
    /// 3). At the contact's first touch it records what it landed on, when,
    /// the editor it went down in, and whether it went down where Back begins.
    func admitsSwipe(_ gesture: DiscreteSwipe, _ touch: UITouch) -> Bool {
        if CanvasInputs.owns(touch.view) { return false }
        #if os(iOS)
        let finger = touch.type == .direct
        if pressBoundary(touch, editors: !finger) { return false }
        if hitsNativeEditor(touch.view), !finger { return false }
        #endif
        if gesture.numberOfTouches == 0 {
            gesture.hit = touch.view
            gesture.began = touch.timestamp
            gesture.editor = Self.editingNode(touch.view, below: self)
            #if os(iOS)
            gesture.atBackEdge = startsAtBackEdge(gesture.word, x: touch.location(in: window).x)
            #endif
        }
        return true
    }
    /// A swipe may recognize: `SwipeInput.allows` admits the node, as it must
    /// for a pan, the drags and the pinch; the contact did not go down where
    /// Back begins for a swipe to the right; and the editor the contact went
    /// down in has not moved its selection since (a handle dragged, a double
    /// tap's word, a long press's caret drag).
    func swipeMayBegin(_ gesture: DiscreteSwipe) -> Bool {
        SwipeInput.allows(self) && !gesture.atBackEdge && (gesture.editor.map { $0.selectionMovedAt < gesture.began } ?? true)
    }
    /// Whether a contact at `x` in the window starts a swipe in `word` where
    /// §7's back gestures begin: to the right, within the left 20 points,
    /// which a swipe yields as `swiperight` does (rule 1, §8.4)
    func startsAtBackEdge(_ word: String, x: CGFloat) -> Bool {
        word == "right" && x < Gesture.edge
    }
    /// The swipe waits for the pan of each scroll view on the contact's hit
    /// path that would begin for travel in its direction, by both halves of
    /// that pan's admission: UIKit's, that it scrolls or bounces on that
    /// axis, and Exact's (`ScrollView.wouldPan`, or `TextArea.pans`), so
    /// scroll wins by construction. It also waits for an inner node's swipe
    /// in the same direction and count, so the innermost wins (rule 3).
    func swipeRequiresFailure(_ gesture: DiscreteSwipe, of other: UIGestureRecognizer) -> Bool {
        if let inner = other as? DiscreteSwipe, let owner = inner.view as? NodeView, owner !== self, owner.isDescendant(of: self) {
            return inner.word == gesture.word && inner.touches == gesture.touches
        }
        guard let scroll = other.view as? UIScrollView, other === scroll.panGestureRecognizer,
              let hit = gesture.hit, hit === scroll || hit.isDescendant(of: scroll) else { return false }
        let travel = gesture.travel, vertical = travel.y != 0
        let size = scroll.bounds.inset(by: scroll.adjustedContentInset).size
        let overflows = vertical ? scroll.contentSize.height > size.height + 0.5 : scroll.contentSize.width > size.width + 0.5
        let bounces = vertical ? scroll.alwaysBounceVertical : scroll.alwaysBounceHorizontal
        guard overflows || bounces else { return false }
        if let area = scroll as? TextArea { return area.pans }
        if let own = scroll as? ScrollView { return own.wouldPan(travel, from: gesture.location(in: scroll)) }
        return true
    }
    /// Whether this node takes a one-finger swipe to the right, which keeps the
    /// content pop off a touch on it unless a `press` node lies between (§8.4).
    var takesRightSwipe: Bool {
        discreteSwipes.contains { $0.word == "right" && $0.touches == 1 } && SwipeInput.allows(self)
    }
    /// The node of the editor `view` is in, up to and including `boundary`
    /// (a swipe on the field's own node).
    static func editingNode(_ view: UIView?, below boundary: UIView) -> NodeView? {
        guard nativeEditor(view, below: boundary) else { return nil }
        var current = view
        while let v = current {
            if let node = v as? NodeView, node.textArea != nil || node.field != nil { return node }
            if v === boundary { return nil }
            current = v.superview
        }
        return nil
    }
}
#endif
