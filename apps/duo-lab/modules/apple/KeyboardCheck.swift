import Foundation
#if os(iOS)
import UIKit
import Darwin

/// Observe geometry without driving layout or the keyboard. Screens with
/// focus-on-show claim a native editor during UIKit's transition and restore it on cancel.
private final class KeyboardCheck: NSObject, UINavigationControllerDelegate {
    private let temporary: URL
    private var routes: [ObjectIdentifier: ExactRoute] = [:]
    private var roots: [String: ExactElement] = [:]
    private var fields: [String: ExactElement] = [:]
    private weak var navigation: UINavigationController?
    private var link: CADisplayLink?
    private var observers: [NSObjectProtocol] = []
    private var started: CFTimeInterval?
    private var beganAt: Date?
    private var records: [[String: Any]] = []
    private var previous: NSDictionary?
    private var dropped = 0
    private var samples = 0
    private var samplingMs = 0.0
    private var worstSamplingMs = 0.0
    private static let limit = 3000

    init(temporary: URL) { self.temporary = temporary }

    func install(_ value: ExactNavigation) { value.delegate = self }

    func update(_ route: ExactRoute) {
        routes[ObjectIdentifier(route.controller)] = route
        guard route.data[.keyboardScreen] != nil else { return }
        let items = route.controller.navigationItem.rightBarButtonItems ?? []
        if items.contains(where: { $0.accessibilityIdentifier == "keyboard-record" }),
           items.contains(where: { $0.accessibilityIdentifier == "keyboard-share" }) { return }
        let record = UIBarButtonItem(title: link == nil ? "Record" : "Restart", style: .plain, target: self, action: #selector(record(_:)))
        record.accessibilityIdentifier = "keyboard-record"
        let share = UIBarButtonItem(title: "Share", style: .plain, target: self, action: #selector(share(_:)))
        share.accessibilityIdentifier = "keyboard-share"
        let authored = (route.controller.navigationItem.rightBarButtonItems ?? []).filter {
            !["keyboard-record", "keyboard-share"].contains($0.accessibilityIdentifier ?? "")
        }
        route.controller.navigationItem.rightBarButtonItems = [share, record] + authored
    }

    func remove(_ route: ExactRoute) { routes.removeValue(forKey: ObjectIdentifier(route.controller)) }

    func update(_ element: ExactElement) {
        element.reusable = true
        if element.hatch == .keyboardRoute { roots[element.id] = element }
        if element.hatch == .keyboardField { fields[element.id] = element }
    }

    func remove(_ element: ExactElement) {
        if fields[element.id] === element { capture("field-ended", details: ["field": element.id]) }
        if roots[element.id] === element { roots.removeValue(forKey: element.id) }
        if fields[element.id] === element { fields.removeValue(forKey: element.id) }
    }

    func navigationController(_ nav: UINavigationController, willShow controller: UIViewController, animated: Bool) {
        guard let route = routes[ObjectIdentifier(controller)], route.data[.keyboardScreen] != nil else { return }
        navigation = nav
        capture("willShow")
        if let transition = nav.transitionCoordinator {
            let source = transition.viewController(forKey: .from).flatMap { routes[ObjectIdentifier($0)] }
            var claimed = false
            transition.animate(alongsideTransition: { [weak self, weak route] _ in
                if let route { claimed = self?.claim(route) ?? false }
            }) { [weak self, weak source] context in
                if context.isCancelled, claimed, let source { self?.claim(source) }
            }
        } else if let id = route.data[.focusOnShow], !id.isEmpty { route.element(id)?.focus() }
    }

    @discardableResult
    private func claim(_ route: ExactRoute) -> Bool {
        guard let id = route.data[.focusOnShow], !id.isEmpty else { return false }
        // These test fields deliberately use UIKit; hatch focus() waits for a later turn
        guard let element = fields[id], element.isLive, let editor = element.platform as? UIView else {
            capture("focus-claim", details: ["field": id, "accepted": false, "reason": "native field is not mounted"])
            return false
        }
        let accepted = editor.isFirstResponder || editor.becomeFirstResponder()
        capture("focus-claim", details: ["field": id, "accepted": accepted, "delivery": "native"])
        return accepted
    }

    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) {
        capture("didShow")
    }

    @objc private func record(_ sender: UIBarButtonItem) {
        stop()
        records = []; previous = nil; dropped = 0; samples = 0; samplingMs = 0; worstSamplingMs = 0
        started = CACurrentMediaTime(); beganAt = Date()
        for route in routes.values where route.controller.viewIfLoaded?.window != nil {
            if let nav = route.controller.navigationController, nav.topViewController === route.controller { navigation = nav }
        }
        let names: [Notification.Name] = [UIResponder.keyboardWillShowNotification, UIResponder.keyboardWillHideNotification,
            UIResponder.keyboardWillChangeFrameNotification, UIResponder.keyboardDidChangeFrameNotification,
            UITextField.textDidBeginEditingNotification, UITextField.textDidEndEditingNotification,
            UITextView.textDidBeginEditingNotification, UITextView.textDidEndEditingNotification]
        observers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated { self?.notified(note) }
            }
        }
        let value = CADisplayLink(target: self, selector: #selector(tick))
        let maximum = Float(navigation?.view.window?.screen.maximumFramesPerSecond ?? 60)
        value.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: maximum, preferred: maximum)
        value.add(to: .main, forMode: .common)
        link = value
        refreshButtons()
        capture("recording-started")
    }

    func stop() {
        link?.invalidate(); link = nil
        observers.forEach(NotificationCenter.default.removeObserver); observers = []
        refreshButtons()
    }

    private func refreshButtons() {
        for route in routes.values {
            route.controller.navigationItem.rightBarButtonItems?.first {
                $0.accessibilityIdentifier == "keyboard-record"
            }?.title = link == nil ? "Record" : "Restart"
        }
    }

    @objc private func tick() {
        guard let started else { return }
        if CACurrentMediaTime() - started > 180 { capture("recording-limit"); stop(); return }
        let begin = CACurrentMediaTime()
        capture("frame", changesOnly: true)
        let cost = (CACurrentMediaTime() - begin) * 1000
        samples += 1; samplingMs += cost; worstSamplingMs = max(worstSamplingMs, cost)
    }

    private func notified(_ note: Notification) {
        var details: [String: Any] = [:]
        if let duration = note.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? NSNumber { details["duration"] = duration }
        if let curve = note.userInfo?[UIResponder.keyboardAnimationCurveUserInfoKey] as? NSNumber { details["curve"] = curve }
        if let frame = note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue { details["screenEndFrame"] = rect(frame.cgRectValue) }
        if let view = note.object as? UIView {
            details["field"] = fields.values.first { $0.platform === view }?.id ?? "unregistered"
            details["responder"] = view.isFirstResponder
        }
        capture(note.name.rawValue, details: details)
    }

    private func capture(_ event: String, changesOnly: Bool = false, details: [String: Any] = [:]) {
        guard link != nil || event == "share-requested" else { return }
        let state = snapshot()
        let value = state as NSDictionary
        if changesOnly, previous?.isEqual(value) == true { return }
        previous = value
        records.append(["ms": ((CACurrentMediaTime() - (started ?? CACurrentMediaTime())) * 1000).rounded(),
                        "event": event, "details": details, "state": state])
        if records.count > Self.limit { records.removeFirst(); dropped += 1 }
    }

    private func snapshot() -> [String: Any] {
        guard let nav = navigation, let window = nav.viewIfLoaded?.window else { return ["window": NSNull()] }
        let top = nav.topViewController.flatMap { routes[ObjectIdentifier($0)] }
        var state: [String: Any] = ["window": rect(window.bounds), "safeArea": insets(window.safeAreaInsets),
            "screen": top?.data[.keyboardScreen] ?? "", "stack": nav.viewControllers.compactMap { routes[ObjectIdentifier($0)]?.key },
            "interactive": nav.transitionCoordinator?.isInteractive ?? false,
            "percent": nav.transitionCoordinator?.percentComplete ?? 0]
        if let top {
            state["route"] = geometry(top.controller.view)
            if let root = roots["keyboard-route-\(top.key)"]?.view {
                state["root"] = geometry(root)
                state["ancestors"] = sequence(first: root.superview, next: { $0?.superview }).compactMap { $0 }.prefix(12).map(geometry)
            }
            if let scroll = top.contentScrollView { state["scroll"] = geometry(scroll) }
            if let search = top.controller.navigationItem.searchController { state["search"] = ["active": search.isActive, "field": geometry(search.searchBar.searchTextField)] }
        }
        state["fields"] = fields.keys.sorted().compactMap { id -> [String: Any]? in
            guard let element = fields[id], let view = element.view else { return nil }
            var value = geometry(view)
            value["id"] = id
            if let editor = element.platform as? UIView { value["editor"] = geometry(editor); value["responder"] = editor.isFirstResponder }
            if let cell = sequence(first: view.superview, next: { $0?.superview }).compactMap({ $0 as? UICollectionViewCell }).first,
               let list = cell.superview as? UICollectionView {
                value["cell"] = ["frame": rect(cell.frame), "visible": list.visibleCells.contains { $0 === cell }]
            }
            return value
        }
        return state
    }

    private func geometry(_ view: UIView) -> [String: Any] {
        var value: [String: Any] = ["class": String(describing: type(of: view)), "bounds": rect(view.bounds),
            "windowFrame": rect(view.convert(view.bounds, to: nil)), "inWindow": view.window != nil,
            "hidden": sequence(first: view, next: \.superview).contains { $0.isHidden }]
        if let presentation = view.layer.presentation() { value["presentationFrameInParent"] = rect(presentation.frame) }
        // Enumerate existing guides; reading keyboardLayoutGuide would create one
        if let guide = view.layoutGuides.first(where: { $0 is UIKeyboardLayoutGuide }) { value["keyboardGuide"] = rect(guide.layoutFrame) }
        if let scroll = view as? UIScrollView {
            value["offset"] = [scroll.contentOffset.x, scroll.contentOffset.y]
            value["contentSize"] = [scroll.contentSize.width, scroll.contentSize.height]
            value["inset"] = insets(scroll.contentInset); value["adjustedInset"] = insets(scroll.adjustedContentInset)
        }
        return value
    }

    private func rect(_ r: CGRect) -> [Double] { [r.minX, r.minY, r.width, r.height].map { Double(($0 * 100).rounded() / 100) } }
    private func insets(_ i: UIEdgeInsets) -> [CGFloat] { [i.top, i.left, i.bottom, i.right] }

    @objc private func share(_ sender: UIBarButtonItem) {
        guard let nav = navigation ?? routes.values.compactMap({ $0.controller.navigationController }).first(where: { $0.viewIfLoaded?.window != nil }),
              let controller = nav.topViewController else { return }
        navigation = nav
        var host = controller
        while let presented = host.presentedViewController { host = presented }
        // Freeze the evidence before presenting UI that may dismiss the keyboard
        capture("share-requested")
        stop()
        var system = utsname(); uname(&system)
        let machine = withUnsafeBytes(of: &system.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        let keys = ["CFBundleIdentifier", "CFBundleVersion", "ExactCommit", "ExactCommitDirty", "ExactBuildTime", "ExactBuildXcode", "ExactBuildKind"]
        let build = (Bundle.main.infoDictionary ?? [:]).filter { keys.contains($0.key) }
        let report: [String: Any] = ["schema": "exact-keyboard-device-test-1", "build": build,
            "device": ["model": machine, "os": UIDevice.current.systemVersion, "simulator": ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] != nil],
            "recordingStarted": beganAt.map { ISO8601DateFormatter().string(from: $0) } ?? "not recorded",
            "sampling": ["framesObserved": samples, "snapshotTimeMs": samplingMs, "worstSnapshotTimeMs": worstSamplingMs,
                         "retained": records.count, "dropped": dropped, "limitSeconds": 180], "records": records]
        do {
            try FileManager.default.createDirectory(at: temporary, withIntermediateDirectories: true)
            guard JSONSerialization.isValidJSONObject(report) else { throw CocoaError(.propertyListWriteInvalid) }
            let data = try JSONSerialization.data(withJSONObject: report, options: .sortedKeys)
            let url = temporary.appendingPathComponent("keyboard-test-\(Int(Date().timeIntervalSince1970 * 1000)).json")
            try data.write(to: url, options: .atomic)
            let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
            sheet.popoverPresentationController?.barButtonItem = sender
            host.present(sheet, animated: true)
        } catch {
            let alert = UIAlertController(title: "Diagnostics could not be saved", message: "\(error.localizedDescription) Try Share again.", preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: "OK", style: .default))
            host.present(alert, animated: true)
        }
    }
}
#endif

private final class KeyboardModule: ExactModule {
    #if os(iOS)
    private lazy var check = KeyboardCheck(temporary: context.temporary)
    override func navigation(_ value: ExactNavigation) { check.install(value) }
    override func route(_ value: ExactRoute) { check.update(value) }
    override func routeEnded(_ value: ExactRoute) { check.remove(value) }
    override func element(_ value: ExactElement) { check.update(value) }
    override func elementEnded(_ value: ExactElement) { check.remove(value) }
    override func destroy() { check.stop() }
    #endif
}

let exactModule: ExactModule.Type = KeyboardModule.self
