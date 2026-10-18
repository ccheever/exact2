#if os(iOS)
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#media-presentation
import UIKit

/// One module owns native presentations. Source URLs are signed by the shared client; local paths never cross JS.
final class T3MobileMedia {
    private let files: T3MobileMediaFiles
    private let audioSession: T3MobileAudioSession
    private let audioOwner = UUID()
    private var file: T3MobileMediaFilePresentation?
    private var video: T3MobileMediaVideoPresentation?
    private var videoLease: URL?
    private var opening: Task<Void, Never>?
    private var activeID = ""
    private var activeCompletion: ((String) -> Void)?
    private final class ShareAttempt {
        enum Phase: String { case preparing, presented, cancelling, terminal }
        let identifier: String, routeKey: String, generation: Int
        weak var route: ExactRoute?
        var phase = Phase.preparing
        var task: Task<Void, Never>?
        var controller: UIActivityViewController?
        var visibility: T3MobileShareVisibility?
        var lease: URL?
        var dismissing = false
        var error: String?
        var reply: (([String: Any]) -> Void)?
        init(_ source: T3MobileMediaSource, route: ExactRoute?, generation: Int) {
            identifier = source.identifier; routeKey = route?.key ?? ""; self.route = route; self.generation = generation
        }
        var status: [String: Any] { ["identifier": identifier, "routeKey": routeKey, "phase": phase.rawValue, "message": error ?? ""] }
    }
    private var share: ShareAttempt?
    private var shareReceipt: [String: Any] = ["phase": "idle"]
    private let changed: (String) -> Void
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var alive = true

    init(dataRoot: URL, audioSession: T3MobileAudioSession, changed: @escaping (String) -> Void) {
        self.audioSession = audioSession; self.changed = changed; files = T3MobileMediaFiles(dataRoot: dataRoot)
    }
    func observe(_ route: ExactRoute) { routes[route.key] = RouteRef(route) }
    func end(_ route: ExactRoute) {
        if routes[route.key]?.value === route { routes.removeValue(forKey: route.key) }
        if let share, share.route === route { endShare(share, error: "The file route changed.") }
    }
    private static func current(_ attempt: ShareAttempt) -> Bool {
        guard let route = attempt.route, route.isLive, route.key == attempt.routeKey,
              (route.controller.viewIfLoaded?.window != nil || attempt.controller?.viewIfLoaded?.window != nil),
              let navigation = route.controller.navigationController, navigation.topViewController === route.controller else { return false }
        return [navigation.presentedViewController, route.controller.presentedViewController].allSatisfy {
            $0 == nil || $0 === attempt.controller
        }
    }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let instance = T3MobileMediaPresenter(owner: self, events: events)
        try instance.setProps(props); return instance
    }
    private func finish(_ identifier: String, message: String = "") {
        guard identifier == activeID else { return }
        file = nil; video = nil; opening = nil; activeID = ""
        audioSession.release(audioOwner)
        T3MobileMediaFiles.release(videoLease); videoLease = nil
        let completion = activeCompletion; activeCompletion = nil; completion?(message)
    }
    func present(_ source: T3MobileMediaSource, from presenter: UIViewController, completion: @escaping (String) -> Void) {
        guard alive, activeID.isEmpty, share == nil, presenter.view.window != nil else {
            completion("The media preview is no longer available."); return
        }
        presenter.view.endEditing(true)
        activeID = source.identifier; activeCompletion = completion
        audioSession.hold(audioOwner)
        if source.kind != "video" {
            let preview = T3MobileMediaFilePresentation(identifier: source.identifier) { [weak self] error in
                self?.finish(source.identifier, message: error?.localizedDescription ?? "")
            }
            file = preview; preview.present(source: source, files: files, from: presenter)
        } else {
            opening = Task { @MainActor [weak self] in
                guard let self else { return }
                do {
                    let url: URL
                    if let signed = source.url { url = signed }
                    else { url = try await files.prepare(source) }
                    guard !Task.isCancelled, alive, activeID == source.identifier else {
                        if url.isFileURL { T3MobileMediaFiles.release(url) }; return
                    }
                    if url.isFileURL { videoLease = url }
                    let preview = T3MobileMediaVideoPresentation(identifier: source.identifier, url: url, title: source.name) { [weak self] error in
                        self?.finish(source.identifier, message: error?.localizedDescription ?? "")
                    }
                    video = preview; preview.present(from: presenter)
                } catch { finish(source.identifier, message: error.localizedDescription) }
            }
        }
    }
    func cancel(_ identifier: String) {
        guard identifier == activeID else { return }
        opening?.cancel()
        if let file { file.dismiss() }
        else if let video { video.dismiss() }
        else { finish(identifier) }
    }
    private func presenter() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.filter { $0.activationState == .foregroundActive }
        guard var view = scenes.flatMap({ $0.windows }).first(where: { $0.isKeyWindow })?.rootViewController else { return nil }
        while let next = view.presentedViewController { view = next }
        return view
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let operation = request["op"] as? String ?? ""
        if operation == "mobileMediaShareStatus" {
            reply(["ok": true, "generation": generation, "value": share?.status ?? shareReceipt]); return
        }
        if operation == "mobileMediaShareCancel" {
            if let share, request["identifier"] as? String == share.identifier {
                endShare(share, error: "The file route changed.")
            }
            reply(["ok": true, "generation": generation, "value": share?.status ?? shareReceipt]); return
        }
        guard operation == "mobileMediaShare" else { reply(Self.failure("Unknown media operation.", generation)); return }
        do {
            guard alive, share == nil else { throw T3MobileMediaFiles.error("Another media presentation is still open.") }
            let source = try T3MobileMediaSource(request["sourceJSON"] as? String ?? "")
            let routeKey = request["routeKey"] as? String ?? ""
            let attempt = ShareAttempt(source, route: routes[routeKey]?.value, generation: generation)
            let presenter: UIViewController
            if !routeKey.isEmpty {
                guard Self.current(attempt), let route = attempt.route else { throw T3MobileMediaFiles.error("The file route changed.") }
                presenter = route.controller
            } else {
                guard let current = self.presenter() else { throw T3MobileMediaFiles.error("The presenting screen is no longer open.") }
                presenter = current; attempt.reply = reply
            }
            share = attempt; shareReceipt = [:]
            if !routeKey.isEmpty {
                let visibility = T3MobileShareVisibility { [weak self, weak attempt] in
                    guard let self, let attempt, !Self.current(attempt) else { return }
                    self.endShare(attempt, error: "The file route changed.")
                }
                presenter.addChild(visibility); presenter.view.addSubview(visibility.view); visibility.didMove(toParent: presenter)
                attempt.visibility = visibility
            }
            attempt.task = Task { @MainActor [self, attempt] in
                do {
                    let lease = try await files.prepare(source)
                    attempt.task = nil
                    guard share === attempt, !Task.isCancelled, alive, attempt.phase == .preparing,
                          presenter.view.window != nil, routeKey.isEmpty || Self.current(attempt) else {
                        T3MobileMediaFiles.release(lease); endShare(attempt, error: "The presenting screen is no longer open."); return
                    }
                    attempt.lease = lease
                    let activity = UIActivityViewController(activityItems: [lease], applicationActivities: nil)
                    attempt.controller = activity; activity.title = source.name
                    activity.overrideUserInterfaceStyle = presenter.traitCollection.userInterfaceStyle
                    activity.completionWithItemsHandler = { [weak self, weak attempt] _, _, _, error in
                        guard let attempt else { return }; self?.endShare(attempt, error: error?.localizedDescription)
                    }
                    if presenter.traitCollection.userInterfaceIdiom == .pad {
                        activity.popoverPresentationController?.sourceView = presenter.view
                        activity.popoverPresentationController?.sourceRect = CGRect(x: presenter.view.bounds.midX, y: presenter.view.bounds.midY, width: 1, height: 1)
                    } else { activity.modalPresentationStyle = .automatic }
                    attempt.phase = .presented
                    presenter.present(activity, animated: !UIAccessibility.isReduceMotionEnabled)
                    changed("t3.mobile-media")
                } catch {
                    attempt.task = nil; endShare(attempt, error: error.localizedDescription)
                }
            }
            if !routeKey.isEmpty { reply(["ok": true, "generation": generation, "value": attempt.status]) }
            changed("t3.mobile-media")
        } catch { reply(Self.failure(error.localizedDescription, generation)) }
    }
    private func endShare(_ attempt: ShareAttempt, error: String?) {
        guard share === attempt, attempt.phase != .terminal else { return }
        let beganCancelling = attempt.phase != .cancelling
        if beganCancelling { attempt.error = error; attempt.phase = .cancelling; attempt.task?.cancel() }
        if let controller = attempt.controller, !attempt.dismissing {
            attempt.dismissing = true
            let dismissed = { [self, attempt] in
                guard share === attempt else { return }
                attempt.controller = nil; attempt.dismissing = false; finishShare(attempt)
            }
            if controller.presentingViewController != nil || controller.isBeingPresented { controller.dismiss(animated: false, completion: dismissed) }
            else { dismissed() }
        }
        finishShare(attempt)
        if beganCancelling, share === attempt { changed("t3.mobile-media") }
    }
    private func finishShare(_ attempt: ShareAttempt) {
        guard share === attempt, attempt.phase == .cancelling, attempt.task == nil,
              attempt.controller == nil, !attempt.dismissing else { return }
        attempt.phase = .terminal
        if let visibility = attempt.visibility {
            visibility.willMove(toParent: nil); visibility.view.removeFromSuperview(); visibility.removeFromParent(); attempt.visibility = nil
        }
        T3MobileMediaFiles.release(attempt.lease); attempt.lease = nil
        shareReceipt = attempt.status; share = nil
        let reply = attempt.reply; attempt.reply = nil
        if let error = attempt.error { reply?(Self.failure(error, attempt.generation)) }
        else { reply?(["ok": true, "generation": attempt.generation, "value": [:]]) }
        changed("t3.mobile-media")
    }
    private static func failure(_ message: String, _ generation: Int) -> [String: Any] {
        ["ok": false, "generation": generation, "error": ["kind": "Media", "message": message]]
    }
    func destroy() {
        routes.removeAll(); alive = false; cancel(activeID)
        if let share { endShare(share, error: "The mobile session was closed.") }
    }
}

private final class T3MobileShareVisibility: UIViewController {
    private let hidden: () -> Void
    init(hidden: @escaping () -> Void) { self.hidden = hidden; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { nil }
    override func loadView() { view = UIView(); view.isUserInteractionEnabled = false; view.accessibilityElementsHidden = true }
    override func viewDidDisappear(_ animated: Bool) { super.viewDidDisappear(animated); hidden() }
}

private final class T3MobileMediaProbe: UIView {
    var attached: (() -> Void)?
    override func didMoveToWindow() { super.didMoveToWindow(); if window != nil { attached?() } }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

private final class T3MobileMediaPresenter: ExactNativeInstance {
    private let root = T3MobileMediaProbe()
    private weak var owner: T3MobileMedia?
    private var source: T3MobileMediaSource?
    private var started = ""
    private var alive = true
    override var view: UIView { root }
    init(owner: T3MobileMedia, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isAccessibilityElement = false; root.backgroundColor = .clear
        root.attached = { [weak self] in self?.start() }
    }
    override func setProps(_ props: [String: String]) throws {
        let next = try T3MobileMediaSource(props["media-source"] ?? "")
        if let source, source.identifier != next.identifier { owner?.cancel(source.identifier); started = "" }
        source = next; start()
    }
    private func start() {
        guard alive, let source, started != source.identifier, root.window != nil else { return }
        started = source.identifier
        // Wait for the route's own transition, not a guessed delay or polling clock.
        DispatchQueue.main.async { [weak self] in
            guard let self, alive, self.source?.identifier == source.identifier, root.window != nil else { return }
            var next: UIResponder? = root
            while let current = next, !(current is UIViewController) { next = current.next }
            guard let presenter = next as? UIViewController else { complete(source.identifier, "The presenting screen is no longer open."); return }
            let show = { [weak self, weak presenter] in
                guard let self, alive, self.source?.identifier == source.identifier, let presenter else { return }
                owner?.present(source, from: presenter) { [weak self] message in self?.complete(source.identifier, message) }
            }
            if let transition = presenter.transitionCoordinator { transition.animate(alongsideTransition: nil) { context in if !context.isCancelled { show() } } }
            else { show() }
        }
    }
    private func complete(_ identifier: String, _ message: String) {
        guard alive, source?.identifier == identifier,
              let data = try? JSONSerialization.data(withJSONObject: ["identifier": identifier, "message": message]) else { return }
        events.change(String(decoding: data, as: UTF8.self))
    }
    override func destroy() { alive = false; root.attached = nil; if let source { owner?.cancel(source.identifier) } }
}
#endif
