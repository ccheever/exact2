import ExactCore
import ExactSnapshot
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
    let preview: Bool
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
private func snapshotFamily(_ family: WidgetFamily) -> SnapshotEnvironment.Family {
    switch family {
    case .systemSmall: .systemSmall
    case .systemMedium: .systemMedium
    case .systemLarge: .systemLarge
    case .systemExtraLarge: .systemExtraLarge
    case .accessoryCircular: .accessoryCircular
    case .accessoryRectangular: .accessoryRectangular
    case .accessoryInline: .accessoryInline
    @unknown default: .systemSmall
    }
}

// LLP 1126.000 WP5: the driver owns the archived layout and applies margins once.
private func widgetSnapshotView(_ entry: ExactWidgetEntry, displaySize: CGSize, contentSize: CGSize,
                                facts: SessionFacts, family: WidgetFamily) throws -> some View {
    let session = try HeadlessSession(plan: entry.plan, entry: entry.kind, props: entry.props,
                                      size: contentSize, facts: facts, assetRoot: entry.root)
    let environment = SnapshotEnvironment(
        family: snapshotFamily(family),
        // Both sample paths contain no account data. Only placeholder(in:) is redacted.
        placeholder: entry.preview,
        linkBase: (Bundle.main.object(forInfoDictionaryKey: "ExactLinkBase") as? String).flatMap(URL.init(string:)),
        imageRoots: [entry.root, embeddedRoot()], appGroup: WidgetContainer.root, entryDate: entry.date)
    #if EXACT_INSPECT
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
        widgetLog.notice("afterArchive pid=\(getpid()) peak=\(footprint())")
    }
    #endif
    // SnapshotView retains the session until WidgetKit has evaluated its layout and leaves.
    return SnapshotDriver.view(session: session, displaySize: displaySize, environment: environment)
}
struct ExactTimelineProvider: TimelineProvider {
    let kind: String
    private func preview(_ context: Context, placeholder: Bool) -> ExactWidgetEntry {
        let date = Date(), root = embeddedRoot()
        let plan = (try? Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))) ?? Data()
        return ExactWidgetEntry(date: date, kind: kind, plan: plan, props: nil, root: root, preview: true, placeholder: placeholder)
    }
    func placeholder(in context: Context) -> ExactWidgetEntry { preview(context, placeholder: true) }
    func getSnapshot(in context: Context, completion: @escaping (ExactWidgetEntry) -> Void) { completion(preview(context, placeholder: false)) }
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
                return ExactWidgetEntry(date: Date(timeIntervalSince1970: ms / 1000), kind: kind, plan: plan, props: data, root: root, preview: false, placeholder: false)
            }
            #if EXACT_INSPECT
            measurement("getTimeline", start: start, details: "kind=\(kind) entries=\(entries.count) bytes=\(plan.count)")
            #endif
            completion(Timeline(entries: entries, policy: .after(max(date.addingTimeInterval(900), entries.last!.date.addingTimeInterval(1)))))
        } catch {
            widgetLog.error("getTimeline refused: \(String(describing: error), privacy: .private)")
            let entry = ExactWidgetEntry(date: date, kind: kind, plan: Data(), props: nil, root: embeddedRoot(), preview: false, placeholder: false, unavailable: true)
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
            if entry.unavailable { unavailable.padding(margins).containerBackground(.background, for: .widget) }
            else {
                let size = CGSize(width: max(1, geometry.size.width - margins.leading - margins.trailing), height: max(1, geometry.size.height - margins.top - margins.bottom))
                let preferences = Preferences(reducedMotion: motion, reducedTransparency: transparency,
                    contrast: contrast == .increased ? "more" : "no-preference", dark: scheme == .dark,
                    gamut: gamut == .P3 ? "p3" : "srgb", highDynamicRange: false, boldText: legibility == .bold)
                let facts = SessionFacts(displayScale: scale, preferences: preferences, rootFontSize: bodySize, date: entry.date)
                if let view = try? widgetSnapshotView(entry, displaySize: geometry.size, contentSize: size, facts: facts, family: family) {
                    view.redacted(reason: entry.placeholder ? .placeholder : [])
                } else {
                    unavailable.padding(margins).containerBackground(.background, for: .widget)
                        .redacted(reason: entry.placeholder ? .placeholder : [])
                }
            }
        }.privacySensitive(!entry.preview)
    }

    private var bodySize: Double { SnapshotDriver.rootFontSize(for: textSize) }
}
