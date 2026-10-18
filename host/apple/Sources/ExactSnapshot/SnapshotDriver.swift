// @ref LLP 1126 D4, D5 — the snapshot driver: SwiftUI over a headless
// session's node model, for WidgetKit's archive. Layout is the kernel's
// (fixed frames, its line breaks); what the author left unsaid is the
// platform's (the container background, the content margins, the rendering
// modes by role, accessibility, redaction, live time).
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore
import os

/// What the system supplies around one render: the family, whether this is
/// the placeholder (redacted by the system; nothing in it is account
/// content, and it opens nothing), where a route's path resolves, where the
/// entry's images live (the bundle, the app group's generation; never
/// fetched in the render), and, for a harness outside WidgetKit, the
/// content margins and root font size the view would otherwise read from
/// its environment.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct SnapshotEnvironment {
    public enum Family: String, Sendable {
        case systemSmall, systemMedium, systemLarge, systemExtraLarge
        case accessoryRectangular, accessoryCircular, accessoryInline, accessoryCorner
        /// The Home Screen families, whose container background the system
        /// draws; the accessory families have none.
        public var isSystem: Bool { rawValue.hasPrefix("system") }
    }
    public var entryName: String
    public var family: Family
    public var placeholder: Bool
    /// The app's origin or scheme a route path (`/prompt/3`) is a URL under.
    public var linkBase: URL?
    /// Where a bundle-relative `image` name resolves, in order; a path stays
    /// inside one.
    public var imageRoots: [URL]
    /// The app group's container, where `app:/data/…`, `app:/cache/…` and
    /// `app:/tmp/…` resolve (the app's dirs, shared with its extensions).
    public var appGroup: URL?
    /// The invocation's decoded rasters: budgeted, released with it.
    public var rasters: SnapshotRasters
    /// The system's `widgetContentMargins` when nil.
    public var contentMargins: EdgeInsets?
    /// The kernel's root font size; the environment's text size when nil.
    public var rootFontSize: CGFloat?
    /// The timeline entry's date: where a `countdown` starts from.
    public var entryDate: Date
    /// An agent read freezes live leaves at its clock; WidgetKit leaves this nil.
    public var frozenDate: Date?

    public init(family: Family, entryName: String = "snapshot", placeholder: Bool = false, linkBase: URL? = nil, imageRoots: [URL] = [Bundle.main.bundleURL], appGroup: URL? = nil,
                rasters: SnapshotRasters = SnapshotRasters(), contentMargins: EdgeInsets? = nil, rootFontSize: CGFloat? = nil, entryDate: Date = .now, frozenDate: Date? = nil) {
        self.entryName = entryName; self.family = family; self.placeholder = placeholder; self.linkBase = linkBase; self.imageRoots = imageRoots; self.appGroup = appGroup
        self.rasters = rasters; self.contentMargins = contentMargins; self.rootFontSize = rootFontSize; self.entryDate = entryDate; self.frozenDate = frozenDate
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
        #elseif os(watchOS)
        return 16 * bodySize(for: size) / 17
        #else
        return bodySize(for: size)
        #endif
    }

    /// The view WidgetKit archives for an entry: the session is laid out
    /// once, in the view, at the widget's `displaySize` less the system's
    /// content margins and at the reader's text size, both read from the
    /// view's environment. The widget's configuration must say
    /// `.contentMarginsDisabled()`: the view applies the margins itself,
    /// once, around the laid-out entry.
    public static func view(session: HeadlessSession, displaySize: CGSize, environment: SnapshotEnvironment) -> SnapshotView {
        SnapshotView(source: .session(session, displaySize), environment: environment)
    }

    /// The view of a tree already laid out at its size, which the system's
    /// content margins (or the environment's) surround.
    public static func view(_ snapshot: HeadlessSession.Snapshot, environment: SnapshotEnvironment) -> SnapshotView {
        SnapshotView(source: .snapshot(snapshot), environment: environment)
    }

    /// A paragraph's styled runs remain one leaf. Hidden subtrees contribute none.
    package static func admitsInline(_ snapshot: HeadlessSession.Snapshot) -> Bool {
        var stack = snapshot.roots, text = 0, images = 0, other = 0
        while let id = stack.popLast() {
            guard let node = snapshot.nodes[id], node.style["display"]?.string != "none" else { continue }
            switch node.kind {
            case "text", "time": text += 1
            case "image": images += 1
            case "view", "pressable":
                if node.props["type"] != "progress" && ["", "main", "section", "header", "footer", "article", "aside"].contains(node.props["semanticTag"] ?? "") {
                    stack.append(contentsOf: node.children)
                } else { other += 1 }
            default: other += 1
            }
        }
        return text == 1 && images <= 1 && other == 0
    }

    /// The URL a snapshot opens: the first link's, in document order; a
    /// `display: none` subtree has none.
    public static func url(_ snapshot: HeadlessSession.Snapshot, environment: SnapshotEnvironment) -> URL? {
        guard !environment.placeholder else { return nil }
        var stack = Array(snapshot.roots.reversed())
        while let id = stack.popLast() {
            guard let node = snapshot.nodes[id], node.style["display"]?.string != "none" else { continue }
            if let url = SnapshotLinks.url(node.props["href"], base: environment.linkBase) { return url }
            stack.append(contentsOf: node.children.reversed())
        }
        return nil
    }
}

/// The entry's view: its roots at their frames inside the content margins,
/// the root's background as the container's, account content private, the
/// first link as the URL.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct SnapshotView: View {
    enum Source {
        case snapshot(HeadlessSession.Snapshot)
        case session(HeadlessSession, CGSize)
    }
    let source: Source
    let environment: SnapshotEnvironment
    @Environment(\.widgetContentMargins) private var systemMargins
    @Environment(\.dynamicTypeSize) private var dynamicType

    private var margins: EdgeInsets { environment.contentMargins ?? systemMargins }

    /// The laid-out tree: as given, or the session laid out now at the
    /// content size and text size (the archive is made in the extension,
    /// again on a text-size change, so this runs where it is needed).
    private var snapshot: HeadlessSession.Snapshot {
        switch source {
        case .snapshot(let snapshot): return snapshot
        case .session(let session, let display):
            let size = CGSize(width: max(0, display.width - margins.leading - margins.trailing), height: max(0, display.height - margins.top - margins.bottom))
            do {
                try session.setRootFontSize(environment.rootFontSize ?? SnapshotDriver.rootFontSize(for: dynamicType))
                return try session.resize(to: size)
            } catch {
                return HeadlessSession.Snapshot(size: size, roots: [], nodes: [:])
            }
        }
    }

    public var body: some View {
        let snapshot = snapshot
        let roots = snapshot.roots.compactMap { snapshot.nodes[$0] }
        Group {
            if environment.family == .accessoryInline && !SnapshotDriver.admitsInline(snapshot) {
                let _ = Logger(subsystem: "dev.exact.widgets", category: "inline").error("entry \(environment.entryName, privacy: .public): accessoryInline requires one text/time leaf and at most one image")
                Text("Open \(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "the app")")
            } else {
                FrameLayout(frames: roots.map(\.frame), size: snapshot.size) {
                    ForEach(roots, id: \.id) { SnapshotNodeView(node: $0, tree: snapshot, environment: environment, isRoot: true) }
                }
            }
        }
        .frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
        .padding(margins)
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
/// mode): its colour and gradients painted full bleed on the container's
/// real bounds, which the system shapes (a hand-built `containerBackground`
/// has no radius of its own; the root's stays on its border and content
/// inside the margins). Unsaid, it is the system's background.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct ContainerBackground: View {
    let root: HeadlessSession.Node?
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let paint = root.map { SnapshotPaint(style: SnapshotStyle($0), dark: scheme == .dark) }
        if let paint, paint.hasBackground {
            GeometryReader { proxy in
                let bounds = CGRect(origin: .zero, size: proxy.size)
                paint.fill(in: bounds, positioned: bounds, rounded: false)
            }
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
