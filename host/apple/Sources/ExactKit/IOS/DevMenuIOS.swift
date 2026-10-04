// The dev menu (host apparatus, not app content): native UIKit above the
// presenter, so it is alive even when the plan is broken — the moment a
// reload matters most. Four fingers tapped once open the sheet; tapped
// twice, reload. With a hardware keyboard (a simulator, an iPad): ⌘D for
// the sheet, ⌘R or ⇧⌘R to reload — the commands sit on the adapter's
// `AppDelegate`, the responder every chain ends at, so they fire whatever
// has focus (the Simulator's own File menu claims plain ⌘R for Record
// Screen; ⇧⌘R gets through). EXACT_DEV_MENU=0 removes all of it. The
// standalone adapter installs it (LLP 1031 D11); an embedder installs
// nothing.
#if os(iOS) || os(tvOS)
import UIKit

final class DevMenuTarget: NSObject, UIGestureRecognizerDelegate {
    @objc func menuTap(_ g: UIGestureRecognizer) { DevMenu.toggle() }
    @objc func reloadTap(_ g: UIGestureRecognizer) { DevMenu.reload() }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive event: UIEvent) -> Bool {
        // Four fingers means direct touches, never hover/press events. A window
        // recognizer otherwise participates in UIKit's delayed-event queue even
        // for events with no touches (the physical iOS nil-insertion crash).
        event.type == .touches && !(event.allTouches?.isEmpty ?? true)
    }
}

public enum DevMenu {
    static let target = DevMenuTarget()
    nonisolated(unsafe) static weak var sheet: UIAlertController?
    public static var enabled: Bool { ExactEnv.environment["EXACT_DEV_MENU"] != "0" }
    /// The session the menu reloads and describes, and the controller that presents the sheet.
    nonisolated(unsafe) static weak var session: ExactSession?
    nonisolated(unsafe) static weak var controller: UIViewController?
    /// Where a reload without a connection restarts from (`EXACT_DEV_PLAN`, `EXACT_PLAN`), else the baked plan.
    nonisolated(unsafe) static var planPath: String?

    /// The gestures, on the window: above every node, indifferent to what
    /// the plan put there. A recognized tap cancels the touches it claimed
    /// (`NodeView.touchesCancelled` resets its press), and four-finger
    /// touches reach no system text gesture — those stop at three.
    public static func install(on window: UIWindow, session: ExactSession, controller: UIViewController, planPath: String?) {
        DevMenu.session = session
        DevMenu.controller = controller
        DevMenu.planPath = planPath
        guard enabled else { return }
        let reload = UITapGestureRecognizer(target: target, action: #selector(DevMenuTarget.reloadTap(_:)))
        #if !os(tvOS)
        reload.numberOfTouchesRequired = 4
        #endif
        reload.numberOfTapsRequired = 2
        // Developer shortcuts must not hold app touches across a URL restart.
        // iOS 26.6.1 crashed in UIKit's delayed-event queue on the physical phone.
        reload.delaysTouchesEnded = false
        let menu = UITapGestureRecognizer(target: target, action: #selector(DevMenuTarget.menuTap(_:)))
        #if !os(tvOS)
        menu.numberOfTouchesRequired = 4
        #endif
        menu.delaysTouchesEnded = false
        menu.require(toFail: reload)
        for recognizer in [reload, menu] {
            recognizer.delegate = target
            recognizer.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.direct.rawValue)]
            recognizer.allowedPressTypes = []
        }
        window.addGestureRecognizer(reload)
        window.addGestureRecognizer(menu)
    }

    public static func toggle() {
        if let s = sheet { s.dismiss(animated: true) } else { show() }
    }

    static func show() {
        guard let c = controller, c.presentedViewController == nil else { return }
        let text = info()
        let a = UIAlertController(title: "Exact", message: text, preferredStyle: .actionSheet)
        a.addAction(UIAlertAction(title: "Reload", style: .default) { _ in reload() })
        a.addAction(UIAlertAction(title: "Open Project…", style: .default) { _ in openProject() })
        #if !os(tvOS)
        a.addAction(UIAlertAction(title: "Copy Info", style: .default) { _ in UIPasteboard.general.string = text })
        #endif
        // LLP 1079 D5: the session's journal, frames and work, for the agent (`agent.mjs trace <file>`).
        if session?.sampler != nil { a.addAction(UIAlertAction(title: "Save Trace", style: .default) { _ in saveTrace() }) }
        a.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        // An iPad refuses a bare action sheet: anchor it mid-screen, no arrow.
        if let pop = a.popoverPresentationController {
            pop.sourceView = c.view
            pop.sourceRect = CGRect(x: c.view.bounds.midX, y: c.view.bounds.midY, width: 1, height: 1)
            pop.permittedArrowDirections = []
        }
        c.present(a, animated: true)
        sheet = a
    }

    /// Save Trace (LLP 1079 D5): where it went, or why not, as an alert.
    static func saveTrace() {
        guard let c = controller, let session else { return }
        let message: String
        switch session.saveTrace() {
        case .success(let url): message = url.path
        case .failure(let error): message = "Not saved: \(error)"
        }
        let a = UIAlertController(title: "Trace", message: message, preferredStyle: .alert)
        // tvOS has no pasteboard.
        #if !os(tvOS)
        a.addAction(UIAlertAction(title: "Copy Path", style: .default) { _ in UIPasteboard.general.string = message })
        #endif
        a.addAction(UIAlertAction(title: "OK", style: .cancel))
        c.present(a, animated: true)
    }

    /// The typed URL — what a physical iPhone actually uses (LLP 1023
    /// Stage 1: a device launch carries no environment): seeded with the
    /// last value, kept in defaults.
    static func openProject() {
        guard let c = controller else { return }
        let a = UIAlertController(title: "Open Project", message: "The app URL the dev server printed.", preferredStyle: .alert)
        a.addTextField { f in
            f.text = UserDefaults.standard.string(forKey: "exact.dev.url")
                ?? ExactApp.shared.connectedPage?.absoluteString ?? "http://"
            f.keyboardType = .URL
            f.autocapitalizationType = .none
            f.autocorrectionType = .no
        }
        a.addAction(UIAlertAction(title: "Connect", style: .default) { _ in
            let url = (a.textFields?.first?.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !url.isEmpty else { return }
            UserDefaults.standard.set(url, forKey: "exact.dev.url")
            ExactApp.shared.connect(url)
        })
        a.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        c.present(a, animated: true)
    }

    /// Restart: a live connection re-fetches; else from the dev loop's plan
    /// when one is named (the watcher's own path, state carried), else from
    /// the baked plan, fresh.
    public static func reload() {
        sheet?.dismiss(animated: false)
        if ExactApp.shared.connectionStatus != nil { ExactApp.shared.reloadConnection(); return }
        guard let session else { return }
        if let path = planPath {
            ExactDevelopmentPlan(path).apply(to: ExactApp.shared)
            return
        }
        let started = CACurrentMediaTime()
        let batch = session.boot(size: session.presenter.viewportSize)
        print("reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
    }

    static func info() -> String {
        let b = Bundle.main
        var lines = ["\(b.bundleIdentifier ?? "?") \(b.infoDictionary?["CFBundleShortVersionString"] as? String ?? "")"]
        if let status = ExactApp.shared.connectionStatus { lines.append("url: \(status)") }
        if let path = ExactEnv.environment["EXACT_DEV_PLAN"] {
            let m = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
            lines.append("plan: \(path)\(m.map { " (\(time.string(from: $0)))" } ?? " (missing)")")
        } else if let path = ExactEnv.environment["EXACT_PLAN"] {
            lines.append("plan: \(path)")
        } else {
            lines.append("plan: baked")
        }
        lines.append("app dir: \(ExactApp.shared.assetRoot.path)")
        if let s = ExactEnv.environment["EXACT_AGENT_SOCKET"] { lines.append("agent: \(s)") }
        if let session {
            let size = session.presenter.viewportSize
            lines.append("session: \(session.label) · viewport: \(Int(size.width))×\(Int(size.height)) · \(session.viewCount) views")
            lines.append("boot: \(String(format: "%.1f", session.bootMs)) ms")
        }
        return lines.joined(separator: "\n")
    }
    static let time: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()
}
#endif
