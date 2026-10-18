// @ref LLP 1126.001 WP10 — a Live Activity's placements through the snapshot
// driver (D4, D5): one headless session per activity, each placement laid
// out on its own at the width the system gives its region, its height the
// content's, drawn as a widget's entry is. The Lock Screen's background tint
// is the placement's root background; the Island's keyline and region
// layout are the system's.
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import CoreText
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
    /// hold (a leading region that takes the whole proposal leaves the
    /// Island nothing to place the others in, and they are dropped).
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
    /// A placement laid out: the tree, the placement's root in it, and the
    /// viewport it was laid out in.
    public struct Laid {
        public let snapshot: HeadlessSession.Snapshot
        public let root: HeadlessSession.Node
        public let viewport: CGSize
    }
    public let session: HeadlessSession
    private let lock = NSLock()
    private var laid: [String: Laid] = [:]
    private var fitted: [String: CGFloat] = [:]
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

    /// The placement laid out at `width`, its height the content's: the
    /// session's viewport is the region's size (`snapshot_viewport` reads
    /// it), with `height` the system's proposal or, when it proposes none,
    /// the content's own height at that width. The kernel's viewport is
    /// positive on both axes: an empty content is a point.
    public func layout(_ placement: ActivityPlacement, width: CGFloat, height: CGFloat?) throws -> Laid {
        let width = max(1, width)
        let key = "\(placement.rawValue)|\(width)|\(height.map { "\($0)" } ?? "content")"
        lock.lock(); defer { lock.unlock() }
        if let kept = laid[key] { return kept }
        let start = DispatchTime.now().uptimeNanoseconds
        defer { layouts += 1; nanoseconds += DispatchTime.now().uptimeNanoseconds - start }
        var viewport = CGSize(width: width, height: max(1, height ?? 1))
        var snapshot = try session.resize(to: viewport)
        guard var root = Self.root(of: placement, in: snapshot) else {
            throw HeadlessSession.Failure(description: "activity plan has no \(placement.name) placement")
        }
        if height == nil, root.frame.height > 0, root.frame.height != viewport.height {
            viewport.height = root.frame.height
            snapshot = try session.resize(to: viewport)
            root = Self.root(of: placement, in: snapshot) ?? root
            layouts += 1
        }
        let result = Laid(snapshot: snapshot, root: root, viewport: viewport)
        laid[key] = result
        return result
    }

    /// The width a placement's content asks for, at most `bound`: a
    /// shrink-to-fit over the leaves (a text's widest line, a `time`'s
    /// widest reserved value, an image's box) laid out at `bound`, then
    /// again at that width until it settles, so a block that fills its
    /// line and a centred line both come to their content's width.
    public func fittedWidth(_ placement: ActivityPlacement, bound: CGFloat) -> CGFloat {
        let bound = max(0, min(bound, Self.widest))
        let key = "\(placement.rawValue)|\(bound)"
        lock.lock()
        if let kept = fitted[key] { lock.unlock(); return kept }
        lock.unlock()
        var width = bound
        var wider: HeadlessSession.Snapshot?
        for _ in 0..<4 {
            guard let laid = try? layout(placement, width: width, height: nil) else { break }
            let extent = Self.extent(of: laid.root, in: laid.snapshot, available: laid.viewport.width, wider: wider).rounded(.up)
            guard extent < width - 0.5 else { break }
            width = max(0, extent)
            wider = laid.snapshot
        }
        lock.lock(); fitted[key] = width; lock.unlock()
        return width
    }

    /// The size the placement takes for a proposal (a `Layout`'s
    /// `sizeThatFits`): a region's width when the system proposes one and
    /// the placement fills it, else the content's; the content's height.
    /// A zero proposal is the minimum, which is nothing; an unspecified or
    /// infinite width is the ideal: the content's own.
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
        return CGSize(width: width, height: laid.root.frame.height)
    }

    /// How far right a node's content reaches, in its own frame's
    /// coordinates, from its leaves. A box narrower than the width it had
    /// (`available`, its parent's content width), or as wide as it was in a
    /// `wider` layout, was sized by the author or by its own content in a
    /// row, and is taken as it is; one that fills its width is measured by
    /// what it holds.
    static func extent(of node: HeadlessSession.Node, in tree: HeadlessSession.Snapshot, available: CGFloat, wider: HeadlessSession.Snapshot? = nil) -> CGFloat {
        let fixed = wider?.nodes[node.id].map { abs($0.frame.width - node.frame.width) < 0.5 } ?? false
        if fixed || node.frame.width < available - 0.5 || node.kind == "image" || node.props["type"] == "progress" { return node.frame.width }
        let insets = node.frame.width - node.contentBox.maxX
        switch node.kind {
        case "text", "time":
            var widest = node.lines.map(\.width).max() ?? 0
            if let reserved = node.props["textReservation"], let font = node.runs.first?.font {
                for candidate in reserved.split(separator: "\n") where !candidate.isEmpty {
                    let line = CTLineCreateWithAttributedString(NSAttributedString(string: String(candidate), attributes: [.font: font]))
                    widest = max(widest, CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                }
            }
            return node.contentBox.minX + widest + insets
        default:
            let children = node.children.compactMap { tree.nodes[$0] }.filter { $0.style["display"]?.string != "none" }
            let inner = children.map { $0.frame.minX + extent(of: $0, in: tree, available: node.contentBox.width, wider: wider) }.max() ?? node.contentBox.minX
            return max(inner, node.contentBox.minX) + insets
        }
    }
}

public enum SnapshotActivity {
    /// The view the system archives for one placement: sized by the
    /// render for the region's proposal, then the placement's tree at that
    /// size. The Lock Screen placement's root background is the activity's
    /// tint (`tint(_:)`), never drawn twice; the other placements draw
    /// their own.
    public static func view(_ render: ActivityRender, placement: ActivityPlacement, environment: SnapshotEnvironment) -> ActivityPlacementView {
        ActivityPlacementView(render: render, placement: placement, environment: environment)
    }

    /// D5: the Lock Screen placement's root background, as the system's
    /// `activityBackgroundTint`; a gradient or an unsaid background leaves
    /// the system's material.
    public static func tint(_ render: ActivityRender, dark: Bool) -> Color? {
        guard let laid = try? render.layout(.lockScreen, width: ActivityRender.widest, height: nil) else { return nil }
        let style = SnapshotStyle(laid.root)
        guard style.gradients("background_image").isEmpty else { return nil }
        return SnapshotColor.color(style.color("background_color"), dark: dark)
    }

    /// The URL a placement opens (D7): its first link, in document order.
    public static func url(_ laid: ActivityRender.Laid, environment: SnapshotEnvironment) -> URL? {
        SnapshotDriver.url(laid.snapshot, from: [laid.root.id], environment: environment)
    }
}

/// A placement: a `Layout` answers the system's proposals from the render
/// (the content's size at the region's width), and the content inside is
/// the placement's tree at the size the layout gave it. The Lock Screen
/// presentation sits inside the content margins (D5): the system's where it
/// gives them, else the platform's own 16 pt, since the system proposes
/// the card's whole width to the view and a hand-built activity pads.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct ActivityPlacementView: View {
    let render: ActivityRender
    let placement: ActivityPlacement
    let environment: SnapshotEnvironment
    @Environment(\.widgetContentMargins) private var systemMargins

    public static let platformMargins = EdgeInsets(top: 16, leading: 16, bottom: 16, trailing: 16)

    private var margins: EdgeInsets {
        guard placement.hasMargins else { return EdgeInsets() }
        if let margins = environment.contentMargins { return margins }
        let given = [systemMargins.top, systemMargins.leading, systemMargins.bottom, systemMargins.trailing].contains { $0 > 0 }
        return given ? systemMargins : Self.platformMargins
    }

    public var body: some View {
        placed.padding(margins)
    }

    private var placed: some View {
        PlacementLayout(render: render, placement: placement) {
            GeometryReader { proxy in
                if let laid = try? render.layout(placement, width: proxy.size.width, height: proxy.size.height) {
                    let roots = [laid.root]
                    FrameLayout(frames: [CGRect(origin: .zero, size: laid.root.frame.size)], size: proxy.size) {
                        ForEach(roots, id: \.id) { SnapshotNodeView(node: $0, tree: laid.snapshot, environment: environment, isRoot: placement == .lockScreen) }
                    }
                    .privacySensitive(!environment.placeholder)
                    .widgetURL(SnapshotActivity.url(laid, environment: environment))
                }
            }
        }
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
