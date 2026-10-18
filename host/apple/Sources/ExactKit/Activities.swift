// @ref LLP 1126.001 WP9 — the app imports no ActivityKit. Its arm loads on demand.
import Foundation

typealias ActivityCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

final class ActivityLibrary {
    typealias Open = @convention(c) (UnsafeMutableRawPointer?, ActivityCallback?) -> UInt
    typealias Close = @convention(c) (UInt) -> Void
    typealias Send = @convention(c) (UInt, UInt64, UnsafePointer<UInt8>?, Int) -> Void
    let open: Open, close: Close, send: Send
    private init(_ open: Open, _ close: Close, _ send: Send) { self.open = open; self.close = close; self.send = send }
    static let shared: ActivityLibrary? = {
        #if os(iOS)
        let path = embeddedModule(framework: "ExactActivity", dylib: "libexact_activity.dylib")
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else { return nil }
        guard let open = dlsym(library, "exact_activity_open"), let close = dlsym(library, "exact_activity_close"), let send = dlsym(library, "exact_activity_send") else { dlclose(library); return nil }
        return ActivityLibrary(unsafeBitCast(open, to: Open.self), unsafeBitCast(close, to: Close.self), unsafeBitCast(send, to: Send.self))
        #else
        return nil
        #endif
    }()
}

final class ActivityOutput {
    private weak var session: ExactSession?
    private var listener: UInt = 0
    private var context: UInt = 0
    private var library: ActivityLibrary?
    private var snapshot: String?
    private var sends: [UInt64: (UInt64, Int)] = [:]
    private static var nextSend: UInt64 = 0
    private var queued: [([String: Any], Int)] = []
    private final class Weak { weak var output: ActivityOutput?; init(_ output: ActivityOutput) { self.output = output } }
    private static var outputs: [UInt: Weak] = [:]
    private static var next: UInt = 0
    init(_ session: ExactSession) { self.session = session }
    deinit { if let library, listener != 0 { library.close(listener) }; Self.outputs.removeValue(forKey: context) }

    func enqueue(_ op: [String: Any]) {
        guard let session else { return }
        queued.append((op, session.generation))
        if session.firstDrawMs != nil { drain() }
    }
    func drain() {
        guard let session, session.firstDrawMs != nil else { return }
        let work = queued
        queued = []
        for (op, generation) in work where generation == session.generation {
            DispatchQueue.main.async { [weak self] in
                guard let self, let session = self.session, session.state != .destroyed, session.generation == generation else { return }
                self.apply(op)
            }
        }
    }
    private func apply(_ op: [String: Any]) {
        guard let session else { return }
        if library == nil {
            library = ActivityLibrary.shared
            if let library {
                Self.next += 1; context = Self.next; Self.outputs[context] = Weak(self)
                listener = library.open(UnsafeMutableRawPointer(bitPattern: context), { context, bytes, count in
                    guard let context, let bytes else { return }
                    let json = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
                    let key = UInt(bitPattern: context)
                    DispatchQueue.main.async { ActivityOutput.outputs[key]?.output?.report(json) }
                })
            }
        }
        guard let library, listener != 0 else {
            if let ticket = op["ticket"] {
                reportJSON(["ticket": ticket, "status": 501, "error": "ActivityKit is not linked into this artifact"])
            }
            return
        }
        guard let ticket = (op["ticket"] as? NSNumber)?.uint64Value, var request = op["request"] as? [String: Any] else {
            if let snapshot { report(snapshot) }
            return
        }
        do {
            _ = try WidgetContainer.publish(session.app.resolver)
            request["generationDirectory"] = try WidgetContainer.pin().path
            let data = try JSONSerialization.data(withJSONObject: request)
            Self.nextSend += 1
            let global = Self.nextSend
            sends[global] = (ticket, session.generation)
            data.withUnsafeBytes { raw in library.send(listener, global, raw.bindMemory(to: UInt8.self).baseAddress, raw.count) }
        } catch { reportJSON(["ticket": ticket, "status": 400, "error": String(describing: error)]) }
    }
    private func reportJSON(_ value: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: value) else { return }
        guard let session, session.booted, session.state != .destroyed else { return }
        session.apply(session.runtime.activities(String(decoding: data, as: UTF8.self)))
    }
    private func report(_ json: String) {
        guard let session, session.booted, session.state != .destroyed else { return }
        guard let data = json.data(using: .utf8), var value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        if let global = (value["ticket"] as? NSNumber)?.uint64Value {
            guard let (ticket, generation) = sends.removeValue(forKey: global), generation == session.generation else { return }
            value["ticket"] = ticket
            guard let translated = try? JSONSerialization.data(withJSONObject: value) else { return }
            session.apply(session.runtime.activities(String(decoding: translated, as: UTF8.self)))
        } else {
            if value["activities"] != nil { snapshot = json }
            session.apply(session.runtime.activities(json))
        }
    }
}
