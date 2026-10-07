// `<voice-call>` on iOS: Fleet's Codex voice page (fleet serve's
// `GET …/sessions/{id}/voice?thread=…`, through the relay) in a WKWebView.
// The page has the call: start, mute, end, captions, and the coding thread's
// approvals and questions; it negotiates WebRTC itself. This view supplies
// what a browser would: the bearer on the first request's header (never in
// the URL), the microphone for exactly that origin, playback without a second
// tap, a voice-chat audio session on the speaker, and the hang-up
// (`window.ochoEndVoice()`) when the sheet closes or the app leaves the
// foreground. Props: `url`, `auth` (`Bearer …`). Events: `message` with the
// page's `ochoVoice` states (`connecting`, `connected`, `ended`).
import Foundation

#if os(iOS)
import AVFoundation
import UIKit
import WebKit

final class VoiceCall: ExactNativeInstance {
    /// How long a closed call's page lives on to send its `stop`.
    private static let hangUpGrace: TimeInterval = 3
    /// Pages ending their calls after their sheet closed.
    private static var ending: Set<WKWebView> = []

    private let web: WKWebView
    private let bridge = Bridge()
    private var loaded = ""
    private var origin: URL?
    private var background: NSObjectProtocol?

    init(props: [String: String], events: ExactNativeEvents) {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        config.websiteDataStore = .nonPersistent()
        config.userContentController.add(bridge, name: "ochoVoice")
        web = WKWebView(frame: .zero, configuration: config)
        super.init(events: events)
        bridge.owner = self
        web.uiDelegate = bridge
        web.isOpaque = false
        web.backgroundColor = .systemBackground
        web.scrollView.contentInsetAdjustmentBehavior = .always
        background = NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.hangUp() }
        apply(props)
        events.load()
    }

    override var view: ExactNativeView { web }

    override func setProps(_ props: [String: String]) throws { apply(props) }

    private func apply(_ props: [String: String]) {
        let url = props["url"] ?? ""
        // One call per view: the address names the machine answering for the
        // phone, which can change mid-call (a failover, the Mac coming back),
        // and loading it again would hang up.
        guard loaded.isEmpty, let target = URL(string: url), ["https", "http"].contains(target.scheme ?? "") else { return }
        loaded = url
        origin = target
        var request = URLRequest(url: target)
        if let auth = props["auth"], !auth.isEmpty {
            request.setValue(auth, forHTTPHeaderField: "Authorization")
        }
        Self.audioForCall(true)
        web.load(request)
    }

    fileprivate func said(_ state: String) {
        events.message(state)
        if state == "ended" { Self.audioForCall(false) }
    }

    /// Whether `frame` belongs to the page this view loaded.
    fileprivate func sameOrigin(_ frame: WKSecurityOrigin) -> Bool {
        guard let origin else { return false }
        let port = origin.port ?? (origin.scheme == "https" ? 443 : 80)
        return frame.protocol == origin.scheme && frame.host == origin.host && frame.port == port
            || frame.protocol == origin.scheme && frame.host == origin.host && frame.port == 0
    }

    private func hangUp() {
        web.evaluateJavaScript("window.ochoEndVoice && window.ochoEndVoice()", completionHandler: nil)
    }

    override func destroy() {
        if let background { NotificationCenter.default.removeObserver(background) }
        hangUp()
        // The page's `stop` is a request in flight: keep the page alive
        // until it has gone, then let the audio session go.
        let web = self.web
        Self.ending.insert(web)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.hangUpGrace) {
            web.configuration.userContentController.removeScriptMessageHandler(forName: "ochoVoice")
            Self.ending.remove(web)
            Self.audioForCall(false)
        }
    }

    /// A voice chat's session: microphone and speaker together, the speaker
    /// rather than the earpiece unless something else is routed, Bluetooth
    /// headsets allowed.
    private static func audioForCall(_ on: Bool) {
        let session = AVAudioSession.sharedInstance()
        do {
            if on {
                try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.defaultToSpeaker, .allowBluetoothHFP])
                try session.setActive(true)
            } else if ending.isEmpty {
                try session.setActive(false, options: .notifyOthersOnDeactivation)
            }
        } catch {
            NSLog("voice-call: audio session: \(error)")
        }
    }

    /// The page's messages and WebKit's questions, without retaining the view.
    private final class Bridge: NSObject, WKScriptMessageHandler, WKUIDelegate {
        weak var owner: VoiceCall?

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any], let state = body["state"] as? String else { return }
            owner?.said(state)
        }

        // The microphone, for the voice page only; never the camera.
        func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
                     initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
                     decisionHandler: @escaping (WKPermissionDecision) -> Void) {
            let allowed = type == .microphone && owner?.sameOrigin(origin) == true
            decisionHandler(allowed ? .grant : .deny)
        }
    }
}
#endif
