// @ref llp/1106.010-mobile-browser-devices.decision.md#native-lifetime
#if os(iOS)
// Pinned365aa87982 PreviewStreamWebView / DeviceStreamWebView: native ownership around bundled source viewers.
import UIKit
import WebKit

struct T3MobileBrowserTarget: Equatable {
    let owner: String, origin: String, environment: String, thread: String, tab: String, host: String, device: String, platform: String
    let generation: Int, operate: Bool
    init(_ value: [String: Any]) throws {
        guard let owner = value["owner"] as? String, !owner.isEmpty,
              let origin = value["origin"] as? String, let url = URL(string: origin), ["http", "https"].contains(url.scheme ?? ""),
              let environment = value["environmentId"] as? String, !environment.isEmpty,
              let thread = value["threadId"] as? String, !thread.isEmpty, let generation = value["generation"] as? Int else {
            throw ExactNativeRefusal("The preview target is unavailable.")
        }
        self.owner = owner; self.origin = origin; self.environment = environment; self.thread = thread; self.generation = generation
        tab = value["tabId"] as? String ?? ""; host = value["hostId"] as? String ?? ""; device = value["deviceId"] as? String ?? ""
        platform = value["platform"] as? String ?? ""; operate = value["operate"] as? Bool == true
    }
    func matches(_ transport: T3Transport) -> Bool {
        transport.alive && transport.state == "connected" && transport.generation == generation &&
            transport.origin?.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/")) == origin.trimmingCharacters(in: CharacterSet(charactersIn: "/")) &&
            transport.descriptor["environmentId"] as? String == environment
    }
}

class T3MobileBrowser {
    let audioSession: T3MobileAudioSession
    let transport: T3Transport, changed: (String) -> Void, devices: Bool, agent: Bool
    private struct WeakView { weak var value: T3MobileBrowserView? }
    private var views: [String: WeakView] = [:]
    init(transport: T3Transport, changed: @escaping (String) -> Void, agent: Bool, audioSession: T3MobileAudioSession, devices: Bool = false) {
        self.audioSession = audioSession; self.transport = transport; self.changed = changed; self.agent = agent; self.devices = devices
    }
    var topic: String { devices ? "t3.mobile-devices" : "t3.mobile-browser" }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let view = T3MobileBrowserView(owner: self, events: events); try view.setProps(props); return view
    }
    func register(_ view: T3MobileBrowserView, key: String) { views[key] = WeakView(value: view); changed(topic) }
    func unregister(_ view: T3MobileBrowserView, key: String) { if views[key]?.value === view { views.removeValue(forKey: key); changed(topic) } }
    func check(_ target: T3MobileBrowserTarget, done: @escaping (Bool) -> Void) {
        transport.queue.async { [weak self] in
            guard let self else { return DispatchQueue.main.async { done(false) } }
            let valid = target.matches(transport); DispatchQueue.main.async { done(valid) }
        }
    }
    /// Check identity before and after each native await. Tickets never return through Exact.
    func access(_ target: T3MobileBrowserTarget, done: @escaping ([String: Any]?) -> Void) {
        transport.queue.async { [weak self] in
            guard let self, target.matches(transport) else { return DispatchQueue.main.async { done(nil) } }
            transport.perform(["op": "http", "path": "/api/auth/session", "generation": target.generation]) { [weak self] response in
                guard let self, target.matches(transport), response["ok"] as? Bool == true,
                      let session = response["value"] as? [String: Any], session["authenticated"] as? Bool == true else { return DispatchQueue.main.async { done(nil) } }
                let grants = session["permissions"] == nil ? session["scopes"] as? [String] ?? [] : session["permissions"] as? [String] ?? []
                guard grants.contains("orchestration:read") else { return DispatchQueue.main.async { done(nil) } }
                transport.deviceHubAccess { [weak self] origin, ticket in
                    guard let self, target.matches(transport), let origin, let ticket else { return DispatchQueue.main.async { done(nil) } }
                    let base = origin.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + (devices ? "/api/device-hub" : "/api/preview-stream")
                    var query = ["wsTicket": ticket]; if devices { query["hostId"] = target.host }
                    let value: [String: Any] = ["_operate": grants.contains(devices ? "orchestration:operate" : "preview:operate"), "httpBase": base, "wsBase": base.replacingOccurrences(of: "^http", with: "ws", options: .regularExpression), "query": query, "credentials": false]
                    DispatchQueue.main.async { done(value) }
                }
            }
        }
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0, key = request["owner"] as? String ?? ""
        let action = request["action"] as? String ?? ""
        func answer(_ value: [String: Any]) { reply(["ok": true, "generation": generation, "value": value]) }
        guard let view = views[key]?.value else {
            if action == "status" { answer([:]) }
            else { reply(["ok": false, "generation": generation, "error": ["kind": "Preview", "message": "This preview is no longer open."]]) }; return
        }
        if action == "status" { answer(view.status); return }
        view.control(action, input: request["input"]) { error in
            if let error { reply(["ok": false, "generation": generation, "error": ["kind": "Preview", "message": error]]) }
            else { answer([:]) }
        }
    }
    func destroy() { for item in Array(views.values) { item.value?.destroy() }; views.removeAll() }
}
#endif
