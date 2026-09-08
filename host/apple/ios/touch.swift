// A reduction of the real-finger crash, approved by Charlie 2026-09-07.
// @ref LLP 1012 — agent taps do not synthesize UIKit touch events.
// Default: UIKit only. --presenter also compiles the actual ExactKit sources
// and links the client's archive; cases 0..5 do not create a runtime/session.
// --early-session moves case 6's session creation before UIApplicationMain.
// Cases add the input patterns from NodeViewIOS/DevMenuIOS cumulatively.
import UIKit

func record(_ message: String) {
    let line = "\(Date().timeIntervalSince1970) \(message)\n"
    FileHandle.standardError.write(Data(line.utf8))
    guard let directory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
    let path = directory.appendingPathComponent("touch.log")
    if let file = try? FileHandle(forWritingTo: path) {
        file.seekToEndOfFile()
        file.write(Data(line.utf8))
        try? file.close()
    } else { try? Data(line.utf8).write(to: path) }
}

final class ProbeWindow: UIWindow {
    override func sendEvent(_ event: UIEvent) {
        for touch in event.allTouches ?? [] where touch.phase != .moved && touch.phase != .stationary {
            let recognizers = (touch.gestureRecognizers ?? []).map { String(describing: type(of: $0)) }.joined(separator: ",")
            record("before touch phase=\(touch.phase.rawValue) view=\(String(describing: touch.view)) recognizers=\(recognizers)")
        }
        super.sendEvent(event)
    }
}

// The ordinary NodeView hit/press shape, reduced to a colored row.
final class ProbeNode: UIView {
    var pressed = false
    var activated: (() -> Void)?
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        for child in subviews.reversed() {
            if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
        }
        return self
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { pressed = true }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if !pressed { super.touchesMoved(touches, with: event) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard pressed else { return super.touchesEnded(touches, with: event) }
        pressed = false
        window?.endEditing(true)
        if let touch = touches.first, bounds.contains(touch.location(in: self)) { activated?() }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if pressed { pressed = false } else { super.touchesCancelled(touches, with: event) }
    }
    @objc func hovering(_ recognizer: UIHoverGestureRecognizer) { }
}

final class Controller: UIViewController, UIScrollViewDelegate, UIGestureRecognizerDelegate {
    var names: [String] {
        let base = ["Plain UIKit", "+ Exact hit/press shape", "+ Hover observers", "+ Four-finger menu", "+ View replacement"]
        #if EXACT_PRESENTER
        return base + ["Real views · no session", earlyStartup ? "Early session · scroll plan" : "Full Exact · scroll plan"]
        #else
        return base
        #endif
    }
    let titleLabel = UILabel(), status = UILabel(), scroll = UIScrollView()
    let nextButton = UIButton(type: .system), replace = UIButton(type: .system)
    var menuGestures: [UIGestureRecognizer] = []
    var mode = Int(ProcessInfo.processInfo.environment["TOUCH_CASE"] ?? "") ?? (Bundle.main.object(forInfoDictionaryKey: "ExactTouchStartCase") as? Int ?? 0)
    var taps = 0, scrolls = 0, generation = 0
    var replacement: DispatchWorkItem?
    #if EXACT_PRESENTER
    var actualPresenter: Presenter?
    var actualSession: ExactSession?
    var actualView: UIView?
    #endif

    override func viewDidLoad() {
        super.viewDidLoad()
        mode = min(max(mode, 0), names.count - 1)
        view.backgroundColor = .systemBackground
        titleLabel.font = .boldSystemFont(ofSize: 21)
        titleLabel.numberOfLines = 2
        status.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        status.numberOfLines = 3
        nextButton.setTitle("Next case →", for: .normal)
        nextButton.addTarget(self, action: #selector(nextCase), for: .touchUpInside)
        #if EXACT_PRESENTER
        // The early-session comparison is a cold-launch experiment; cycling
        // back would create a late session and mislabel it as early.
        nextButton.isEnabled = !earlyStartup
        #endif
        replace.setTitle("Replace rows", for: .normal)
        replace.addTarget(self, action: #selector(replaceRows), for: .touchUpInside)
        scroll.delegate = self
        scroll.contentInsetAdjustmentBehavior = .never
        for child in [titleLabel, status, nextButton, replace, scroll] { view.addSubview(child) }
        enterCase()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let safe = view.bounds.inset(by: view.safeAreaInsets)
        titleLabel.frame = CGRect(x: 20, y: safe.minY + 8, width: safe.width - 40, height: 58)
        status.frame = CGRect(x: 20, y: safe.minY + 68, width: safe.width - 40, height: 52)
        nextButton.frame = CGRect(x: 12, y: safe.minY + 120, width: safe.width / 2 - 12, height: 44)
        replace.frame = CGRect(x: safe.width / 2, y: safe.minY + 120, width: safe.width / 2 - 12, height: 44)
        scroll.frame = CGRect(x: 0, y: safe.minY + 170, width: safe.width, height: max(0, safe.height - 170))
        #if EXACT_PRESENTER
        actualView?.frame = scroll.frame
        if let p = actualPresenter {
            p.root.frame = CGRect(x: 0, y: 0, width: safe.width, height: 1150)
            p.viewport.contentSize = p.root.bounds.size
            if let nested = p.views[1] {
                nested.frame = CGRect(x: 12, y: 20, width: safe.width - 24, height: 280)
                nested.scroll?.frame = nested.bounds
                nested.content = CGSize(width: nested.bounds.width, height: 1450)
                nested.fitScroll()
                for row in nested.container.subviews where row is NodeView {
                    row.frame.size.width = nested.bounds.width
                    row.subviews.first?.frame = row.bounds.insetBy(dx: 12, dy: 8)
                }
            }
        }
        #endif
        layoutRows()
    }

    func layoutRows() {
        for (index, row) in scroll.subviews.filter({ $0.tag == 101 }).enumerated() {
            row.frame = CGRect(x: 20, y: CGFloat(index) * 72 + 10, width: max(0, scroll.bounds.width - 40), height: 60)
            row.subviews.first?.frame = row.bounds.insetBy(dx: 12, dy: 8)
        }
        scroll.contentSize = CGSize(width: scroll.bounds.width, height: 20 * 72 + 10)
    }

    func enterCase() {
        replacement?.cancel()
        if let window = view.window { for gesture in menuGestures { window.removeGestureRecognizer(gesture) } }
        menuGestures.removeAll()
        #if EXACT_PRESENTER
        actualView?.removeFromSuperview()
        actualView = nil
        actualPresenter = nil
        actualSession?.destroy()
        actualSession = nil
        #endif
        taps = 0; scrolls = 0
        titleLabel.text = "\(mode + 1)/\(names.count) · \(names[mode])"
        record("CASE \(mode) \(names[mode])")
        scroll.isHidden = mode >= 5
        replace.isEnabled = mode < 5
        #if EXACT_PRESENTER
        if mode >= 5 { mountActual(); updateStatus() } else { populate() }
        #else
        populate()
        #endif
        view.setNeedsLayout()
        // viewDidLoad precedes the window attachment on cold launch.
        DispatchQueue.main.async { [weak self] in self?.installMenu() }
    }

    #if EXACT_PRESENTER
    func mountActual() {
        if mode == 5 {
            let p = Presenter()
            actualPresenter = p
            p.onPress = { [weak self] _ in self?.rowTapped() }
            let nested = NodeView(id: 1, kind: "view", presenter: p)
            p.views[1] = nested
            p.root.addSubview(nested)
            nested.applyStyle(["overflow_y": "scroll", "background_color": "#e0eeee"])
            for i in 0..<20 {
                let row = NodeView(id: UInt32(i + 2), kind: "view", presenter: p)
                p.views[row.id] = row
                row.handlers = ["press"]
                row.frame = CGRect(x: 0, y: i * 72, width: 300, height: 60)
                let label = UILabel()
                label.text = "Real NodeView row \(i + 1)"
                row.addSubview(label)
                nested.container.addSubview(row)
            }
            let below = UILabel(frame: CGRect(x: 20, y: 340, width: 300, height: 50))
            below.text = "Outer scroll — drag here too"
            p.root.addSubview(below)
            actualView = p.viewport
            record("REAL views: production Presenter/NodeView/ScrollView; no session created")
        } else {
            guard let url = Bundle.main.url(forResource: "scroll", withExtension: "plan"), let bytes = try? Data(contentsOf: url) else {
                record("ERROR missing bundled scroll.plan"); return
            }
            let session = earlySession ?? ExactApp.shared.makeSession(label: "touch-repro")
            earlySession = nil
            actualSession = session
            let batch = session.boot(plan: bytes, size: CGSize(width: view.bounds.width, height: 500))
            record("REAL session: views=\(session.viewCount) error=\(batch.error ?? "none"); no URL/store/menu")
            // Never let ExactView fall back to the baked Caltrain plan after a
            // fixture refusal: that would silently change the experiment.
            guard batch.error == nil else {
                session.destroy()
                actualSession = nil
                let error = UILabel()
                error.numberOfLines = 0
                error.text = "Fixture refused — do not test this case.\n\(batch.error!)"
                actualView = error
                view.addSubview(error)
                return
            }
            actualView = ExactView(session: session)
        }
        if let actualView { view.addSubview(actualView) }
    }
    #endif

    func populate() {
        generation += 1
        for row in scroll.subviews where row.tag == 101 { row.removeFromSuperview() }
        for index in 0..<20 {
            let row: UIView
            if mode == 0 {
                let button = UIButton(type: .system)
                button.setTitle("Tap row \(index + 1)", for: .normal)
                button.addTarget(self, action: #selector(rowTapped), for: .touchUpInside)
                row = button
            } else {
                let node = ProbeNode()
                let label = UILabel()
                label.text = "Tap row \(index + 1)"
                node.addSubview(label)
                node.activated = { [weak self] in self?.rowTapped() }
                if mode >= 2 {
                    let hover = UIHoverGestureRecognizer(target: node, action: #selector(ProbeNode.hovering(_:)))
                    hover.delaysTouchesBegan = false
                    hover.delaysTouchesEnded = false
                    hover.cancelsTouchesInView = false
                    node.addGestureRecognizer(hover)
                }
                row = node
            }
            row.tag = 101
            row.backgroundColor = index.isMultiple(of: 2) ? .systemTeal.withAlphaComponent(0.2) : .systemOrange.withAlphaComponent(0.2)
            scroll.addSubview(row)
        }
        layoutRows()
        updateStatus()
        record("ROWS case=\(mode) generation=\(generation)")
    }

    func updateStatus() {
        status.text = "Taps \(taps) · Scrolls \(scrolls) · Rows v\(generation)\nTap a row; scroll up/down.\nThen tap Next case."
    }
    @objc func rowTapped() { taps += 1; updateStatus(); record("TAP case=\(mode) count=\(taps)") }
    @objc func nextCase() { mode = (mode + 1) % names.count; enterCase() }
    @objc func replaceRows() { populate() }
    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        scrolls += 1; updateStatus(); record("SCROLL case=\(mode) count=\(scrolls)")
        if mode == 4 {
            replacement?.cancel()
            let work = DispatchWorkItem { [weak self] in self?.populate() }
            replacement = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
        }
    }

    func installMenu() {
        guard (3...4).contains(mode), menuGestures.isEmpty, let window = view.window else { return }
        let single = UITapGestureRecognizer(target: self, action: #selector(menuTap))
        let double = UITapGestureRecognizer(target: self, action: #selector(replaceRows))
        double.numberOfTapsRequired = 2
        single.require(toFail: double)
        for gesture in [single, double] {
            gesture.numberOfTouchesRequired = 4
            gesture.delaysTouchesEnded = false
            gesture.delegate = self
            gesture.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.direct.rawValue)]
            gesture.allowedPressTypes = []
            window.addGestureRecognizer(gesture)
        }
        menuGestures = [single, double]
    }
    func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldReceive event: UIEvent) -> Bool {
        event.type == .touches && !(event.allTouches?.isEmpty ?? true)
    }
    @objc func menuTap() {
        record("MENU case=\(mode)")
        let alert = UIAlertController(title: "Touch test", message: "Four-finger tap recognized", preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        present(alert, animated: true)
    }
}

final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let config = UISceneConfiguration(name: nil, sessionRole: session.role)
        config.delegateClass = SceneDelegate.self
        return config
    }
}
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = ProbeWindow(windowScene: scene)
        window.rootViewController = Controller()
        self.window = window
        window.makeKeyAndVisible()
        #if EXACT_PRESENTER
        record("READY pid=\(getpid()) Exact sources linked; runtime only in case 6")
        #else
        record("READY pid=\(getpid()) pure UIKit")
        #endif
    }
}
#if EXACT_PRESENTER
let earlyStartup = ProcessInfo.processInfo.environment["TOUCH_EARLY_SESSION"].map { $0 == "1" }
    ?? (Bundle.main.object(forInfoDictionaryKey: "ExactTouchEarlySession") as? Bool ?? false)
var earlySession: ExactSession?
if earlyStartup {
    earlySession = ExactApp.shared.makeSession(label: "before-UIApplicationMain")
    record("EARLY session created before UIApplicationMain; views=\(earlySession!.viewCount)")
}
record("ENTER UIApplicationMain early-session=\(earlyStartup)")
#endif
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
