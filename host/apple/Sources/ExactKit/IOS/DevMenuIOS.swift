// The dev menu (host apparatus, not app content): native UIKit above the
// presenter, so it is alive even when the plan is broken — the moment a
// reload matters most. Four fingers tapped once, or held, open the sheet;
// tapped twice, reload. Each journals a line, so a trace shows it fired. With a hardware keyboard (a simulator, an iPad): ⌘D for
// the sheet, ⌘R or ⇧⌘R to reload — the commands sit on the adapter's
// `AppDelegate`, the responder every chain ends at, so they fire whatever
// has focus (the Simulator's own File menu claims plain ⌘R for Record
// Screen; ⇧⌘R gets through). EXACT_DEV_MENU=0 removes all of it. The
// standalone adapter installs it (LLP 1031 D11); an embedder installs
// nothing.
#if os(iOS) || os(tvOS)
import UIKit

final class DevMenuTarget: NSObject, UIGestureRecognizerDelegate {
    @objc func menuTap(_ g: UIGestureRecognizer) {
        DevMenu.note("dev menu: four-finger tap")
        DevMenu.toggle()
    }
    @objc func menuPress(_ g: UIGestureRecognizer) {
        guard g.state == .began else { return }
        DevMenu.note("dev menu: four-finger press")
        DevMenu.toggle()
    }
    @objc func reloadTap(_ g: UIGestureRecognizer) {
        DevMenu.note("dev menu: four-finger double tap")
        DevMenu.reload()
    }
    /// Beside whatever the app's views recognize: a row's swipe, a list's
    /// pan, a context menu's press would otherwise win four fingers that
    /// drift a few points on glass, and the menu never opened on a phone.
    /// Never with each other: a hold that opened the sheet must not be a tap too.
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        other.delegate !== self
    }
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
        // Four fingers held: the trigger that survives fingers that drift
        // past a tap's slop, or a tap another recognizer took.
        let press = UILongPressGestureRecognizer(target: target, action: #selector(DevMenuTarget.menuPress(_:)))
        #if !os(tvOS)
        press.numberOfTouchesRequired = 4
        #endif
        press.minimumPressDuration = 0.6
        press.allowableMovement = 40
        press.delaysTouchesEnded = false
        // Once four touches are held, the row under them is not pressed on release.
        press.cancelsTouchesInView = true
        menu.require(toFail: press)
        reload.require(toFail: press)
        for recognizer in [reload, menu, press] {
            recognizer.delegate = target
            recognizer.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.direct.rawValue)]
            recognizer.allowedPressTypes = []
        }
        window.addGestureRecognizer(reload)
        window.addGestureRecognizer(menu)
        window.addGestureRecognizer(press)
    }

    /// One presentation waiting for a transition to finish: replaced by a
    /// newer one, dropped by a reload or by closing the menu.
    nonisolated(unsafe) static var pending = 0
    /// What that presentation is, while it waits.
    nonisolated(unsafe) static weak var waiting: UIViewController?

    /// `vc` over whatever is presented, once no presentation or dismissal
    /// is under way there (UIKit refuses one then, silently); a few tries.
    /// On an iPad, anchored mid-screen with no arrow (a bare action sheet is
    /// refused there).
    static func present(_ vc: UIViewController) {
        pending += 1
        let ticket = pending
        func go(_ attempt: Int) {
            guard ticket == pending, let c = presenter else { return }
            if c.isBeingPresented || c.isBeingDismissed || c.presentedViewController != nil {
                guard attempt < 8 else { waiting = nil; note("dev menu: not shown, a presentation never finished"); return }
                waiting = vc
                if attempt == 0 { note("dev menu: waiting for a presentation to finish") }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { go(attempt + 1) }
                return
            }
            if let pop = vc.popoverPresentationController {
                pop.sourceView = c.view
                pop.sourceRect = CGRect(x: c.view.bounds.midX, y: c.view.bounds.midY, width: 1, height: 1)
                pop.permittedArrowDirections = []
            }
            waiting = nil
            c.present(vc, animated: true)
        }
        go(0)
    }

    /// A line in the session's journal, so a trace shows the gesture fired.
    static func note(_ line: String) { session?.log(line) }

    /// Where the menu and its alerts present: over whatever the root
    /// controller has presented (a route's sheet, a UIKit sheet), never
    /// refused for it.
    static var presenter: UIViewController? {
        guard var top = controller else { return nil }
        while let next = top.presentedViewController, !next.isBeingDismissed { top = next }
        return top
    }

    public static func toggle() {
        // A sheet still held but no longer shown (dismissed by its own
        // action) is not open: show a new one.
        // One still waiting to open: a second toggle cancels it.
        if let s = sheet, waiting === s { pending += 1; waiting = nil; sheet = nil; return }
        if let s = sheet, s.presentingViewController != nil { pending += 1; s.dismiss(animated: true) } else { show() }
    }

    static func show() {
        guard controller != nil else { return }
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
        sheet = a
        present(a)
    }

    /// Save Trace (LLP 1079 D5): where it went, or why not, as an alert.
    static func saveTrace() {
        guard controller != nil, let session else { return }
        let message: String, saved: URL?
        switch session.saveTrace() {
        case .success(let url): message = url.path; saved = url
        case .failure(let error): message = "Not saved: \(error)"; saved = nil
        }
        let a = UIAlertController(title: "Trace", message: message, preferredStyle: .alert)
        // tvOS has no pasteboard and no share sheet.
        #if !os(tvOS)
        // A phone's trace reaches the Mac the agent reads it on: AirDrop or
        // Files from the share sheet, or `agent.mjs trace --phone` over the cable.
        if let saved { a.addAction(UIAlertAction(title: "Share…", style: .default) { _ in share(saved) }) }
        a.addAction(UIAlertAction(title: "Copy Path", style: .default) { _ in UIPasteboard.general.string = message })
        #endif
        a.addAction(UIAlertAction(title: "OK", style: .cancel))
        present(a)
    }

    #if !os(tvOS)
    /// The share sheet for a saved trace, anchored mid-screen on an iPad.
    static func share(_ url: URL) {
        guard controller != nil else { return }
        present(UIActivityViewController(activityItems: [url], applicationActivities: nil))
    }
    #endif

    /// The typed URL — what a physical iPhone actually uses (LLP 1023
    /// Stage 1: a device launch carries no environment): seeded with the
    /// last value, kept in defaults.
    static func openProject() {
        guard controller != nil else { return }
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
        present(a)
    }

    /// Restart: a live connection re-fetches; else from the dev loop's plan
    /// when one is named (the watcher's own path, state carried), else from
    /// the baked plan, fresh.
    public static func reload() {
        pending += 1
        sheet?.dismiss(animated: false)
        if ExactApp.shared.connectionStatus != nil { ExactApp.shared.reloadConnection(); return }
        guard let session else { return }
        if let path = planPath {
            ExactDevelopmentPlan(path).apply(to: ExactApp.shared)
            return
        }
        let started = CACurrentMediaTime()
        let batch = session.boot(size: session.presenter.viewportSize)
        // stderr, as on macOS, where stdout is the agent's reply channel.
        fputs("exact: reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")\n", stderr)
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
