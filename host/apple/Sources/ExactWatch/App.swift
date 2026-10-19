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

/// The launch's phases for the startup budget (`rules/RULES.md`: an interactive
/// first frame at 100 ms p50), each in ms after the kernel started the process.
enum LaunchPhases {
    nonisolated(unsafe) private static var stamps: [(String, Date)] = []
    nonisolated(unsafe) private static var reported = false
    static func mark(_ phase: String) { stamps.append((phase, Date())) }
    static func report() {
        guard !reported else { return }
        reported = true
        let exec = processStart()
        let line = stamps.map { "\($0.0) \(Int(($0.1.timeIntervalSince(exec) * 1000).rounded()))" }.joined(separator: ", ")
        print("exact-watch phases exec 0, \(line); footprint \(String(format: "%.1f", footprintMiB())) MiB")
    }
}

/// The watch entry booted while SwiftUI sets up its scene (the startup budget,
/// LLP 1126.002 §4): `App.init` starts it at the screen's size on a background
/// queue (the session's work runs on Exact's owner thread); the root view takes
/// it at its first layout and resizes it to the size it was given.
final class EarlyBoot: @unchecked Sendable {
    private static let shared = EarlyBoot()
    private let done = DispatchSemaphore(value: 0)
    private var result: Result<(HeadlessSession, TimeInterval), Error>?
    private var taken = false

    /// The app's watch entry (`ExactWatchPlan`, LLP 1126.002 D4): its own live plan, booted
    /// from its bytes, which opens no update store.
    static func session(size: CGSize, facts: SessionFacts) throws -> HeadlessSession {
        guard let name = Bundle.main.object(forInfoDictionaryKey: "ExactWatchPlan") as? String else {
            throw HeadlessSession.Failure(description: "this app declares no watch entry (app.json `watch`)")
        }
        let plan = try Data(contentsOf: Bundle.main.bundleURL.appendingPathComponent(name))
        return try HeadlessSession(plan: plan, size: size, facts: facts, assetRoot: Bundle.main.bundleURL)
    }

    static func start() {
        let device = WKInterfaceDevice.current()
        let size = device.screenBounds.size, scale = Double(device.screenScale)
        DispatchQueue.global(qos: .userInteractive).async {
            LaunchPhases.mark("session")
            let start = liveUptime()
            // The watch draws dark; the reader's text size arrives with the first layout, which resizes.
            let facts = SessionFacts(displayScale: scale, preferences: [.dark],
                                     rootFontSize: Double(SnapshotDriver.rootFontSize(for: .large)), date: Date(), clock: 0)
            shared.result = Result { (try session(size: size, facts: facts), start) }
            LaunchPhases.mark("booted")
            shared.done.signal()
        }
    }

    /// The early session, waiting for it if it is still booting; nil after a failure or once taken.
    static func take() throws -> (HeadlessSession, TimeInterval)? {
        guard !shared.taken, Bundle.main.object(forInfoDictionaryKey: "ExactWatchPlan") != nil else { return nil }
        shared.taken = true
        shared.done.wait()
        return try? shared.result?.get()
    }
}

/// The app itself, live (LLP 1126 D10): the linked plan booted at the
/// screen's safe size, drawn by the live driver.
private struct LiveRoot: View {
    @State private var model: LiveModel?
    @State private var failure: String?
    @Environment(\.scenePhase) private var phase
    @Environment(\.isLuminanceReduced) private var dimmed
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
                            LaunchPhases.mark("first frame")
                            LaunchPhases.report()
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
        .onChange(of: dimmed) { _, dimmed in print("exact-watch luminance reduced \(dimmed)") }
    }

    private func boot(size: CGSize) {
        guard model == nil, failure == nil else { return }
        do {
            // The session `App.init` booted while SwiftUI set up its scene, else one booted now.
            let session: HeadlessSession, start: TimeInterval
            if let early = try EarlyBoot.take() {
                (session, start) = early
            } else {
                LaunchPhases.mark("session")
                start = liveUptime()
                let facts = SessionFacts(displayScale: scale, preferences: scheme == .dark ? [.dark] : [],
                                         rootFontSize: Double(SnapshotDriver.rootFontSize(for: textSize)), date: Date(), clock: 0)
                session = try EarlyBoot.session(size: size, facts: facts)
                LaunchPhases.mark("booted")
            }
            if try session.snapshot().size != size { try session.resize(to: size) }
            LaunchPhases.mark("sized")
            let live = try LiveModel(session: session, zero: start)
            LaunchPhases.mark("first batch")
            live.onCommand = { command in
                print("exact-watch command \(command.name) \(command.args)")
                switch command.name {
                case "haptic": Self.haptic(command.args.first as? String ?? "")
                default: log.notice("command \(command.name, privacy: .public) has no watch arm")
                }
            }
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

    /// `haptic(kind)` (iOS's kinds, `ExactKit` Haptics) as the watch's nearest
    /// `WKHapticType`; a kind with none is logged, as iOS logs one it does not play.
    private static func haptic(_ kind: String) {
        let type: WKHapticType? = switch kind {
        case "selection": .click
        case "success": .success
        case "warning": .retry
        case "error": .failure
        case "impact", "impact-light", "impact-medium", "impact-soft": .click
        case "impact-heavy", "impact-rigid": .start
        default: nil
        }
        guard let type else {
            Logger(subsystem: "dev.exact.watch", category: "live").notice("haptic \(kind, privacy: .public) is not one this platform plays")
            return
        }
        WKInterfaceDevice.current().play(type)
    }
}

@main struct ExactWatch: App {
    init() {
        LaunchPhases.mark("main")
        if Bundle.main.object(forInfoDictionaryKey: "ExactWatchPlan") != nil { EarlyBoot.start() }
        WidgetContainer.configure()
        setvbuf(stdout, nil, _IOLBF, 0) // the measurement lines reach `devicectl --console` as they are written
    }
    var body: some Scene {
        WindowGroup {
            // The app's own screen when it declares a watch entry; its routes bring their
            // own stack. Otherwise the complications' placeholders.
            if Bundle.main.object(forInfoDictionaryKey: "ExactWatchPlan") != nil {
                LiveRoot()
            } else {
                NavigationStack { Gallery() }
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
