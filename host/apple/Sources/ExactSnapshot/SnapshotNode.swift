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

    /// The content in its box, painted and clipped, then its semantics.
    @ViewBuilder
    private var decorated: some View {
        let clips = (style.string("overflow_x") ?? "visible") != "visible" || (style.string("overflow_y") ?? "visible") != "visible"
        // D7: the root's link is the widget's URL (`SnapshotView`); a nested
        // one is a `Link` where the family takes one (the medium family and
        // larger; a small or accessory widget opens one URL).
        let nested = !isRoot && [.systemMedium, .systemLarge, .systemExtraLarge].contains(environment.family)
        let link = nested ? SnapshotLinks.url(node.props["href"], base: environment.linkBase) : nil
        content
            .frame(width: node.frame.width, height: node.frame.height, alignment: .topLeading)
            // The root's background is the container's (D5), never drawn twice.
            .background { if !isRoot { paint.fill(in: bounds) } }
            .overlay { paint.border(in: bounds) }
            .modifier(Clip(shape: clips ? paint.shape(in: bounds) : nil))
            .opacity(style.number("opacity").map { Double(max(0, min(1, $0))) } ?? 1)
            .modifier(Hidden(hidden: style.string("visibility") == "hidden"))
            .modifier(Accented(on: accented))
            .modifier(Privacy(public: publicContent && !environment.placeholder))
            .modifier(Semantics(node: node))
            .modifier(Linked(url: environment.placeholder ? nil : link))
    }

    /// D5: the accented group, under the accented rendering mode, is what a
    /// hand-built widget marks: a heading, and anything in the tint.
    private var accented: Bool {
        node.props["accessibilityRole"] == "heading" || style.color("text_color")?.namesTint == true || style.color("tint_color")?.namesTint == true
    }

    /// `data-privacy="public"` (the node's `dataset`): not account content,
    /// so it stays while the Lock Screen redacts the rest (D8).
    private var publicContent: Bool {
        guard let json = node.props["dataset"]?.data(using: .utf8),
              let words = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return false }
        return words["privacy"] as? String == "public"
    }

    @ViewBuilder
    private var content: some View {
        switch node.kind {
        case "text": SnapshotTextView(node: node, environment: environment)
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

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Clip: ViewModifier {
    let shape: AnyShape?
    func body(content: Content) -> some View {
        if let shape { content.clipShape(shape) } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Hidden: ViewModifier {
    let hidden: Bool
    func body(content: Content) -> some View {
        if hidden { content.hidden() } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Accented: ViewModifier {
    let on: Bool
    func body(content: Content) -> some View {
        if on { content.widgetAccentable() } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Privacy: ViewModifier {
    let `public`: Bool
    func body(content: Content) -> some View {
        if `public` { content.privacySensitive(false) } else { content }
    }
}

/// A nested link's content opens its URL.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Linked: ViewModifier {
    let url: URL?
    func body(content: Content) -> some View {
        if let url { Link(destination: url) { content } } else { content }
    }
}

/// Accessibility carries over from the node (D5): its label, a heading's
/// trait, a paragraph as one element, a decorative image hidden.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Semantics: ViewModifier {
    let node: HeadlessSession.Node

    func body(content: Content) -> some View {
        let label = node.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let hidden = node.props["accessibilityElementsHidden"] == "true" || (node.kind == "image" && label == nil)
        let combined = node.kind == "text" || label != nil
        content
            .modifier(Combined(on: combined && !hidden))
            .modifier(Labelled(label: hidden ? nil : label, value: node.props["accessibilityValueText"]))
            .modifier(Traits(heading: node.props["accessibilityRole"] == "heading", button: node.kind == "button" && node.props["href"] == nil))
            .accessibilityHidden(hidden)
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Combined: ViewModifier {
    let on: Bool
    func body(content: Content) -> some View {
        if on { content.accessibilityElement(children: .combine) } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Labelled: ViewModifier {
    let label: String?
    let value: String?
    func body(content: Content) -> some View {
        content
            .modifier(Label(text: label))
            .modifier(Value(text: value))
    }
    private struct Label: ViewModifier {
        let text: String?
        func body(content: Content) -> some View { if let text { content.accessibilityLabel(text) } else { content } }
    }
    private struct Value: ViewModifier {
        let text: String?
        func body(content: Content) -> some View { if let text, !text.isEmpty { content.accessibilityValue(text) } else { content } }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Traits: ViewModifier {
    let heading: Bool
    let button: Bool
    func body(content: Content) -> some View {
        content
            .accessibilityAddTraits(heading ? .isHeader : [])
            .accessibilityAddTraits(button ? .isButton : [])
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
enum SnapshotLinks {
    /// A route's path under the app's origin or scheme; any other `href` as
    /// it is. Empty is no link (the grammar's rule).
    static func url(_ href: String?, base: URL?) -> URL? {
        guard let href, !href.isEmpty else { return nil }
        if href.hasPrefix("/") {
            guard let base else { return URL(string: href) }
            return base.appending(path: href)
        }
        return URL(string: href)
    }
}
#endif
