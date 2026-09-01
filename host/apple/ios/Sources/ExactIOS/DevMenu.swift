// The dev menu (host apparatus, not app content): native UIKit above the
// presenter, so it is alive even when the plan is broken — the moment a
// reload matters most. Four fingers tapped once open the sheet; tapped
// twice, reload. With a hardware keyboard (a simulator, an iPad): ⌘D for
// the sheet, ⌘R or ⇧⌘R to reload — the commands sit on `AppDelegate`, the
// responder every chain ends at, so they fire whatever has focus (the
// Simulator's own File menu claims plain ⌘R for Record Screen; ⇧⌘R gets
// through). EXACT_DEV_MENU=0 removes all of it.
import UIKit

final class DevMenuTarget: NSObject {
    @objc func menuTap(_ g: UIGestureRecognizer) { DevMenu.toggle() }
    @objc func reloadTap(_ g: UIGestureRecognizer) { DevMenu.reload() }
}

enum DevMenu {
    static let target = DevMenuTarget()
    nonisolated(unsafe) static weak var sheet: UIAlertController?
    static var enabled: Bool { environment["EXACT_DEV_MENU"] != "0" }

    /// The gestures, on the window: above every node, indifferent to what
    /// the plan put there. A recognized tap cancels the touches it claimed
    /// (`NodeView.touchesCancelled` resets its press), and four-finger
    /// touches reach no system text gesture — those stop at three.
    static func install(on window: UIWindow) {
        guard enabled else { return }
        let reload = UITapGestureRecognizer(target: target, action: #selector(DevMenuTarget.reloadTap(_:)))
        reload.numberOfTouchesRequired = 4
        reload.numberOfTapsRequired = 2
        let menu = UITapGestureRecognizer(target: target, action: #selector(DevMenuTarget.menuTap(_:)))
        menu.numberOfTouchesRequired = 4
        menu.require(toFail: reload)
        window.addGestureRecognizer(reload)
        window.addGestureRecognizer(menu)
    }

    static func toggle() {
        if let s = sheet { s.dismiss(animated: true) } else { show() }
    }

    static func show() {
        guard let c = controller, c.presentedViewController == nil else { return }
        let text = info()
        let a = UIAlertController(title: "Exact", message: text, preferredStyle: .actionSheet)
        a.addAction(UIAlertAction(title: "Reload", style: .default) { _ in reload() })
        a.addAction(UIAlertAction(title: "Open Project…", style: .default) { _ in openProject() })
        a.addAction(UIAlertAction(title: "Copy Info", style: .default) { _ in UIPasteboard.general.string = text })
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

    /// The typed URL — what a physical iPhone actually uses (LLP 1023
    /// Stage 1: a device launch carries no environment): seeded with the
    /// last value, kept in defaults.
    static func openProject() {
        guard let c = controller else { return }
        let a = UIAlertController(title: "Open Project", message: "The app URL the dev server printed.", preferredStyle: .alert)
        a.addTextField { f in
            f.text = UserDefaults.standard.string(forKey: "exact.dev.url")
                ?? PlanURL.current?.page.absoluteString ?? "http://"
            f.keyboardType = .URL
            f.autocapitalizationType = .none
            f.autocorrectionType = .no
        }
        a.addAction(UIAlertAction(title: "Connect", style: .default) { _ in
            let url = (a.textFields?.first?.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !url.isEmpty else { return }
            UserDefaults.standard.set(url, forKey: "exact.dev.url")
            PlanURL.open(url)
        })
        a.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        c.present(a, animated: true)
    }

    /// Restart: from the dev loop's plan when one is named (EXACT_DEV_PLAN,
    /// else EXACT_PLAN) — the watcher's own path, state carried — else from
    /// the baked plan, fresh.
    static func reload() {
        sheet?.dismiss(animated: false)
        // A live URL session re-fetches (and clears a rebuilt stop).
        if let s = PlanURL.current { s.reload(); return }
        let started = CACurrentMediaTime()
        let size = presenter.viewport.bounds.size
        Exact.wake = exactWake
        let batch: Batch
        if let path = environment["EXACT_DEV_PLAN"] ?? environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
            batch = Exact.bootPlan(bytes, width: size.width, height: size.height)
        } else {
            batch = Exact.boot(width: size.width, height: size.height)
        }
        if let error = batch.error {
            FileHandle.standardError.write(Data("exact: \(error)\n".utf8))
        } else {
            presenter.reset()
            apply(batch)
            controller?.rebooted()
        }
        print("reloaded in \(String(format: "%.1f", (CACurrentMediaTime() - started) * 1000)) ms\(batch.error.map { " — \($0)" } ?? "")")
    }

    static func info() -> String {
        let b = Bundle.main
        var lines = ["\(b.bundleIdentifier ?? "?") \(b.infoDictionary?["CFBundleShortVersionString"] as? String ?? "")"]
        if let s = PlanURL.current {
            lines.append("url: \(s.page)\(s.terminal != nil ? " (rebuild the host)" : " (live)")")
        }
        if let path = environment["EXACT_DEV_PLAN"] {
            let m = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
            lines.append("plan: \(path)\(m.map { " (\(time.string(from: $0)))" } ?? " (missing)")")
        } else if let path = environment["EXACT_PLAN"] {
            lines.append("plan: \(path)")
        } else {
            lines.append("plan: baked")
        }
        lines.append("app dir: \(environment["EXACT_ASSETS"] ?? Bundle.main.bundlePath)")
        if let s = environment["EXACT_AGENT_SOCKET"] { lines.append("agent: \(s)") }
        let size = presenter.viewport.bounds.size
        lines.append("viewport: \(Int(size.width))×\(Int(size.height)) · \(presenter.views.count) views")
        lines.append("boot: \(String(format: "%.1f", bootMs)) ms")
        return lines.joined(separator: "\n")
    }
    static let time: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()
}
