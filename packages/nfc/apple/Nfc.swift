// @exact/nfc on iOS (LLP 1133.035 stage 2): Core NFC's reader sessions,
// answered in the interface's shapes. The app's composed module calls it
// outside the agent; under the agent the runner's declared substitute
// answers and no session starts, unless the drive binds it (`--bind nfc`).
// Core NFC is iOS's alone, so on the Mac and on tvOS this file compiles to
// a module that reads `available` false and refuses every request.
//
// - One registry per process (D6): a scan or a write is a claim of one
//   session's instance; the registry runs one reader session while any
//   claim lives and ends it with the last. A reading goes to every session
//   with a live scan, each with its own reading id. One write at a time: a
//   newer write or lock replaces the waiting one, which fails `AbortError`.
// - Claims share one reader's settings (D6): the NDEF reader session, or
//   the tag reader session for `apple.polling` with its options. A claim
//   whose settings differ from the running reader's fails
//   `InvalidStateError` `busy`.
// - The sheet's message is the latest claim's (D6, D10): a claim's start
//   sets its `alertMessage`, a success its `doneMessage`, a failure its
//   `errorMessages` sentence; the last claim to end ends the session with
//   its own sentence.
// - A tag is read once while it stays in the field: polling restarts only
//   after it leaves (D11), which also keeps the leases on a tag valid for
//   the commands of tier 2.
// - The sheet's Cancel ends every claim (`cancelled`, a write
//   `AbortError`); the system's timeout `timeout`; leaving the foreground
//   ends the session (`interrupted`, a write `AbortError`).
import ExactPackage
import Foundation
#if os(iOS)
import CoreNFC
#endif

/// A claim's words beside its kind (D6, D11).
struct NfcClaimWords {
    var alert: String?
    var done: String?
    var errors: [String: String] = [:]
    /// `apple.polling`: the tag reader session's options, empty for the NDEF reader session.
    var polling: [String] = []

    init(_ args: [String: Any]) {
        let apple = args["apple"] as? [String: Any] ?? [:]
        alert = apple["alertMessage"] as? String
        done = apple["doneMessage"] as? String
        for (key, value) in apple["errorMessages"] as? [String: Any] ?? [:] {
            if let text = value as? String { errors[NfcClaimWords.cause(key)] = text }
        }
        polling = (apple["polling"] as? [String] ?? []).sorted()
    }

    /// The declaration's camelCase key as D3's cause (`readOnly` is `read-only`).
    static func cause(_ key: String) -> String {
        ["readOnly": "read-only", "tooSmall": "too-small", "notNdef": "not-ndef"][key] ?? key
    }
}

/// Bytes as they cross the seam (LLP 1133.035 D2).
enum NfcWire {
    static func bytes(_ data: Data) -> [String: Any] { ["base64": data.base64EncodedString()] }

    static func data(_ value: Any?) -> Data? {
        if let s = value as? String { return Data(s.utf8) }
        if let o = value as? [String: Any], let b = o["base64"] as? String { return Data(base64Encoded: b) }
        return nil
    }

    static func hex(_ data: Data, separator: String = "") -> String {
        data.map { String(format: "%02x", $0) }.joined(separator: separator)
    }
}

/// Web NFC's record types from an NDEF record's type name format and type
/// ("Mapping NDEF record to NDEFRecord"), and back.
enum NfcRecords {
    static let prefixes = ["", "http://www.", "https://www.", "http://", "https://", "tel:", "mailto:",
                           "ftp://anonymous:anonymous@", "ftp://ftp.", "ftps://", "sftp://", "smb://", "nfs://",
                           "ftp://", "dav://", "news:", "telnet://", "imap:", "rtsp://", "urn:", "pop:", "sip:",
                           "sips:", "tftp:", "btspp://", "btl2cap://", "btgoep://", "tcpobex://", "irdaobex://",
                           "file://", "urn:epc:id:", "urn:epc:tag:", "urn:epc:pat:", "urn:epc:raw:", "urn:epc:",
                           "urn:nfc:"]

    /// `recordType` and `mediaType` of a record with type name format `tnf` and `type`.
    static func kind(tnf: UInt8, type: Data) -> (String, String?) {
        let name = String(decoding: type, as: UTF8.self)
        switch tnf {
        case 0: return ("empty", nil)
        case 1:
            switch name {
            case "T": return ("text", nil)
            case "U": return ("url", nil)
            case "Sp": return ("smart-poster", nil)
            default: return (":" + name, nil)
            }
        case 2: return ("mime", name)
        case 3: return ("absolute-url", nil)
        case 4: return (name, nil)
        default: return ("unknown", nil)
        }
    }

    /// A record as Contract reads it (`NDEFRecordInfo`), and, `full`, as
    /// data code does (`NDEFRecord`, its bytes and own records).
    static func record(tnf: UInt8, type: Data, identifier: Data, payload: Data, full: Bool) -> [String: Any] {
        let (recordType, mediaType) = kind(tnf: tnf, type: type)
        var data = payload, text: String?, encoding: String?, lang: String?
        var inner: [[String: Any]]?
        switch recordType {
        case "url":
            let at = Int(payload.first ?? 0)
            data = Data((at < prefixes.count ? prefixes[at] : "").utf8) + payload.dropFirst()
            text = String(decoding: data, as: UTF8.self)
        case "absolute-url":
            text = String(decoding: type, as: UTF8.self)
        case "text":
            let status = payload.first ?? 0, length = Int(status & 0x3f)
            encoding = status & 0x80 == 0 ? "utf-8" : "utf-16be"
            lang = String(decoding: payload.dropFirst().prefix(length), as: UTF8.self)
            data = payload.dropFirst(1 + length)
            text = String(data: data, encoding: encoding == "utf-8" ? .utf8 : .utf16BigEndian)
        case "smart-poster":
            inner = parse(Data(payload), full: full)
            text = inner?.first { $0["recordType"] as? String == "url" }?["text"] as? String
        default: break
        }
        var out: [String: Any] = ["recordType": recordType, "mediaType": mediaType ?? NSNull(),
                                  "id": identifier.isEmpty ? NSNull() : String(decoding: identifier, as: UTF8.self),
                                  "encoding": encoding ?? NSNull(), "lang": lang ?? NSNull(),
                                  "text": text ?? NSNull(), "size": data.count]
        if full {
            out["data"] = NfcWire.bytes(Data(data))
            // One level deep (`NDEFInnerRecord`): a deeper record is in its parent's bytes.
            out["records"] = inner?.map { $0.filter { $0.key != "records" } } ?? NSNull()
        }
        return out
    }

    /// The records of an NDEF message's bytes (NFC Forum NDEF 1.0, §3.2).
    static func parse(_ bytes: Data, full: Bool) -> [[String: Any]]? {
        var at = bytes.startIndex, out: [[String: Any]] = []
        while at < bytes.endIndex {
            let header = bytes[at]; at += 1
            let short = header & 0x10 != 0, hasId = header & 0x08 != 0
            guard at < bytes.endIndex else { return nil }
            let typeLength = Int(bytes[at]); at += 1
            var payloadLength = 0
            if short { guard at < bytes.endIndex else { return nil }; payloadLength = Int(bytes[at]); at += 1 } else {
                guard at + 4 <= bytes.endIndex else { return nil }
                payloadLength = bytes[at..<at + 4].reduce(0) { $0 << 8 | Int($1) }; at += 4
            }
            var idLength = 0
            if hasId { guard at < bytes.endIndex else { return nil }; idLength = Int(bytes[at]); at += 1 }
            guard at + typeLength + idLength + payloadLength <= bytes.endIndex else { return nil }
            let type = bytes[at..<at + typeLength]; at += typeLength
            let id = bytes[at..<at + idLength]; at += idLength
            let payload = bytes[at..<at + payloadLength]; at += payloadLength
            out.append(record(tnf: header & 0x07, type: Data(type), identifier: Data(id), payload: Data(payload), full: full))
        }
        return out
    }

    /// Why a record cannot be encoded, the message of its `TypeError`.
    struct RecordError: Error, ExpressibleByStringInterpolation {
        let message: String
        init(stringLiteral message: String) { self.message = message }
        init(stringInterpolation: DefaultStringInterpolation) { message = String(stringInterpolation: stringInterpolation) }
    }

    /// An `NDEFRecordInit` as (type name format, type, identifier, payload),
    /// or why it cannot be encoded (`TypeError`, Web NFC's "create NDEF record").
    static func encode(_ r: [String: Any], lang fallback: String) -> Result<(UInt8, Data, Data, Data), RecordError> {
        let recordType = r["recordType"] as? String ?? ""
        let id = Data((r["id"] as? String ?? "").utf8)
        let data = NfcWire.data(r["data"])
        switch recordType {
        case "empty": return .success((0, Data(), id, Data()))
        case "text":
            guard let text = r["data"] as? String ?? data.map({ String(decoding: $0, as: UTF8.self) }) else { return .failure("a text record has data") }
            let lang = Data((r["lang"] as? String ?? fallback).utf8)
            return .success((1, Data("T".utf8), id, Data([UInt8(lang.count & 0x3f)]) + lang + Data(text.utf8)))
        case "url":
            guard let url = r["data"] as? String ?? data.map({ String(decoding: $0, as: UTF8.self) }), url.contains(":") else { return .failure("a url record's data is a URL") }
            let (code, rest) = prefixes.enumerated().dropFirst().filter { url.hasPrefix($0.element) }
                .max { $0.element.count < $1.element.count }
                .map { (UInt8($0.offset), String(url.dropFirst($0.element.count))) } ?? (0, url)
            return .success((1, Data("U".utf8), id, Data([code]) + Data(rest.utf8)))
        case "absolute-url":
            guard let url = r["data"] as? String, url.contains(":") else { return .failure("an absolute-url record's data is a URL") }
            return .success((3, Data(url.utf8), id, Data()))
        case "mime":
            guard let data else { return .failure("a mime record has data") }
            return .success((2, Data((r["mediaType"] as? String ?? "application/octet-stream").utf8), id, data))
        case "unknown":
            return .success((5, Data(), id, data ?? Data()))
        default:
            guard recordType.contains(":"), let data else { return .failure("`\(recordType)` is no record type this module writes") }
            return .success((4, Data(recordType.utf8), id, data))
        }
    }
}

/// One session's call waiting on the reader: a scan, a write or a lock.
final class NfcClaim {
    enum Kind { case scan, write, lock }
    let kind: Kind
    let request: Int?
    let once: Bool
    let words: NfcClaimWords
    let message: [[String: Any]]
    let overwrite: Bool
    weak var owner: NfcModule?
    var reply: (([String: Any]) -> Void)?

    init(_ kind: Kind, owner: NfcModule, request: Int?, args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        self.kind = kind; self.owner = owner; self.request = request; self.reply = reply
        once = args["once"] as? Bool == true
        words = NfcClaimWords(args)
        message = (args["message"] as? [String: Any])?["records"] as? [[String: Any]] ?? []
        overwrite = args["overwrite"] as? Bool != false
    }

    /// Answers once.
    func settle(_ answer: [String: Any]) {
        let r = reply
        reply = nil
        r?(answer)
    }
}

public final class NfcModule: ExactPackage {
    let context: ExactPackageContext
    /// The last 16 readings' records with their bytes, by reading id (D2).
    var readings: [(id: String, records: [[String: Any]])] = []
    /// The tags of this session's live scans, by their `NFCTag` id (D11).
    var leases: [String: AnyObject] = [:]
    var last = 0

    public init(context: ExactPackageContext) { self.context = context }

    func say(_ line: String) { context.log("nfc: \(line)") }

    /// Under the agent the substitute answers; bound, the implementation
    /// runs, and on the simulator `readingAvailable` is false.
    static var available: Bool {
        #if os(iOS)
        NFCNDEFReaderSession.readingAvailable
        #else
        false
        #endif
    }

    func status() -> [String: Any] {
        ["available": Self.available, "radio": "unknown", "state": "granted", "readerUI": "system"]
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "nfcStatusSync" || name == "nfcStatus" ? status() : ["error": "NotSupportedError"]
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "nfcStatus": reply(status())
        case "scan", "write", "makeReadOnly":
            guard Self.available else { return reply(["error": "NotSupportedError"]) }
            let kind: NfcClaim.Kind = name == "scan" ? .scan : (name == "write" ? .write : .lock)
            if kind == .write {
                let records = (args["message"] as? [String: Any])?["records"] as? [[String: Any]] ?? []
                if records.isEmpty { return reply(["error": "TypeError", "message": "a message has a record"]) }
                for r in records {
                    if case .failure(let why) = NfcRecords.encode(r, lang: "en") { return reply(["error": "TypeError", "message": why.message]) }
                }
            }
            NfcRegistry.shared.claim(NfcClaim(kind, owner: self, request: context.request, args: args, reply: reply))
        case "abortScan":
            NfcRegistry.shared.end(owner: self, kinds: [.scan], answer: ["reason": "stopped", "reading": NSNull(), "message": NSNull()])
            reply([:])
        case "abortWrite":
            NfcRegistry.shared.end(owner: self, kinds: [.write, .lock], answer: ["error": "AbortError", "message": NSNull()])
            reply([:])
        case "ndefRecords":
            let id = args["reading"] as? String ?? ""
            if let kept = readings.first(where: { $0.id == id }) { reply(["exact:answer": kept.records]) }
            else { reply(["error": "NotFoundError", "message": id]) }
        case "sendCommand", "sendMiFareCommand":
            NfcRegistry.shared.command(name, owner: self, args: args, reply: reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    /// A call let go (LLP 1133.016 D15 item 2): its scan stops, its write aborts.
    public func cancel(_ request: Int) {
        NfcRegistry.shared.cancel(owner: self, request: request)
    }

    /// iOS ends a reader session when the app leaves the foreground (D6).
    public func foreground(_ shown: Bool) {
        if !shown { NfcRegistry.shared.background() }
    }

    public func destroy(_ done: @escaping () -> Void) {
        NfcRegistry.shared.end(owner: self, kinds: [.scan], answer: ["reason": "stopped", "reading": NSNull(), "message": NSNull()])
        NfcRegistry.shared.end(owner: self, kinds: [.write, .lock], answer: ["error": "AbortError", "message": NSNull()])
        done()
    }

    /// A reading of this session: kept for `ndefRecords`, raised on the root.
    func read(serial: String, apple: [String: Any]?, records: [[String: Any]], full: [[String: Any]], tag: AnyObject?) -> String {
        last += 1
        let id = "r\(last)"
        readings.append((id, full))
        if readings.count > 16 { readings.removeFirst() }
        if let tag { leases[id] = tag }
        context.emit("reading", [["id": id, "serialNumber": serial, "apple": apple ?? NSNull(), "android": NSNull(), "records": records]])
        return id
    }

    /// A tag with no NDEF, seen by a polling scan (D8, D11).
    func unread(serial: String, apple: [String: Any]?, tag: AnyObject?) {
        last += 1
        let id = "t\(last)"
        if let tag { leases[id] = tag }
        context.emit("readingerror", [["id": id, "serialNumber": serial, "apple": apple ?? NSNull(), "android": NSNull()]])
    }
}
