// @ref LLP 1115.002 D8.2 — the containing block a host gives a subtree it
// shows in a view of its own: a pane or a sheet (BlockHostMac.swift).
import CoreGraphics
import Foundation
import CExact

/// The containing block a native container gives one subtree, in points.
struct HostBlock: Equatable {
    /// One axis: a size, a maximum, a limit for automatic content fitting, or
    /// the region a `position-area` option places the box in (fit-content
    /// across, min-content winning; its content's height down).
    enum Axis: Equatable {
        case size(CGFloat), atMost(CGFloat), fit(CGFloat), region(CGFloat)
        fileprivate var wire: (length: CGFloat, policy: CGFloat) {
            switch self {
            case .size(let length): (length, 0)
            case .atMost(let length): (length, 1)
            case .fit(let length): (length, 2)
            case .region(let length): (length, 3)
            }
        }
    }
    var width: Axis, height: Axis
    /// A pane's: the block is its size.
    static func size(_ size: CGSize) -> HostBlock { HostBlock(width: .size(size.width), height: .size(size.height)) }
    /// A sheet's: as large as its content, at most this.
    static func atMost(_ size: CGSize) -> HostBlock { HostBlock(width: .atMost(size.width), height: .atMost(size.height)) }
    /// A modal route fits an automatic size; its authored size and minimum win.
    static func fit(_ size: CGSize) -> HostBlock { HostBlock(width: .fit(size.width), height: .fit(size.height)) }
    /// A popover's position-area option (`PositionArea.place`).
    static func region(_ size: CGSize) -> HostBlock { HostBlock(width: .region(size.width), height: .region(size.height)) }
}

/// Where a host shows a block in place (LLP 1123 D2): its rectangle in the
/// local coordinates of `within`, a node that holds it in the host's
/// hierarchy as in the document.
struct BlockPlacement: Equatable {
    var rect: CGRect
    var within: UInt32
    /// A scrollport's offset in block points, including zero; nil for a fixed allocation.
    var contentOffset: CGPoint?
}

/// How a host shows a route's block against its stack's top (LLP 1127 D8),
/// as kind 18 says it: in its stack, beside the top and live (a split's
/// list), or under a pane over it and not live (a split's detail under the
/// list that overlays it).
enum BlockShown: Float {
    case inStack = 0, beside = 1, under = 2
}

extension Runtime {
    /// Subtrees' containing blocks (nil takes one back) under one layout,
    /// through the door the covers use (`exact_host_covers`, kinds 4 and 5),
    /// and where the blocks are shown when the host shows them in place
    /// (kinds 6 and 13; kind 7 forgets one), which only the runner's geometry
    /// reads (LLP 1123 D2). `shows` shows a split's own placeholder, or hides
    /// it again (kinds 14 and 15, LLP 1116.006 D2), in the same layout;
    /// `shown` says how a block is shown against its stack's top (kind 18, LLP 1127).
    func blocks(_ blocks: [(UInt32, HostBlock?)], placements: [(UInt32, BlockPlacement?)] = [], shows: [(UInt32, Bool)] = [], shown: [(UInt32, BlockShown)] = []) -> Batch {
        return on {
            var bytes = Data(capacity: (blocks.count + placements.count + shows.count) * 36)
            let append = { (words: [UInt32]) in
                for word in words { withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) } }
            }
            for (view, shown) in shows { append([view, shown ? 14 : 15, 0, 0, 0, 0]) }
            for (view, block) in blocks {
                let width = block?.width.wire, height = block?.height.wire
                let lengths = [width?.length ?? 0, height?.length ?? 0, width?.policy ?? 0, height?.policy ?? 0].map { Float($0).bitPattern }
                append([view, block == nil ? 5 : 4] + lengths)
            }
            for (view, placement) in placements {
                guard let placement else {
                    append([view, 7, 0, 0, 0, 0])
                    continue
                }
                let r = placement.rect
                let offset = placement.contentOffset ?? .zero
                append([view, placement.contentOffset == nil ? 6 : 13] + [r.minX, r.minY, r.width, r.height].map { Float($0).bitPattern } + [placement.within] + [offset.x, offset.y].map { Float($0).bitPattern })
            }
            for (view, how) in shown { append([view, 18, how.rawValue.bitPattern, 0, 0, 0]) }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
}

extension Runtime {
    /// What `view`'s subtree would measure in `block`, its border box and
    /// margins as that layout resolves them (`exact_measure_block`, a trial
    /// layout): nothing is laid out or sent, so a host can weigh several
    /// blocks for one box; nil when refused (a view that holds no block).
    func measureBlock(_ view: UInt32, in block: HostBlock) -> PositionArea.Measured? {
        return on(busy: nil) {
            let width = block.width.wire, height = block.height.wire
            let len = exact_measure_block(rt, view, Float(width.length), Float(height.length), UInt32(width.policy), UInt32(height.policy))
            guard len == 24 else { return nil }
            guard let out = UnsafeRawPointer(exact_out(rt)) else { return nil }
            let f = { (i: Int) in CGFloat(Float(bitPattern: UInt32(littleEndian: out.loadUnaligned(fromByteOffset: 4 * i, as: UInt32.self)))) }
            return PositionArea.Measured(size: CGSize(width: f(0), height: f(1)),
                                         margins: PositionArea.Margins(top: f(2), right: f(3), bottom: f(4), left: f(5)))
        }
    }

    /// What moves whenever `measureBlock` could answer otherwise (`exact_layout_stamp`).
    func layoutStamp() -> UInt64 {
        on(busy: 0) { exact_layout_stamp(rt) }
    }
}
