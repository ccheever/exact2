import ExactCore
import ExactWidgetRenderer
import Foundation
import SwiftUI
import WidgetKit
import UIKit
import os
#if EXACT_INSPECT
import Darwin
#endif

private struct TimelineFailure: Error { let description: String }
private let widgetLog = Logger(subsystem: "dev.exact.widgets", category: "timeline")
struct ExactWidgetEntry: TimelineEntry {
    let date: Date
    let kind: String
    let plan: Data
    let props: Data?
    let root: URL
    let placeholder: Bool
    var unavailable = false
}
private func embeddedRoot() -> URL {
    Bundle.main.bundleURL.deletingLastPathComponent().deletingLastPathComponent()
}
private func facts(_ traits: UITraitCollection, at date: Date) -> SessionFacts {
    SessionFacts(displayScale: max(1, traits.displayScale), preferences: Preferences(traits: traits),
                 rootFontSize: UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits).pointSize, date: date)
}
private func layout(_ entry: ExactWidgetEntry, size: CGSize, facts: SessionFacts) throws -> HeadlessSession.Snapshot {
    #if EXACT_INSPECT
    let start = DispatchTime.now().uptimeNanoseconds
    #endif
    let session = try HeadlessSession(plan: entry.plan, entry: entry.kind, props: entry.props, size: size, facts: facts, assetRoot: entry.root)
    defer { session.close() }
    let snapshot = try session.snapshot()
    for node in snapshot.nodes.values { _ = node.lines }
    #if EXACT_INSPECT
    measurement("archive", start: start, details: "kind=\(entry.kind) width=\(size.width) height=\(size.height) nodes=\(snapshot.nodes.count)")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
        widgetLog.notice("afterArchive pid=\(getpid()) peak=\(footprint())")
    }
    #endif
    return snapshot
}
struct ExactTimelineProvider: TimelineProvider {
    let kind: String
    private func preview(_ context: Context) -> ExactWidgetEntry {
        let date = Date(), root = embeddedRoot()
        let plan = (try? Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))) ?? Data()
        return ExactWidgetEntry(date: date, kind: kind, plan: plan, props: nil, root: root, placeholder: true)
    }
    func placeholder(in context: Context) -> ExactWidgetEntry { preview(context) }
    func getSnapshot(in context: Context, completion: @escaping (ExactWidgetEntry) -> Void) { completion(preview(context)) }
    func getTimeline(in context: Context, completion: @escaping (Timeline<ExactWidgetEntry>) -> Void) {
        #if EXACT_INSPECT
        let start = DispatchTime.now().uptimeNanoseconds
        #endif
        let date = Date()
        do {
            WidgetContainer.configure()
            let root = try WidgetContainer.pin()
            let plan = try Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))
            let refresh = try HeadlessSession(plan: plan, entry: kind, size: context.displaySize, facts: facts(.current, at: date), assetRoot: root)
            let rows: [[String: Any]]
            do {
                defer { refresh.close() }
                guard let answer = try JSONSerialization.jsonObject(with: refresh.timeline(at: date)) as? [[String: Any]], !answer.isEmpty, answer.count <= 100 else {
                    throw TimelineFailure(description: "refresh must return 1 to 100 timeline entries")
                }
                rows = answer
            }
            var last = -Double.infinity
            let entries = try rows.map { row -> ExactWidgetEntry in
                guard let ms = row["date"] as? Double, ms.isFinite, ms > last, let props = row["props"] else {
                    throw TimelineFailure(description: "timeline dates must be finite and strictly increasing")
                }
                last = ms
                let data = try JSONSerialization.data(withJSONObject: props, options: [.sortedKeys])
                return ExactWidgetEntry(date: Date(timeIntervalSince1970: ms / 1000), kind: kind, plan: plan, props: data, root: root, placeholder: false)
            }
            #if EXACT_INSPECT
            measurement("getTimeline", start: start, details: "kind=\(kind) entries=\(entries.count) bytes=\(plan.count)")
            #endif
            completion(Timeline(entries: entries, policy: .after(max(date.addingTimeInterval(900), entries.last!.date.addingTimeInterval(1)))))
        } catch {
            widgetLog.error("getTimeline refused: \(String(describing: error), privacy: .public)")
            let entry = ExactWidgetEntry(date: date, kind: kind, plan: Data(), props: nil, root: embeddedRoot(), placeholder: false, unavailable: true)
            completion(Timeline(entries: [entry], policy: .after(date.addingTimeInterval(900))))
        }
    }
}
#if EXACT_INSPECT
private func measurement(_ phase: String, start: UInt64, details: String) {
    let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
    widgetLog.notice("\(phase, privacy: .public) pid=\(getpid()) \(details, privacy: .public) ms=\(elapsed) peak=\(footprint())")
}
private func footprint() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
    let status = withUnsafeMutablePointer(to: &info) { pointer in
        pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
    }
    return status == KERN_SUCCESS ? max(info.phys_footprint, UInt64(max(0, info.ledger_phys_footprint_peak))) : 0
}
#endif
// SwiftUI has no public gamut environment key. The configuration seeds this
// from the archive's traits; no app or screen lookup is involved.
private struct WidgetDisplayGamut: EnvironmentKey { static let defaultValue = UIDisplayGamut.SRGB }
extension EnvironmentValues {
    var widgetDisplayGamut: UIDisplayGamut {
        get { self[WidgetDisplayGamut.self] }
        set { self[WidgetDisplayGamut.self] = newValue }
    }
}
struct ExactWidgetView: View {
    let entry: ExactWidgetEntry
    @Environment(\.widgetFamily) private var family
    @Environment(\.displayScale) private var scale
    @Environment(\.colorScheme) private var scheme
    @Environment(\.colorSchemeContrast) private var contrast
    @Environment(\.legibilityWeight) private var legibility
    @Environment(\.accessibilityReduceTransparency) private var transparency
    @Environment(\.accessibilityReduceMotion) private var motion
    @Environment(\.widgetDisplayGamut) private var gamut
    @Environment(\.dynamicTypeSize) private var textSize
    @Environment(\.widgetContentMargins) private var margins
    private var unavailable: some View { Text("Open \(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "the app")") }
    var body: some View {
        GeometryReader { geometry in
            if entry.unavailable { unavailable.padding(margins) }
            else {
                let size = CGSize(width: max(1, geometry.size.width - margins.leading - margins.trailing), height: max(1, geometry.size.height - margins.top - margins.bottom))
                let preferences = Preferences(reducedMotion: motion, reducedTransparency: transparency,
                    contrast: contrast == .increased ? "more" : "no-preference", dark: scheme == .dark,
                    gamut: gamut == .P3 ? "p3" : "srgb", highDynamicRange: false, boldText: legibility == .bold)
                let facts = SessionFacts(displayScale: scale, preferences: preferences, rootFontSize: bodySize, date: entry.date)
                if let snapshot = try? layout(entry, size: size, facts: facts) {
                    widgetSnapshotView(snapshot: snapshot, family: family, environment: WidgetSnapshotEnvironment(family: family, placeholder: entry.placeholder, assetRoot: entry.root, entryDate: entry.date))
                        .redacted(reason: entry.placeholder ? .placeholder : []).padding(margins)
                } else { unavailable.padding(margins) }
            }
        }.containerBackground(.background, for: .widget).privacySensitive()
    }
    private var bodySize: Double {
        let sizes: [DynamicTypeSize: Double] = [.xSmall: 14, .small: 15, .medium: 16, .large: 17, .xLarge: 19, .xxLarge: 21, .xxxLarge: 23, .accessibility1: 28, .accessibility2: 33, .accessibility3: 40, .accessibility4: 47, .accessibility5: 53]
        return sizes[textSize] ?? 17
    }
}
