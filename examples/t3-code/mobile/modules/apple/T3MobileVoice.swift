#if os(iOS)
// @ref llp/1109.008-mobile-voice.decision.md#native-lifetime
import UIKit
import AVFoundation
import Speech

/// Lazy, local-only recorder. No audio is opened by status queries or agent fixtures.
final class T3MobileVoice: NSObject, AVAudioRecorderDelegate {
    let editor = T3MobileVoiceEditor()
    private let audioSession: T3MobileAudioSession
    private let audioOwner = UUID()
    private let agent: Bool
    private let changed: (String) -> Void
    private var alive = true
    private var session = ""
    private var phase = "idle"
    private var recorder: AVAudioRecorder?
    private var file: URL?
    private var locale = Locale.current.identifier
    private var timer: Timer?
    private var observers: [NSObjectProtocol] = []
    private var elapsed = 0
    private var levels = Array(repeating: -160.0, count: 64)
    private var event = 0
    private var eventKind = ""
    private var error = ""
    private var configured = false
    private var previousIdleDisabled: Bool?
    private var work: Task<Void, Never>?
    private var workID: UUID?
    private var completion: (([String: Any]) -> Void)?
    private var workGeneration = 0

    init(agent: Bool, audioSession: T3MobileAudioSession, changed: @escaping (String) -> Void) {
        self.audioSession = audioSession
        self.agent = agent; self.changed = changed; super.init()
        observers.append(NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.phase == "preparing" || self.phase == "recording" else { return }
            self.stopRecording(); self.cancelWork(); self.releaseAudio(); self.removeFile()
            self.emit("background", "Voice recording was interrupted.")
        })
        for notification in [AVAudioSession.interruptionNotification, AVAudioSession.mediaServicesWereResetNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: notification, object: nil, queue: .main) { [weak self] notice in
                guard let self, self.phase == "recording" else { return }
                if notification == AVAudioSession.interruptionNotification,
                   (notice.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) != AVAudioSession.InterruptionType.began.rawValue { return }
                self.stopRecording(); self.releaseAudio(); self.removeFile(); self.emit("interrupted", "Voice recording was interrupted.")
            })
        }
    }
    private var available: Bool {
        #if targetEnvironment(simulator)
        return false
        #else
        if #available(iOS 26, *) { return SpeechTranscriber.isAvailable }
        return false
        #endif
    }
    private func status() -> [String: Any] {
        ["available": available && !agent, "locale": locale,
         "reason": agent ? "Voice recording is unavailable during automated app drives." : available ? "" : "Voice transcription requires a supported device with iOS 26 or later.",
         "session": session, "phase": phase, "uri": file?.absoluteString ?? "", "event": event, "eventKind": eventKind,
         "error": error, "elapsed": elapsed, "levels": levels]
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.alive else { reply(Self.failure("cancelled", "The voice session was closed.", request)); return }
            self.handle(request, reply: reply)
        }
    }
    private static func failure(_ code: String, _ message: String, _ request: [String: Any]) -> [String: Any] {
        ["ok": false, "generation": request["generation"] as? Int ?? 0, "error": ["kind": code, "message": message]]
    }
    private func handle(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let action = request["action"] as? String ?? "", id = request["session"] as? String ?? ""
        func answer(_ value: [String: Any] = [:]) { reply(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": value]) }
        if action == "status" { answer(status()); return }
        if action == "selection" {
            do { answer(try editor.selection(owner: request["owner"] as? String ?? "", text: request["text"] as? String ?? "", optional: request["optional"] as? Bool == true)) }
            catch { reply(Self.failure("superseded", "The draft editor changed before voice input could start.", request)) }
            return
        }
        if action == "selection-commit" {
            editor.stage(owner: request["owner"] as? String ?? "", text: request["text"] as? String ?? "",
                start: request["start"] as? Int ?? 0, end: request["end"] as? Int ?? 0, revision: request["revision"] as? Int ?? 0)
            answer(); return
        }
        if action == "settings" {
            guard !agent, let url = URL(string: UIApplication.openSettingsURLString) else { answer(); return }
            UIApplication.shared.open(url); answer(); return
        }
        if action == "permission" {
            guard !agent, available else { reply(Self.failure("unavailable", "Voice transcription is not available.", request)); return }
            guard session.isEmpty || phase == "idle" || phase == "error" || session == id else {
                reply(Self.failure("busy", "Another voice recording is already active.", request)); return
            }
            session = id; phase = "preparing"; eventKind = ""; error = ""; elapsed = 0; levels = Array(repeating: -160, count: 64)
            AVAudioApplication.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, self.session == id, self.phase == "preparing" else { reply(Self.failure("cancelled", "Voice input was cancelled.", request)); return }
                    answer(["granted": granted, "canAskAgain": AVAudioApplication.shared.recordPermission == .undetermined])
                }
            }
            return
        }
        // Publish is permitted to establish a preparing state before permission, but cannot acquire audio.
        if action == "publish" {
            if session.isEmpty || phase == "idle" || phase == "error" || session == id {
                session = id; phase = request["phase"] as? String ?? "idle"; changed("t3.mobile-voice")
            }
            answer(); return
        }
        guard id == session else { reply(Self.failure("superseded", "The voice recording changed.", request)); return }
        do {
            if ["configure", "recorder-prepare", "record"].contains(action), UIApplication.shared.applicationState == .background {
                throw VoiceFailure("cancelled", "Voice input stopped when the app moved to the background.")
            }
            switch action {
            case "prepare":
                guard !agent, available else { throw VoiceFailure("unavailable", "Voice transcription is not available.") }
                startWork(request, reply: reply) {
                    if #available(iOS 26, *) {
                        let supported = try await T3MobileVoiceTranscription.prepare(self.locale)
                        return ["locale": supported]
                    }
                    throw VoiceFailure("unavailable", "Voice transcription requires iOS 26 or later.")
                }
            case "configure":
                guard !agent, available else { throw VoiceFailure("unavailable", "Voice recording is unavailable.") }
                let audio = AVAudioSession.sharedInstance()
                do { try audio.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker]); try audioSession.acquire(audioOwner); configured = true }
                catch { releaseAudio(force: true); throw error }
                answer()
            case "recorder-prepare":
                guard configured, !agent else { throw VoiceFailure("unavailable", "The recording session is not ready.") }
                removeFile()
                let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-voice-\(UUID().uuidString).m4a")
                file = url
                let recorder = try AVAudioRecorder(url: url, settings: [AVFormatIDKey: kAudioFormatMPEG4AAC,
                    AVSampleRateKey: 44100, AVNumberOfChannelsKey: 2, AVEncoderBitRateKey: 128000, AVEncoderAudioQualityKey: AVAudioQuality.max.rawValue])
                recorder.delegate = self; recorder.isMeteringEnabled = true
                guard recorder.prepareToRecord() else { throw VoiceFailure("recording", "Could not prepare voice recording.") }
                self.recorder = recorder; answer(["uri": url.absoluteString])
            case "record":
                guard let recorder, configured, !agent else { throw VoiceFailure("recording", "The recorder is not ready.") }
                guard recorder.record(forDuration: 300) else { throw VoiceFailure("recording", "Could not start voice recording.") }
                phase = "recording"; previousIdleDisabled = UIApplication.shared.isIdleTimerDisabled; UIApplication.shared.isIdleTimerDisabled = true
                timer?.invalidate(); timer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in self?.sample() }
                changed("t3.mobile-voice"); answer()
            case "stop": stopRecording(); answer(["uri": file?.absoluteString ?? ""])
            case "release":
                guard releaseAudio() else { throw VoiceFailure("recording", "Could not release the microphone audio session.") }; answer()
            case "delete":
                if request["uri"] as? String == file?.absoluteString { removeFile() }
                answer()
            case "cancel":
                cancelWork(); stopRecording(); releaseAudio(); removeFile(); phase = "idle"; changed("t3.mobile-voice"); answer()
            case "transcribe":
                guard let file, request["uri"] as? String == file.absoluteString, !agent else { throw VoiceFailure("transcription-failed", "This recording is no longer available.") }
                let language = request["locale"] as? String ?? locale
                startWork(request, reply: reply) {
                    if #available(iOS 26, *) { return ["transcript": try await T3MobileVoiceTranscription.transcribe(file, locale: language)] }
                    throw VoiceFailure("unavailable", "Voice transcription requires iOS 26 or later.")
                }
            default: throw VoiceFailure("voice", "Unknown voice operation.")
            }
        } catch {
            if ["configure", "recorder-prepare", "record"].contains(action) { stopRecording(); releaseAudio(); removeFile() }
            let failure = error as? VoiceFailure
            reply(Self.failure(failure?.code ?? "voice", failure?.message ?? error.localizedDescription, request))
        }
    }
    private func startWork(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void,
                           operation: @escaping () async throws -> [String: Any]) {
        guard work == nil else { reply(Self.failure("busy", "Voice transcription is still finishing.", request)); return }
        let id = UUID(); workID = id; completion = reply; workGeneration = request["generation"] as? Int ?? 0
        work = Task { @MainActor [weak self] in
            guard let self else { return }
            var result: [String: Any]
            do { try Task.checkCancellation(); let value = try await operation(); try Task.checkCancellation(); result = ["ok": true, "generation": self.workGeneration, "value": value] }
            catch { let failure = error as? VoiceFailure; result = Self.failure(failure?.code ?? "cancelled", failure?.message ?? error.localizedDescription, request) }
            guard self.workID == id else { return }
            let done = self.completion; self.completion = nil; self.work = nil; self.workID = nil; done?(result)
        }
    }
    private func cancelWork() { work?.cancel() }
    private func sample() {
        guard let recorder, recorder.isRecording else { return }
        recorder.updateMeters(); levels.removeFirst(); levels.append(Double(recorder.averagePower(forChannel: 0)))
        elapsed = min(300, max(0, Int(recorder.currentTime))); changed("t3.mobile-voice")
    }
    private func stopRecording() {
        timer?.invalidate(); timer = nil
        if let recorder { elapsed = min(300, max(0, Int(recorder.currentTime))); recorder.delegate = nil; recorder.stop() }
        if let previousIdleDisabled { UIApplication.shared.isIdleTimerDisabled = previousIdleDisabled; self.previousIdleDisabled = nil }
    }
    @discardableResult private func releaseAudio(force: Bool = false) -> Bool {
        guard configured || force else { return true }
        let audio = AVAudioSession.sharedInstance()
        try? audio.setCategory(.playback, mode: .default)
        if audioSession.release(audioOwner) { configured = false; return true }; return false
    }
    private func removeFile() { if let file { try? FileManager.default.removeItem(at: file) }; file = nil; recorder = nil }
    private func emit(_ kind: String, _ message: String = "") { event += 1; eventKind = kind; error = message; changed("t3.mobile-voice") }
    func audioRecorderDidFinishRecording(_ recorder: AVAudioRecorder, successfully flag: Bool) {
        guard self.recorder === recorder else { return }
        stopRecording(); releaseAudio(); emit(flag ? "finished" : "interrupted", flag ? "" : "Voice recording was interrupted.")
    }
    func audioRecorderEncodeErrorDidOccur(_ recorder: AVAudioRecorder, error: Error?) {
        guard self.recorder === recorder else { return }
        stopRecording(); releaseAudio(); emit("interrupted", error?.localizedDescription ?? "Voice recording was interrupted.")
    }
    func destroy() {
        alive = false; for observer in observers { NotificationCenter.default.removeObserver(observer) }; observers.removeAll()
        cancelWork(); stopRecording(); releaseAudio(); removeFile(); editor.destroy()
    }
}
#endif
