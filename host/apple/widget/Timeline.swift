import ExactCore
import ExactSnapshot
import Foundation
import SwiftUI
import WidgetKit
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
    Bundle.main.bundleURL
}
private func snapshotFamily(_ family: WidgetFamily) -> SnapshotEnvironment.Family {
    switch family {
    #if !os(watchOS)
    case .systemSmall: .systemSmall
    case .systemMedium: .systemMedium
    case .systemLarge: .systemLarge
    case .systemExtraLarge, .systemExtraLargePortrait: .systemExtraLarge
    #endif
    #if os(watchOS)
    case .accessoryCorner: .accessoryCorner
    #endif
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
        imageRoots: [entry.root, embeddedRoot(), embeddedRoot().deletingLastPathComponent().deletingLastPathComponent()], appGroup: WidgetContainer.root, entryDate: entry.date)
    #if EXACT_INSPECT
    widgetLog.notice("render family=\(String(describing: family), privacy: .public) width=\(contentSize.width) height=\(contentSize.height)")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
        widgetLog.notice("afterArchive pid=\(getpid()) now=\(footprintNow()) peak=\(footprint())")
    }
    #endif
    // SnapshotView retains the session until WidgetKit has evaluated its layout and leaves.
    return SnapshotDriver.view(session: session, displaySize: displaySize, environment: environment)
}
// Families share an in-flight answer only within one generation and store revision.
private enum WidgetRefresh {
    static let queue = DispatchQueue(label: "exact.widget-refresh", qos: .userInitiated)
    /// The group's environment is set once, before any refresh reads it.
    static let configured: Void = WidgetContainer.configure()
    static let lock = NSLock()
    static var waiting: [WidgetTimelineCache: [(Timeline<ExactWidgetEntry>) -> Void]] = [:]
    static func begin(_ cache: WidgetTimelineCache, completion: @escaping (Timeline<ExactWidgetEntry>) -> Void) -> Bool {
        lock.lock(); defer { lock.unlock() }
        let first = waiting[cache] == nil
        waiting[cache, default: []].append(completion)
        return first
    }
    static func finish(_ cache: WidgetTimelineCache, _ timeline: Timeline<ExactWidgetEntry>, fallback: Timeline<ExactWidgetEntry>) {
        lock.lock()
        let callbacks = waiting.removeValue(forKey: cache) ?? []
        lock.unlock()
        for callback in callbacks { callback(cache.isCurrent ? timeline : fallback) }
    }
    static func entries(_ bytes: Data, kind: String, plan: Data, root: URL) throws -> [ExactWidgetEntry] {
        guard bytes.count <= 1 << 20,
              let rows = try JSONSerialization.jsonObject(with: bytes) as? [[String: Any]],
              !rows.isEmpty, rows.count <= 100 else {
            throw TimelineFailure(description: "refresh must return 1 to 100 entries within 1 MiB")
        }
        var last = -Double.infinity
        return try rows.map { row in
            guard let ms = row["date"] as? Double, ms.isFinite, ms > last, let props = row["props"] else {
                throw TimelineFailure(description: "timeline dates must be finite and strictly increasing")
            }
            last = ms
            return ExactWidgetEntry(date: Date(timeIntervalSince1970: ms / 1000), kind: kind, plan: plan,
                props: try JSONSerialization.data(withJSONObject: props, options: [.sortedKeys]), root: root, preview: false, placeholder: false)
        }
    }
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
        let start = DispatchTime.now()
        #endif
        let size = context.displaySize, date = Date(), inputs = SessionFacts(displayScale: 2, preferences: .platformInput, date: date)
        // Privacy modifiers can suppress redaction: an unredacted placeholder
        // would show fixture values as live account data. Fail with "Open app".
        var unavailable = preview(context, placeholder: true)
        unavailable.unavailable = true
        let fallback = Timeline(entries: [unavailable], policy: .after(date.addingTimeInterval(900)))
        _ = WidgetRefresh.configured
        // A binary update can precede the app's first launch/publication.
        let root = (try? WidgetContainer.pin()) ?? embeddedRoot()
        let generation = root == embeddedRoot()
            ? "embedded-" + (Bundle.main.object(forInfoDictionaryKey: "ExactWidgetGeneration") as? String ?? "")
            : root.lastPathComponent
        guard let group = WidgetContainer.root,
              let cache = try? WidgetTimelineCache(root: group, kind: kind, generation: generation) else {
            widgetLog.error("getTimeline refused: app group unavailable")
            completion(fallback)
            return
        }
        guard WidgetRefresh.begin(cache, completion: completion) else { return }
        WidgetRefresh.queue.async {
            // Kinds share this queue: each refresh's 900 ms, boot included,
            // starts when it leaves the queue, not while another kind runs.
            let deadline = DispatchTime.now() + .milliseconds(900)
            let timeline: Timeline<ExactWidgetEntry> = autoreleasepool {
                let plan = try? Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))
                do {
                    guard let plan else { throw TimelineFailure(description: "entry plan unavailable") }
                    let bytes = try cache.resolve {
                        guard DispatchTime.now() < deadline else { throw TimelineFailure(description: "refresh deadline expired") }
                        let refresh = try HeadlessSession(plan: plan, entry: kind, size: size, facts: inputs, assetRoot: root)
                        defer { refresh.close() }
                        let module = Bundle.main.object(forInfoDictionaryKey: "ExactWidgetTypeScript") as? Bool == true ? root : nil
                        let bytes = try refresh.timeline(at: date, moduleRoot: module, deadline: deadline)
                        _ = try WidgetRefresh.entries(bytes, kind: kind, plan: plan, root: root)
                        return bytes
                    }
                    let entries = try WidgetRefresh.entries(bytes, kind: kind, plan: plan, root: root)
                    #if EXACT_INSPECT
                    measurement("getTimeline", start: start.uptimeNanoseconds, details: "kind=\(kind) entries=\(entries.count) bytes=\(plan.count)")
                    #endif
                    return Timeline(entries: entries, policy: .after(max(date.addingTimeInterval(900), entries.last!.date.addingTimeInterval(1))))
                } catch {
                    #if EXACT_INSPECT
                    measurement("getTimeline refused", start: start.uptimeNanoseconds, details: "kind=\(kind)")
                    #endif
                    widgetLog.error("getTimeline refused: \(String(describing: error), privacy: .private)")
                    return fallback
                }
            }
            WidgetRefresh.finish(cache, timeline, fallback: fallback)
        }
    }
}
#if EXACT_INSPECT
private func measurement(_ phase: String, start: UInt64, details: String) {
    let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
    widgetLog.notice("\(phase, privacy: .public) pid=\(getpid()) \(details, privacy: .public) ms=\(elapsed) now=\(footprintNow()) peak=\(footprint())")
}
/// The resident footprint now, apart from the ledger's lifetime peak: a
/// series of these over renders tells a high-water mark from a leak.
private func footprintNow() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
    let status = withUnsafeMutablePointer(to: &info) { pointer in
        pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
    }
    return status == KERN_SUCCESS ? info.phys_footprint : 0
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
private struct WidgetDisplayGamut: EnvironmentKey { static let defaultValue = "srgb" }
extension EnvironmentValues {
    var widgetDisplayGamut: String {
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
                    gamut: gamut, highDynamicRange: false, boldText: legibility == .bold)
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
