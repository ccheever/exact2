// @exact/calls on Apple (LLP 1133.033 D9, D10, D15, D19). The runner keeps the
// call list and hands it here (`exact:listed`, Charlie's 2026-10-27 ruling):
// this module follows it with CallKit on iOS, and reports what the person did
// on the system's screen as the list's declared choices (`exact-choice:`).
//
// - One `CXProvider` and one `CXCallController` per process, made when the
//   first session binds the module outside the agent; each session's module
//   attaches to it. Under the agent none is made (D15), and the runner's
//   substitute is the call.
// - The app's own acts (`startCall`, `answerCall`, `setCallHeld`,
//   `setCallMuted`, `endCall` with `local` or `declined`) have already moved
//   the list; each becomes CallKit's transaction, and the provider's
//   `perform` for it fulfils it and reports nothing back. A transaction the
//   system refuses ends the call `failed`.
// - CallKit's actions no request of ours started are the person's: answered
//   on the lock screen, ended from the Dynamic Island or a headset, held for
//   a cellular call, muted on the in-call screen. Each fulfils at once and is
//   reported as the choice `answer`, `hangup`, `hold`/`unhold` or
//   `mute`/`unmute`, which raises the app's event with a person's origin.
// - Audio (D10): the call's claim on the host's audio owner is made in the
//   start's or answer's `perform`, before it is fulfilled, and held while any
//   call lasts. `didActivate`/`didDeactivate` reach the owner and the list
//   (`activated`/`deactivated`, the call's `audio`).
// - Tier 3 (D15): `CallsModule.providerConfiguration(_:)` changes the
//   provider's configuration after the module sets its own words, and
//   `CallsModule.callUpdate(_:)` each `CXCallUpdate` before it is reported.
// - macOS has no CallKit provider: the call is the list alone, `systemUI`
//   false.
import ExactPackage
import Foundation
#if os(iOS)
import AVFoundation
import CallKit
#endif

public final class CallsModule: ExactPackage {
    private let context: ExactPackageContext

    public init(context: ExactPackageContext) {
        self.context = context
        #if os(iOS)
        if !context.agent { CallProvider.shared.attach(self) }
        #endif
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "exact:listed":
            #if os(iOS)
            if !context.agent { CallProvider.shared.follow(args["entries"] as? [[String: Any]] ?? [], from: self) }
            #endif
            reply([:])
        case "callsStatus": reply(status())
        // The list's names are the runner's (`declared/handed.rs`), never sent here.
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "callsStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    private func status() -> [String: Any] {
        #if os(iOS)
        ["available": true, "systemUI": !context.agent]
        #else
        ["available": true, "systemUI": false]
        #endif
    }

    /// The person's act on the system's screen, as the list's declared choice.
    func choose(_ id: String, _ choice: String) {
        let body: [String: Any] = ["extension": "calls", "state": "calls", "key": id, "choice": choice]
        guard let data = try? JSONSerialization.data(withJSONObject: body), let text = String(data: data, encoding: .utf8) else { return }
        context.log("calls: \(id) \(choice) (the system's screen)")
        context.changed("exact-choice:" + text)
    }

    var audioSession: ExactAudioSession { context.audioSession }
    func log(_ line: String) { context.log(line) }

    #if os(iOS)
    nonisolated(unsafe) static var configurationHatch: ((CXProviderConfiguration) -> Void)?
    nonisolated(unsafe) static var updateHatch: ((CXCallUpdate, [String: Any]) -> Void)?

    /// The app's access hatch (D15): the provider's configuration, after the
    /// module sets `localizedName`, `supportsVideo` and the call groups.
    public static func providerConfiguration(_ hatch: @escaping (CXProviderConfiguration) -> Void) {
        configurationHatch = hatch
        CallProvider.reconfigure()
    }

    /// The app's access hatch (D15): each `CXCallUpdate` before it is
    /// reported, with the call's entry; the handle, the caller's name and
    /// `hasVideo` stay the module's.
    public static func callUpdate(_ hatch: @escaping (CXCallUpdate, [String: Any]) -> Void) {
        updateHatch = hatch
    }
    #endif
}

#if os(iOS)
/// The process's provider and controller (module doc). Main thread.
final class CallProvider: NSObject, CXProviderDelegate {
    nonisolated(unsafe) static let shared = CallProvider()
    private var provider: CXProvider?
    private let controller = CXCallController()
    private weak var module: CallsModule?
    /// Each call's id in the list, its CallKit UUID, and its last entry.
    private var uuids: [String: UUID] = [:]
    private var last: [String: [String: Any]] = [:]
    /// The actions our own transactions asked, by UUID: their `perform` reports nothing.
    private var asked: [UUID: Set<String>] = [:]
    private var claim: ExactSubscription?

    func attach(_ module: CallsModule) {
        self.module = module
        guard provider == nil else { return }
        let p = CXProvider(configuration: Self.configuration())
        p.setDelegate(self, queue: nil)
        provider = p
        module.log("calls: CallKit's provider made")
    }

    static func configuration() -> CXProviderConfiguration {
        let c = CXProviderConfiguration()
        c.supportsVideo = true
        c.maximumCallGroups = 2
        c.maximumCallsPerCallGroup = 1
        c.supportedHandleTypes = [.generic, .phoneNumber, .emailAddress]
        CallsModule.configurationHatch?(c)
        return c
    }

    static func reconfigure() {
        DispatchQueue.main.async { shared.provider?.configuration = configuration() }
    }

    private func id(of uuid: UUID) -> String? { uuids.first { $0.value == uuid }?.key }

    private func expect(_ uuid: UUID, _ action: String) { asked[uuid, default: []].insert(action) }
    /// Whether our own transaction asked `action` of `uuid`, forgetting it.
    private func ours(_ uuid: UUID, _ action: String) -> Bool { asked[uuid]?.remove(action) != nil }

    private func request(_ action: CXCallAction, _ word: String, id: String) {
        expect(action.callUUID, word)
        controller.request(CXTransaction(action: action)) { [weak self] error in
            guard let error, let self else { return }
            DispatchQueue.main.async {
                _ = self.ours(action.callUUID, word)
                self.module?.log("calls: \(id) \(word) refused by CallKit: \(error.localizedDescription)")
                self.module?.choose(id, "failed")
            }
        }
    }

    private func handle(_ e: [String: Any]) -> CXHandle {
        let type: CXHandle.HandleType = switch e["handleType"] as? String {
        case "phoneNumber": .phoneNumber
        case "emailAddress": .emailAddress
        default: .generic
        }
        return CXHandle(type: type, value: e["handle"] as? String ?? "")
    }

    private func update(_ e: [String: Any]) -> CXCallUpdate {
        let u = CXCallUpdate()
        CallsModule.updateHatch?(u, e)
        u.remoteHandle = handle(e)
        u.localizedCallerName = e["displayName"] as? String
        u.hasVideo = e["video"] as? Bool ?? false
        return u
    }

    /// The runner's list now: each change of an entry since the last becomes
    /// CallKit's (module doc).
    func follow(_ entries: [[String: Any]], from module: CallsModule) {
        self.module = module
        var seen = Set<String>()
        for e in entries {
            guard let id = e["id"] as? String, let state = e["state"] as? String else { continue }
            seen.insert(id)
            let was = last[id], before = was?["state"] as? String
            last[id] = e
            if was == nil || uuids[id] == nil {
                guard state != "ended" else { continue }
                let uuid = UUID()
                uuids[id] = uuid
                if e["direction"] as? String == "incoming" {
                    provider?.reportNewIncomingCall(with: uuid, update: update(e)) { [weak self] error in
                        guard let error else { return }
                        DispatchQueue.main.async {
                            self?.module?.log("calls: \(id) not reported: \(error.localizedDescription)")
                            self?.module?.choose(id, "failed")
                        }
                    }
                } else {
                    let start = CXStartCallAction(call: uuid, handle: handle(e))
                    start.isVideo = e["video"] as? Bool ?? false
                    start.contactIdentifier = e["displayName"] as? String
                    request(start, "start", id: id)
                }
                if state == "connected" { connected(id, e, from: "dialing") }
                continue
            }
            guard let uuid = uuids[id] else { continue }
            if state != before {
                switch (before, state) {
                case (_, "ended"): ended(id, uuid, e)
                case ("ringing", "connected"):
                    if !ours(uuid, "answer-person") { request(CXAnswerCallAction(call: uuid), "answer", id: id) }
                case (_, "connecting"): provider?.reportOutgoingCall(with: uuid, startedConnectingAt: Date())
                case ("held", "connected"):
                    if !ours(uuid, "unhold-person") { request(CXSetHeldCallAction(call: uuid, onHold: false), "held", id: id) }
                case (_, "held"):
                    if !ours(uuid, "hold-person") { request(CXSetHeldCallAction(call: uuid, onHold: true), "held", id: id) }
                case (_, "connected"): connected(id, e, from: before ?? "")
                default: break
                }
            }
            if let muted = e["muted"] as? Bool, muted != (was?["muted"] as? Bool ?? false), !ours(uuid, "mute-person") {
                request(CXSetMutedCallAction(call: uuid, muted: muted), "muted", id: id)
            }
        }
        // An entry gone from the list without our seeing it end has ended.
        for (id, uuid) in uuids where !seen.contains(id) {
            if last[id]?["state"] as? String != "ended" { ended(id, uuid, ["endedReason": "remote"]) }
            uuids[id] = nil
            last[id] = nil
            asked[uuid] = nil
        }
        if uuids.values.isEmpty || last.values.allSatisfy({ $0["state"] as? String == "ended" }) { releaseClaim() }
    }

    /// An outgoing call the app reports connected: told CallKit once its start is performed, never before.
    private func connected(_ id: String, _ e: [String: Any], from: String) {
        guard let uuid = uuids[id], e["direction"] as? String != "incoming", asked[uuid]?.contains("start") != true else { return }
        provider?.reportOutgoingCall(with: uuid, connectedAt: Date())
    }

    private func ended(_ id: String, _ uuid: UUID, _ e: [String: Any]) {
        if ours(uuid, "end-person") { return }
        let reason = e["endedReason"] as? String ?? "local"
        switch reason {
        case "local", "declined": request(CXEndCallAction(call: uuid), "end", id: id)
        default:
            let r: CXCallEndedReason = switch reason {
            case "remote": .remoteEnded
            case "unanswered": .unanswered
            case "answeredElsewhere": .answeredElsewhere
            case "declinedElsewhere": .declinedElsewhere
            default: .failed
            }
            provider?.reportCall(with: uuid, endedAt: Date(), reason: r)
        }
    }

    private func claimAudio(video: Bool) {
        guard claim == nil, let module else { return }
        claim = module.audioSession.claimCall(video: video) { _ in }
    }

    private func releaseClaim() {
        claim?.cancel()
        claim = nil
    }

    // MARK: CXProviderDelegate. The host enforces, then tells (D9): each action fulfils before the app hears it.

    func providerDidReset(_ provider: CXProvider) {
        for (id, _) in uuids where last[id]?["state"] as? String != "ended" { module?.choose(id, "failed") }
        releaseClaim()
    }

    func provider(_ provider: CXProvider, perform action: CXStartCallAction) {
        claimAudio(video: action.isVideo)
        action.fulfill()
        _ = ours(action.callUUID, "start")
        provider.reportOutgoingCall(with: action.callUUID, startedConnectingAt: nil)
        if let id = id(of: action.callUUID), last[id]?["state"] as? String == "connected" {
            provider.reportOutgoingCall(with: action.callUUID, connectedAt: Date())
        }
    }

    func provider(_ provider: CXProvider, perform action: CXAnswerCallAction) {
        guard let id = id(of: action.callUUID) else { return action.fail() }
        claimAudio(video: last[id]?["video"] as? Bool ?? false)
        action.fulfill()
        if !ours(action.callUUID, "answer") {
            expect(action.callUUID, "answer-person")
            module?.choose(id, "answer")
        }
    }

    func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
        guard let id = id(of: action.callUUID) else { return action.fail() }
        action.fulfill()
        if !ours(action.callUUID, "end") {
            expect(action.callUUID, "end-person")
            module?.choose(id, "hangup")
        }
    }

    func provider(_ provider: CXProvider, perform action: CXSetHeldCallAction) {
        guard let id = id(of: action.callUUID) else { return action.fail() }
        action.fulfill()
        if !ours(action.callUUID, "held") {
            expect(action.callUUID, action.isOnHold ? "hold-person" : "unhold-person")
            module?.choose(id, action.isOnHold ? "hold" : "unhold")
        }
    }

    func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) {
        guard let id = id(of: action.callUUID) else { return action.fail() }
        action.fulfill()
        if !ours(action.callUUID, "muted") {
            expect(action.callUUID, "mute-person")
            module?.choose(id, action.isMuted ? "mute" : "unmute")
        }
    }

    func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) {
        module?.audioSession.callAudio(true)
        for (id, e) in last where e["state"] as? String == "connected" { module?.choose(id, "activated") }
    }

    func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) {
        module?.audioSession.callAudio(false)
        for (id, e) in last where ["connected", "held"].contains(e["state"] as? String ?? "") { module?.choose(id, "deactivated") }
        if last.values.allSatisfy({ $0["state"] as? String == "ended" }) { releaseClaim() }
    }
}
#endif
