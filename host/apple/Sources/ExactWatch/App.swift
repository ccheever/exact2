// @ref LLP 1126 D10 — the watch window runs the app live (spike); its
// toolbar opens the entries' placeholders (LLP 1126.001 WP17).
#if os(watchOS)
import ExactCore
import ExactSnapshot
import os
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
/// The process's footprint in MiB (`phys_footprint`, what Xcode's memory gauge shows).
func footprintMiB() -> Double {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
    let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
    }
    return result == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : -1
}

/// When the kernel started this process (`kinfo_proc`), for the first-frame time.
private func processStart() -> Date {
    var info = kinfo_proc(), size = MemoryLayout<kinfo_proc>.stride
    var name: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
    guard sysctl(&name, 4, &info, &size, nil, 0) == 0 else { return Date() }
    let start = info.kp_proc.p_starttime
    return Date(timeIntervalSince1970: Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000)
}

/// The app itself, live (LLP 1126 D10): the linked plan booted at the
/// screen's safe size, drawn by the live driver.
private struct LiveRoot: View {
    @State private var model: LiveModel?
    @State private var failure: String?
    @Environment(\.scenePhase) private var phase
    @Environment(\.displayScale) private var scale
    @Environment(\.colorScheme) private var scheme
    @Environment(\.dynamicTypeSize) private var textSize
    private let log = Logger(subsystem: "dev.exact.watch", category: "live")

    var body: some View {
        GeometryReader { proxy in
            Group {
                if let model {
                    LiveView(model: model, rootFontSize: SnapshotDriver.rootFontSize(for: textSize))
                        .onAppear {
                            let since = Date().timeIntervalSince(processStart()) * 1000
                            log.notice("first frame \(since, format: .fixed(precision: 0)) ms after process start")
                            print("exact-watch first frame \(Int(since)) ms after process start; footprint \(String(format: "%.1f", footprintMiB())) MiB")
                        }
                } else if let failure {
                    ScrollView { Text(failure).font(.footnote) }
                } else {
                    ProgressView()
                }
            }
            .onChange(of: proxy.size, initial: true) { _, size in
                guard size.width > 0, size.height > 0 else { return }
                if let model { model.resize(to: size) } else { boot(size: size) }
            }
        }
        .onChange(of: phase) { _, phase in model?.setActive(phase == .active) }
        // The platform's Back: the navigation bar's chevron presses the author's Escape control.
        .toolbar {
            if let back = model?.escapeControl {
                ToolbarItem(placement: .topBarLeading) {
                    Button { model?.press(back.id) } label: { Image(systemName: "chevron.backward") }
                        .accessibilityLabel(back.label)
                }
            }
        }
    }

    private func boot(size: CGSize) {
        guard model == nil, failure == nil else { return }
        let start = liveUptime()
        let rootFontSize = SnapshotDriver.rootFontSize(for: textSize)
        let facts = SessionFacts(displayScale: scale, preferences: scheme == .dark ? [.dark] : [],
                                 rootFontSize: Double(rootFontSize), date: Date(), clock: 0)
        do {
            let session = try HeadlessSession(plan: nil, size: size, facts: facts, assetRoot: Bundle.main.bundleURL)
            let live = try LiveModel(session: session, zero: start)
            live.onPress = { _ in WKInterfaceDevice.current().play(.click) }
            var ticks = 0
            live.onTurn = { label in
                ticks += 1
                if label == "press" || ticks % 10 == 0 { print("exact-watch footprint \(String(format: "%.1f", footprintMiB())) MiB after \(ticks) turns") }
            }
            model = live
            #if EXACT_INSPECT
            WatchAgent.start(model: live)
            #endif
            let ms = (liveUptime() - start) * 1000
            log.notice("boot \(ms, format: .fixed(precision: 1)) ms at \(size.width, format: .fixed(precision: 0))x\(size.height, format: .fixed(precision: 0)) pt")
            print("exact-watch boot \(String(format: "%.1f", ms)) ms at \(Int(size.width))x\(Int(size.height)) pt")
        } catch {
            failure = "\(error)"
            log.error("boot failed: \(String(describing: error), privacy: .public)")
        }
    }
}

@main struct ExactWatch: App {
    init() {
        WidgetContainer.configure()
        setvbuf(stdout, nil, _IOLBF, 0) // the measurement lines reach `devicectl --console` as they are written
    }
    var body: some Scene {
        WindowGroup {
            NavigationStack {
                LiveRoot()
                    .toolbar {
                        if !WatchEntry.all.isEmpty {
                            ToolbarItem(placement: .topBarTrailing) {
                                NavigationLink { Gallery() } label: { Image(systemName: "square.grid.2x2") }
                            }
                        }
                    }
            }
        }
    }
}

/// The entries' complications, as placeholders.
private struct Gallery: View {
    var body: some View {
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
#else
@main enum ExactWatch { static func main() {} }
#endif
