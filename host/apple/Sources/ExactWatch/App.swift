// @ref LLP 1126.001 WP17 — the watch window shows the entries' placeholders.
#if os(watchOS)
import ExactCore
import ExactSnapshot
import SwiftUI
import WatchKit

private struct WatchEntry: Identifiable {
    let kind: String
    let title: String
    let families: [String]
    var id: String { kind }
    static var all: [Self] {
        (Bundle.main.object(forInfoDictionaryKey: "ExactWatchEntries") as? [[String: Any]] ?? []).compactMap { row in
            guard let kind = row["kind"] as? String, let families = row["families"] as? [String] else { return nil }
            return Self(kind: kind, title: row["title"] as? String ?? kind, families: families)
        }
    }
}
private struct WatchPlaceholder: View {
    let entry: WatchEntry
    let family: String
    @Environment(\.displayScale) private var scale
    @Environment(\.colorScheme) private var scheme
    @Environment(\.dynamicTypeSize) private var textSize
    var size: CGSize {
        // Gallery stand-ins scale with the case; the extension uses WidgetKit's proposal.
        let screen = WKInterfaceDevice.current().screenBounds.size
        switch family {
        case "accessoryCorner": return CGSize(width: screen.width * 39 / 211, height: screen.width * 39 / 211)
        case "accessoryCircular": return CGSize(width: screen.width * 51 / 211, height: screen.width * 51 / 211)
        case "accessoryInline": return CGSize(width: screen.width * 168 / 211, height: screen.height * 11.5 / 257)
        default: return CGSize(width: screen.width * 184 / 211, height: screen.height * 69.5 / 257)
        }
    }
    var body: some View {
        if let view = try? render() { view.frame(width: size.width, height: size.height) }
        else { Text("Open \(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "the app") on your iPhone") }
    }
    private func render() throws -> some View {
        let root = Bundle.main.bundleURL
        let plan = try Data(contentsOf: root.appendingPathComponent("plans/\(entry.kind).plan"))
        let session = try HeadlessSession(plan: plan, entry: entry.kind, size: size,
            facts: SessionFacts(displayScale: scale, preferences: scheme == .dark ? [.dark] : [], rootFontSize: SnapshotDriver.rootFontSize(for: textSize)), assetRoot: root)
        return SnapshotDriver.view(session: session, displaySize: size,
            environment: SnapshotEnvironment(family: SnapshotEnvironment.Family(rawValue: family)!, entryName: entry.kind, placeholder: true, imageRoots: [root], contentMargins: EdgeInsets(), rootFontSize: SnapshotDriver.rootFontSize(for: textSize)))
    }
}
@main struct ExactWatch: App {
    init() { WidgetContainer.configure() }
    var body: some Scene {
        WindowGroup {
            NavigationStack {
                List(WatchEntry.all) { entry in
                    #if EXACT_INSPECT
                    ForEach(entry.families, id: \.self) { family in
                        NavigationLink(family) { WatchPlaceholder(entry: entry, family: family).navigationTitle(entry.title) }
                    }
                    #else
                    VStack(alignment: .leading) {
                        Text(entry.title).font(.headline)
                        WatchPlaceholder(entry: entry, family: entry.families.contains("accessoryRectangular") ? "accessoryRectangular" : entry.families[0])
                    }
                    #endif
                }.navigationTitle(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "Entries")
            }
        }
    }
}
#else
@main enum ExactWatch { static func main() {} }
#endif
