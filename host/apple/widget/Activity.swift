// @ref LLP 1126.001 WP10 — the one ActivityConfiguration for every Exact
// activity: the envelope's attributes name the kind and the generation, and
// each placement is the snapshot driver over the entry's plan in that
// generation. No app string reaches this file: the bundle lists it when the
// app declares `live_activities` (widgets.mjs).
import ExactCore
import ExactSnapshot
import Foundation
import SwiftUI
import WidgetKit
import ActivityKit
import UIKit
import os

private let activityLog = Logger(subsystem: "dev.exact.widgets", category: "activity")

/// What one activity renders from: the pinned generation's plan and
/// registry row, the state the plan decodes (the envelope's, else the last
/// decodable one kept for this id, else the preview), and the session laid
/// out once per state and environment.
final class ActivityRendering {
    let kind: String
    let render: ActivityRender
    /// The kept last-good state is shown, not the envelope's (D9).
    let outdated: Bool
    /// The preview state stands in: nothing decodable was ever kept.
    let placeholder: Bool
    let root: URL
    /// The identity of what was rendered: the activity, its content, the
    /// generation and the environment that shaped the layout.
    let key: String

    private struct Registry: Decodable {
        let kind: String
        let shape: String
        let preview: ActivityJSON
    }

    init(context: ActivityViewContext<ExactActivityAttributes>, key: String, facts: SessionFacts) throws {
        self.key = key
        let kind = context.attributes.kind
        self.kind = kind
        // A generation the app published is pinned by name; before the app's
        // first launch in this binary, the embedded plans stand in.
        let embedded = Bundle.main.bundleURL
        let root = (try? WidgetContainer.pin(generation: context.attributes.generation)) ?? (try? WidgetContainer.pin()) ?? embedded
        self.root = root
        guard let plan = try? Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan")) else {
            throw HeadlessSession.Failure(description: "activity \(kind): entry plan unavailable")
        }
        let registry = (try? JSONDecoder().decode([Registry].self, from: Data(contentsOf: root.appendingPathComponent("plans/activities.json"))))?.first { $0.kind == kind }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        func boot(_ state: Data?) throws -> ActivityRender {
            try ActivityRender(plan: plan, kind: kind, state: state, facts: facts, assetRoot: root)
        }
        // A state the plan cannot decode (another shape's digest, or fields
        // it does not know) is refused by the boot; the cache then answers
        // the last state this id decoded. A boot that succeeds is the render.
        var booted: [Data: ActivityRender] = [:]
        func accepts(_ bytes: Data) -> Bool {
            if booted[bytes] == nil, let render = try? boot(bytes) { booted[bytes] = render }
            return booted[bytes] != nil
        }
        let envelope = context.state.shape == registry?.shape ? try? encoder.encode(context.state.state) : nil
        let cache = WidgetContainer.root.map { ActivityStateCache(root: $0, id: context.activityID) }
        var resolved: (state: Data?, outdated: Bool)
        do { resolved = try cache?.resolve(envelope, accepts: accepts) ?? (nil, false) }
        catch {
            activityLog.error("activity \(kind, privacy: .public): state cache: \(String(describing: error), privacy: .public)")
            resolved = (nil, false)
        }
        if resolved.state == nil, let envelope, accepts(envelope) { resolved = (envelope, false) }
        if let state = resolved.state, let render = booted[state] ?? (try? boot(state)) {
            self.render = render; outdated = resolved.outdated; placeholder = false
        } else {
            let preview = try registry.map { try encoder.encode($0.preview) }
            render = try boot(preview); outdated = false; placeholder = true
        }
    }
}

/// The renders alive in this process, by key: the eight placements of one
/// push share one session (one session per activity, D9), and a state the
/// system asks for again (another region, a text-size change) is not
/// booted twice. Bounded: an activity's old states leave as new ones come.
enum ActivityRenders {
    private static let lock = NSLock()
    private static var kept: [ActivityRendering] = []
    private static let limit = 8

    static func rendering(for context: ActivityViewContext<ExactActivityAttributes>, facts: SessionFacts, dark: Bool, rootFontSize: Double) throws -> ActivityRendering {
        let key = [context.activityID, context.attributes.generation, context.state.shape, String(context.state.state.hashValue), dark ? "dark" : "light", String(rootFontSize)].joined(separator: "|")
        lock.lock(); defer { lock.unlock() }
        if let found = kept.first(where: { $0.key == key }) { return found }
        #if EXACT_INSPECT
        let start = DispatchTime.now()
        #endif
        let made = try ActivityRendering(context: context, key: key, facts: facts)
        kept.removeAll { $0.key.hasPrefix(context.activityID + "|") }
        kept.append(made)
        if kept.count > limit { kept.removeFirst(kept.count - limit) }
        #if EXACT_INSPECT
        measurement("activity boot", start: start.uptimeNanoseconds, details: "kind=\(made.kind) outdated=\(made.outdated) placeholder=\(made.placeholder)")
        #endif
        return made
    }
}

/// One placement of an activity, as the system archives it: the driver's
/// placement view over the shared render, the preview redacted as a
/// placeholder, a kept state and the system's stale state dimmed.
struct ExactActivityPlacement: View {
    let context: ActivityViewContext<ExactActivityAttributes>
    let placement: ActivityPlacement
    @Environment(\.displayScale) private var scale
    @Environment(\.colorScheme) private var scheme
    @Environment(\.colorSchemeContrast) private var contrast
    @Environment(\.legibilityWeight) private var legibility
    @Environment(\.accessibilityReduceTransparency) private var transparency
    @Environment(\.accessibilityReduceMotion) private var motion
    @Environment(\.dynamicTypeSize) private var textSize

    private var unavailable: some View { Text("Open \(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "the app")") }

    var body: some View {
        let preferences = Preferences(reducedMotion: motion, reducedTransparency: transparency,
            contrast: contrast == .increased ? "more" : "no-preference", dark: scheme == .dark,
            gamut: "srgb", highDynamicRange: false, boldText: legibility == .bold)
        let rootFontSize = SnapshotDriver.rootFontSize(for: textSize)
        let facts = SessionFacts(displayScale: scale, preferences: preferences, rootFontSize: rootFontSize, date: Date())
        switch Result(catching: { try ActivityRenders.rendering(for: context, facts: facts, dark: scheme == .dark, rootFontSize: rootFontSize) }) {
        case .success(let rendering):
            let environment = SnapshotEnvironment(family: .liveActivity, placement: placement, placeholder: rendering.placeholder,
                linkBase: (Bundle.main.object(forInfoDictionaryKey: "ExactLinkBase") as? String).flatMap(URL.init(string:)),
                imageRoots: [rendering.root, Bundle.main.bundleURL, Bundle.main.bundleURL.deletingLastPathComponent().deletingLastPathComponent()],
                appGroup: WidgetContainer.root, entryDate: Date())
            #if EXACT_INSPECT
            // The render's cost so far (§5 Q4): the layouts this state has
            // run and their time, at each placement's turn, then once more
            // after the archive has asked for every region.
            let _ = inspect(rendering, placement: placement)
            #endif
            SnapshotActivity.view(rendering.render, placement: placement, environment: environment)
                .redacted(reason: rendering.placeholder ? .placeholder : [])
                .opacity(rendering.outdated || context.isStale ? 0.55 : 1)
                .activityBackgroundTint(placement == .lockScreen ? SnapshotActivity.tint(rendering.render, dark: scheme == .dark) : nil)
        case .failure(let error):
            let _ = activityLog.error("activity \(context.attributes.kind, privacy: .public) \(placement.name, privacy: .public) refused: \(String(describing: error), privacy: .public)")
            unavailable
        }
    }
}

#if EXACT_INSPECT
private let inspected = NSLock()
private var inspectedKeys: Set<String> = []
/// One `activity placement` line per placement as the archive asks for
/// it, and one `activity archived` line a quarter second after a state's
/// first placement, when the archive has asked for every region.
private func inspect(_ rendering: ActivityRendering, placement: ActivityPlacement) {
    let render = rendering.render
    activityLog.notice("activity placement pid=\(getpid()) kind=\(rendering.kind, privacy: .public) placement=\(placement.name, privacy: .public) layouts=\(render.layoutCount) ms=\(render.layoutMilliseconds) now=\(footprintNow()) peak=\(footprint())")
    inspected.lock(); defer { inspected.unlock() }
    guard !inspectedKeys.contains(rendering.key) else { return }
    inspectedKeys.insert(rendering.key)
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
        activityLog.notice("activity archived pid=\(getpid()) kind=\(rendering.kind, privacy: .public) outdated=\(rendering.outdated) placeholder=\(rendering.placeholder) layouts=\(render.layoutCount) ms=\(render.layoutMilliseconds) now=\(footprintNow()) peak=\(footprint())")
    }
}
#endif

/// The one configuration (the API is one per attributes type): the
/// envelope's kind picks the plan at render time, so a new kind is a bundle
/// change, never a binary one (D2, D9).
struct ExactActivities: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: ExactActivityAttributes.self) { context in
            ExactActivityPlacement(context: context, placement: .lockScreen)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) { ExactActivityPlacement(context: context, placement: .expandedLeading) }
                DynamicIslandExpandedRegion(.trailing) { ExactActivityPlacement(context: context, placement: .expandedTrailing) }
                DynamicIslandExpandedRegion(.center) { ExactActivityPlacement(context: context, placement: .expandedCenter) }
                DynamicIslandExpandedRegion(.bottom) { ExactActivityPlacement(context: context, placement: .expandedBottom) }
            } compactLeading: {
                ExactActivityPlacement(context: context, placement: .compactLeading)
            } compactTrailing: {
                ExactActivityPlacement(context: context, placement: .compactTrailing)
            } minimal: {
                ExactActivityPlacement(context: context, placement: .minimal)
            }
        }
    }
}
