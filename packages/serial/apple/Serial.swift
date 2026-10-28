// @exact/serial on macOS (LLP 1133.037 D7–D9): IOKit's serial family for the ports and their USB names, the callout
// device (`/dev/cu.*`) opened exclusive (`TIOCEXCL`) with termios, and a read loop on a queue of the module's own.
//
// - The attached list is every serial BSD client IOKit lists, updated on its match and termination notifications;
//   tier 2's `apple.path` and `locationID` (D15) are not given yet;
//   a development build also offers the ports `EXACT_DEVICE_FIXTURE` names (a pseudo-terminal a drive made, D19 item 3).
// - `requestPort` is a sheet listing the matching ports, on the session's window (D2); the remembered ports are kept
//   in the app's defaults by id, matched again when plugged in.
// - Reading loses nothing (D8): the read source is suspended while the undelivered bytes reach the retained bound, so
//   the rest waits in the OS's and the device's buffers; each `serialInput` answer takes every byte since the last.
import AppKit
import ExactPackage
import Foundation
import IOKit
import IOKit.serial

public final class SerialModule: ExactPackage {
    private let context: ExactPackageContext
    private let queue = DispatchQueue(label: "exact.serial")
    private var connections: [String: Connection] = [:]
    private var nextConnection = 0
    private var notify: IONotificationPortRef?
    private var iterators: [io_iterator_t] = []
    private static let rememberedKey = "exact.serial.remembered"
    private static let most = 16 * 1024 * 1024

    private final class Connection {
        let id: String, path: String, fd: Int32, options: [String: Any], bound: Int
        var source: DispatchSourceRead?
        var suspended = false
        var pending = Data(), errors: [[String: Any]] = [], position = 0
        var open = true, done = false, ended = false, unplugged = false
        init(id: String, path: String, fd: Int32, options: [String: Any], bound: Int) {
            self.id = id; self.path = path; self.fd = fd; self.options = options; self.bound = bound
        }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        watch()
    }

    // MARK: ports

    /// One serial port: its callout path is its id; its USB names come from the device it hangs off.
    private struct Port { let id: String, name: String, path: String, vendor: Int?, product: Int? }

    private func ports() -> [Port] {
        var out: [Port] = []
        var iterator: io_iterator_t = 0
        if IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching(kIOSerialBSDServiceValue), &iterator) == KERN_SUCCESS {
            defer { IOObjectRelease(iterator) }
            while case let service = IOIteratorNext(iterator), service != 0 {
                defer { IOObjectRelease(service) }
                guard let path = IORegistryEntryCreateCFProperty(service, kIOCalloutDeviceKey as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue() as? String else { continue }
                let up = { (key: String) in IORegistryEntrySearchCFProperty(service, kIOServicePlane, key as CFString, kCFAllocatorDefault, IOOptionBits(kIORegistryIterateRecursively | kIORegistryIterateParents)) }
                let product = up("USB Product Name") as? String
                let name = product ?? (path as NSString).lastPathComponent
                out.append(Port(id: path, name: name, path: path, vendor: up("idVendor") as? Int, product: up("idProduct") as? Int))
            }
        }
        return out + injected()
    }

    /// The ports a development build's drive injects (`EXACT_DEVICE_FIXTURE`, a JSON file `{"serial": [{"id", "name",
    /// "path"}]}`): a pseudo-terminal, opened with the same termios code as a real port. A production build
    /// compiles none of it (`EXACT_DEVELOPMENT`, which `host/apple/build.mjs` defines for development builds).
    private func injected() -> [Port] {
        #if EXACT_DEVELOPMENT
        guard let file = ProcessInfo.processInfo.environment["EXACT_DEVICE_FIXTURE"],
              let data = FileManager.default.contents(atPath: file),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = json["serial"] as? [[String: Any]] else { return [] }
        return list.compactMap { p in
            guard let path = p["path"] as? String, FileManager.default.fileExists(atPath: path) else { return nil }
            return Port(id: p["id"] as? String ?? path, name: p["name"] as? String ?? path, path: path, vendor: p["usbVendorId"] as? Int, product: p["usbProductId"] as? Int)
        }
        #else
        return []
        #endif
    }

    private func record(_ p: Port, connected: Bool = true) -> [String: Any] {
        // An option the port lacks is null, not left out: data code passes the record on to Contract as it is.
        ["id": p.id, "name": p.name, "connected": connected, "granted": true, "usbVendorId": p.vendor ?? NSNull(), "usbProductId": p.product ?? NSNull()]
    }

    private var remembered: [String] {
        get { UserDefaults.standard.stringArray(forKey: Self.rememberedKey) ?? [] }
        set { UserDefaults.standard.set(newValue, forKey: Self.rememberedKey) }
    }

    private func remember(_ id: String) {
        if !remembered.contains(id) { remembered.append(id) }
    }

    private func matches(_ p: Port, _ filters: [[String: Any]]?) -> Bool {
        guard let filters, !filters.isEmpty else { return true }
        return filters.contains { f in
            (f["usbVendorId"] as? Int).map { $0 == p.vendor } ?? true && (f["usbProductId"] as? Int).map { $0 == p.product } ?? true
        }
    }

    /// IOKit's match and termination notifications for serial ports: their readers are asked again.
    private func watch() {
        guard let port = IONotificationPortCreate(kIOMainPortDefault) else { return }
        notify = port
        IONotificationPortSetDispatchQueue(port, queue)
        let me = Unmanaged.passUnretained(self).toOpaque()
        let callback: IOServiceMatchingCallback = { refcon, iterator in
            while case let s = IOIteratorNext(iterator), s != 0 { IOObjectRelease(s) }
            guard let refcon else { return }
            Unmanaged<SerialModule>.fromOpaque(refcon).takeUnretainedValue().plugged()
        }
        for kind in [kIOFirstMatchNotification, kIOTerminatedNotification] {
            var iterator: io_iterator_t = 0
            if IOServiceAddMatchingNotification(port, kind, IOServiceMatching(kIOSerialBSDServiceValue), callback, me, &iterator) == KERN_SUCCESS {
                while case let s = IOIteratorNext(iterator), s != 0 { IOObjectRelease(s) }
                iterators.append(iterator)
            }
        }
    }

    private func plugged() {
        let here = Set(ports().map(\.path))
        for c in connections.values where c.open && !here.contains(c.path) {
            c.unplugged = true
            stopReading(c, error: "NetworkError")
        }
        for topic in ["serial.attachedPorts", "serial.getPorts", "serial.serialConnection", "serial.serialInput"] { context.changed(topic) }
    }

    // MARK: names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let list = { (value: [Any]) in reply(["exact:answer": value]) }
        switch name {
        case "serialStatus": reply(["available": true, "attachedList": true, "state": "granted"])
        case "attachedPorts": list(ports().filter { matches($0, args["filters"] as? [[String: Any]]) }.map { record($0) })
        case "getPorts":
            let here = Dictionary(ports().map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
            list(remembered.map { id in here[id].map { record($0) } ?? ["id": id, "name": (id as NSString).lastPathComponent, "connected": false, "granted": true, "usbVendorId": NSNull(), "usbProductId": NSNull()] })
        case "requestPort": choose(args, reply: reply)
        case "open": queue.async { reply(self.open(args)) }
        case "serialConnection":
            queue.async {
                let c = self.connection(args)
                reply(c.map { ["open": $0.open && !$0.unplugged, "connected": !$0.unplugged, "options": $0.options] } ?? ["open": false, "connected": false])
            }
        case "serialInput": queue.async { reply(self.input(args)) }
        case "write": queue.async { reply(self.write(args)) }
        case "setSignals": queue.async { reply(self.setSignals(args)) }
        case "getSignals": queue.async { reply(self.getSignals(args)) }
        case "close": queue.async { reply(self.close(args["connection"] as? String ?? "")) }
        case "forget":
            let id = args["port"] as? String ?? ""
            remembered.removeAll { $0 == id }
            queue.async {
                for c in self.connections.values where c.path == id { _ = self.close(c.id) }
                reply([:])
                self.context.changed("serial.getPorts")
            }
        default: reply(["error": "NotSupportedError"])
        }
    }

    /// The module's own sheet (D2): the matching ports by name, Cancel failing `NotFoundError`.
    private func choose(_ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let found = ports().filter { matches($0, args["filters"] as? [[String: Any]]) }
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = "Choose a Serial Port"
            alert.informativeText = found.isEmpty ? "No matching port is connected." : "The app can read and write the port you choose."
            let menu = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 300, height: 26), pullsDown: false)
            menu.addItems(withTitles: found.map(\.name))
            if !found.isEmpty { alert.accessoryView = menu }
            alert.addButton(withTitle: "Connect").isEnabled = !found.isEmpty
            alert.addButton(withTitle: "Cancel")
            let done = { (response: NSApplication.ModalResponse) in
                guard response == .alertFirstButtonReturn, menu.indexOfSelectedItem >= 0, menu.indexOfSelectedItem < found.count else {
                    return reply(["error": "NotFoundError"])
                }
                let p = found[menu.indexOfSelectedItem]
                self.remember(p.id)
                self.context.changed("serial.getPorts")
                reply(self.record(p))
            }
            if let window = self.context.presentation?.window { alert.beginSheetModal(for: window, completionHandler: done) } else { done(alert.runModal()) }
        }
    }

    // MARK: a connection, on the queue

    private func open(_ args: [String: Any]) -> [String: Any] {
        let id = args["port"] as? String ?? ""
        guard let port = ports().first(where: { $0.id == id }) else { return ["error": "NotFoundError"] }
        if connections.values.contains(where: { $0.open && $0.path == port.path }) { return ["error": "InvalidStateError", "message": "already open"] }
        let baud = args["baudRate"] as? Int ?? Int(args["baudRate"] as? Double ?? 0)
        let options: [String: Any] = ["baudRate": baud, "dataBits": args["dataBits"] as? Int ?? 8, "stopBits": args["stopBits"] as? Int ?? 1, "parity": args["parity"] as? String ?? "none", "bufferSize": args["bufferSize"] as? Int ?? 255, "flowControl": args["flowControl"] as? String ?? "none"]
        guard baud > 0 else { return ["error": "TypeError", "message": "baudRate"] }
        let fd = Darwin.open(port.path, O_RDWR | O_NOCTTY | O_NONBLOCK)
        guard fd >= 0 else { return ["error": errno == EBUSY ? "InvalidStateError" : "NetworkError", "message": String(cString: strerror(errno))] }
        // Another process's open now fails EBUSY (tty_ioctl, D5).
        _ = ioctl(fd, TIOCEXCL)
        var t = termios()
        guard tcgetattr(fd, &t) == 0 else { Darwin.close(fd); return ["error": "NetworkError", "message": String(cString: strerror(errno))] }
        cfmakeraw(&t)
        t.c_cflag &= ~tcflag_t(CSIZE | CSTOPB | PARENB | PARODD | CRTSCTS)
        t.c_cflag |= tcflag_t(CREAD | CLOCAL) | tcflag_t((options["dataBits"] as? Int) == 7 ? CS7 : CS8)
        if (options["stopBits"] as? Int) == 2 { t.c_cflag |= tcflag_t(CSTOPB) }
        switch options["parity"] as? String {
        case "even": t.c_cflag |= tcflag_t(PARENB)
        case "odd": t.c_cflag |= tcflag_t(PARENB | PARODD)
        default: break
        }
        if (options["flowControl"] as? String) == "hardware" { t.c_cflag |= tcflag_t(CRTSCTS) }
        cfsetspeed(&t, speed_t(baud))
        guard tcsetattr(fd, TCSANOW, &t) == 0 else { Darwin.close(fd); return ["error": "NetworkError", "message": String(cString: strerror(errno))] }
        nextConnection += 1
        let handle = "serial-\(nextConnection)"
        let bound = min(Self.most, max(options["bufferSize"] as? Int ?? 255, baud / 10 * 2))
        let c = Connection(id: handle, path: port.path, fd: fd, options: options, bound: bound)
        connections[handle] = c
        startReading(c)
        DispatchQueue.main.async { self.remember(port.id); self.context.changed("serial.getPorts") }
        return ["connection": handle, "port": record(port), "options": options]
    }

    private func startReading(_ c: Connection) {
        let source = DispatchSource.makeReadSource(fileDescriptor: c.fd, queue: queue)
        source.setEventHandler { [weak self, weak c] in
            guard let self, let c else { return }
            var buffer = [UInt8](repeating: 0, count: 4096)
            while c.pending.count < c.bound {
                let n = read(c.fd, &buffer, min(buffer.count, c.bound - c.pending.count))
                if n > 0 { c.pending.append(contentsOf: buffer[0..<n]); continue }
                if n < 0 && (errno == EAGAIN || errno == EINTR) { break }
                // End of file, EIO or ENXIO: the port went away (D9).
                c.unplugged = true
                self.stopReading(c, error: "NetworkError")
                break
            }
            // Backpressure (D8): no more reads while the undelivered bytes reach the bound.
            if c.pending.count >= c.bound, !c.suspended, let s = c.source { s.suspend(); c.suspended = true }
            self.context.changed("serial.serialInput")
        }
        c.source = source
        source.resume()
    }

    private func stopReading(_ c: Connection, error: String?) {
        if let error { c.errors.append(["at": c.position + c.pending.count, "code": error]) }
        c.done = true
        if let s = c.source {
            if c.suspended { s.resume(); c.suspended = false }
            s.cancel()
            c.source = nil
        }
    }

    private func connection(_ args: [String: Any]) -> Connection? {
        connections[args["connection"] as? String ?? ""]
    }

    private func input(_ args: [String: Any]) -> [String: Any] {
        guard let c = connection(args) else { return ["error": "InvalidStateError", "message": "no such connection"] }
        if c.ended { return ["error": "InvalidStateError", "message": "the stream has ended"] }
        let bytes = c.pending
        c.pending = Data()
        c.position += bytes.count
        let errors = c.errors
        c.errors = []
        if c.suspended, let s = c.source { s.resume(); c.suspended = false }
        let done = c.done || !c.open
        if done { c.ended = true }
        return ["data": bytes.base64EncodedString(), "errors": errors, "done": done, "position": c.position]
    }

    private func write(_ args: [String: Any]) -> [String: Any] {
        guard let c = connection(args), c.open else { return ["error": "InvalidStateError"] }
        if c.unplugged { return ["error": "NetworkError"] }
        guard let data = Data(base64Encoded: args["data"] as? String ?? "") else { return ["error": "TypeError"] }
        var sent = 0
        let ok = data.withUnsafeBytes { raw -> Bool in
            while sent < raw.count {
                let n = Darwin.write(c.fd, raw.baseAddress! + sent, raw.count - sent)
                if n > 0 { sent += n; continue }
                if n < 0 && (errno == EAGAIN || errno == EINTR) {
                    var p = pollfd(fd: c.fd, events: Int16(POLLOUT), revents: 0)
                    _ = poll(&p, 1, 1000)
                    continue
                }
                return false
            }
            return true
        }
        return ok ? [:] : ["error": "NetworkError", "message": String(cString: strerror(errno))]
    }

    private func setSignals(_ args: [String: Any]) -> [String: Any] {
        guard let c = connection(args), c.open else { return ["error": "InvalidStateError"] }
        var failed = false
        for (key, bit) in [("dataTerminalReady", TIOCM_DTR), ("requestToSend", TIOCM_RTS)] {
            guard let on = args[key] as? Bool else { continue }
            var bits = Int32(bit)
            if ioctl(c.fd, on ? TIOCMBIS : TIOCMBIC, &bits) != 0 { failed = true }
        }
        if let on = args["break"] as? Bool, ioctl(c.fd, on ? TIOCSBRK : TIOCCBRK) != 0 { failed = true }
        // A pseudo-terminal carries no modem lines (ENOTTY).
        return failed ? ["error": "NetworkError", "message": String(cString: strerror(errno))] : [:]
    }

    private func getSignals(_ args: [String: Any]) -> [String: Any] {
        guard let c = connection(args), c.open else { return ["error": "InvalidStateError"] }
        var bits: Int32 = 0
        guard ioctl(c.fd, TIOCMGET, &bits) == 0 else { return ["error": "NetworkError", "message": String(cString: strerror(errno))] }
        return ["dataCarrierDetect": bits & Int32(TIOCM_CD) != 0, "clearToSend": bits & Int32(TIOCM_CTS) != 0, "ringIndicator": bits & Int32(TIOCM_RI) != 0, "dataSetReady": bits & Int32(TIOCM_DSR) != 0]
    }

    private func close(_ handle: String) -> [String: Any] {
        guard let c = connections[handle], c.open else { return ["error": "InvalidStateError"] }
        c.open = false
        stopReading(c, error: nil)
        Darwin.close(c.fd)
        context.changed("serial.serialConnection")
        context.changed("serial.serialInput")
        return [:]
    }

    public func destroy(_ done: @escaping () -> Void) {
        queue.async {
            for c in self.connections.values where c.open { _ = self.close(c.id) }
            for i in self.iterators { IOObjectRelease(i) }
            if let n = self.notify { IONotificationPortDestroy(n) }
            done()
        }
    }
}
