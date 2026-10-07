#if os(iOS)
// AudioFilePreview365aa87982. AVPlayer replaces expo-audio; Contract owns controls.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import AVFoundation
import UIKit

final class T3MobileAudio {
    private let session: T3MobileAudioSession
    init(session: T3MobileAudioSession) { self.session = session }
    private weak var active: T3MobileAudioView?
    func makeView(dataRoot: URL, props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let view = T3MobileAudioView(dataRoot: dataRoot, session: session, events: events)
        active?.destroy(); active = view
        try view.setProps(props); return view
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        guard let active, active.identifier == request["identifier"] as? String else {
            reply(["ok": false, "generation": generation, "error": ["kind": "superseded", "message": "The audio preview was closed."]]); return
        }
        active.control(request["operation"] as? String ?? "")
        reply(["ok": true, "generation": generation, "value": [:]])
    }
    func destroy() { active?.destroy(); active = nil }
}

private final class T3MobileAudioView: ExactNativeInstance {
    private let root = UIView()
    private let files: T3MobileMediaFiles
    private var player: AVPlayer?
    private var itemObservation: NSKeyValueObservation?
    private var rateObservation: NSKeyValueObservation?
    private var timer: Any?
    private var notifications: [NSObjectProtocol] = []
    private var lifecycleObservers: [NSObjectProtocol] = []
    private var opening: Task<Void, Never>?
    private var lease: URL?
    private var alive = true
    private var enabled = true
    private let audioSession: T3MobileAudioSession
    private let audioOwner = UUID()
    private var backgrounded = false
    private var interrupted = false
    private var resumeWanted = false
    private var command = 0
    private var failed = false
    private var finished = false
    private(set) var identifier = ""
    override var view: UIView { root }
    init(dataRoot: URL, session: T3MobileAudioSession, events: ExactNativeEvents) {
        audioSession = session
        files = T3MobileMediaFiles(dataRoot: dataRoot)
        super.init(events: events)
        root.isUserInteractionEnabled = false; root.isAccessibilityElement = false
        backgrounded = UIApplication.shared.applicationState == .background
        observeLifecycle()
    }
    override func setProps(_ props: [String: String]) throws {
        let source = try T3MobileMediaSource(props["media-source"] ?? "")
        enabled = props["audio-active"] != "false"
        if !enabled { pause() }
        guard alive, source.identifier != identifier else { return }
        cleanup(); identifier = source.identifier; failed = false; finished = false
        publish()
        opening = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let url: URL
                if let remote = source.url { url = remote } else { url = try await files.prepare(source) }
                guard alive, !Task.isCancelled, identifier == source.identifier else {
                    if url.isFileURL { T3MobileMediaFiles.release(url) }; return
                }
                if url.isFileURL { lease = url }
                install(url)
            } catch {
                guard alive, !Task.isCancelled, identifier == source.identifier else { return }
                failed = true; pause()
            }
        }
    }
    private func install(_ url: URL) {
        let item = AVPlayerItem(url: url)
        let next = AVPlayer(playerItem: item); player = next
        itemObservation = item.observe(\.status, options: [.initial, .new]) { [weak self, weak item] _, _ in
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive, let item, self.player?.currentItem === item else { return }
                if item.status == .failed { self.failed = true; self.pause() }
                self.publish()
            }
        }
        rateObservation = next.observe(\.rate, options: [.new]) { [weak self, weak next] _, _ in
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive, let next, self.player === next else { return }; self.publish()
            }
        }
        timer = next.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.5, preferredTimescale: 600), queue: .main) { [weak self, weak next] _ in
            guard let self, self.alive, let next, self.player === next else { return }; self.publish()
        }
        notifications.append(NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self, weak item] _ in
            guard let self, alive, let item, player?.currentItem === item else { return }
            finished = true; releaseAudio(); publish()
        })
        notifications.append(NotificationCenter.default.addObserver(forName: .AVPlayerItemFailedToPlayToEndTime, object: item, queue: .main) { [weak self, weak item] _ in
            guard let self, alive, let item, player?.currentItem === item else { return }
            failed = true; pause()
        })
    }
    private func observeLifecycle() {
        lifecycleObservers.append(NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, alive else { return }
            backgrounded = true; resumeWanted = resumeWanted || (player?.rate ?? 0) != 0
            suspend()
        })
        lifecycleObservers.append(NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, alive else { return }
            backgrounded = false; resumeIfAllowed()
        })
        lifecycleObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] event in
            guard let self, alive, let type = event.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt else { return }
            if type == AVAudioSession.InterruptionType.began.rawValue {
                interrupted = true; resumeWanted = resumeWanted || (player?.rate ?? 0) != 0
                suspend()
            } else if type == AVAudioSession.InterruptionType.ended.rawValue {
                interrupted = false
                let options = AVAudioSession.InterruptionOptions(rawValue: event.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0)
                resumeWanted = resumeWanted && options.contains(.shouldResume)
                resumeIfAllowed()
            }
        })
        lifecycleObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] event in
            if event.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue { self?.pause() }
        })
    }

    private func seconds(_ time: CMTime) -> Double { let value = time.seconds; return value.isFinite ? max(0, value) : 0 }
    private func publish() {
        guard alive, !identifier.isEmpty else { return }
        let value: [String: Any] = ["identifier": identifier, "loaded": player?.currentItem?.status == .readyToPlay,
            "playing": (player?.rate ?? 0) != 0, "currentTime": player.map { seconds($0.currentTime()) } ?? 0,
            "duration": player?.currentItem.map { seconds($0.duration) } ?? 0, "error": failed ? "playback" : ""]
        if let data = try? JSONSerialization.data(withJSONObject: value) { events.change(String(decoding: data, as: UTF8.self)) }
    }
    private func start(_ player: AVPlayer) {
        guard alive, enabled, !backgrounded, !interrupted, UIApplication.shared.applicationState == .active else { return }
        do {
            try audioSession.acquire(audioOwner)
            failed = false; player.play()
        } catch { failed = true }
        publish()
    }
    private func releaseAudio() { audioSession.release(audioOwner) }
    private func resumeIfAllowed() {
        guard resumeWanted, !backgrounded, !interrupted, enabled, let player else { return }
        resumeWanted = false; start(player)
    }
    private func suspend() { command += 1; player?.pause(); releaseAudio(); publish() }
    private func pause() { resumeWanted = false; suspend() }
    func control(_ operation: String) {
        guard alive, enabled, let player, let item = player.currentItem, item.status == .readyToPlay else { return }
        command += 1; resumeWanted = false
        let captured = command
        if operation == "toggle", player.rate != 0 { pause(); return }
        let position = seconds(player.currentTime()), duration = seconds(item.duration)
        let target: Double
        let play: Bool
        switch operation {
        case "back": target = max(0, position - 15); play = false
        case "forward": target = min(duration, position + 15); play = false
        case "toggle":
            if !finished && position < duration { start(player); return }
            target = 0; play = true
        default: return
        }
        failed = false; finished = false; publish()
        player.seek(to: CMTime(seconds: target, preferredTimescale: 600)) { [weak self, weak player] completed in
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive, let player, self.player === player, self.command == captured else { return }
                self.failed = !completed
                if completed && play { self.start(player) }
                self.publish()
            }
        }
    }
    private func cleanup() {
        command += 1; resumeWanted = false; opening?.cancel(); opening = nil
        player?.pause(); releaseAudio(); itemObservation = nil; rateObservation = nil
        if let timer { player?.removeTimeObserver(timer) }; timer = nil
        notifications.forEach(NotificationCenter.default.removeObserver); notifications = []
        player?.replaceCurrentItem(with: nil); player = nil
        T3MobileMediaFiles.release(lease); lease = nil
    }
    override func destroy() {
        guard alive else { return }; alive = false
        lifecycleObservers.forEach(NotificationCenter.default.removeObserver); lifecycleObservers = []
        cleanup()
    }
}
#endif
