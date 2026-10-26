// @ref LLP 1115.002 D8.2 — a subtree of the session's page in a view of its
// own: a split's pane, a sheet's window. The host moves the subtree's node
// view into the container's view, as a dialog's is moved to the top layer,
// and gives the kernel the box it is laid out in (a containing block: a
// pane's size, or a sheet's maximum). The kernel lays the subtree out in
// that box and nothing else again, sends its frames in the box's own space
// and the box's used size, which a sheet gives its window. Where the
// container is and how it clips are AppKit's. Stage 6a's proof drives it
// (`HostBlockMacTests`); a `dialog`'s sheet is its first caller in an app
// (stage 5, SheetsMac.swift), and the split (6b) and the modal route's
// sheet (6c) are the next.
#if os(macOS)
import AppKit
import CExact

/// The view a native container shows a subtree in: flipped, as node views
/// are, clipping what overflows the block, and saying when the container
/// (a divider, a window) gave it another size.
final class BlockView: NSView {
    var sizeChanged: ((CGSize) -> Void)?
    var positionChanged: (() -> Void)?
    /// The moves of a free pointer over a window of the session's own (a
    /// popover's, a sheet's), as the page's viewport reports its window's
    /// (PageScrollView.swift): an authored ancestor out in the page hears
    /// them, though no tracking area of its own is in this window.
    var pointerTracking: ((NSEvent) -> Void)? { didSet { updateTrackingAreas() } }
    private var pointerArea: NSTrackingArea?
    override var isFlipped: Bool { true }
    override init(frame: NSRect) { super.init(frame: frame); clipsToBounds = true }
    required init?(coder: NSCoder) { nil }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        guard pointerTracking != nil, pointerArea == nil else { return }
        // In the active app, not only the key window: a popover need not be
        // key, and it stands in front of the page that is.
        let area = NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .mouseMoved, .activeInActiveApp, .inVisibleRect], owner: self, userInfo: nil)
        addTrackingArea(area); pointerArea = area
    }
    override func mouseEntered(with event: NSEvent) { pointerTracking?(event) }
    override func mouseMoved(with event: NSEvent) { pointerTracking?(event) }
    override func mouseExited(with event: NSEvent) { pointerTracking?(event) }
    override func setFrameOrigin(_ newOrigin: NSPoint) {
        let old = frame.origin
        super.setFrameOrigin(newOrigin)
        if newOrigin != old { positionChanged?() }
    }
    override func setFrameSize(_ newSize: NSSize) {
        let old = frame.size
        super.setFrameSize(newSize)
        if newSize != old { sizeChanged?(newSize) }
    }
}

/// The subtrees shown in views of their own.
final class BlockHost {
    private weak var presenter: Presenter?
    private struct Entry {
        let node: NodeView
        /// Its authored parent's view, and the sibling that followed it.
        weak var home: NSView?
        weak var next: NSView?
        /// Its authored siblings before it as it left, nearest last, and
        /// its turn among the subtrees shown.
        var before: [Sibling] = []
        var turn = 0
        var block: HostBlock
        var placement: BlockPlacement?
        /// What its container does with its used size (a sheet sizes its window).
        var used: ((CGSize) -> Void)?
        /// It is a split's own placeholder, shown while its container has it.
        var placeholder = false
    }
    private struct Sibling { weak var node: NodeView? }
    private var entries: [UInt32: Entry] = [:]
    private var turns = 0
    /// Each block's used size, as the last layout returned it.
    private(set) var used: [UInt32: CGSize] = [:]
    /// Each block root's margins when its container was last told its size.
    private var margined: [UInt32: PositionArea.Margins] = [:]
    /// Each block's rectangle in its root's coordinates (`BatchOp.blockRect`),
    /// a popover's in the page too: what a painted popover is reported by.
    private(set) var rects: [UInt32: CGRect] = [:]
    /// A block's used size changed: a sheet sizes its window from it.
    var onUsed: ((UInt32, CGSize) -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Whether `node`'s view lives in a container's view: a `children` op of
    /// its authored parent leaves it there.
    func owns(_ node: NodeView) -> Bool { entries[node.id]?.node === node }

    /// The subtrees in containers of their own, in the order they were
    /// shown: the page a walk of the root does not reach.
    var shown: [NodeView] { entries.values.sorted { $0.turn < $1.turn }.map(\.node) }

    /// Event paths keep the authored parent across a native container.
    /// A home that has disappeared ends the path.
    func parent(of node: NodeView) -> NSView? {
        guard let entry = entries[node.id], entry.node === node else { return node.superview }
        return entry.home
    }

    /// The node children `home` holds, with the subtrees that left it for a
    /// container of their own back in their places: its authored children,
    /// for a reader of the tree's shape (a stack's routes). Each goes after
    /// the nearest sibling that was before it and is still there, so a
    /// sibling mounted since (a route pushed over it) comes after it.
    func authored(_ mounted: [NodeView], in home: NSView) -> [NodeView] {
        guard !entries.isEmpty else { return mounted }
        var out = mounted
        for entry in entries.values.sorted(by: { $0.turn < $1.turn }) where entry.home === home && !out.contains(where: { $0 === entry.node }) {
            let after = entry.before.reversed().lazy.compactMap { sibling in out.firstIndex { $0 === sibling.node } }.first
            out.insert(entry.node, at: after.map { $0 + 1 } ?? 0)
        }
        return out
    }

    /// Put `node`'s subtree in `container` and lay it out in `block`; `used`
    /// hears its used size, now and whenever layout gives it another.
    /// `placeholder`: `node` is a split's own placeholder, `display: none`
    /// but where its container shows it (LLP 1116.006 D2).
    func show(_ node: NodeView, in container: NSView, block: HostBlock, placement: BlockPlacement? = nil, used: ((CGSize) -> Void)? = nil, placeholder: Bool = false) {
        guard let presenter, presenter.views[node.id] === node else { return }
        // Outside the root (a sheet, a popover, a split's pane), still this
        // presenter's theme (LLP 1121 stage 5).
        ThemeRoles.adopt(container, from: presenter.root)
        // A node of another runner may have had this id (a restart): its entry is not this node's.
        if entries[node.id]?.node !== node {
            let home = node.superview, siblings = home?.subviews ?? []
            let next = siblings.firstIndex(of: node).flatMap { $0 + 1 < siblings.count ? siblings[$0 + 1] : nil }
            // Its authored siblings, those already in containers of their own among them.
            let nodes = home.map { authored($0.subviews.compactMap { $0 as? NodeView }, in: $0) } ?? []
            let before = nodes.prefix { $0 !== node }.map { Sibling(node: $0) }
            turns += 1
            entries[node.id] = Entry(node: node, home: home, next: next, before: before, turn: turns, block: block, placement: placement, used: used, placeholder: placeholder)
        } else { entries[node.id]?.block = block; entries[node.id]?.placement = placement; entries[node.id]?.used = used; entries[node.id]?.placeholder = placeholder }
        if node.superview !== container {
            container.addSubview(node)
            presenter.topLayerChanged()
        }
        tell([(node.id, block)], placements: [(node.id, placement)], shows: placeholder ? [(node.id, true)] : [])
    }

    /// A split's placeholder this shows was restyled by a commit: its shown
    /// style is asked for again, which the runner answers with nothing
    /// where it stands.
    func showAgain(_ node: NodeView) {
        guard entries[node.id]?.node === node, entries[node.id]?.placeholder == true else { return }
        tell([], shows: [(node.id, true)])
    }

    /// The container gave the subtree another box: a divider moved, a
    /// sheet's maximum changed with its parent window.
    func resize(_ node: NodeView, _ block: HostBlock, placement: BlockPlacement? = nil) {
        guard owns(node), entries[node.id]?.block != block || entries[node.id]?.placement != placement else { return }
        entries[node.id]?.block = block
        entries[node.id]?.placement = placement
        tell([(node.id, block)], placements: [(node.id, placement)])
    }

    /// Give the subtree back to the page, where its authored parent has it.
    func release(_ node: NodeView) {
        guard owns(node), let entry = entries[node.id] else { return }
        let siblings = entry.home.map { authored($0.subviews.compactMap { $0 as? NodeView }, in: $0) } ?? []
        let following = siblings.drop { $0 !== node }.dropFirst().first { $0.superview === entry.home }
        entries[node.id] = nil
        used[node.id] = nil
        if presenter?.views[node.id] === node, let home = entry.home {
            // Before the sibling that followed it, when that is still there;
            // the authored parent's next `children` op settles any other order.
            if let next = following ?? entry.next, next.superview === home { home.addSubview(node, positioned: .below, relativeTo: next) }
            else { home.addSubview(node) }
            presenter?.topLayerChanged()
        } else { node.removeFromSuperview() }
        tell([(node.id, nil)], shows: entry.placeholder ? [(node.id, false)] : [])
    }

    /// The batch's `block` op: a block's used size.
    func applied(_ op: BatchOp) {
        if op.payload["op"] as? String == "block" { rects[op.id] = op.blockRect } // none: no block
        guard op.payload["op"] as? String == "block", entries[op.id] != nil,
              let w = op.payload["w"] as? NSNumber, let h = op.payload["h"] as? NSNumber else { return }
        let size = CGSize(width: w.doubleValue, height: h.doubleValue)
        // Its root's margins too: a container that shows the root without them
        // (AppKit's popover) resizes when a margin is traded for content.
        let edges = entries[op.id].map { PositionArea.margins(of: $0.node) } ?? .init()
        let resized = used[op.id] != size
        guard resized || margined[op.id] != edges else { return }
        used[op.id] = size
        margined[op.id] = edges
        entries[op.id]?.used?(size)
        if resized { onUsed?(op.id, size) }
    }

    /// A subtree's root is being destroyed, its view still in its container:
    /// a sheet keeps what it showed while it leaves (RouteSheetMac.swift).
    var onDestroyed: ((NodeView) -> Void)?

    /// The runtime is replaced: no rectangle it sent stands.
    func forgetRects() { rects = [:] }

    /// The node is gone, and its block with it.
    func destroyed(_ id: UInt32) {
        if let node = entries[id]?.node { onDestroyed?(node) }
        entries[id] = nil; used[id] = nil; rects[id] = nil; margined[id] = nil
    }

    private func tell(_ blocks: [(UInt32, HostBlock?)], placements: [(UInt32, BlockPlacement?)] = [], shows: [(UInt32, Bool)] = []) {
        guard let session = presenter?.session else { return }
        session.whenIdle(bound: true) { [weak session] in
            guard let session else { return }
            session.apply(session.runtime.blocks(blocks, placements: placements, shows: shows))
        }
    }
}
#endif
