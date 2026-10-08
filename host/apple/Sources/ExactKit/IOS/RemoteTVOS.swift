// The Siri Remote on tvOS (LLP 1008 §9). UIKit's focus engine
// moves the focus among exact2's nodes — what a keyboard can focus, and any
// enabled press target — and reports each move as focus and blur. Select
// presses the focused node. Menu presses the active route's Back control
// (LLP 1075 D1) while a route can pop, else a shown button that declares
// `aria-keyshortcuts="Escape"` (the key macOS presses it with). With neither,
// its recognizer is removed, so Menu reaches tvOS and leaves the app, as
// tvOS requires at an app's root. A keyboard's Space presses no node here:
// UIKit turns an unhandled Space into the remote's Play/Pause. A scroll
// container with nothing focusable inside is a remote stop itself, as the
// web's keyboard-focusable scrollers are (Chrome 130): the remote's arrow
// clicks scroll it until its edge, then go on to the next stop; a swipe on
// the touch surface moves the focus out at once, so a long one is no trap.
#if os(tvOS)
import UIKit

extension NodeView {
    /// An explicit negative `tabindex` is no remote stop, as it is no Tab
    /// stop (LLP 1088 D7.3); the remote's order stays UIKit's geometry.
    package override var canBecomeFocused: Bool {
        if let index = explicitTabIndex, index < 0 { return false }
        if cssVisibilityHidden { return false } // no remote stop, nor Select (e28279b3b)
        return canBecomeFirstResponder || (!disabled && !inert && handlers.contains("press")) || focusableScroller
    }

    /// A scroll container that can scroll and holds no focusable node.
    var focusableScroller: Bool {
        guard let scroll, !inert, scroll.canScroll else { return false }
        return !scroll.holdsFocusableNode
    }

    /// An arrow click's direction: the remote's press, or a keyboard's
    /// arrow key (the Simulator sends both for one click).
    static func arrowHeading(_ press: UIPress) -> UIFocusHeading? {
        switch press.type {
        case .upArrow: return .up
        case .downArrow: return .down
        case .leftArrow: return .left
        case .rightArrow: return .right
        default: break
        }
        switch press.key?.keyCode {
        case .keyboardUpArrow: return .up
        case .keyboardDownArrow: return .down
        case .keyboardLeftArrow: return .left
        case .keyboardRightArrow: return .right
        default: return nil
        }
    }
    /// The arrow presses a scroller stepped on, whose ends it takes too.
    static var steppedPresses = Set<ObjectIdentifier>()

    package override func didUpdateFocus(in context: UIFocusUpdateContext, with coordinator: UIFocusAnimationCoordinator) {
        super.didUpdateFocus(in: context, with: coordinator)
        if context.nextFocusedItem === self {
            presenter?.focusKey = props["testId"]
            presenter?.focusGuides.focused(self)
            showFocusRing(true)
            repaintThrough()
            if handlers.contains("focus") { presenter?.focus(id) }
        } else if context.previouslyFocusedItem === self {
            showFocusRing(false)
            repaintThrough()
            if handlers.contains("blur") { presenter?.blur(id) }
        }
    }

    /// Whether `presses` hold Select for this node; a Select going down
    /// presses it. On a focusable scroller, an arrow click it can take
    /// scrolls it; at its edge the click goes on to move the focus. Swipes
    /// are not presses: they move the focus as anywhere else.
    func remoteSelect(_ presses: Set<UIPress>, down: Bool) -> Bool {
        if focusableScroller, let press = presses.first(where: { NodeView.arrowHeading($0) != nil }), let heading = NodeView.arrowHeading(press) {
            // A press's end goes where its start went.
            let key = ObjectIdentifier(press)
            if !down { return NodeView.steppedPresses.remove(key) != nil }
            guard scroll?.remoteStep(heading) == true else { return false }
            NodeView.steppedPresses.insert(key)
            return true
        }
        guard presses.contains(where: { $0.type == .select }), !disabled, !cssVisibilityHidden, handlers.contains("press") else { return false }
        if down { presenter?.press(id) }
        return true
    }
}

extension ScrollView {
    /// How far it can scroll on each axis it scrolls.
    private var maxOffset: CGPoint {
        let inset = adjustedContentInset
        return CGPoint(x: scrollsX ? contentSize.width + inset.left + inset.right - bounds.width : 0,
                       y: scrollsY ? contentSize.height + inset.top + inset.bottom - bounds.height : 0)
    }
    var canScroll: Bool { maxOffset.x > 1 || maxOffset.y > 1 }

    var holdsFocusableNode: Bool {
        var pending: [UIView] = subviews
        while let view = pending.popLast() {
            if let node = view as? NodeView, node.canBecomeFocused { return true }
            pending.append(contentsOf: view.subviews)
        }
        return false
    }

    /// Scrolls most of a screenful toward `heading`, when there is more that
    /// way; false at the edge, where the move leaves.
    func remoteStep(_ heading: UIFocusHeading) -> Bool {
        let inset = adjustedContentInset, end = maxOffset
        let now = CACurrentMediaTime()
        // One input is one step: the Simulator (and a keyboard) sends an
        // arrow both as a key and as the remote's press, milliseconds apart.
        if let last = remoteStepTarget, now - last.at < 0.06 { return true }
        // A step's animation runs about 0.3 s; a press during it adds to its target.
        let pending = remoteStepTarget.flatMap { now - $0.at < 0.35 ? $0.offset : nil }
        let start = pending ?? contentOffset
        var offset = start
        if heading.contains(.down) { offset.y = min(offset.y + bounds.height * 0.8, end.y - inset.top) }
        else if heading.contains(.up) { offset.y = max(offset.y - bounds.height * 0.8, -inset.top) }
        else if heading.contains(.right) { offset.x = min(offset.x + bounds.width * 0.8, end.x - inset.left) }
        else if heading.contains(.left) { offset.x = max(offset.x - bounds.width * 0.8, -inset.left) }
        guard abs(offset.x - start.x) > 0.5 || abs(offset.y - start.y) > 0.5 else { return false }
        remoteStepTarget = (offset, now)
        setContentOffset(offset, animated: true)
        return true
    }
}

extension Presenter {
    /// The shown, focusable node with the `testId` that last held the focus.
    var focusReturn: NodeView? {
        guard let key = focusKey else { return nil }
        return views.values.first { $0.props["testId"] == key && $0.window != nil && $0.canBecomeFocused }
    }
}

extension ExactView {
    /// When the focused view goes (a branch that swaps its subtree), the
    /// focus engine asks where focus belongs: the replacement with the same
    /// `testId`, rather than whatever the engine would pick.
    override public var preferredFocusEnvironments: [any UIFocusEnvironment] {
        if let node = session.presenter.focusReturn { return [node] }
        return super.preferredFocusEnvironments
    }
}

/// `focusGuide="auto"`: a focus guide over each such node, so a remote move
/// that enters its box from outside lands on the descendant that last held
/// the focus, else on the node (the engine then picks its first focusable
/// item) — react-native-tvos's TVFocusGuideView `autoFocus`. A guide is off
/// while the focus is inside its node, so moves within it stay the engine's.
final class FocusGuides {
    unowned let presenter: Presenter
    private var guides: [UInt32: (node: NodeView, guide: UIFocusGuide)] = [:]
    private var last: [UInt32: WeakNode] = [:]
    private final class WeakNode { weak var node: NodeView?; init(_ node: NodeView) { self.node = node } }

    init(presenter: Presenter) { self.presenter = presenter }

    func sync() {
        let wanted = Set(presenter.chrome.ids("focusGuide").filter { presenter.views[$0]?.props["focusGuide"] == "auto" })
        for (id, entry) in guides where !wanted.contains(id) || presenter.views[id] !== entry.node {
            entry.node.removeLayoutGuide(entry.guide)
            guides[id] = nil
            last[id] = nil
        }
        for id in wanted where guides[id] == nil {
            guard let node = presenter.views[id] else { continue }
            let guide = UIFocusGuide()
            node.addLayoutGuide(guide)
            NSLayoutConstraint.activate([
                guide.leadingAnchor.constraint(equalTo: node.leadingAnchor), guide.trailingAnchor.constraint(equalTo: node.trailingAnchor),
                guide.topAnchor.constraint(equalTo: node.topAnchor), guide.bottomAnchor.constraint(equalTo: node.bottomAnchor),
            ])
            guides[id] = (node, guide)
        }
        let window = presenter.session?.view?.window
        update(window.flatMap { UIFocusSystem.focusSystem(for: $0)?.focusedItem } as? UIView)
    }

    /// `view` took the focus: each guide around it remembers it.
    func focused(_ view: NodeView) {
        for (id, entry) in guides where view.isDescendant(of: entry.node) { last[id] = WeakNode(view) }
        update(view)
    }

    private func update(_ focused: UIView?) {
        for (id, entry) in guides {
            entry.guide.isEnabled = !(focused?.isDescendant(of: entry.node) ?? false)
            let remembered = last[id]?.node.flatMap { $0.window != nil && $0.isDescendant(of: entry.node) && $0.canBecomeFocused ? $0 : nil }
            entry.guide.preferredFocusEnvironments = [remembered ?? entry.node]
        }
    }
}

extension Presenter {
    /// While a video's full-screen player shows, AVKit handles the remote:
    /// the session's own Menu and Play/Pause recognizers come off.
    var remoteKeysSuspended: Bool { views.values.contains { $0.video?.isFullscreen == true } }
    func remoteKeysChanged() {
        menuKey.sync()
        playPauseKey.sync()
    }
}

/// The remote's Play/Pause: the first video on screen plays or pauses, as
/// its own controls would. Present only while a video is mounted.
final class PlayPauseKey: NSObject {
    unowned let presenter: Presenter
    private var tap: UITapGestureRecognizer?

    init(presenter: Presenter) { self.presenter = presenter }

    private var video: VideoView? {
        presenter.views.values.filter { $0.video != nil && $0.window != nil }.min { $0.id < $1.id }?.video
    }

    func sync() {
        let wanted = video != nil && !presenter.remoteKeysSuspended
        if wanted, tap == nil, let view = presenter.session?.view {
            let recognizer = UITapGestureRecognizer(target: self, action: #selector(playPause))
            recognizer.allowedPressTypes = [NSNumber(value: UIPress.PressType.playPause.rawValue)]
            view.addGestureRecognizer(recognizer)
            tap = recognizer
        } else if !wanted, let recognizer = tap {
            recognizer.view?.removeGestureRecognizer(recognizer)
            tap = nil
        }
    }

    @objc private func playPause() { video?.togglePlayPause() }
}

/// The Menu recognizer, present only while Menu goes back.
final class MenuKey: NSObject {
    unowned let presenter: Presenter
    private var tap: UITapGestureRecognizer?

    init(presenter: Presenter) { self.presenter = presenter }

    func sync() {
        let wanted = !presenter.remoteKeysSuspended && (presenter.navigation.menuGoesBack || escapeControl != nil)
        if wanted, tap == nil, let view = presenter.session?.view {
            let recognizer = UITapGestureRecognizer(target: self, action: #selector(menu))
            recognizer.allowedPressTypes = [NSNumber(value: UIPress.PressType.menu.rawValue)]
            view.addGestureRecognizer(recognizer)
            tap = recognizer
        } else if !wanted, let recognizer = tap {
            recognizer.view?.removeGestureRecognizer(recognizer)
            tap = nil
        }
    }

    /// The shown, enabled press target that declares Escape as its key.
    private var escapeControl: NodeView? {
        presenter.carrying("accessibilityKeyShortcuts").first {
            $0.window != nil && !sequence(first: $0 as UIView, next: \.superview).contains(where: \.isHidden) && !$0.disabled && !$0.inert && $0.handlers.contains("press")
                && ($0.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).contains("Escape")
        }
    }

    @objc private func menu() {
        if presenter.navigation.menuGoesBack { presenter.navigation.menuBack() } else if let control = escapeControl { presenter.press(control.id) }
    }
}
#endif
