// @ref LLP 1115.002 D8.2 — the containing block a host gives a subtree it
// shows in a view of its own: a pane or a sheet (BlockHostMac.swift).
import CoreGraphics
import Foundation
import CExact

/// The containing block a native container gives one subtree, in points.
struct HostBlock: Equatable {
    /// One axis: a size, a maximum, or a limit for automatic content fitting.
    enum Axis: Equatable {
        case size(CGFloat), atMost(CGFloat), fit(CGFloat)
        fileprivate var wire: (length: CGFloat, policy: CGFloat) {
            switch self {
            case .size(let length): (length, 0)
            case .atMost(let length): (length, 1)
            case .fit(let length): (length, 2)
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
}

/// Where a host shows a block in place (LLP 1123 D2): its rectangle in the
/// local coordinates of `within`, a node that holds it in the host's
/// hierarchy as in the document.
struct BlockPlacement: Equatable {
    var rect: CGRect
    var within: UInt32
}

extension Runtime {
    /// Subtrees' containing blocks (nil takes one back) under one layout,
    /// through the door the covers use (`exact_host_covers`, kinds 4 and 5),
    /// and where the blocks are shown when the host shows them in place
    /// (kinds 6 and 7; nil forgets one), which only the runner's geometry
    /// reads (LLP 1123 D2).
    /// `covers` go through the same door, under the same layout.
    func blocks(_ blocks: [(UInt32, HostBlock?)], placements: [(UInt32, BlockPlacement?)] = [], covers: [(UInt32, HostCover?)] = []) -> Batch {
        return on {
            var bytes = Data(capacity: (blocks.count + placements.count + covers.count) * 28)
            let append = { (words: [UInt32]) in
                for word in words { withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) } }
            }
            for (view, cover) in covers { append(Self.coverRecord(view, cover)) }
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
                append([view, 6] + [r.minX, r.minY, r.width, r.height].map { Float($0).bitPattern } + [placement.within])
            }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
}
