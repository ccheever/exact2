// @ref LLP 1126.001 WP10 — a Live Activity's placements through the snapshot
// driver (D4, D5): one headless session per activity, each placement laid
// out on its own at the width the system gives its region, its height the
// content's, drawn as a widget's entry is. The Lock Screen's background is
// the placement's root background; the Island's keyline and region layout
// are the system's.
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
    /// (the Lock Screen, the expanded bottom) or as wide as its content:
    /// the compact regions, the minimal view and the expanded leading,
    /// trailing and centre regions, which the Island sizes to what they
    /// hold ("the system first determines the width of the center content
    /// while taking into account the minimal width of the leading and
    /// trailing content", ActivityKit, Displaying live data with Live
    /// Activities); a region that takes its whole proposal leaves the
    /// Island nothing to place the others in.
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
/// resized for each placement's width; the layouts are kept by size, so the
/// archive's repeated proposals cost one layout each.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public final class ActivityRender {
    /// A placement laid out: the tree, the placement's root in it (its
    /// border box, at its own left margin in the container), the margins
    /// the root resolved, and the viewport it was laid out in (the region's
    /// size, what `snapshot_viewport` read).
    public struct Laid {
        public let snapshot: HeadlessSession.Snapshot
        public let root: HeadlessSession.Node
        public let margins: (left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat)
        public let viewport: CGSize
        /// The placement's margin box: what its region holds.
        public var size: CGSize { CGSize(width: root.frame.width + margins.left + margins.right, height: root.frame.height + margins.top + margins.bottom) }
    }
    public let session: HeadlessSession
    private let lock = NSLock()
    private var laid: [String: Laid] = [:]
    private var fitted: [String: CGFloat] = [:]
    private var layouts = 0
    private var nanoseconds: UInt64 = 0
    /// How many layouts this state has run (the kernel's trials counted),
    /// and their time: the render's cost per push (LLP 1126.001 §5 Q4),
    /// which the extension reports.
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

    private func timed<T>(_ body: () throws -> T) rethrows -> T {
        let start = DispatchTime.now().uptimeNanoseconds
        defer { layouts += 1; nanoseconds += DispatchTime.now().uptimeNanoseconds - start }
        return try body()
    }

    /// The placement laid out in a region `width` wide (its margin box; the
    /// border box is narrower by the margins the kernel resolves), its
    /// height the content's: the session's viewport is the region's size
    /// (`snapshot_viewport` reads it), with `height` the system's proposal
    /// or, when it proposes none, the content's own height at that width.
    /// The kernel's viewport is positive on both axes: an empty content is
    /// a point.
    public func layout(_ placement: ActivityPlacement, width: CGFloat, height: CGFloat?) throws -> Laid {
        let width = max(1, width)
        let key = "\(placement.rawValue)|\(width)|\(height.map { "\($0)" } ?? "content")"
        lock.lock(); defer { lock.unlock() }
        if let kept = laid[key] { return kept }
        var viewport = CGSize(width: width, height: max(1, height ?? 1))
        var snapshot = try timed { try session.resize(to: viewport) }
        guard var root = Self.root(of: placement, in: snapshot) else {
            throw HeadlessSession.Failure(description: "activity plan has no \(placement.name) placement")
        }
        // The margins are the kernel's (the batch carries none): a trial of
        // the root alone in a row of the region's width resolves them.
        let box = timed { session.fitContentBox(root.id, available: width) }
        let margins: (left: CGFloat, top: CGFloat, right: CGFloat, bottom: CGFloat) = box?.margins ?? (0, 0, 0, 0)
        let outerHeight = root.frame.height + margins.top + margins.bottom
        if height == nil, outerHeight > 0, abs(outerHeight - viewport.height) > 0.5 {
            viewport.height = outerHeight
            snapshot = try timed { try session.resize(to: viewport) }
            root = Self.root(of: placement, in: snapshot) ?? root
        }
        let result = Laid(snapshot: snapshot, root: root, margins: margins, viewport: viewport)
        laid[key] = result
        return result
    }

    /// The width a placement's content asks for in a region at most `bound`
    /// wide: the kernel's shrink-to-fit box (`HeadlessSession.fitContentBox`:
    /// the root laid out alone as a flex item in a row that wide, so its
    /// width is its max-content size clamped to the row, an authored width
    /// kept as written, limits and percentages resolved against the
    /// bound), with its margins; a content the row cannot hold takes the
    /// row. A paragraph that wraps to the row is then as wide as its widest
    /// line, as SwiftUI's `Text` reports its own: the trial runs again at
    /// the lines' width until it settles.
    public func fittedWidth(_ placement: ActivityPlacement, bound: CGFloat) -> CGFloat {
        let bound = max(1, min(bound, Self.widest))
        let key = "\(placement.rawValue)|\(bound)"
        lock.lock()
        if let kept = fitted[key] { lock.unlock(); return kept }
        lock.unlock()
        var width = bound
        for _ in 0..<3 {
            guard let laid = try? layout(placement, width: width, height: nil) else { break }
            lock.lock()
            let box = timed { session.fitContentBox(laid.root.id, available: width) }
            lock.unlock()
            guard let box else { break }
            let outer = box.outerWidth.rounded(.up)
            // Past the row: an authored width is kept; a content the row
            // cannot hold (an unbreakable word, a `time`'s reserved value)
            // takes the row, as a `Text` would, and is cut there.
            if outer > width + 0.5 { if box.authoredWidth { width = outer }; break }
            if outer < width - 0.5 { width = max(1, outer); break }
            // The row clamped the content: a paragraph that wrapped is as
            // wide as its widest line (a line the author clamped, or a box
            // the author sized to the row, keeps the row's width).
            guard Self.wraps(under: laid.root, in: laid.snapshot) else { break }
            let widest = (Self.widestLine(under: laid.root, in: laid.snapshot) + box.margins.left + box.margins.right).rounded(.up)
            guard widest > 0, widest < width - 0.5 else { break }
            width = widest
        }
        lock.lock(); fitted[key] = width; lock.unlock()
        return width
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

    /// The right edge the text lines under `node` reach, in the node's
    /// frame coordinates: a wrapped paragraph's widest line.
    package static func widestLine(under node: HeadlessSession.Node, in tree: HeadlessSession.Snapshot) -> CGFloat {
        var widest: CGFloat = 0
        func visit(_ node: HeadlessSession.Node, x: CGFloat) {
            if node.kind == "text" || node.kind == "time" {
                let box = node.contentBox
                for line in node.lines { widest = max(widest, x + box.minX + line.origin.x + line.width) }
            }
            for id in node.children {
                guard let child = tree.nodes[id], child.style["display"]?.string != "none" else { continue }
                visit(child, x: x + child.frame.minX)
            }
        }
        visit(node, x: 0)
        return widest + (node.frame.width - node.contentBox.maxX)
    }

    /// The size the placement takes for a proposal (a `Layout`'s
    /// `sizeThatFits`): a region's width when the system proposes one and
    /// the placement fills it, else the content's; the content's height,
    /// margins included. A zero proposal is the minimum, which is nothing;
    /// an unspecified or infinite width is the ideal: the content's own.
    public func size(_ placement: ActivityPlacement, proposal: ProposedViewSize) -> CGSize {
        if proposal.width == 0 || proposal.height == 0 { return .zero }
        let width: CGFloat
        if let proposed = proposal.width, proposed.isFinite {
            width = placement.fillsWidth ? proposed : fittedWidth(placement, bound: proposed)
        } else {
            width = fittedWidth(placement, bound: Self.widest)
        }
        let height = proposal.height.flatMap { $0.isFinite ? $0 : nil }
        guard let laid = try? layout(placement, width: width, height: height) else { return CGSize(width: width, height: 0) }
        return CGSize(width: width, height: laid.size.height)
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public enum SnapshotActivity {
    /// The view the system archives for one placement: sized by the
    /// render for the region's proposal, then the placement's tree at that
    /// size. The Lock Screen placement's root background is the activity's
    /// (`background(of:)`), never drawn twice; the other placements draw
    /// their own.
    public static func view(_ render: ActivityRender, placement: ActivityPlacement, environment: SnapshotEnvironment) -> ActivityPlacementView {
        ActivityPlacementView(render: render, placement: placement, environment: environment)
    }

    /// D5: the Lock Screen placement's root background, from the placement
    /// as laid out: a `background-color` is the system's
    /// `activityBackgroundTint` (which it extends to the whole screen in
    /// StandBy), a `background-image`'s gradients paint full bleed behind
    /// the margins, as a widget's container background does; both unsaid
    /// leaves the system's material.
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
/// tint, the gradients the view paints full bleed.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct ActivityBackground {
    public let tint: Color?
    let gradients: SnapshotPaint?
    public var hasGradients: Bool { gradients != nil }
}

/// A placement: a `Layout` answers the system's proposals from the render
/// (the content's size at the region's width), and the content inside is
/// the placement's tree at the size the layout gave it, the root at its
/// own margins. The Lock Screen presentation sits inside the content
/// margins (D5): the system's where it gives them, else the standard
/// layout margin for Live Activities on the Lock Screen, 14 points (Human
/// Interface Guidelines, Live Activities, "Use standard margins to align
/// your design with notifications"), since the system proposes the card's
/// whole width to the view; its root's gradients paint full bleed behind
/// them and its colour is the system's tint.
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
        placed
            .padding(margins)
            .background {
                if placement.hasMargins {
                    GeometryReader { proxy in
                        let inner = max(1, proxy.size.width - margins.leading - margins.trailing)
                        if let laid = try? render.layout(placement, width: inner, height: nil),
                           let paint = SnapshotActivity.background(of: laid, dark: scheme == .dark).gradients {
                            let bounds = CGRect(origin: .zero, size: proxy.size)
                            paint.fill(in: bounds, positioned: bounds, rounded: false)
                        }
                    }
                }
            }
    }

    private var placed: some View {
        PlacementLayout(render: render, placement: placement) {
            GeometryReader { proxy in
                if let laid = try? render.layout(placement, width: proxy.size.width, height: proxy.size.height) {
                    let roots = [laid.root]
                    let frame = CGRect(x: laid.margins.left, y: laid.margins.top, width: laid.root.frame.width, height: laid.root.frame.height)
                    FrameLayout(frames: [frame], size: proxy.size) {
                        ForEach(roots, id: \.id) { SnapshotNodeView(node: $0, tree: laid.snapshot, environment: environment, isRoot: placement.hasMargins) }
                    }
                    .widgetURL(SnapshotActivity.url(laid, environment: environment))
                    .modifier(Tinted(color: placement.hasMargins ? SnapshotActivity.background(of: laid, dark: scheme == .dark).tint : nil))
                }
            }
        }
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

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        render.size(placement, proposal: proposal)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for subview in subviews {
            subview.place(at: bounds.origin, anchor: .topLeading, proposal: ProposedViewSize(bounds.size))
        }
    }
}
#endif
