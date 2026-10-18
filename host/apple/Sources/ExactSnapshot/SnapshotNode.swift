// One node: its box at the kernel's frame, its children at theirs, and the
// platform's presentation where the author was silent (D5).
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotNodeView: View {
    let node: HeadlessSession.Node
    let tree: HeadlessSession.Snapshot
    let environment: SnapshotEnvironment
    var isRoot = false
    @Environment(\.colorScheme) private var scheme

    private var style: SnapshotStyle { SnapshotStyle(node) }
    private var paint: SnapshotPaint { SnapshotPaint(style: style, dark: scheme == .dark) }
    private var bounds: CGRect { CGRect(origin: .zero, size: node.frame.size) }

    var body: some View {
        if style.string("display") != "none" {
            decorated
        }
    }

    /// The content in its box, painted and clipped, then its semantics. A
    /// `visibility: hidden` node paints and says nothing of its own; its
    /// descendants decide for themselves (CSS Display 3 §2.?: visibility
    /// inherits, and a `visible` descendant shows).
    @ViewBuilder
    private var decorated: some View {
        let visible = style.visible
        let clips = (style.string("overflow_x") ?? "visible") != "visible" || (style.string("overflow_y") ?? "visible") != "visible"
        let nested = !isRoot && [.systemMedium, .systemLarge, .systemExtraLarge].contains(environment.family)
        let link = nested && !environment.placeholder ? SnapshotLinks.url(node.props["href"], base: environment.linkBase) : nil
        content
            .frame(width: node.frame.width, height: node.frame.height, alignment: .topLeading)
            // The root's background is the container's (D5), never drawn twice.
            .background { if !isRoot && visible { paint.fill(in: bounds) } }
            .overlay { if visible { paint.border(in: bounds) } }
            .modifier(Clip(outline: clips ? paint.outline(in: bounds) : nil, path: style.clipPath("clip_path")))
            .opacity(style.number("opacity").map { Double(max(0, min(1, $0))) } ?? 1)
            .modifier(Scheme(scheme: style.string("color_scheme")))
            .modifier(Privacy(public: publicContent && !environment.placeholder))
            .modifier(Semantics(node: node, visible: visible))
            .modifier(Linked(url: link))
    }

    /// `data-privacy="public"` (the node's `dataset`): not account content,
    /// so it stays while the Lock Screen redacts the rest (D8).
    private var publicContent: Bool { SnapshotData.words(node)["privacy"] == "public" }

    @ViewBuilder
    private var content: some View {
        switch node.kind {
        case "text", "time": SnapshotTextView(node: node, environment: environment)
        case "image": SnapshotImageView(node: node, environment: environment)
        default:
            if node.props["type"] == "progress" {
                SnapshotProgressView(node: node)
            } else {
                // Document order, then `z-index`: a stable sort keeps the
                // order among equals (CSS 2 §E, the painting order).
                let children = node.children.compactMap { tree.nodes[$0] }.enumerated()
                    .sorted { a, b in
                        let (za, zb) = (SnapshotStyle(a.element).number("z_index") ?? 0, SnapshotStyle(b.element).number("z_index") ?? 0)
                        return za != zb ? za < zb : a.offset < b.offset
                    }.map(\.element)
                FrameLayout(frames: children.map(\.frame), size: node.frame.size) {
                    ForEach(children, id: \.id) { SnapshotNodeView(node: $0, tree: tree, environment: environment) }
                }
            }
        }
    }
}

/// A node's `data-*` words (`dataset`, one JSON object of strings).
enum SnapshotData {
    static func words(_ node: HeadlessSession.Node) -> [String: String] {
        guard let json = node.props["dataset"]?.data(using: .utf8), let words = try? JSONSerialization.jsonObject(with: json) as? [String: String] else { return [:] }
        return words
    }
}

/// Overflow clips to the rounded border box; `clip-path` clips to its
/// path; both apply to the node and everything in it.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Clip: ViewModifier {
    let outline: RoundedOutline?
    let path: ClipPathRow?
    func body(content: Content) -> some View {
        content
            .modifier(Outline(outline: outline))
            .modifier(PathClip(path: path))
    }
    private struct Outline: ViewModifier {
        let outline: RoundedOutline?
        func body(content: Content) -> some View {
            if let outline { content.clipShape(outline) } else { content }
        }
    }
    private struct PathClip: ViewModifier {
        let path: ClipPathRow?
        func body(content: Content) -> some View {
            if let path { content.clipShape(path, style: path.fillStyle) } else { content }
        }
    }
}

/// CSS `color-scheme` on a subtree (LLP 1034 §8): its `light-dark()` pairs
/// and platform colours resolve in that scheme.
private struct Scheme: ViewModifier {
    let scheme: String?
    func body(content: Content) -> some View {
        switch scheme {
        case "dark": content.environment(\.colorScheme, .dark)
        case "light": content.environment(\.colorScheme, .light)
        default: content
        }
    }
}

private struct Privacy: ViewModifier {
    let `public`: Bool
    func body(content: Content) -> some View {
        if `public` { content.privacySensitive(false) } else { content }
    }
}

/// A nested link's content opens its URL (D7); the root's link is the
/// widget's URL (`SnapshotView`).
private struct Linked: ViewModifier {
    let url: URL?
    func body(content: Content) -> some View {
        if let url { Link(destination: url) { content } } else { content }
    }
}

/// Accessibility carries over from the node (D5): its label, hint and
/// value, a heading with its level, a paragraph as one element, a
/// decorative image hidden, a hidden node unspoken.
private struct Semantics: ViewModifier {
    let node: HeadlessSession.Node
    let visible: Bool

    func body(content: Content) -> some View {
        let label = node.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let hidden = !visible || node.props["accessibilityElementsHidden"] == "true" || (node.kind == "image" && label == nil)
        let combined = node.kind == "text" || node.kind == "time" || label != nil
        content
            .modifier(Combined(on: combined && !hidden))
            .modifier(Labelled(label: hidden ? nil : label, hint: node.props["accessibilityHint"], value: node.props["accessibilityValueText"]))
            .modifier(Heading(level: node.props["accessibilityRole"] == "heading" ? Int(node.props["accessibilityHeadingLevel"] ?? "") ?? 0 : nil))
            .accessibilityAddTraits(node.kind == "button" && node.props["href"] == nil ? .isButton : [])
            .accessibilityHidden(hidden)
    }
}

private struct Combined: ViewModifier {
    let on: Bool
    func body(content: Content) -> some View {
        if on { content.accessibilityElement(children: .combine) } else { content }
    }
}

private struct Labelled: ViewModifier {
    let label: String?
    let hint: String?
    let value: String?
    func body(content: Content) -> some View {
        content
            .modifier(Text(text: label, which: .label))
            .modifier(Text(text: hint, which: .hint))
            .modifier(Text(text: value, which: .value))
    }
    private struct Text: ViewModifier {
        enum Which { case label, hint, value }
        let text: String?
        let which: Which
        func body(content: Content) -> some View {
            if let text, !text.isEmpty {
                switch which {
                case .label: content.accessibilityLabel(text)
                case .hint: content.accessibilityHint(text)
                case .value: content.accessibilityValue(text)
                }
            } else { content }
        }
    }
}

/// A heading's trait and its level (`aria-level`): h1 to h6, else unspecified.
private struct Heading: ViewModifier {
    let level: Int?
    func body(content: Content) -> some View {
        if let level {
            let levels: [AccessibilityHeadingLevel] = [.unspecified, .h1, .h2, .h3, .h4, .h5, .h6]
            content.accessibilityAddTraits(.isHeader).accessibilityHeading(levels[min(max(level, 0), 6)])
        } else { content }
    }
}

enum SnapshotLinks {
    /// A route's path as a URL reference against the app's origin or
    /// scheme, its query, fragment and escapes kept (RFC 3986 §5); an
    /// absolute `href` as it is. Empty is no link (the grammar's rule). An
    /// inline link (a run's `href` inside a paragraph) is not a link region
    /// in this stage; WP2's profile refuses it with "make the link its own
    /// element".
    static func url(_ href: String?, base: URL?) -> URL? {
        guard let href, !href.isEmpty else { return nil }
        return URL(string: href, relativeTo: base)?.absoluteURL
    }
}
#endif
