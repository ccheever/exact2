// @ref LLP 1126.001 WP10 — the one ActivityConfiguration for every Exact
// activity: the envelope's attributes name the kind and the generation, and
// each placement is the snapshot driver over the entry's plan in that
// generation (`ActivityResolver`, in `ExactSnapshot`, which the tests
// drive). No app string reaches this file: the bundle lists it when the app
// declares `live_activities` (widgets.mjs).
import ExactCore
import ExactSnapshot
import Foundation
import SwiftUI
import WidgetKit
import ActivityKit
import UIKit
import os

private let activityLog = Logger(subsystem: "dev.exact.widgets", category: "activity")

/// One placement of an activity, as the system archives it: the request
/// and the facts the environment gives, resolved once per push and shared
/// by the eight placements (`ActivityRenders.shared`), then the driver's
/// presentation. A resolution the resolver refuses (a generation the app
/// has not published in this cohort, a plan or state nothing decodes) is
/// the degraded presentation, said in the extension's log; nothing else
/// is rendered in its place.
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

    private var request: ActivityRequest? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let state = try? encoder.encode(context.state.state) else { return nil }
        return ActivityRequest(activityID: context.activityID, kind: context.attributes.kind, generation: context.attributes.generation,
                               shape: context.attributes.shape, stateShape: context.state.shape, state: state)
    }

    var body: some View {
        let preferences = Preferences(reducedMotion: motion, reducedTransparency: transparency,
            contrast: contrast == .increased ? "more" : "no-preference", dark: scheme == .dark,
            gamut: "srgb", highDynamicRange: false, boldText: legibility == .bold)
        let facts = ActivityFacts(displayScale: scale, preferences: preferences, rootFontSize: SnapshotDriver.rootFontSize(for: textSize))
        let resolved = Result { () throws -> ActivityRendering in
            guard let request else { throw ActivityResolver.Failure.stateUndecodable(kind: context.attributes.kind) }
            return try ActivityRenders.shared.rendering(for: request, facts: facts, sources: .main(),
                                                        stateCache: WidgetContainer.root.map { ActivityStateCache(root: $0, id: context.activityID) })
        }
        switch resolved {
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
            ActivityPresentation(rendering: rendering, placement: placement, environment: environment)
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
    let key = "\(ObjectIdentifier(rendering).hashValue)"
    inspected.lock(); defer { inspected.unlock() }
    guard !inspectedKeys.contains(key) else { return }
    inspectedKeys.insert(key)
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
