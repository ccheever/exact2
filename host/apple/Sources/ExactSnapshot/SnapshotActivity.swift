// @ref LLP 1126.001 WP10 — a Live Activity's placements through the snapshot
// driver (D4, D5): one headless session per activity, each placement laid
// out in its region's viewport, where the bake positioned its root
// absolutely at the top left, so CSS 2.1 §10.3.7 gives the root its
// shrink-to-fit width (or the region's, for the Lock Screen and the expanded
// bottom); that one layout is measured and drawn. The Lock Screen's
// background is the placement's root background; the Island's keyline and
// region layout are the system's.
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore

/// ActivityKit's placements, in the bake's order (`exact_plan::ACTIVITY_PLACEMENTS`):
/// the plan's container holds one root per placement, in this order.
public enum ActivityPlacement: Int, CaseIterable, Sendable {
    case lockScreen, compactLeading, compactTrailing, minimal
    case expandedLeading, expandedTrailing, expandedCenter, expandedBottom

    public static let names = ["lockScreen", "compactLeading", "compactTrailing", "minimal",
                               "expanded.leading", "expanded.trailing", "expanded.center", "expanded.bottom"]
    public var name: String { Self.names[rawValue] }
    public init?(name: String) {
        guard let index = Self.names.firstIndex(of: name) else { return nil }
        self.init(rawValue: index)
    }
    /// Whether the placement is as wide as the region the system proposes
    /// (the Lock Screen, the expanded bottom; the bake gives their roots
    /// `right: 0`) or as wide as its content: the compact regions, the
    /// minimal view and the expanded leading, trailing and centre regions,
    /// which the Island sizes to what they hold ("the system first
    /// determines the width of the center content while taking into account
    /// the minimal width of the leading and trailing content", ActivityKit,
    /// Displaying live data with Live Activities).
    public var fillsWidth: Bool { self == .lockScreen || self == .expandedBottom }
    /// Whether a nested link is a `Link` here (the Lock Screen and the
    /// expanded presentation take them); elsewhere the first link is the
    /// presentation's URL.
    public var takesLinks: Bool { self == .lockScreen || rawValue >= Self.expandedLeading.rawValue }
    /// The Lock Screen presentation, where the system's content margins
    /// surround the entry (the Island's regions have their own insets).
    public var hasMargins: Bool { self == .lockScreen }
}

/// One activity's session, laid out placement by placement. The session is
/// booted once per state (the plan with `snapshot_state` as its props) and
/// resized to each region's viewport; the layouts are kept by viewport, so
/// the archive's repeated proposals cost one layout each, and a view draws
/// the layout its size was measured from, never one another region's
/// proposal left in the session.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public final class ActivityRender {
    /// A placement laid out in a viewport: the tree, the placement's root in
    /// it (its border box, at its own margins from the viewport's top
    /// left), the margins the layout resolved, whether the root's width is
    /// the author's, and the viewport (what `snapshot_viewport` read).
    public struct Laid {
        public let snapshot: HeadlessSession.Snapshot
        public let root: HeadlessSession.Node
        public let margins: (left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat)
        public let authoredWidth: Bool
        public let minimumWidth: CGFloat
        public let viewport: CGSize
        /// The placement's margin box: what its region holds.
        public var size: CGSize { CGSize(width: root.frame.width + margins.left + margins.right, height: root.frame.height + margins.top + margins.bottom) }
    }
    public let session: HeadlessSession
    private let lock = NSLock()
    private var laid: [String: Laid] = [:]
    private var layouts = 0
    private var nanoseconds: UInt64 = 0
    /// How many layouts this state has run, and their time: the render's
    /// cost per push (LLP 1126.001 §5 Q4), which the extension reports.
    public var layoutCount: Int { lock.lock(); defer { lock.unlock() }; return layouts }
    public var layoutMilliseconds: Double { lock.lock(); defer { lock.unlock() }; return Double(nanoseconds) / 1_000_000 }
    /// The widest a placement is laid out at when the system proposes no
    /// width (the ideal size): wider than any Island region or Lock Screen.
    public static let widest: CGFloat = 400

    /// Boots the activity's plan with `state` (the JSON the envelope
    /// carries; nil boots the bake's preview state). Throws when the plan
    /// cannot decode the state.
    public init(plan: Data, kind: String, state: Data?, facts: SessionFacts, assets: AssetResolver) throws {
        session = try HeadlessSession(plan: plan, entry: kind, props: state, size: CGSize(width: Self.widest, height: 1), facts: facts, assets: assets)
    }

    /// The same, with the entry's images under `assetRoot` (the generation's
    /// directory, or the bundle).
    public convenience init(plan: Data, kind: String, state: Data?, facts: SessionFacts, assetRoot: URL) throws {
        try self.init(plan: plan, kind: kind, state: state, facts: facts, assets: AssetResolver(root: assetRoot))
    }

    deinit { session.close() }

    /// The placement's root in a laid-out tree: the container's child at
    /// the placement's index (`ACTIVITY_PLACEMENTS` order).
    public static func root(of placement: ActivityPlacement, in snapshot: HeadlessSession.Snapshot) -> HeadlessSession.Node? {
        guard let container = snapshot.roots.first.flatMap({ snapshot.nodes[$0] }),
              placement.rawValue < container.children.count else { return nil }
        return snapshot.nodes[container.children[placement.rawValue]]
    }


    /// The placement laid out in a viewport `width` wide (the region's;
    /// `snapshot_viewport` reads it) and `height` tall, the system's
    /// proposal or, when it proposes none, the content's own height at that
    /// width. The margins and the width's authorship are read from that
    /// layout before the session is laid out again. The kernel's viewport is
    /// positive on both axes: an empty content is a point.
    public func layout(_ placement: ActivityPlacement, width: CGFloat, height: CGFloat?) throws -> Laid {
        let width = max(1, width)
        let key = "\(placement.rawValue)|\(width)|\(height.map { "\($0)" } ?? "content")"
        lock.lock(); defer { lock.unlock() }
        if let kept = laid[key] { return kept }
        func pass(_ viewport: CGSize) throws -> Laid {
            let start = DispatchTime.now().uptimeNanoseconds
            defer { layouts += 1; nanoseconds += DispatchTime.now().uptimeNanoseconds - start }
            let snapshot = try session.resize(to: viewport)
            guard let root = Self.root(of: placement, in: snapshot) else {
                throw HeadlessSession.Failure(description: "activity plan has no \(placement.name) placement")
            }
            let box = session.placementBox(root.id)
            return Laid(snapshot: snapshot, root: root, margins: box?.margins ?? (0, 0, 0, 0), authoredWidth: box?.authoredWidth ?? false, minimumWidth: box?.minimumWidth ?? 0, viewport: viewport)
        }
        var result = try pass(CGSize(width: width, height: max(1, height ?? 1)))
        if height == nil, result.size.height > 0, abs(result.size.height - result.viewport.height) > 0.5 {
            result = try pass(CGSize(width: width, height: result.size.height))
        }
        laid[key] = result
        return result
    }

    /// The size the placement takes for a proposal (a `Layout`'s
    /// `sizeThatFits`), and the layout its view draws: a region's width
    /// when the system proposes one and the placement fills it; else the
    /// root's margin box as the layout in that region gave it (its
    /// shrink-to-fit width, an authored `width` or `min-width` past the
    /// region kept, a content the region cannot hold cut at the region), a
    /// paragraph the region wrapped settling to its widest ink line; the
    /// content's height, margins included. A zero proposal is the minimum,
    /// which is nothing; an unspecified or infinite width is the ideal: the
    /// content's own.
    public struct Measurement {
        public let size: CGSize
        public let laid: Laid?
    }

    public func size(_ placement: ActivityPlacement, proposal: ProposedViewSize) -> CGSize {
        measure(placement, proposal: proposal).size
    }

    public func measure(_ placement: ActivityPlacement, proposal: ProposedViewSize) -> Measurement {
        if proposal.width == 0 || proposal.height == 0 { return Measurement(size: .zero, laid: nil) }
        var width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? Self.widest
        let height = proposal.height.flatMap { $0.isFinite ? $0 : nil }
        guard var laid = try? layout(placement, width: width, height: height) else { return Measurement(size: CGSize(width: width, height: 0), laid: nil) }
        var reported = CGSize(width: width, height: laid.size.height)
        if !placement.fillsWidth {
            // A region that wrapped a paragraph is as wide as the paragraph's
            // widest ink line, as SwiftUI's `Text` reports its own: the
            // region is laid out again that wide, until the width settles.
            for _ in 0..<8 {
                guard !laid.authoredWidth, laid.minimumWidth < width, laid.size.width >= width - 0.5, Self.wraps(under: laid.root, in: laid.snapshot) else { break }
                let ink = max(laid.minimumWidth + laid.margins.left + laid.margins.right, (Self.widestLine(under: laid.root, in: laid.snapshot) + laid.margins.left + laid.margins.right).rounded(.up))
                guard ink > 0, ink < width - 0.5, let narrower = try? layout(placement, width: ink, height: height) else { break }
                width = ink; laid = narrower
            }
            let used = laid.size.width
            reported.width = laid.authoredWidth ? used : max(laid.minimumWidth + laid.margins.left + laid.margins.right, min(width, used))
            reported.height = laid.size.height
        }
        return Measurement(size: reported, laid: laid)
    }

    public func laidOut(_ placement: ActivityPlacement, proposal: ProposedViewSize) -> Laid? {
        measure(placement, proposal: proposal).laid
    }

    /// Whether a paragraph under `node` broke into lines.
    package static func wraps(under node: HeadlessSession.Node, in tree: HeadlessSession.Snapshot) -> Bool {
        var stack = [node.id]
        while let id = stack.popLast() {
            guard let n = tree.nodes[id], n.style["display"]?.string != "none" else { continue }
            if (n.kind == "text" || n.kind == "time") && n.lines.count > 1 { return true }
            stack.append(contentsOf: n.children)
        }
        return false
    }

    /// The widest ink under `node`, in the node's frame coordinates: each
    /// text's widest line by its typographic width (where it was aligned
    /// in its box says nothing of the ink), from the box's content edge,
    /// plus the box's trailing padding and border and those of the boxes
    /// above it.
    package static func widestLine(under node: HeadlessSession.Node, in tree: HeadlessSession.Snapshot) -> CGFloat {
        var widest: CGFloat = 0
        func visit(_ node: HeadlessSession.Node, x: CGFloat, trailing: CGFloat) {
            let after = trailing + (node.frame.width - node.contentBox.maxX)
            if node.kind == "text" || node.kind == "time" {
                let width = node.lines.map(\.width).max() ?? 0
                if width > 0 { widest = max(widest, x + node.contentBox.minX + width + after) }
            }
            for id in node.children {
                guard let child = tree.nodes[id], child.style["display"]?.string != "none" else { continue }
                visit(child, x: x + child.frame.minX, trailing: after)
            }
        }
        visit(node, x: 0, trailing: 0)
        return widest
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public enum SnapshotActivity {
    /// The view the system archives for one placement: sized by the
    /// render for the region's proposal, then the placement's tree as that
    /// layout placed it. The Lock Screen placement's root background is the
    /// activity's (`background(of:)`), never drawn twice; the other
    /// placements draw their own.
    public static func view(_ render: ActivityRender, placement: ActivityPlacement, environment: SnapshotEnvironment) -> ActivityPlacementView {
        ActivityPlacementView(render: render, placement: placement, environment: environment)
    }

    /// D5: the Lock Screen placement's root background, from the placement
    /// as laid out: a `background-color` is the system's
    /// `activityBackgroundTint` (which it extends to the whole screen in
    /// StandBy), a `background-image`'s gradients paint full bleed behind
    /// the margins over that tint, as a widget's container background does;
    /// both unsaid leaves the system's material.
    public static func background(of laid: ActivityRender.Laid, dark: Bool) -> ActivityBackground {
        let style = SnapshotStyle(laid.root)
        let tint = SnapshotColor.color(style.color("background_color"), dark: dark)
        let gradients = style.gradients("background_image").isEmpty ? nil : SnapshotPaint(style: style, dark: dark)
        return ActivityBackground(tint: tint, gradients: gradients)
    }

    /// The URL a placement opens (D7): its first link, in document order.
    public static func url(_ laid: ActivityRender.Laid, environment: SnapshotEnvironment) -> URL? {
        SnapshotDriver.url(laid.snapshot, from: [laid.root.id], environment: environment)
    }
}

/// A Lock Screen placement's background (D5): the colour as the system's
/// tint, the gradients the view paints over it.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct ActivityBackground {
    public let tint: Color?
    let gradients: SnapshotPaint?
    public var hasGradients: Bool { gradients != nil }
}

/// A placement: a `Layout` answers the system's proposals from the render
/// (the content's size in the region), and the content inside is the
/// placement's tree as the layout that size was measured from placed it,
/// the root at its own margins. The Lock Screen presentation sits inside
/// the content margins (D5): the system's where it gives them, else the
/// standard layout margin for Live Activities on the Lock Screen, 14 points
/// (Human Interface Guidelines, Live Activities, "Use standard margins to
/// align your design with notifications"), since the system proposes the
/// card's whole width to the view; its root's gradients paint full bleed
/// behind them from the same layout, and its colour is the system's tint.
/// The content is account content (D8), private but for a node that says
/// `data-privacy="public"`; the placeholder carries none.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct ActivityPlacementView: View {
    let render: ActivityRender
    let placement: ActivityPlacement
    let environment: SnapshotEnvironment
    @Environment(\.widgetContentMargins) private var systemMargins
    @Environment(\.colorScheme) private var scheme

    public static let platformMargins = EdgeInsets(top: 14, leading: 14, bottom: 14, trailing: 14)

    private var margins: EdgeInsets {
        guard placement.hasMargins else { return EdgeInsets() }
        if let margins = environment.contentMargins { return margins }
        let given = [systemMargins.top, systemMargins.leading, systemMargins.bottom, systemMargins.trailing].contains { $0 > 0 }
        return given ? systemMargins : Self.platformMargins
    }

    public var body: some View {
        let margins = margins
        PlacementLayout(render: render, placement: placement) {
            GeometryReader { proxy in
                if let laid = try? render.layout(placement, width: proxy.size.width, height: proxy.size.height) {
                    let background = placement.hasMargins ? SnapshotActivity.background(of: laid, dark: scheme == .dark) : ActivityBackground(tint: nil, gradients: nil)
                    let bleed = CGRect(x: -margins.leading, y: -margins.top, width: proxy.size.width + margins.leading + margins.trailing, height: proxy.size.height + margins.top + margins.bottom)
                    let frame = CGRect(x: laid.root.frame.minX, y: laid.root.frame.minY, width: laid.root.frame.width, height: laid.root.frame.height)
                    FrameLayout(frames: [bleed, frame], size: proxy.size) {
                        if let paint = background.gradients { paint.gradients(in: CGRect(origin: .zero, size: bleed.size)) } else { Color.clear }
                        SnapshotNodeView(node: laid.root, tree: laid.snapshot, environment: environment, isRoot: placement.hasMargins)
                    }
                    .clipShape(RegionClip(size: CGSize(width: laid.authoredWidth ? laid.size.width : max(laid.minimumWidth, min(proxy.size.width, laid.size.width)), height: laid.size.height), margins: margins))
                    .widgetURL(SnapshotActivity.url(laid, environment: environment))
                    .modifier(Tinted(color: background.tint))
                }
            }
        }
        .padding(margins)
    }
}

private struct RegionClip: Shape {
    let size: CGSize
    let margins: EdgeInsets
    func path(in rect: CGRect) -> Path {
        Path(CGRect(x: -margins.leading, y: -margins.top,
                    width: size.width + margins.leading + margins.trailing,
                    height: size.height + margins.top + margins.bottom))
    }
}

/// The system's Lock Screen tint (iOS; the other platforms have no Live
/// Activity of their own), set from the placement as laid out.
private struct Tinted: ViewModifier {
    let color: Color?
    func body(content: Content) -> some View {
        #if os(iOS)
        content.activityBackgroundTint(color)
        #else
        content
        #endif
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct PlacementLayout: Layout {
    let render: ActivityRender
    let placement: ActivityPlacement

    struct Cache { var measurement: ActivityRender.Measurement? }
    func makeCache(subviews: Subviews) -> Cache { Cache() }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Cache) -> CGSize {
        let measured = render.measure(placement, proposal: proposal)
        cache.measurement = measured
        return measured.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout Cache) {
        for subview in subviews {
            let measured = cache.measurement ?? render.measure(placement, proposal: proposal)
            let viewport = measured.laid?.viewport ?? .zero
            subview.place(at: bounds.origin, anchor: .topLeading, proposal: ProposedViewSize(viewport))
        }
    }
}
#endif
