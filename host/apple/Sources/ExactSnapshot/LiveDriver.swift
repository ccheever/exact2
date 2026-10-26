// @ref LLP 1126.002 D1, D3, D5, D6 — the live driver: SwiftUI over a live
// headless session. Layout is the kernel's, drawn with the snapshot driver's
// leaves; the session's turns (presses, timers, replies) replace the snapshot
// the view reads.
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
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var phase: ScenePhase
    /// While inactive, the earliest a timer may run (the runner clock): a minute after
    /// the scene went inactive or after the last inactive timer, which other turns keep.
    @ObservationIgnored private var inactiveNotBefore: Double?
    @ObservationIgnored private let log = Logger(subsystem: "dev.exact.watch", category: "live")

    /// While the scene is inactive (a lowered wrist, Always On), timers run at most
    /// this often: `TimelineView`'s minutes cadence (LLP 1126.002 D6, as amended 2026-10-23).
    public static let inactiveInterval: TimeInterval = 60

    /// `session` is a live one (`HeadlessSession(…, live: true)`), on the clock `now()` reads;
    /// `phase` is the scene's at birth, which paces the first timer.
    public init(session: HeadlessSession, phase: ScenePhase) throws {
        self.session = session; self.phase = phase
        snapshot = try session.snapshot()
        if phase == .inactive { inactiveNotBefore = now() + Self.inactiveInterval * 1000 }
        session.onWake { [weak self] in self?.run("reply") { try $0.session.pump(at: $0.now()) } }
        schedule()
    }

    /// The runner clock (`HeadlessSession.liveNow()`).
    public func now() -> Double { HeadlessSession.liveNow() }

    /// After the first frame, the data the boot deferred is asked for, as iOS
    /// does after its first pixel; its replies arrive through the wake.
    public func firstFrame() {
        guard !shown else { return }
        shown = true
        run("data") { try $0.session.refresh(at: $0.now()) }
    }
    @ObservationIgnored private var shown = false

    public func press(_ node: UInt32) {
        run("press") { try $0.session.press(node, at: $0.now()) }
    }

    /// A field's whole text, as the watch's input sheet returns it on submit. A dismissed
    /// sheet sends nothing; the app's own writes to `value` reach the field as its prop.
    public func edit(_ node: UInt32, text: String) {
        run("input") { try $0.session.replaceText(node, with: text, at: $0.now()) }
    }

    /// The platform's Back from each route in `keys`, the top first, as a pop of
    /// several levels takes them (`HeadlessSession.back(root:routes:at:)`); a route
    /// that stays stops it, and the snapshot republished puts the stack back where
    /// the runner is.
    public func back(root: UInt32, routes keys: [String]) {
        run("back") { model in
            try model.session.back(root: root, routes: keys, at: model.now()) ?? model.session.snapshot()
        }
    }

    /// A link the reader followed: true when it was one of the app's routes, which
    /// the session took; otherwise the host opens it.
    public func follow(_ href: String) -> Bool {
        var taken = false
        run("follow") { model in
            guard let next = try model.session.follow(href, at: model.now()) else { return model.snapshot }
            taken = true
            return next
        }
        return taken
    }

    public func scrolled(_ node: UInt32, to offset: CGPoint) {
        session.scrolled(node, left: offset.x, top: offset.y)
    }

    public func resize(to size: CGSize) {
        guard size != snapshot.size else { return }
        run("resize") { try $0.session.resize(to: size) }
    }

    /// The reader's text size changed: the root size its Dynamic Type step gives.
    public func setRootFontSize(_ px: CGFloat) {
        run("text size") { try $0.session.setRootFontSize(px) }
    }

    /// Active, timers run when due; inactive (a lowered wrist, Always On), at most once
    /// a minute (`inactiveInterval`); in the background, not at all. On return to active
    /// the clock catches up once, so the screen is current when the wrist rises.
    public func setPhase(_ phase: ScenePhase) {
        guard phase != self.phase else { return }
        let rises = phase == .active
        self.phase = phase
        inactiveNotBefore = phase == .inactive ? now() + Self.inactiveInterval * 1000 : nil
        log.notice("phase \(String(describing: phase), privacy: .public) at \(self.now(), format: .fixed(precision: 0)) ms")
        if rises { run("resume") { try $0.session.advance(at: $0.now()) } } else { schedule() }
    }

    private func run(_ label: StaticString, _ turn: (LiveModel) throws -> HeadlessSession.Snapshot) {
        let start = liveUptime()
        var failed = false
        do {
            snapshot = try turn(self); failure = nil
        } catch {
            failure = "\(error)"; failed = true
            log.error("\(label, privacy: .public) failed: \(String(describing: error), privacy: .public)")
        }
        let ms = (liveUptime() - start) * 1000
        log.debug("turn \(label, privacy: .public) \(ms, format: .fixed(precision: 2)) ms at \(self.now(), format: .fixed(precision: 0))")
        for command in session.takeCommands() { onCommand?(command) }
        // A failed turn left the runner's deadline where it was, often past: a second
        // passes before the next try, not none.
        schedule(after: failed ? 1000 : 0)
    }

    /// Arms the timer at the runner's deadline, no sooner than `after` ms from now.
    private func schedule(after floor: Double = 0) {
        timer?.invalidate(); timer = nil
        guard phase != .background, let due = session.timerDue else { return }
        let active = phase == .active
        let clock = now()
        let delay = (max(due, clock + floor, active ? clock : inactiveNotBefore ?? clock) - clock) / 1000
        log.debug("deadline \(due, format: .fixed(precision: 0)) in \(delay * 1000, format: .fixed(precision: 0)) ms")
        // In the common modes, so a deadline fires while the Crown scrolls a list.
        let timer = Timer(timeInterval: delay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                if self.phase == .inactive { self.inactiveNotBefore = self.now() + Self.inactiveInterval * 1000 }
                self.run("timer") { try $0.session.advance(at: $0.now()) }
            }
        }
        // A tenth of the delay lets the system group this wake with others (Apple's
        // Energy Efficiency Guide: at least 10%), at most 100 ms on an active screen;
        // the live clock runs what is due by the time it fires.
        timer.tolerance = min(delay * 0.1, active ? 0.1 : 6)
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }
}

/// The live session's tree at the kernel's frames, taking presses and scrolls.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct LiveView: View {
    let model: LiveModel
    let rootFontSize: CGFloat
    @Environment(\.openURL) private var openURL

    public init(model: LiveModel, rootFontSize: CGFloat) {
        self.model = model; self.rootFontSize = rootFontSize
    }

    public var body: some View {
        let snapshot = model.snapshot
        let roots = snapshot.roots.compactMap { snapshot.nodes[$0] }
        // The leaves read the environment's clock and root font size; its family and links are a widget's.
        let environment = SnapshotEnvironment(family: .systemLarge, entryName: "app", rootFontSize: rootFontSize, entryDate: .now)
        let input = LiveInput(press: { model.press($0) }, scrolled: { model.scrolled($0, to: $1) },
                              edited: { model.edit($0, text: $1) },
                              back: { model.back(root: $0, routes: $1) },
                              follow: { href in
                                  if !model.follow(href), let url = URL(string: href), url.scheme != nil { openURL(url) }
                              })
        return FrameLayout(frames: roots.map(\.frame), size: snapshot.size) {
            ForEach(roots, id: \.id) { LiveNodeView(node: $0, tree: snapshot, environment: environment, input: input) }
        }
        .frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
    }
}

/// What a live session takes from the view: a press on a node with a `press`
/// handler, a scroller's offset, a field's text, the platform's Back and a link.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct LiveInput {
    var press: (UInt32) -> Void
    var scrolled: (UInt32, CGPoint) -> Void
    var edited: (UInt32, String) -> Void
    /// The platform's Back from a navigation root's routes, the top first: (root, route keys).
    var back: (UInt32, [String]) -> Void
    /// A link's `href`: one of the app's routes, else a URL the system opens.
    var follow: (String) -> Void
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
    /// The nearest ancestor's `data-privacy`; an app's content is not private unless it says so.
    var privacy: String?
    @Environment(\.colorScheme) private var scheme

    private var style: SnapshotStyle { SnapshotStyle(node) }
    private var paint: SnapshotPaint { SnapshotPaint(style: style, dark: scheme == .dark) }
    private var bounds: CGRect { CGRect(origin: .zero, size: node.frame.size) }
    private var ownPrivacy: String? { SnapshotData.words(node)["privacy"] ?? privacy }
    private var leaf: Bool { ["text", "time", "image", "control", "input"].contains(node.kind) || node.props["type"] == "progress" }

    var body: some View {
        if style.string("display") != "none" {
            content
                .frame(width: node.frame.width, height: node.frame.height, alignment: .topLeading)
                .modifier(Clip(outline: style.clips ? paint.paddingOutline(in: bounds) : nil))
                .background { paint.fill(in: bounds) }
                .overlay { paint.border(in: bounds) }
                .modifier(PathClip(path: style.clipPath("clip_path")))
                .opacity(style.number("opacity").map { Double(max(0, min(1, $0))) } ?? 1)
                // Redacted in Always On when private (D6), at leaves as a widget's are.
                .modifier(Privacy(word: leaf ? ownPrivacy : nil))
                .modifier(Pressed(node: node.id, input: node.handlers.contains("press") ? input : nil))
                .modifier(Followed(href: node.handlers.contains("press") ? nil : node.props["href"], input: input))
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
                LiveNavigation(root: node, tree: tree, environment: environment, input: input, privacy: ownPrivacy)
            } else if node.props["type"] == "progress" {
                SnapshotProgressView(node: node, environment: environment)
            } else {
                let children = node.children.compactMap { tree.nodes[$0] }
                // A scroller's children are laid out over its content (`content` op), not its box.
                let layout = FrameLayout(frames: children.map(\.frame), size: node.scrollContent ?? node.frame.size) {
                    ForEach(children, id: \.id) { LiveNodeView(node: $0, tree: tree, environment: environment, input: input, privacy: ownPrivacy) }
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
    /// The root's `data-privacy`, or its nearest ancestor's, which its routes inherit.
    let privacy: String?

    private var routes: [HeadlessSession.Node] {
        root.children.compactMap { tree.nodes[$0] }.filter { $0.props["navigationKey"] != nil }
    }

    var body: some View {
        let routes = routes, active = root.props["navigationKey"] ?? ""
        let top = routes.firstIndex { $0.props["navigationKey"] == active } ?? 0
        let pushed = top > 0 ? routes[1...top].compactMap { $0.props["navigationKey"] } : []
        // A pop goes to the session after SwiftUI's update, never inside it.
        let path = Binding(get: { pushed }, set: { next in
            guard next.count < pushed.count else { return }
            let popped = Array(pushed[next.count...].reversed())
            DispatchQueue.main.async { input.back(root.id, popped) }
        })
        NavigationStack(path: path) {
            page(routes.first)
                .navigationDestination(for: String.self) { key in page(routes.first { $0.props["navigationKey"] == key }) }
        }
    }

    @ViewBuilder
    private func page(_ route: HeadlessSession.Node?) -> some View {
        if let route {
            FrameLayout(frames: [CGRect(origin: .zero, size: route.frame.size)], size: route.frame.size) {
                LiveNodeView(node: route, tree: tree, environment: environment, input: input, privacy: privacy)
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
    @State private var submitted = false
    @FocusState private var editing: Bool

    var body: some View {
        if node.kind == "input" {
            TextField(node.props["placeholder"] ?? "", text: $text)
                .focused($editing)
                .onAppear { text = node.props["value"] ?? "" }
                .onChange(of: node.props["value"] ?? "") { _, value in if value != text { text = value } }
                .onSubmit { submitted = true; input.edited(node.id, text) }
                // A sheet dismissed without submit sends nothing; the field shows the app's value again.
                .onChange(of: editing) { _, now in
                    guard !now else { submitted = false; return }
                    DispatchQueue.main.async { if !submitted { text = node.props["value"] ?? "" } }
                }
        } else {
            Text(node.controlTitle ?? node.props["accessibilityLabel"] ?? "")
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
private struct Followed: ViewModifier {
    let href: String?
    let input: LiveInput
    func body(content: Content) -> some View {
        if let href, !href.isEmpty {
            Button { input.follow(href) } label: { content }.buttonStyle(.plain).accessibilityAddTraits(.isLink)
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
