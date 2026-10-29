// @exact/tracking-consent on iOS and tvOS (LLP 1133.028 D12-D14): App
// Tracking Transparency, a platform module with no other implementation.
// Under the agent it asks the framework for nothing: the runner holds each
// request as a `tracking` ticket, whose tap reaches the implementation as
// the call's `preferred`, and the status is this session's own, laid under
// the agent's preferred one (§7). `--bind tracking-consent` asks ATT.
//
// - ATT shows nothing to an app that is not active, and answers at once; a
//   request made then is held until the app becomes active, so a request at
//   launch shows the alert as soon as it can. A held request whose call is
//   let go or whose session ends is dropped, never shown later (D13).
// - The request never fails: it answers the status, `denied` and
//   `notDetermined` (a dismissal) included. `state` is LLP 1133.002 D8's
//   word for it.
// - `advertisingIdentifier` is data code's alone: zeros unless the person
//   authorized tracking, and always in the Simulator.
import AdSupport
import AppTrackingTransparency
import ExactPackage
import Foundation
import UIKit

public final class TrackingConsentModule: ExactPackage {
    let context: ExactPackageContext
    /// Under the agent: the status the person's taps gave this session.
    private var agentStatus = "notDetermined"
    private var overlay: [String: Any] = [:]
    /// Requests waiting to be asked, by the runner's request number.
    private var held: [(request: Int?, reply: ([String: Any]) -> Void)] = []
    private var observer: NSObjectProtocol?
    private var asking = false
    static let zeros = "00000000-0000-0000-0000-000000000000"

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("tracking-consent: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "trackingAuthorizationStatus": reply(status())
        case "requestTrackingAuthorization":
            if context.agent { return reply(agentRequest(preferred as? [String: Any])) }
            request(context.request, reply)
        case "advertisingIdentifier":
            let id = context.agent
                ? (status()["status"] as? String == "authorized" ? "6F1D3C2A-8B4E-4F5A-9C7D-0E1F2A3B4C5D" : Self.zeros)
                : ASIdentifierManager.shared().advertisingIdentifier.uuidString
            reply(["identifier": id])
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name.hasPrefix("trackingAuthorizationStatus") ? status() : ["error": "NotSupportedError"]
    }

    public func preferred(_ values: [String: Any]) {
        overlay = values["trackingAuthorizationStatus"] as? [String: Any] ?? [:]
    }

    /// The person changes it in Settings, outside the app (D13).
    public func foreground(_ shown: Bool) {
        if shown { context.changed("tracking-consent.trackingAuthorizationStatus") }
    }

    public func cancel(_ request: Int) {
        let dropped = held.filter { $0.request == request }
        held.removeAll { $0.request == request }
        if !dropped.isEmpty { say("a held request was let go: dropped, never shown") }
        dropped.forEach { $0.reply(["error": "AbortError"]) }
    }

    public func destroy(_ done: @escaping () -> Void) {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        held.forEach { $0.reply(["error": "AbortError"]) }
        held = []
        done()
    }

    // MARK: the status

    private func status() -> [String: Any] {
        var word = context.agent ? agentStatus : Self.word(ATTrackingManager.trackingAuthorizationStatus)
        if let over = overlay["status"] as? String { word = over }
        var out: [String: Any] = ["available": true, "status": word, "state": Self.state(word)]
        if let available = overlay["available"] as? Bool { out["available"] = available }
        return out
    }

    static func word(_ status: ATTrackingManager.AuthorizationStatus) -> String {
        switch status {
        case .authorized: "authorized"
        case .denied: "denied"
        case .restricted: "restricted"
        default: "notDetermined"
        }
    }

    /// LLP 1133.002 D8: `restricted` is a denial the person did not make here.
    static func state(_ word: String) -> String {
        switch word {
        case "authorized": "granted"
        case "notDetermined": "prompt"
        default: "denied"
        }
    }

    // MARK: asking

    /// The ticket's tap, while the status is undecided; else the status, as
    /// ATT answers without asking.
    private func agentRequest(_ sheet: [String: Any]?) -> [String: Any] {
        guard status()["status"] as? String == "notDetermined" else { return status() }
        switch sheet?["tap"] as? String {
        case "allow": agentStatus = "authorized"
        case "deny": agentStatus = "denied"
        default: break // dismissed: asked again next time
        }
        say("answered \(agentStatus)")
        context.changed("tracking-consent.trackingAuthorizationStatus")
        return status()
    }

    private func request(_ number: Int?, _ reply: @escaping ([String: Any]) -> Void) {
        held.append((number, reply))
        pump()
    }

    /// Asks for the oldest waiting request once the app is active, one at a
    /// time: ATT answers a second request at once while one is pending.
    private func pump() {
        guard !asking, !held.isEmpty else { return }
        guard UIApplication.shared.applicationState == .active else {
            say("held until the app is active")
            if observer == nil {
                observer = NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                    self?.pump()
                }
            }
            return
        }
        let (_, reply) = held.removeFirst()
        asking = true
        ATTrackingManager.requestTrackingAuthorization { [weak self] answer in
            DispatchQueue.main.async { self?.answered(answer, reply) }
        }
    }

    private func answered(_ answer: ATTrackingManager.AuthorizationStatus, _ reply: ([String: Any]) -> Void) {
        say("answered \(Self.word(answer))")
        context.changed("tracking-consent.trackingAuthorizationStatus")
        reply(status())
        asking = false
        pump()
    }
}
