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
    /// Called after a press reaches the session (the host's haptic).
    @ObservationIgnored public var onPress: ((UInt32) -> Void)?
    /// Called after each turn with its label (`press`, `timer`, `reply`, …): the host's measurements.
    @ObservationIgnored public var onTurn: ((String) -> Void)?
    @ObservationIgnored private let zero: TimeInterval
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var active = true
    @ObservationIgnored private let log = Logger(subsystem: "dev.exact.watch", category: "live")

    /// `zero` is the `liveUptime()` at which the session's facts said clock 0.
    public init(session: HeadlessSession, zero: TimeInterval) throws {
        self.session = session; self.zero = zero
        snapshot = try session.snapshot()
        session.onWake { [weak self] in self?.run("reply") { try $0.session.refresh(at: $0.now()) } }
        schedule()
    }

    /// The shown press target that declares Escape as its key (`aria-keyshortcuts`),
    /// which the platform's Back presses, as a TV remote's Menu does (`RemoteTVOS`).
    public var escapeControl: (id: UInt32, label: String)? {
        var stack = Array(snapshot.roots.reversed())
        while let id = stack.popLast() {
            guard let node = snapshot.nodes[id], node.style["display"]?.string != "none" else { continue }
            let keys = (node.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace)
            if node.handlers.contains("press") && keys.contains("Escape") { return (id, node.props["accessibilityLabel"] ?? "Back") }
            stack.append(contentsOf: node.children.reversed())
        }
        return nil
    }

    /// The runner clock: milliseconds since the session's clock 0.
    public func now() -> Double { (liveUptime() - zero) * 1000 }

    public func press(_ node: UInt32) {
        run("press") { try $0.session.press(node, at: $0.now()) }
        onPress?(node)
    }

    /// A field's whole text, as the watch's input sheet returns it: one
    /// `insertReplacementText` input event (`ExactKit` FieldInput's wire),
    /// the caret at the end.
    public func edit(_ node: UInt32, text: String) {
        let caret = text.utf16.count
        let payload = "insertReplacementText,0,0,\(text.utf8.count),\(text)\(caret),\(caret),none,\(text)"
        run("input") { try $0.session.fieldEvent(node, kind: 54, payload: payload, at: $0.now()) }
    }

    public func scrolled(_ node: UInt32, to offset: CGPoint) {
        session.scrolled(node, left: offset.x, top: offset.y)
    }

    public func resize(to size: CGSize) {
        guard size != snapshot.size else { return }
        run("resize") { try $0.session.resize(to: size) }
    }

    /// Timers stop while the app is not active; the clock catches up when it is again.
    public func setActive(_ active: Bool) {
        self.active = active
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
        log.notice("turn \(label, privacy: .public) \(ms, format: .fixed(precision: 2)) ms")
        print("exact-watch turn \(label) \(String(format: "%.2f", ms)) ms")
        onTurn?("\(label)")
        schedule()
    }

    private func schedule() {
        timer?.invalidate(); timer = nil
        guard active, let due = session.timerDue else { return }
        let delay = max(0, due - now()) / 1000
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
        var environment = SnapshotEnvironment(family: .systemLarge, entryName: "app", rootFontSize: rootFontSize, entryDate: .now)
        environment.live = SnapshotLiveInput(press: { model.press($0) }, scrolled: { model.scrolled($0, to: $1) },
                                             title: { model.session.controlTitle($0) }, edited: { model.edit($0, text: $1) })
        return FrameLayout(frames: roots.map(\.frame), size: snapshot.size) {
            ForEach(roots, id: \.id) { SnapshotNodeView(node: $0, tree: snapshot, environment: environment) }
        }
        .frame(width: snapshot.size.width, height: snapshot.size.height, alignment: .topLeading)
    }
}
#endif
