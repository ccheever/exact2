// @ref LLP 1126 D10 — the live driver (spike): SwiftUI over a live headless
// session. Layout is the kernel's, drawn by the snapshot driver's nodes; the
// session's turns (presses, timers, replies) replace the snapshot the view reads.
#if canImport(WidgetKit)
import SwiftUI
import ExactCore
import os

/// Seconds on a monotonic clock (Core Animation's is not on watchOS).
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public func liveUptime() -> TimeInterval { ProcessInfo.processInfo.systemUptime }

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
@MainActor @Observable
public final class LiveModel {
    public private(set) var snapshot: HeadlessSession.Snapshot
    public private(set) var failure: String?
    @ObservationIgnored public let session: HeadlessSession
    /// Runs each command a turn sent (`haptic`, …), in order, after the turn.
    @ObservationIgnored public var onCommand: ((HeadlessSession.Command) -> Void)?
    /// Called after each turn with its label (`press`, `timer`, `reply`, …): the host's measurements.
    @ObservationIgnored public var onTurn: ((String) -> Void)?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var active = true
    @ObservationIgnored private let log = Logger(subsystem: "dev.exact.watch", category: "live")

    /// `session` is a live one (`HeadlessSession(…, live: true)`), on the clock `now()` reads.
    public init(session: HeadlessSession) throws {
        self.session = session
        snapshot = try session.snapshot()
        session.onWake { [weak self] in self?.run("reply") { try $0.session.refresh(at: $0.now()) } }
        schedule()
    }

    /// The runner clock (`HeadlessSession.liveNow()`).
    public func now() -> Double { HeadlessSession.liveNow() }

    public func press(_ node: UInt32) {
        run("press") { try $0.session.press(node, at: $0.now()) }
    }

    /// A field's whole text, as the watch's input sheet returns it on submit: one `input`
    /// event (kind 54). A dismissed sheet sends nothing; the app's own writes to `value`
    /// reach the field as its prop.
    public func edit(_ node: UInt32, text: String) {
        run("input") { try $0.session.fieldEvent(node, kind: 54, payload: FieldWire.replacement(text), at: $0.now()) }
    }

    /// The platform's Back from the route keyed `key` (`HeadlessSession.back`).
    public func back(root: UInt32, route key: String) {
        run("back") { model in try model.session.back(root: root, route: key, at: model.now()) ?? model.snapshot }
    }

    public func scrolled(_ node: UInt32, to offset: CGPoint) {
        session.scrolled(node, left: offset.x, top: offset.y)
    }

    public func resize(to size: CGSize) {
        guard size != snapshot.size else { return }
        run("resize") { try $0.session.resize(to: size) }
    }

    /// Timers stop while the app is not active (a lowered wrist, Always On); the clock
    /// catches up once when it is again.
    public func setActive(_ active: Bool) {
        self.active = active
        log.notice("active \(active, privacy: .public) at \(self.now(), format: .fixed(precision: 0)) ms")
        print("exact-watch active \(active) at \(Int(now())) ms")
        if active { run("resume") { try $0.session.advance(at: $0.now()) } } else { timer?.invalidate(); timer = nil }
    }

    private func run(_ label: StaticString, _ turn: (LiveModel) throws -> HeadlessSession.Snapshot) {
        let start = liveUptime()
        do {
            snapshot = try turn(self); failure = nil
        } catch {
            failure = "\(error)"
            log.error("\(label, privacy: .public) failed: \(String(describing: error), privacy: .public)")
        }
        let ms = (liveUptime() - start) * 1000
        log.notice("turn \(label, privacy: .public) \(ms, format: .fixed(precision: 2)) ms at \(self.now(), format: .fixed(precision: 0))")
        print("exact-watch turn \(label) \(String(format: "%.2f", ms)) ms at \(Int(now()))")
        for command in session.takeCommands() { onCommand?(command) }
        onTurn?("\(label)")
        schedule()
    }

    private func schedule() {
        timer?.invalidate(); timer = nil
        guard active, let due = session.timerDue else { return }
        let delay = max(0, due - now()) / 1000
        print("exact-watch deadline \(Int(due)) in \(Int(delay * 1000)) ms")
        timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.run("timer") { try $0.session.advance(at: $0.now()) } }
        }
    }
}

/// The live session's tree at the kernel's frames, taking presses and scrolls.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct LiveView: View {
    let model: LiveModel
    let rootFontSize: CGFloat

    public init(model: LiveModel, rootFontSize: CGFloat) {
        self.model = model; self.rootFontSize = rootFontSize
    }

    public var body: some View {
        let snapshot = model.snapshot
        let roots = snapshot.roots.compactMap { snapshot.nodes[$0] }
        // The leaves read the environment's clock and root font size; its family and links are a widget's.
        let environment = SnapshotEnvironment(family: .systemLarge, entryName: "app", rootFontSize: rootFontSize, entryDate: .now)
        let input = LiveInput(press: { model.press($0) }, scrolled: { model.scrolled($0, to: $1) },
                              title: { model.session.controlTitle($0) }, edited: { model.edit($0, text: $1) },
                              back: { model.back(root: $0, route: $1) })
        return FrameLayout(frames: roots.map(\.frame), size: snapshot.size) {
            ForEach(roots, id: \.id) { LiveNodeView(node: $0, tree: snapshot, environment: environment, input: input) }
        }
        .frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
    }
}

/// What a live session takes from the view: a press on a node with a `press`
/// handler, a scroller's offset, a field's text; and a native control's title.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct LiveInput {
    var press: (UInt32) -> Void
    var scrolled: (UInt32, CGPoint) -> Void
    var title: (UInt32) -> String?
    var edited: (UInt32, String) -> Void
    /// The platform's Back from a navigation root's active route: (root, route key).
    var back: (UInt32, String) -> Void
}

/// One live node: the snapshot driver's box and leaves, which a widget's
/// archive keeps as its own type, with what a live session adds — presses,
/// scrolling, the platform's control and field.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct LiveNodeView: View {
    let node: HeadlessSession.Node
    let tree: HeadlessSession.Snapshot
    let environment: SnapshotEnvironment
    let input: LiveInput
    @Environment(\.colorScheme) private var scheme

    private var style: SnapshotStyle { SnapshotStyle(node) }
    private var paint: SnapshotPaint { SnapshotPaint(style: style, dark: scheme == .dark) }
    private var bounds: CGRect { CGRect(origin: .zero, size: node.frame.size) }

    var body: some View {
        if style.string("display") != "none" {
            content
                .frame(width: node.frame.width, height: node.frame.height, alignment: .topLeading)
                .modifier(Clip(outline: style.clips ? paint.paddingOutline(in: bounds) : nil))
                .background { paint.fill(in: bounds) }
                .overlay { paint.border(in: bounds) }
                .modifier(PathClip(path: style.clipPath("clip_path")))
                .opacity(style.number("opacity").map { Double(max(0, min(1, $0))) } ?? 1)
                .modifier(Pressed(node: node.id, input: node.handlers.contains("press") ? input : nil))
                .modifier(Semantics(node: node))
        }
    }

    @ViewBuilder
    private var content: some View {
        switch node.kind {
        case "text", "time": SnapshotTextView(node: node, environment: environment)
        case "image": SnapshotImageView(node: node, environment: environment)
        case "control", "input": LiveLeaf(node: node, input: input)
        default:
            if node.props["navigationKey"] != nil, node.children.contains(where: { tree.nodes[$0]?.props["navigationKey"] != nil }) {
                LiveNavigation(root: node, tree: tree, environment: environment, input: input)
            } else if node.props["type"] == "progress" {
                SnapshotProgressView(node: node, environment: environment)
            } else {
                let children = node.children.compactMap { tree.nodes[$0] }
                // A scroller's children are laid out over its content (`content` op), not its box.
                let layout = FrameLayout(frames: children.map(\.frame), size: node.scrollContent ?? node.frame.size) {
                    ForEach(children, id: \.id) { LiveNodeView(node: $0, tree: tree, environment: environment, input: input) }
                }
                if ["scroll", "auto"].contains(style.string("overflow_y") ?? "") {
                    ScrollView(.vertical) { layout }.modifier(ScrollReport(node: node.id, input: input))
                } else {
                    layout
                }
            }
        }
    }
}

/// A navigation root (`navigationKey` on it and on its routes, LLP 1038) as the
/// platform's stack: the first route the base, the routes up to the active one
/// pushed, so a push slides in and the edge swipe or the bar's Back pops. A pop
/// SwiftUI makes goes to the session's Back; the runner's next tree sets the path.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct LiveNavigation: View {
    let root: HeadlessSession.Node
    let tree: HeadlessSession.Snapshot
    let environment: SnapshotEnvironment
    let input: LiveInput

    private var routes: [HeadlessSession.Node] {
        root.children.compactMap { tree.nodes[$0] }.filter { $0.props["navigationKey"] != nil }
    }

    var body: some View {
        let routes = routes, active = root.props["navigationKey"] ?? ""
        let top = routes.firstIndex { $0.props["navigationKey"] == active } ?? 0
        let pushed = top > 0 ? routes[1...top].compactMap { $0.props["navigationKey"] } : []
        let path = Binding(get: { pushed }, set: { next in if next.count < pushed.count { input.back(root.id, active) } })
        NavigationStack(path: path) {
            page(routes.first)
                .navigationDestination(for: String.self) { key in page(routes.first { $0.props["navigationKey"] == key }) }
        }
    }

    @ViewBuilder
    private func page(_ route: HeadlessSession.Node?) -> some View {
        if let route {
            FrameLayout(frames: [CGRect(origin: .zero, size: route.frame.size)], size: route.frame.size) {
                LiveNodeView(node: route, tree: tree, environment: environment, input: input)
            }
        }
    }
}

/// The platform's leaves for what the kernel left to it: a native button
/// (`control`) and a text field (`input`), at the kernel's frame.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct LiveLeaf: View {
    let node: HeadlessSession.Node
    let input: LiveInput
    @State private var text = ""

    var body: some View {
        if node.kind == "input" {
            TextField(node.props["placeholder"] ?? "", text: $text)
                .onAppear { text = node.props["value"] ?? "" }
                .onChange(of: node.props["value"] ?? "") { _, value in if value != text { text = value } }
                .onSubmit { input.edited(node.id, text) }
        } else {
            Text(input.title(node.id) ?? node.props["accessibilityLabel"] ?? "")
                .lineLimit(1)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Capsule().fill(.secondary.opacity(0.25)))
        }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct Pressed: ViewModifier {
    let node: UInt32
    let input: LiveInput?
    func body(content: Content) -> some View {
        if let input {
            Button { input.press(node) } label: { content }.buttonStyle(.plain)
        } else { content }
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
private struct ScrollReport: ViewModifier {
    let node: UInt32
    let input: LiveInput
    func body(content: Content) -> some View {
        if #available(iOS 18.0, macOS 15.0, watchOS 11.0, visionOS 2.0, *) {
            content.onScrollGeometryChange(for: CGPoint.self) { $0.contentOffset } action: { _, offset in
                input.scrolled(node, offset)
            }
        } else { content }
    }
}
#endif
