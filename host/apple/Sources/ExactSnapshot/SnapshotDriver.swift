// @ref LLP 1126 D4, D5 — the snapshot driver: SwiftUI over a headless
// session's node model, for WidgetKit's archive. Layout is the kernel's
// (fixed frames, its line breaks); what the author left unsaid is the
// platform's (the container background, the content margins, the rendering
// modes by role, accessibility, redaction, live time).
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore

/// What the system supplies around one render: the family, whether this is
/// the placeholder (redacted by the system; nothing in it is account
/// content, and it opens nothing), where a route's path resolves, and where
/// the entry's images live (the bundle or the app group's generation; never
/// fetched in the render).
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct SnapshotEnvironment {
    public enum Family: String, Sendable {
        case systemSmall, systemMedium, systemLarge, systemExtraLarge
        case accessoryRectangular, accessoryCircular, accessoryInline, accessoryCorner
        /// The Home Screen families, whose container background the system
        /// draws; the accessory families have none.
        public var isSystem: Bool { rawValue.hasPrefix("system") }
    }
    public var family: Family
    public var placeholder: Bool
    /// The app's origin or scheme a route path (`/prompt/3`) is a URL under.
    public var linkBase: URL?
    public var assetRoot: URL

    public init(family: Family, placeholder: Bool = false, linkBase: URL? = nil, assetRoot: URL = Bundle.main.bundleURL) {
        self.family = family; self.placeholder = placeholder; self.linkBase = linkBase; self.assetRoot = assetRoot
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public enum SnapshotDriver {
    /// Apple's body size at a text size (`kernel/tables/schema.json`'s
    /// `textStyleBodies`, iOS's Dynamic Type ramp).
    public static func bodySize(for size: DynamicTypeSize) -> CGFloat {
        switch size {
        case .xSmall: return 14
        case .small: return 15
        case .medium: return 16
        case .large: return 17
        case .xLarge: return 19
        case .xxLarge: return 21
        case .xxxLarge: return 23
        case .accessibility1: return 28
        case .accessibility2: return 33
        case .accessibility3: return 40
        case .accessibility4: return 47
        case .accessibility5: return 53
        @unknown default: return 17
        }
    }

    /// The root font size the kernel sizes a text style's ramp and an unsaid
    /// `font-size` from, at the reader's text size: the ramp's body on iOS,
    /// and on the Mac its 13 pt body scaled by the ramp.
    public static func rootFontSize(for size: DynamicTypeSize) -> CGFloat {
        #if os(macOS)
        return 13 * bodySize(for: size) / 17
        #else
        return bodySize(for: size)
        #endif
    }

    /// Lay the entry out at the content size the system proposes (its
    /// `displaySize` less its content margins) and the reader's text size,
    /// and return the view WidgetKit archives.
    public static func render(_ session: HeadlessSession, size: CGSize, dynamicType: DynamicTypeSize = .large,
                              environment: SnapshotEnvironment) throws -> SnapshotView {
        try render(session, size: size, rootFontSize: rootFontSize(for: dynamicType), environment: environment)
    }

    /// The same at an explicit root font size (a harness emulating another
    /// platform's ramp).
    public static func render(_ session: HeadlessSession, size: CGSize, rootFontSize: CGFloat,
                              environment: SnapshotEnvironment) throws -> SnapshotView {
        try session.setRootFontSize(rootFontSize)
        return SnapshotView(snapshot: try session.resize(to: size), environment: environment)
    }

    /// The view of a tree already laid out at its size.
    public static func view(_ snapshot: HeadlessSession.Snapshot, environment: SnapshotEnvironment) -> SnapshotView {
        SnapshotView(snapshot: snapshot, environment: environment)
    }

    /// The URL a snapshot opens: the first link's, in document order.
    public static func url(_ snapshot: HeadlessSession.Snapshot, environment: SnapshotEnvironment) -> URL? {
        guard !environment.placeholder else { return nil }
        var stack = Array(snapshot.roots.reversed())
        while let id = stack.popLast() {
            guard let node = snapshot.nodes[id] else { continue }
            if let url = SnapshotLinks.url(node.props["href"], base: environment.linkBase) { return url }
            stack.append(contentsOf: node.children.reversed())
        }
        return nil
    }
}

/// The entry's view: its roots at their frames, the root's background as
/// the container's, account content private, the first link as the URL.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct SnapshotView: View {
    let snapshot: HeadlessSession.Snapshot
    let environment: SnapshotEnvironment

    public var body: some View {
        let roots = snapshot.roots.compactMap { snapshot.nodes[$0] }
        FrameLayout(frames: roots.map(\.frame), size: snapshot.size) {
            ForEach(roots, id: \.id) { SnapshotNodeView(node: $0, tree: snapshot, environment: environment, isRoot: true) }
        }
        .frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
        // D8: account content by default. A `privacySensitive` modifier makes
        // its subtree ignore placeholder redaction (stage 0), so the
        // placeholder carries none.
        .privacySensitive(!environment.placeholder)
        .widgetURL(SnapshotDriver.url(snapshot, environment: environment))
        .containerBackground(for: .widget) { ContainerBackground(root: roots.first) }
    }
}

/// D5: the root's background is the widget's, which the system draws and
/// removes where widgets have none (StandBy, the Lock Screen, the accented
/// mode). Unsaid, it is the system's background.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct ContainerBackground: View {
    let root: HeadlessSession.Node?
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let paint = root.map { SnapshotPaint(style: SnapshotStyle($0), dark: scheme == .dark) }
        if let paint, paint.hasBackground {
            paint.fill(in: CGRect(origin: .zero, size: root?.frame.size ?? .zero))
        } else {
            Rectangle().fill(.background)
        }
    }
}

/// Places each subview at the kernel's frame for it, in the parent's
/// coordinate space (stage 0: a custom `Layout` survives the archive).
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct FrameLayout: Layout {
    let frames: [CGRect]
    let size: CGSize

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize { size }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (index, subview) in subviews.enumerated() {
            let frame = index < frames.count ? frames[index] : .zero
            subview.place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                          anchor: .topLeading, proposal: ProposedViewSize(frame.size))
        }
    }
}
#endif
