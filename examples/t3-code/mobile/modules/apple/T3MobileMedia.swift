#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
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
    private var shareTask: Task<Void, Never>?
    private var shareController: UIActivityViewController?
    private var shareLease: URL?
    private var shareCompletion: (([String: Any]) -> Void)?
    private var shareGeneration = 0
    private var alive = true

    init(dataRoot: URL, audioSession: T3MobileAudioSession) { self.audioSession = audioSession; files = T3MobileMediaFiles(dataRoot: dataRoot) }
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
        guard alive, activeID.isEmpty, shareCompletion == nil, presenter.view.window != nil else {
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
        guard request["op"] as? String == "mobileMediaShare" else {
            reply(Self.failure("Unknown media operation.", generation)); return
        }
        do {
            guard alive, shareCompletion == nil, let presenter = presenter() else { throw T3MobileMediaFiles.error("The presenting screen is no longer open.") }
            let source = try T3MobileMediaSource(request["sourceJSON"] as? String ?? "")
            shareCompletion = reply; shareGeneration = generation
            shareTask = Task { @MainActor [weak self] in
                guard let self else { return }
                do {
                    let lease = try await files.prepare(source)
                    guard !Task.isCancelled, alive, shareCompletion != nil, presenter.view.window != nil else {
                        T3MobileMediaFiles.release(lease); finishShare("The presenting screen is no longer open."); return
                    }
                    shareLease = lease
                    let activity = UIActivityViewController(activityItems: [lease], applicationActivities: nil)
                    shareController = activity; activity.title = source.name
                    activity.overrideUserInterfaceStyle = presenter.traitCollection.userInterfaceStyle
                    activity.completionWithItemsHandler = { [weak self] _, _, _, error in self?.finishShare(error?.localizedDescription) }
                    if presenter.traitCollection.userInterfaceIdiom == .pad {
                        activity.popoverPresentationController?.sourceView = presenter.view
                        activity.popoverPresentationController?.sourceRect = CGRect(x: presenter.view.bounds.midX, y: presenter.view.bounds.midY, width: 1, height: 1)
                    } else { activity.modalPresentationStyle = .automatic }
                    presenter.present(activity, animated: !UIAccessibility.isReduceMotionEnabled)
                } catch { finishShare(error.localizedDescription) }
            }
        } catch { reply(Self.failure(error.localizedDescription, generation)) }
    }
    private func finishShare(_ error: String?) {
        let reply = shareCompletion; shareCompletion = nil; shareTask = nil; shareController = nil
        T3MobileMediaFiles.release(shareLease); shareLease = nil
        if let error { reply?(Self.failure(error, shareGeneration)) }
        else { reply?(["ok": true, "generation": shareGeneration, "value": [:]]) }
    }
    private static func failure(_ message: String, _ generation: Int) -> [String: Any] {
        ["ok": false, "generation": generation, "error": ["kind": "Media", "message": message]]
    }
    func destroy() {
        alive = false; cancel(activeID)
        shareTask?.cancel(); shareController?.dismiss(animated: false)
        finishShare("The mobile session was closed.")
    }
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
