import ExactCore
import Foundation
import SwiftUI
import WidgetKit
import UIKit
import os
import Darwin

private struct TimelineFailure: Error { let description: String }
private let widgetLog = Logger(subsystem: "dev.exact.widgets", category: "timeline")
struct ExactWidgetEntry: TimelineEntry {
    let date: Date
    let kind: String
    let plan: Data
    let props: Data?
    let root: URL
    let placeholder: Bool
    let snapshot: HeadlessSession.Snapshot?
}
private func embeddedRoot() -> URL {
    Bundle.main.bundleURL.deletingLastPathComponent().deletingLastPathComponent()
}
private func facts(_ traits: UITraitCollection, at date: Date) -> SessionFacts {
    SessionFacts(displayScale: max(1, traits.displayScale), preferences: Preferences(traits: traits),
                 rootFontSize: UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits).pointSize, date: date)
}
private func layout(plan: Data, kind: String, props: Data?, root: URL, size: CGSize, facts: SessionFacts) throws -> HeadlessSession.Snapshot {
    let session = try HeadlessSession(plan: plan, entry: kind, props: props, size: size, facts: facts, assetRoot: root)
    defer { session.close() }
    let snapshot = try session.snapshot()
    // Materialize while measurement caches are alive; timeline entries retain
    // only the laid-out values, never a runtime or its text cache.
    for node in snapshot.nodes.values { _ = node.lines }
    return snapshot
}
struct ExactTimelineProvider: TimelineProvider {
    let kind: String
    private func preview(_ context: Context) -> ExactWidgetEntry {
        let date = Date(), root = embeddedRoot()
        let plan = (try? Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))) ?? Data()
        let snapshot = try? layout(plan: plan, kind: kind, props: nil, root: root, size: context.displaySize, facts: facts(.current, at: date))
        return ExactWidgetEntry(date: date, kind: kind, plan: plan, props: nil, root: root, placeholder: true, snapshot: snapshot)
    }
    func placeholder(in context: Context) -> ExactWidgetEntry { preview(context) }
    func getSnapshot(in context: Context, completion: @escaping (ExactWidgetEntry) -> Void) { completion(preview(context)) }
    func getTimeline(in context: Context, completion: @escaping (Timeline<ExactWidgetEntry>) -> Void) {
        let start = DispatchTime.now().uptimeNanoseconds, date = Date()
        do {
            WidgetContainer.configure()
            let root = try WidgetContainer.pin()
            let plan = try Data(contentsOf: root.appendingPathComponent("plans/\(kind).plan"))
            let sessionFacts = facts(.current, at: date)
            let refresh = try HeadlessSession(plan: plan, entry: kind, size: context.displaySize, facts: sessionFacts, assetRoot: root)
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
                let entryDate = Date(timeIntervalSince1970: ms / 1000)
                let data = try JSONSerialization.data(withJSONObject: props, options: [.sortedKeys])
                let snapshot = try layout(plan: plan, kind: kind, props: data, root: root, size: context.displaySize, facts: sessionFacts)
                return ExactWidgetEntry(date: entryDate, kind: kind, plan: plan, props: data, root: root, placeholder: false, snapshot: snapshot)
            }
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
            widgetLog.notice("getTimeline kind=\(kind, privacy: .public) pid=\(getpid()) entries=\(entries.count) nodes=\(entries.first?.snapshot?.nodes.count ?? 0) bytes=\(plan.count) ms=\(elapsed) peak=\(footprint())")
            completion(Timeline(entries: entries, policy: .atEnd))
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                widgetLog.notice("afterArchive pid=\(getpid()) peak=\(footprint())")
            }
        } catch {
            widgetLog.error("getTimeline refused: \(String(describing: error), privacy: .public)")
            completion(Timeline(entries: [preview(context)], policy: .after(date.addingTimeInterval(900))))
        }
    }
}
private func footprint() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
    let status = withUnsafeMutablePointer(to: &info) { pointer in
        pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
    }
    return status == KERN_SUCCESS ? max(info.phys_footprint, UInt64(max(0, info.ledger_phys_footprint_peak))) : 0
}
struct ExactWidgetView: View {
    let entry: ExactWidgetEntry
    @Environment(\.widgetFamily) private var family
    @Environment(\.displayScale) private var scale
    @Environment(\.colorScheme) private var scheme
    @Environment(\.dynamicTypeSize) private var textSize
    @Environment(\.widgetContentMargins) private var margins
    var body: some View {
        GeometryReader { geometry in
            let size = CGSize(width: max(1, geometry.size.width - margins.leading - margins.trailing), height: max(1, geometry.size.height - margins.top - margins.bottom))
            let preferences: Preferences = scheme == .dark ? [.dark, .coarsePointer, .noHover] : [.coarsePointer, .noHover]
            let facts = SessionFacts(displayScale: scale, preferences: preferences, rootFontSize: bodySize, date: entry.date)
            let snapshot = try? layout(plan: entry.plan, kind: entry.kind, props: entry.props, root: entry.root, size: size, facts: facts)
            if let snapshot {
                widgetSnapshotView(snapshot: snapshot, family: family, environment: WidgetSnapshotEnvironment(family: family, placeholder: entry.placeholder, assetRoot: entry.root, entryDate: entry.date))
                    .padding(margins)
            } else { Text("Open the app to refresh").padding(margins) }
        }.containerBackground(.background, for: .widget)
    }
    private var bodySize: Double {
        let sizes: [DynamicTypeSize: Double] = [.xSmall: 14, .small: 15, .medium: 16, .large: 17, .xLarge: 19, .xxLarge: 21, .xxxLarge: 23, .accessibility1: 28, .accessibility2: 33, .accessibility3: 40, .accessibility4: 47, .accessibility5: 53]
        return sizes[textSize] ?? 17
    }
}
