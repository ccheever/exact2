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

/// The containing block a native container gives one subtree, in points.
struct HostBlock: Equatable {
    /// One axis: the block is this long, or at most this long and as long
    /// as its content.
    enum Axis: Equatable {
        case size(CGFloat), atMost(CGFloat)
        fileprivate var wire: (length: CGFloat, most: CGFloat) {
            switch self {
            case .size(let length): (length, 0)
            case .atMost(let length): (length, 1)
            }
        }
    }
    var width: Axis, height: Axis
    /// A pane's: the block is its size.
    static func size(_ size: CGSize) -> HostBlock { HostBlock(width: .size(size.width), height: .size(size.height)) }
    /// A sheet's: as large as its content, at most this.
    static func atMost(_ size: CGSize) -> HostBlock { HostBlock(width: .atMost(size.width), height: .atMost(size.height)) }
}

extension Runtime {
    /// Subtrees' containing blocks (nil takes one back) under one layout,
    /// through the door the covers use (`exact_host_covers`, kinds 4 and 5).
    func blocks(_ blocks: [(UInt32, HostBlock?)]) -> Batch {
        return on {
            var bytes = Data(capacity: blocks.count * 24)
            for (view, block) in blocks {
                let width = block?.width.wire, height = block?.height.wire
                let lengths = [width?.length ?? 0, height?.length ?? 0, width?.most ?? 0, height?.most ?? 0].map { Float($0).bitPattern }
                for word in [view, block == nil ? 5 : 4] + lengths {
                    withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
                }
            }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
}

/// The view a native container shows a subtree in: flipped, as node views
/// are, clipping what overflows the block, and saying when the container
/// (a divider, a window) gave it another size.
final class BlockView: NSView {
    var sizeChanged: ((CGSize) -> Void)?
    override var isFlipped: Bool { true }
    override init(frame: NSRect) { super.init(frame: frame); clipsToBounds = true }
    required init?(coder: NSCoder) { nil }
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
        /// What its container does with its used size (a sheet sizes its window).
        var used: ((CGSize) -> Void)?
    }
    private struct Sibling { weak var node: NodeView? }
    private var entries: [UInt32: Entry] = [:]
    private var turns = 0
    /// Each block's used size, as the last layout returned it.
    private(set) var used: [UInt32: CGSize] = [:]
    /// A block's used size changed: a sheet sizes its window from it.
    var onUsed: ((UInt32, CGSize) -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Whether `node`'s view lives in a container's view: a `children` op of
    /// its authored parent leaves it there.
    func owns(_ node: NodeView) -> Bool { entries[node.id]?.node === node }

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
    func show(_ node: NodeView, in container: NSView, block: HostBlock, used: ((CGSize) -> Void)? = nil) {
        guard let presenter, presenter.views[node.id] === node else { return }
        // A node of another runner may have had this id (a restart): its entry is not this node's.
        if entries[node.id]?.node !== node {
            let home = node.superview, siblings = home?.subviews ?? []
            let next = siblings.firstIndex(of: node).flatMap { $0 + 1 < siblings.count ? siblings[$0 + 1] : nil }
            // Its authored siblings, those already in containers of their own among them.
            let nodes = home.map { authored($0.subviews.compactMap { $0 as? NodeView }, in: $0) } ?? []
            let before = nodes.prefix { $0 !== node }.map { Sibling(node: $0) }
            turns += 1
            entries[node.id] = Entry(node: node, home: home, next: next, before: before, turn: turns, block: block, used: used)
        } else { entries[node.id]?.block = block; entries[node.id]?.used = used }
        if node.superview !== container {
            container.addSubview(node)
            presenter.topLayerChanged()
        }
        tell([(node.id, block)])
    }

    /// The container gave the subtree another box: a divider moved, a
    /// sheet's maximum changed with its parent window.
    func resize(_ node: NodeView, _ block: HostBlock) {
        guard owns(node), entries[node.id]?.block != block else { return }
        entries[node.id]?.block = block
        tell([(node.id, block)])
    }

    /// Give the subtree back to the page, where its authored parent has it.
    func release(_ node: NodeView) {
        guard owns(node), let entry = entries.removeValue(forKey: node.id) else { return }
        used[node.id] = nil
        if presenter?.views[node.id] === node, let home = entry.home {
            // Before the sibling that followed it, when that is still there;
            // the authored parent's next `children` op settles any other order.
            if let next = entry.next, next.superview === home { home.addSubview(node, positioned: .below, relativeTo: next) }
            else { home.addSubview(node) }
            presenter?.topLayerChanged()
        } else { node.removeFromSuperview() }
        tell([(node.id, nil)])
    }

    /// The batch's `block` op: a block's used size.
    func applied(_ op: BatchOp) {
        guard op.payload["op"] as? String == "block", entries[op.id] != nil,
              let w = op.payload["w"] as? NSNumber, let h = op.payload["h"] as? NSNumber else { return }
        let size = CGSize(width: w.doubleValue, height: h.doubleValue)
        guard used[op.id] != size else { return }
        used[op.id] = size
        entries[op.id]?.used?(size)
        onUsed?(op.id, size)
    }

    /// A subtree's root is being destroyed, its view still in its container:
    /// a sheet keeps what it showed while it leaves (RouteSheetMac.swift).
    var onDestroyed: ((NodeView) -> Void)?

    /// The node is gone, and its block with it.
    func destroyed(_ id: UInt32) {
        if let node = entries[id]?.node { onDestroyed?(node) }
        entries[id] = nil; used[id] = nil
    }

    private func tell(_ blocks: [(UInt32, HostBlock?)]) {
        guard let session = presenter?.session else { return }
        session.whenIdle { [weak session] in
            guard let session else { return }
            session.apply(session.runtime.blocks(blocks))
        }
    }
}
#endif
