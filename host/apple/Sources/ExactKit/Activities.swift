// @ref LLP 1126.001 WP9 — the app imports no ActivityKit. Its arm loads on demand.
import Foundation

typealias ActivityCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

final class ActivityLibrary {
    typealias Open = @convention(c) (UnsafeMutableRawPointer?, ActivityCallback?) -> UInt
    typealias Close = @convention(c) (UInt) -> Void
    typealias Send = @convention(c) (UInt, UInt64, UnsafePointer<UInt8>?, Int) -> Void
    let open: Open, close: Close, send: Send
    private init(_ open: Open, _ close: Close, _ send: Send) { self.open = open; self.close = close; self.send = send }
    static var packaged: Bool {
        #if os(iOS)
        FileManager.default.fileExists(atPath: embeddedModule(framework: "ExactActivity", dylib: "libexact_activity.dylib"))
        #else
        false
        #endif
    }
    private static var shared: ActivityLibrary?
    static func load() throws -> ActivityLibrary {
        #if os(iOS)
        if let shared { return shared }
        let loaded = try load(path: embeddedModule(framework: "ExactActivity", dylib: "libexact_activity.dylib"))
        shared = loaded
        return loaded
        #else
        throw HeadlessSession.Failure(description: "ActivityKit is not linked into this artifact")
        #endif
    }
    static func load(path: String) throws -> ActivityLibrary {
        _ = dlerror()
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            throw HeadlessSession.Failure(description: "ActivityKit arm: \(dlerror().map { String(cString: $0) } ?? "dlopen failed"): \(path)")
        }
        func symbol(_ name: String) throws -> UnsafeMutableRawPointer {
            _ = dlerror()
            guard let symbol = dlsym(library, name) else {
                throw HeadlessSession.Failure(description: "ActivityKit arm: \(dlerror().map { String(cString: $0) } ?? "missing \(name)"): \(path)")
            }
            return symbol
        }
        do {
            return try ActivityLibrary(unsafeBitCast(symbol("exact_activity_open"), to: Open.self), unsafeBitCast(symbol("exact_activity_close"), to: Close.self), unsafeBitCast(symbol("exact_activity_send"), to: Send.self))
        } catch { dlclose(library); throw error }
    }
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
        guard let session, session.firstDrawMs != nil, session.dataActivated else { return }
        let work = queued
        queued = []
        for (op, generation) in work where generation == session.generation {
            let apply: () -> Void = { [weak self] in
                guard let self, let session = self.session, session.state != .destroyed, session.generation == generation else { return }
                self.apply(op)
            }
            if op["ticket"] == nil && library == nil {
                // First enumeration costs a main-thread framework load. Let dataReady's
                // Core Animation commit finish before starting it in an idle turn.
                let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue | CFRunLoopActivity.exit.rawValue, false, 2_000_001) { _, _ in
                    DispatchQueue.main.async(execute: apply)
                }
                CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
            } else { DispatchQueue.main.async(execute: apply) }
        }
    }
    private func apply(_ op: [String: Any]) {
        guard let session else { return }
        do {
            if library == nil { library = try ActivityLibrary.load() }
            if let library, listener == 0 {
                Self.next += 1; context = Self.next; Self.outputs[context] = Weak(self)
                listener = library.open(UnsafeMutableRawPointer(bitPattern: context), { context, bytes, count in
                    guard let context, let bytes else { return }
                    let json = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
                    let key = UInt(bitPattern: context)
                    DispatchQueue.main.async { ActivityOutput.outputs[key]?.output?.report(json) }
                })
                if listener == 0 {
                    Self.outputs.removeValue(forKey: context)
                    throw HeadlessSession.Failure(description: "ActivityKit arm: exact_activity_open returned no listener")
                }
            }
        } catch {
            let message = String(describing: error)
            FileHandle.standardError.write(Data("\(message)\n".utf8))
            // Record a demand failure even without a send ticket; a future demand retries.
            reportJSON(["error": message])
            if let ticket = op["ticket"] { reportJSON(["ticket": ticket, "status": 501, "error": message]) }
            return
        }
        guard let library else { return }
        guard let ticket = (op["ticket"] as? NSNumber)?.uint64Value, var request = op["request"] as? [String: Any] else {
            if let snapshot { report(snapshot) }
            return
        }
        do {
            request["generationDirectory"] = try WidgetContainer.pinCurrent(session.app.resolver).path
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
