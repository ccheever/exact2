// Paragraphs by where they sit in their scroller's content, so a scroll
// frame's text passes (`paintVisibleText`, `refreshVisibleText`) visit the
// ones near each scrollport, not every paragraph of a long list: r24's inbox
// keeps ~2,600 that are never rastered, and visiting each, twice a frame,
// was most of its main thread while scrolling.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// Bumped whenever a box may have moved within its scroller's content:
    /// its frame, bounds or centre set, a transform put on or taken off, its
    /// parent changed, a paragraph added or removed. A `TextIndex` built at
    /// another value is built again before it answers.
    static var textGeometry: UInt64 = 0
    package override var frame: CGRect { didSet { Self.textGeometry &+= 1 } }
    package override var bounds: CGRect { didSet { Self.textGeometry &+= 1 } }
    package override var center: CGPoint { didSet { Self.textGeometry &+= 1 } }
    // A box under a transform is not indexed (`TextIndex.build`), so only
    // putting one on or taking it off moves what the index holds: a sticky
    // header moved each frame by its translation does not rebuild it.
    package override var transform: CGAffineTransform {
        didSet { if oldValue.isIdentity != transform.isIdentity { Self.textGeometry &+= 1 } }
    }
}

/// The document holds the page's paragraphs (`TextIndex` places them through it).
extension PlainView {
    package override var frame: CGRect { didSet { NodeView.textGeometry &+= 1 } }
    package override var bounds: CGRect { didSet { NodeView.textGeometry &+= 1 } }
    package override var center: CGPoint { didSet { NodeView.textGeometry &+= 1 } }
    package override func didMoveToSuperview() { super.didMoveToSuperview(); NodeView.textGeometry &+= 1 }
}

final class TextIndex {
    private struct Entry {
        /// The paragraph's bounds in its scroller's bounds space, which
        /// scrolling does not move.
        let rect: CGRect
        let shift: CGPoint
        let node: NodeView
    }
    /// One scroller's paragraphs, those not under a transform or a view
    /// other than a node between them and it. A mount (the document's in the
    /// viewport's place, a route's or a sheet's view: `DocumentMountIOS.swift`,
    /// `PresentationMount`) is a port that does not scroll: what is outside
    /// its bounds is off screen. Without them every paragraph of a list in a
    /// route was visited every pass (r27's inbox: 3,300, most of a frame).
    private final class Port {
        weak var scroller: UIView?
        /// By `minY`.
        var entries: [Entry] = []
        /// Taller than `tallLimit`, each tested whenever the scroller is.
        var tall: [Entry] = []
        /// The tallest of `entries`: one starting that far above a band can
        /// still reach into it.
        var tallest: CGFloat = 0
        /// A point of reach in the viewport, in the scroller's points: more
        /// under a scale that shrinks it (`unit(_:in:)`).
        var unit: CGFloat = 1
        init(_ scroller: UIView) { self.scroller = scroller }
    }
    private static let tallLimit: CGFloat = 2048
    private var ports: [Port] = []
    /// Paragraphs not in a port, visited every time.
    private var loose: [NodeView] = []
    private var built: UInt64?
    private(set) var builds = 0

    /// Each of `views` whose bounds may meet its scroller's port grown by
    /// `reach` (in `viewport`'s points), and every paragraph whose place is
    /// not indexed. A paragraph not visited has no band at that reach
    /// (`Presenter.textBand`): its scroller clips it out. `body` returns
    /// false to stop.
    func visit(_ views: [UInt32: NodeView], reach: CGFloat, viewport: UIView, _ body: (NodeView) -> Bool) {
        if built != NodeView.textGeometry { build(views, viewport: viewport) }
        for node in loose where !body(node) { return }
        for port in ports {
            guard let scroller = port.scroller, scroller.window != nil, !scroller.isHidden else { continue }
            guard scroller.clipsToBounds || !(scroller is UIScrollView) else {
                for entry in port.entries where !body(entry.node) { return }
                for entry in port.tall where !body(entry.node) { return }
                continue
            }
            let band = scroller.bounds.insetBy(dx: -reach * port.unit - 1, dy: -reach * port.unit - 1)
            func meets(_ entry: Entry) -> Bool {
                let r = entry.rect
                guard r.maxY >= band.minY && r.minY <= band.maxY else { return false }
                guard let accepted = entry.node.currentTextRaster else { return true }
                let at = entry.node.textPlace(scale: accepted.key.scale)
                let support = accepted.result.coverage.support.offsetBy(dx: at.x + entry.shift.x, dy: at.y + entry.shift.y)
                return support.intersects(band)
            }
            for entry in port.tall where meets(entry) && !body(entry.node) { return }
            let entries = port.entries
            var lo = 0, hi = entries.count
            let start = band.minY - port.tallest
            while lo < hi {
                let mid = (lo + hi) / 2
                if entries[mid].rect.minY < start { lo = mid + 1 } else { hi = mid }
            }
            var i = lo
            while i < entries.count, entries[i].rect.minY <= band.maxY {
                if meets(entries[i]) && !body(entries[i].node) { return }
                i += 1
            }
        }
    }

    /// `Presenter.textBand` grows each clip by the reach in the viewport's
    /// points (`TextClips`) or the paragraph's (its walk), which differ under
    /// a scale: the larger of the two, as the scale was when the index was
    /// built. One that changes since (a flight's) moves only what a reach
    /// leads by, which a refresh works ahead on; what shows is at no reach.
    private static func unit(_ scroller: UIView, in viewport: UIView) -> CGFloat {
        let unit = scroller.convert(CGRect(x: 0, y: 0, width: 1, height: 1), to: viewport)
        let scale = min(unit.width, unit.height)
        return scale.isFinite && scale > 0 ? max(1, 1 / scale) : 1
    }

    private func build(_ views: [UInt32: NodeView], viewport: UIView) {
        built = NodeView.textGeometry
        builds += 1
        loose.removeAll(keepingCapacity: true)
        var ports: [ObjectIdentifier: Port] = [:]
        /// For a view holding paragraphs: its scroller's port and where its
        /// bounds' origin is in that scroller's bounds space; nil when a
        /// transform or a view other than a node comes between.
        var origins: [ObjectIdentifier: (port: Port, shift: CGPoint)?] = [:]
        func identity(_ view: UIView) -> Bool { view.transform.isIdentity && CATransform3DIsIdentity(view.layer.transform) }
        func origin(_ view: UIView) -> (port: Port, shift: CGPoint)? {
            let key = ObjectIdentifier(view)
            if let known = origins[key] { return known }
            var found: (port: Port, shift: CGPoint)?
            if view is UIScrollView || view is DocumentMount || view is PresentationMount {
                let port = ports[key] ?? Port(view)
                ports[key] = port
                found = (port, .zero)
            } else if view is NodeView || view is PlainView, identity(view), let parent = view.superview, let above = origin(parent) {
                found = (above.port, CGPoint(x: above.shift.x + view.frame.minX - view.bounds.minX,
                                             y: above.shift.y + view.frame.minY - view.bounds.minY))
            }
            origins[key] = .some(found)
            return found
        }
        for node in views.values {
            guard identity(node), let parent = node.superview, let above = origin(parent) else { loose.append(node); continue }
            let shift = CGPoint(x: above.shift.x + node.frame.minX - node.bounds.minX,
                                y: above.shift.y + node.frame.minY - node.bounds.minY)
            let rect = node.bounds.offsetBy(dx: shift.x, dy: shift.y).insetBy(dx: 0, dy: -TextRasterJob.maxShadowReach)
            let entry = Entry(rect: rect, shift: shift, node: node)
            if rect.height > Self.tallLimit { above.port.tall.append(entry) } else { above.port.entries.append(entry) }
        }
        self.ports = ports.values.filter { !$0.entries.isEmpty || !$0.tall.isEmpty }
        for port in self.ports {
            if let scroller = port.scroller { port.unit = Self.unit(scroller, in: viewport) }
            port.entries.sort { $0.rect.minY < $1.rect.minY }
            port.tallest = port.entries.reduce(0) { max($0, $1.rect.height) }
        }
    }
}
#endif
