// The iOS refresh wake's entry point, which the host keeps (LLP 1133.017
// D8): `BGTaskScheduler` takes a launch handler only before the app finishes
// launching, and a package's Swift loads with the app's module, after first
// pixel. So the host registers `<bundle id>.refresh` when Info.plist permits
// it (a grant line derived it, D14) and hands each task to the package that
// asks for it, through `NotificationCenter`, the one channel both sides
// share in the process:
//
// - `exact.background.refresh`: posted with the task, as an
//   `NSMutableDictionary` `{"task": BGAppRefreshTask}`; a package that takes
//   it sets `"taken"` and completes the task.
// - `exact.background.ask`: a package made after a task arrived asks, and
//   the task not taken is posted again.
//
// A task no package takes before its expiry ends unsuccessful. With no
// session booted none does: the headless boot is not built (D7).
#if os(iOS)
import BackgroundTasks
import Foundation

enum BackgroundRefresh {
    static let refresh = Notification.Name("exact.background.refresh")
    static let ask = Notification.Name("exact.background.ask")
    /// The task not yet taken.
    private static var waiting: BGTask?

    static func install() {
        let id = "\(Bundle.main.bundleIdentifier ?? "app").refresh"
        let permitted = Bundle.main.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String] ?? []
        guard permitted.contains(id) else { return }
        let registered = BGTaskScheduler.shared.register(forTaskWithIdentifier: id, using: .main) { task in
            task.expirationHandler = {
                if waiting === task { waiting = nil }
                task.setTaskCompleted(success: false)
            }
            offer(task)
        }
        if !registered { print("background: \(id) was not registered") }
        NotificationCenter.default.addObserver(forName: ask, object: nil, queue: .main) { _ in
            if let task = waiting { offer(task) }
        }
    }

    private static func offer(_ task: BGTask) {
        let offered = NSMutableDictionary(dictionary: ["task": task])
        NotificationCenter.default.post(name: refresh, object: offered)
        waiting = offered["taken"] as? Bool == true ? nil : task
    }
}

extension ExactApp {
    /// Install at launch, before `application(_:didFinishLaunchingWithOptions:)` returns.
    public func installBackgroundRefresh() { BackgroundRefresh.install() }
}
#endif
