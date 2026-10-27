// The process's one reader (LLP 1133.035 D6), shared by every session's
// `NfcModule`: Core NFC allows one reader session in the whole system. On
// the Mac and tvOS there is no Core NFC, and the registry refuses what
// reaches it (the module has already answered `available` false).
import ExactPackage
import Foundation
#if os(iOS)
import CoreNFC
#endif

/// The app's access hatch (D14), on iOS: told each reader session after it
/// begins, and each tag before the module reads it, on the session's queue
/// (the main queue). A tag the hatch connects to is read after it returns.
public enum NfcHatch {
    #if os(iOS)
    public static var session: ((NFCReaderSession) -> Void)?
    public static var tag: ((NFCReaderSession, Any) -> Void)?
    #endif
}

final class NfcRegistry: NSObject {
    static let shared = NfcRegistry()
    var claims: [NfcClaim] = []

    #if os(iOS)
    /// The running session, and the polling options it was made with.
    private var session: NFCReaderSession?
    private var polling: [String] = []
    /// Whether the session has begun (`didBecomeActive`): a failure before
    /// then fails the claims that started it rather than ending them.
    private var active = false
    /// The tag in the field the last reading came from, until it leaves.
    private var present: AnyObject?
    #endif

    // MARK: claims

    func claim(_ c: NfcClaim) {
        #if os(iOS)
        if !claims.isEmpty, session != nil, c.words.polling != polling {
            return c.settle(["error": "InvalidStateError", "message": "busy"])
        }
        if c.kind != .scan {
            for old in claims where old.kind != .scan {
                old.settle(["error": "AbortError", "message": "replaced"])
            }
            claims.removeAll { $0.kind != .scan }
        }
        claims.append(c)
        if session == nil { begin(c.words.polling) }
        if let alert = c.words.alert { session?.alertMessage = alert }
        #else
        c.settle(["error": "NotSupportedError"])
        #endif
    }

    /// `owner`'s claims of `kinds` end with `answer`.
    func end(owner: NfcModule, kinds: [NfcClaim.Kind], answer: [String: Any]) {
        let ended = claims.filter { $0.owner === owner && kinds.contains($0.kind) }
        claims.removeAll { c in ended.contains { $0 === c } }
        ended.forEach { $0.settle(answer) }
        if kinds.contains(.scan) && !claims.contains(where: { $0.owner === owner && $0.kind == .scan }) { owner.leases = [:] }
        finishIfIdle(nil)
    }

    func cancel(owner: NfcModule, request: Int) {
        guard let c = claims.first(where: { $0.owner === owner && $0.request == request }) else { return }
        owner.say("request \(request) was let go")
        claims.removeAll { $0 === c }
        c.settle(c.kind == .scan ? ["reason": "stopped", "reading": NSNull(), "message": NSNull()] : ["error": "AbortError", "message": "let go"])
        finishIfIdle(nil)
    }

    /// iOS ends the session when the app leaves the foreground (D6).
    func background() {
        #if os(iOS)
        guard session != nil else { return }
        endAll(scan: "interrupted", error: "AbortError", message: "background")
        session?.invalidate()
        reset()
        #endif
    }

    /// The session ends when no claim remains, with `sentence` as its last line.
    private func finishIfIdle(_ sentence: String?, failed: Bool = false) {
        #if os(iOS)
        guard claims.isEmpty, let s = session else { return }
        if failed, let sentence { s.invalidate(errorMessage: sentence) } else {
            if let sentence { s.alertMessage = sentence }
            s.invalidate()
        }
        reset()
        #endif
    }

    private func endAll(scan reason: String, error: String, message: String?) {
        let all = claims
        claims = []
        for c in all {
            c.settle(c.kind == .scan ? ["reason": reason, "reading": NSNull(), "message": message ?? NSNull()] : ["error": error, "message": message ?? NSNull()])
            c.owner?.leases = [:]
        }
    }

    // MARK: tier 2

    func command(_ name: String, owner: NfcModule, args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        #if os(iOS)
        let id = args["tag"] as? String ?? ""
        guard let tag = owner.leases[id] else {
            return reply(["error": "InvalidStateError", "message": "the scan that read `\(id)` has ended"])
        }
        guard (tag as? NFCTag)?.isAvailable ?? (tag as? NFCNDEFTag)?.isAvailable ?? false else {
            return reply(["error": "NetworkError", "message": "lost"])
        }
        let answer: (Data, UInt8?, UInt8?, Error?) -> Void = { data, sw1, sw2, error in
            DispatchQueue.main.async {
                if let error { return reply(["error": "NetworkError", "message": error.localizedDescription]) }
                reply(["data": NfcWire.bytes(data), "sw1": sw1.map { Int($0) } ?? NSNull(), "sw2": sw2.map { Int($0) } ?? NSNull()])
            }
        }
        switch (name, tag as? NFCTag) {
        case ("sendCommand", .iso7816(let t)?):
            guard let apdu = NfcWire.data(args["apdu"]).flatMap(NFCISO7816APDU.init(data:)) else {
                return reply(["error": "TypeError", "message": "an APDU"])
            }
            t.sendCommand(apdu: apdu) { data, sw1, sw2, error in answer(data, sw1, sw2, error) }
        case ("sendMiFareCommand", .miFare(let t)?):
            guard let packet = NfcWire.data(args["packet"]) else { return reply(["error": "TypeError", "message": "a packet"]) }
            t.sendMiFareCommand(commandPacket: packet) { data, error in answer(data, nil, nil, error) }
        default:
            reply(["error": "NotSupportedError", "message": "this tag takes no `\(name)`: scan with apple.polling"])
        }
        #else
        reply(["error": "NotSupportedError"])
        #endif
    }

    #if os(iOS)
    // MARK: the session

    private func begin(_ options: [String]) {
        polling = options
        active = false
        if options.isEmpty {
            session = NFCNDEFReaderSession(delegate: self, queue: .main, invalidateAfterFirstRead: false)
        } else {
            var o: NFCTagReaderSession.PollingOption = []
            if options.contains("iso14443") { o.insert(.iso14443) }
            if options.contains("iso15693") { o.insert(.iso15693) }
            if options.contains("iso18092") { o.insert(.iso18092) }
            session = NFCTagReaderSession(pollingOption: o, delegate: self, queue: .main)
        }
        session?.begin()
        if let s = session { NfcHatch.session?(s) }
    }

    private func reset() {
        session = nil
        polling = []
        active = false
        present = nil
    }

    /// The session ended itself (D4, D6): Cancel, the timeout, the system.
    fileprivate func invalidated(_ error: Error) {
        guard session != nil else { return }
        let code = (error as? NFCReaderError)?.code
        switch code {
        case .readerSessionInvalidationErrorUserCanceled?: endAll(scan: "cancelled", error: "AbortError", message: nil)
        case .readerSessionInvalidationErrorSessionTimeout?: endAll(scan: "timeout", error: "AbortError", message: "timeout")
        case .readerErrorRadioDisabled? where !active:
            let all = claims
            claims = []
            all.forEach { $0.settle(["error": "NotReadableError", "message": "radio-off"]) }
        case .readerErrorRadioDisabled?: endAll(scan: "interrupted", error: "AbortError", message: "radio-off")
        case .readerErrorUnsupportedFeature? where !active, .readerErrorSecurityViolation? where !active:
            let all = claims
            claims = []
            all.forEach { $0.settle(["error": "NotSupportedError", "message": error.localizedDescription]) }
        default: endAll(scan: "interrupted", error: "AbortError", message: error.localizedDescription)
        }
        reset()
    }

    /// Several tags at once are neither read nor written (D5): polling
    /// restarts, as Apple's sample does.
    fileprivate func several() {
        session?.alertMessage = claims.last?.words.alert ?? ""
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(500)) { [weak self] in self?.restart() }
    }

    private func restart() {
        if let s = session as? NFCNDEFReaderSession { s.restartPolling() }
        if let s = session as? NFCTagReaderSession { s.restartPolling() }
    }

    /// Restart polling once `tag` has left the field (D11).
    private func whenGone(_ available: @escaping () -> Bool) {
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(300)) { [weak self] in
            guard let self, self.session != nil else { return }
            if available() { self.whenGone(available) } else {
                self.present = nil
                self.claims.filter { $0.kind == .scan }.forEach { $0.owner?.leases = [:] }
                self.restart()
            }
        }
    }

    /// A tag with NDEF: written for the waiting write, else read for every live scan.
    fileprivate func arrived(_ tag: NFCNDEFTag, identity: (serial: String, apple: [String: Any]?), handle: AnyObject, connected: @escaping (@escaping (Error?) -> Void) -> Void) {
        connected { [weak self] error in
            guard let self else { return }
            if let error { self.session?.alertMessage = self.claims.last?.words.alert ?? ""; return self.retry(error) }
            if let write = self.claims.last(where: { $0.kind != .scan }) {
                return self.write(write, to: tag, serial: identity.serial)
            }
            tag.readNDEF { message, error in
                DispatchQueue.main.async {
                    let code = (error as? NFCReaderError)?.code
                    if let error, code != .ndefReaderSessionErrorZeroLengthMessage { return self.retry(error) }
                    self.read(message, identity: identity, handle: handle)
                    self.whenGone { tag.isAvailable }
                }
            }
        }
    }

    private func retry(_ error: Error) {
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(500)) { [weak self] in self?.restart() }
    }

    private func read(_ message: NFCNDEFMessage?, identity: (serial: String, apple: [String: Any]?), handle: AnyObject) {
        let payloads = message?.records ?? []
        let info = payloads.map { NfcRecords.record(tnf: $0.typeNameFormat.rawValue, type: $0.type, identifier: $0.identifier, payload: $0.payload, full: false) }
        let full = payloads.map { NfcRecords.record(tnf: $0.typeNameFormat.rawValue, type: $0.type, identifier: $0.identifier, payload: $0.payload, full: true) }
        let scans = claims.filter { $0.kind == .scan }
        var seen: [ObjectIdentifier: String] = [:]
        for c in scans {
            guard let owner = c.owner else { continue }
            let key = ObjectIdentifier(owner)
            if seen[key] == nil { seen[key] = owner.read(serial: identity.serial, apple: identity.apple, records: info, full: full, tag: handle) }
        }
        let done = scans.filter(\.once)
        claims.removeAll { c in done.contains { $0 === c } }
        for c in done { c.settle(["reason": "read", "reading": seen[ObjectIdentifier(c.owner!)] ?? NSNull(), "message": NSNull()]) }
        if let alert = claims.last?.words.alert { session?.alertMessage = alert }
        finishIfIdle(done.last?.words.done)
    }

    /// D7's steps on `tag` for `c`.
    private func write(_ c: NfcClaim, to tag: NFCNDEFTag, serial: String) {
        let fail: (String, String) -> Void = { [weak self] error, cause in
            guard let self else { return }
            self.claims.removeAll { $0 === c }
            c.settle(["error": error, "message": cause])
            let sentence = c.words.errors[cause]
            if self.claims.isEmpty { self.finishIfIdle(sentence ?? cause, failed: true) }
            else if let sentence { self.session?.alertMessage = sentence }
        }
        let succeed: () -> Void = { [weak self] in
            guard let self else { return }
            self.claims.removeAll { $0 === c }
            c.settle(["serialNumber": serial])
            if self.claims.isEmpty { self.finishIfIdle(c.words.done) } else if let done = c.words.done { self.session?.alertMessage = done }
        }
        let records = c.message.compactMap { r -> NFCNDEFPayload? in
            guard case .success(let (tnf, type, id, payload)) = NfcRecords.encode(r, lang: Locale.current.language.languageCode?.identifier ?? "en") else { return nil }
            return NFCNDEFPayload(format: NFCTypeNameFormat(rawValue: tnf) ?? .unknown, type: type, identifier: id, payload: payload)
        }
        let message = NFCNDEFMessage(records: records)
        let proceed: () -> Void = {
            tag.queryNDEFStatus { status, capacity, error in
                DispatchQueue.main.async {
                    if error != nil { return fail("NetworkError", "lost") }
                    switch status {
                    case .notSupported: return fail("NotSupportedError", "not-ndef")
                    case .readOnly: return fail("NetworkError", "read-only")
                    default: break
                    }
                    if c.kind == .lock {
                        return tag.writeLock { error in DispatchQueue.main.async { error == nil ? succeed() : fail("NetworkError", "failed") } }
                    }
                    if message.length > capacity { return fail("NetworkError", "too-small") }
                    tag.writeNDEF(message) { error in
                        DispatchQueue.main.async {
                            guard let error else { return succeed() }
                            let code = (error as? NFCReaderError)?.code
                            switch code {
                            case .ndefReaderSessionErrorTagNotWritable?: fail("NetworkError", "read-only")
                            case .ndefReaderSessionErrorTagSizeTooSmall?: fail("NetworkError", "too-small")
                            case .readerTransceiveErrorTagConnectionLost?: fail("NetworkError", "lost")
                            default: fail("NetworkError", "failed")
                            }
                        }
                    }
                }
            }
        }
        guard c.kind == .write, !c.overwrite else { return proceed() }
        tag.readNDEF { existing, _ in
            DispatchQueue.main.async {
                if let existing, !existing.records.isEmpty { return fail("NotAllowedError", "overwrite") }
                proceed()
            }
        }
    }

    /// A polling scan's tag with no NDEF (D8, D11): `readingerror`, and the
    /// sheet's `unreadable` sentence while the scan goes on.
    fileprivate func unreadable(identity: (serial: String, apple: [String: Any]?), handle: AnyObject, available: @escaping () -> Bool) {
        var told = Set<ObjectIdentifier>()
        for c in claims where c.kind == .scan {
            guard let owner = c.owner, told.insert(ObjectIdentifier(owner)).inserted else { continue }
            owner.unread(serial: identity.serial, apple: identity.apple, tag: handle)
        }
        if let line = claims.last(where: { $0.kind == .scan })?.words.errors["unreadable"] { session?.alertMessage = line }
        whenGone(available)
    }
    #endif
}

#if os(iOS)
extension NfcRegistry: NFCNDEFReaderSessionDelegate {
    func readerSessionDidBecomeActive(_ session: NFCNDEFReaderSession) { active = true }

    func readerSession(_ session: NFCNDEFReaderSession, didInvalidateWithError error: Error) {
        DispatchQueue.main.async { if self.session === session { self.invalidated(error) } }
    }

    // Required, though `didDetect tags` is implemented and takes precedence.
    func readerSession(_ session: NFCNDEFReaderSession, didDetectNDEFs messages: [NFCNDEFMessage]) {}

    func readerSession(_ session: NFCNDEFReaderSession, didDetect tags: [NFCNDEFTag]) {
        guard self.session === session else { return }
        guard tags.count == 1, let tag = tags.first else { return several() }
        NfcHatch.tag?(session, tag)
        arrived(tag, identity: ("", nil), handle: tag) { done in session.connect(to: tag, completionHandler: done) }
    }
}

extension NfcRegistry: NFCTagReaderSessionDelegate {
    func tagReaderSessionDidBecomeActive(_ session: NFCTagReaderSession) { active = true }

    func tagReaderSession(_ session: NFCTagReaderSession, didInvalidateWithError error: Error) {
        DispatchQueue.main.async { if self.session === session { self.invalidated(error) } }
    }

    func tagReaderSession(_ session: NFCTagReaderSession, didDetect tags: [NFCTag]) {
        guard self.session === session else { return }
        guard tags.count == 1, let tag = tags.first else { return several() }
        NfcHatch.tag?(session, tag)
        // A tag type newer than this build is left to the system, as a tag in no polling option is.
        guard let (identity, ndef) = Self.identity(tag) else { return session.restartPolling() }
        session.connect(to: tag) { [weak self] error in
            DispatchQueue.main.async {
                guard let self else { return }
                if let error { return self.retry(error) }
                ndef.queryNDEFStatus { status, _, _ in
                    DispatchQueue.main.async {
                        if status == .notSupported, !self.claims.contains(where: { $0.kind != .scan }) {
                            return self.unreadable(identity: identity, handle: tag as AnyObject) { tag.isAvailable }
                        }
                        self.arrived(ndef, identity: identity, handle: tag as AnyObject) { $0(nil) }
                    }
                }
            }
        }
    }

    /// `serialNumber` and `apple.tag` (D11) of a tag the tag session found.
    static func identity(_ tag: NFCTag) -> ((serial: String, apple: [String: Any]?), NFCNDEFTag)? {
        switch tag {
        case .miFare(let t):
            let family = ["unknown", "ultralight", "plus", "desfire"][min(Int(t.mifareFamily.rawValue) - 1, 3).clamped]
            return ((NfcWire.hex(t.identifier, separator: ":"), ["type": "miFare", "identifier": NfcWire.hex(t.identifier), "mifareFamily": family,
                     "historicalBytes": t.historicalBytes.map { NfcWire.hex($0) } ?? NSNull(), "initialSelectedAID": NSNull()]), t)
        case .iso7816(let t):
            return ((NfcWire.hex(t.identifier, separator: ":"), ["type": "iso7816", "identifier": NfcWire.hex(t.identifier), "mifareFamily": NSNull(),
                     "historicalBytes": t.historicalBytes.map { NfcWire.hex($0) } ?? NSNull(), "initialSelectedAID": t.initialSelectedAID]), t)
        case .iso15693(let t):
            return ((NfcWire.hex(t.identifier, separator: ":"), ["type": "iso15693", "identifier": NfcWire.hex(t.identifier), "mifareFamily": NSNull(),
                     "historicalBytes": NSNull(), "initialSelectedAID": NSNull()]), t)
        case .feliCa(let t):
            return ((NfcWire.hex(t.currentIDm, separator: ":"), ["type": "feliCa", "identifier": NfcWire.hex(t.currentIDm), "mifareFamily": NSNull(),
                     "historicalBytes": NSNull(), "initialSelectedAID": NSNull()]), t)
        @unknown default:
            return nil
        }
    }
}

private extension Int {
    var clamped: Int { Swift.max(0, self) }
}
#endif
