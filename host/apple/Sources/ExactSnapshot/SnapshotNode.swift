// One node: its box at the kernel's frame, its children at theirs (in
// document order: the vocabulary's paint order), and the platform's
// presentation where the author was silent (D5).
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

    /// The content in its box: children clipped at the padding edge when
    /// the box clips (CSS Overflow 3 §3), the background and border on the
    /// border box, `clip-path` over all of it, then its semantics.
    @ViewBuilder
    private var decorated: some View {
        let nested = !isRoot && [.systemMedium, .systemLarge, .systemExtraLarge].contains(environment.family)
        let link = nested && !environment.placeholder ? SnapshotLinks.url(node.props["href"], base: environment.linkBase) : nil
        content
            .frame(width: node.frame.width, height: node.frame.height, alignment: .topLeading)
            .modifier(Clip(outline: style.clips ? paint.paddingOutline(in: bounds) : nil))
            // The root's background is the container's (D5), never drawn twice.
            .background { if !isRoot { paint.fill(in: bounds) } }
            .overlay { paint.border(in: bounds) }
            .modifier(PathClip(path: style.clipPath("clip_path")))
            .opacity(style.number("opacity").map { Double(max(0, min(1, $0))) } ?? 1)
            .modifier(Privacy(public: publicContent && !environment.placeholder))
            .modifier(Semantics(node: node))
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
                let children = node.children.compactMap { tree.nodes[$0] }
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

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Clip: ViewModifier {
    let outline: RoundedOutline?
    func body(content: Content) -> some View {
        if let outline { content.clipShape(outline) } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct PathClip: ViewModifier {
    let path: ClipPathRow?
    func body(content: Content) -> some View {
        if let path { content.clipShape(path, style: path.fillStyle) } else { content }
    }
}

private struct Privacy: ViewModifier {
    let `public`: Bool
    func body(content: Content) -> some View {
        if `public` { content.privacySensitive(false) } else { content }
    }
}

/// A nested link's content opens its URL (D7) in the families that take a
/// `Link`; the root's, or the one link of a small or accessory family, is
/// the widget's URL (`SnapshotView`).
private struct Linked: ViewModifier {
    let url: URL?
    func body(content: Content) -> some View {
        if let url { Link(destination: url) { content } } else { content }
    }
}

/// Accessibility carries over from the node (D5): its label, hint and
/// value, a heading with its level, a paragraph as one element, a
/// decorative image hidden, `aria-hidden` hiding the subtree.
private struct Semantics: ViewModifier {
    let node: HeadlessSession.Node

    func body(content: Content) -> some View {
        let label = node.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let hidden = node.props["accessibilityElementsHidden"] == "true" || (node.kind == "image" && label == nil)
        let combined = node.kind == "text" || node.kind == "time" || label != nil
        content
            .modifier(Combined(on: (combined || SnapshotSemantics.role(node) == "img") && !hidden))
            .modifier(Labelled(label: hidden ? nil : label, hint: node.props["accessibilityHint"], value: node.props["accessibilityValueText"]))
            .modifier(Heading(level: SnapshotSemantics.role(node) == "heading" ? Int(node.props["accessibilityHeadingLevel"] ?? "") ?? 0 : nil))
            .accessibilityAddTraits(SnapshotSemantics.traits(node))
            .accessibilityHidden(hidden)
    }
}

/// The node's role (`role=`, else what its element implies) and the
/// SwiftUI traits it carries: an image, a button, a link; a heading's
/// trait and level are `Heading`'s.
package enum SnapshotSemantics {
    package static func role(_ node: HeadlessSession.Node) -> String? {
        if let role = node.props["accessibilityRole"], !role.isEmpty { return role }
        switch node.kind {
        case "button": return node.props["href"].map { $0.isEmpty ? "button" : "link" } ?? "button"
        case "image": return "img"
        default: return nil
        }
    }
    package static func traits(_ node: HeadlessSession.Node) -> AccessibilityTraits {
        switch role(node) {
        case "img": return .isImage
        case "button": return .isButton
        case "link": return .isLink
        default: return []
        }
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
    /// inline link (a run's `href` inside a paragraph) is refused by the
    /// profile: "make the link its own element".
    static func url(_ href: String?, base: URL?) -> URL? {
        guard let href, !href.isEmpty else { return nil }
        return URL(string: href, relativeTo: base)?.absoluteURL
    }
}
#endif
