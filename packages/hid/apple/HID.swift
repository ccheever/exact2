// @exact/hid on macOS (LLP 1133.037 D10–D12): IOKit's `IOHIDManager` for the devices and their plug events, an
// `IOHIDDevice` opened once per process and leased to each connection (D5), input reports into a log each reader
// takes a batch from, and output and feature reports through `IOHIDDeviceSetReport`.
//
// - Not offered (D6, D11): a device whose every top-level collection is on WebHID's blocklist, any device holding a
//   game controller's collection (`@exact/gamepad` owns pads), and any device holding a keyboard or keypad
//   collection, which macOS opens only after the person's Input Monitoring approval, never asked here.
// - Not built yet: dropping a report of a blocked collection on a device that also has an allowed one (D11's
//   per-report rule); such a device is offered, and its blocked reports are not yet told apart.
// - `requestDevice` is a sheet listing the matching devices (D2); the chosen ones are remembered in the app's
//   defaults by vendor, product, serial number and location.
import AppKit
import ExactPackage
import Foundation
import IOKit.hid

public final class HIDModule: ExactPackage {
    private let context: ExactPackageContext
    private let manager = IOHIDManagerCreate(kCFAllocatorDefault, IOOptionBits(kIOHIDOptionsTypeNone))
    private var opened: [String: Shared] = [:]
    private var connections: [String: Connection] = [:]
    private var nextConnection = 0
    private static let rememberedKey = "exact.hid.remembered"
    private static let kept = 1024

    private final class Shared {
        let device: IOHIDDevice
        var leases = 0, sequence = 0, log: [(sequence: Int, reportId: Int, data: Data, at: Double)] = []
        var buffer = [UInt8](repeating: 0, count: 4096)
        init(_ device: IOHIDDevice) { self.device = device }
    }
    private final class Connection {
        let id: String, device: String, shared: Shared
        var open = true, unplugged = false, readers: [String: Int] = [:]
        init(id: String, device: String, shared: Shared) { self.id = id; self.device = device; self.shared = shared }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        IOHIDManagerSetDeviceMatching(manager, nil)
        let me = Unmanaged.passUnretained(self).toOpaque()
        IOHIDManagerRegisterDeviceMatchingCallback(manager, { refcon, _, _, _ in
            guard let refcon else { return }
            Unmanaged<HIDModule>.fromOpaque(refcon).takeUnretainedValue().plugged(nil)
        }, me)
        IOHIDManagerRegisterDeviceRemovalCallback(manager, { refcon, _, _, device in
            guard let refcon else { return }
            Unmanaged<HIDModule>.fromOpaque(refcon).takeUnretainedValue().plugged(device)
        }, me)
        IOHIDManagerScheduleWithRunLoop(manager, CFRunLoopGetMain(), CFRunLoopMode.defaultMode.rawValue)
        IOHIDManagerOpen(manager, IOOptionBits(kIOHIDOptionsTypeNone))
    }

    // MARK: devices

    private func property<T>(_ d: IOHIDDevice, _ key: String) -> T? { IOHIDDeviceGetProperty(d, key as CFString) as? T }

    /// A device's id: stable across launches while it stays in the same port.
    private func idOf(_ d: IOHIDDevice) -> String {
        let vendor: Int = property(d, kIOHIDVendorIDKey) ?? 0, product: Int = property(d, kIOHIDProductIDKey) ?? 0
        let location: Int = property(d, kIOHIDLocationIDKey) ?? 0, usagePage: Int = property(d, kIOHIDPrimaryUsagePageKey) ?? 0
        let usage: Int = property(d, kIOHIDPrimaryUsageKey) ?? 0
        return String(format: "%04x:%04x@%08x/%x.%x", vendor, product, location, usagePage, usage)
    }

    /// The top-level collections, from the device's usage pairs.
    private func collections(_ d: IOHIDDevice) -> [(page: Int, usage: Int)] {
        let pairs: [[String: Any]] = property(d, kIOHIDDeviceUsagePairsKey) ?? []
        return pairs.map { ($0[kIOHIDDeviceUsagePageKey] as? Int ?? 0, $0[kIOHIDDeviceUsageKey] as? Int ?? 0) }
    }

    /// WebHID's blocklist (https://github.com/whatwg/hid/blob/main/blocklist.txt), by collection: FIDO, and Generic
    /// Desktop's pointer, mouse, keyboard, keypad and system controls.
    private static func blocked(_ c: (page: Int, usage: Int)) -> Bool {
        c.page == 0xF1D0 || (c.page == 0x01 && [0x01, 0x02, 0x06, 0x07].contains(c.usage)) || (c.page == 0x01 && (0x80...0x8F).contains(c.usage))
    }

    private func offered(_ d: IOHIDDevice) -> Bool {
        let cs = collections(d)
        if cs.contains(where: { $0.page == 0x01 && [0x04, 0x05, 0x08].contains($0.usage) }) { return false }
        if cs.contains(where: { $0.page == 0x01 && [0x06, 0x07].contains($0.usage) }) { return false }
        return cs.contains { !Self.blocked($0) }
    }

    private func devices() -> [IOHIDDevice] {
        let set = IOHIDManagerCopyDevices(manager) as? Set<IOHIDDevice> ?? []
        return set.filter(offered).sorted { idOf($0) < idOf($1) }
    }

    private func record(_ d: IOHIDDevice, connected: Bool = true) -> [String: Any] {
        [
            "id": idOf(d), "vendorId": property(d, kIOHIDVendorIDKey) as Int? ?? 0, "productId": property(d, kIOHIDProductIDKey) as Int? ?? 0,
            "productName": property(d, kIOHIDProductKey) as String? ?? "",
            "collections": collections(d).map { ["usagePage": $0.page, "usage": $0.usage] },
            "connected": connected, "granted": true,
        ]
    }

    private func matches(_ d: IOHIDDevice, _ filters: [[String: Any]]?, _ exclusions: [[String: Any]]? = nil) -> Bool {
        let one = { (f: [String: Any]) -> Bool in
            let cs = self.collections(d)
            return (f["vendorId"] as? Int).map { $0 == self.property(d, kIOHIDVendorIDKey) } ?? true
                && (f["productId"] as? Int).map { $0 == self.property(d, kIOHIDProductIDKey) } ?? true
                && (f["usagePage"] as? Int).map { page in cs.contains { c in c.page == page && ((f["usage"] as? Int).map { $0 == c.usage } ?? true) } } ?? true
        }
        guard let filters, !filters.isEmpty else { return false }
        return filters.contains(where: one) && !(exclusions ?? []).contains(where: one)
    }

    private var remembered: [String] {
        get { UserDefaults.standard.stringArray(forKey: Self.rememberedKey) ?? [] }
        set { UserDefaults.standard.set(newValue, forKey: Self.rememberedKey) }
    }

    private func plugged(_ removed: IOHIDDevice?) {
        if let removed {
            let id = idOf(removed)
            for c in connections.values where c.device == id { c.unplugged = true; c.open = false }
            opened[id] = nil
        }
        for topic in ["hid.attachedDevices", "hid.getDevices", "hid.hidConnection", "hid.inputReports"] { context.changed(topic) }
    }

    // MARK: names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { self.answer(name, args, reply: reply) }
    }

    private func answer(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let list = { (value: [Any]) in reply(["exact:answer": value]) }
        switch name {
        case "hidStatus": reply(["available": true, "attachedList": true, "state": "granted"])
        case "attachedDevices": list(devices().filter { matches($0, args["filters"] as? [[String: Any]]) }.map { record($0) })
        case "getDevices":
            let here = Dictionary(devices().map { (idOf($0), $0) }, uniquingKeysWith: { a, _ in a })
            list(remembered.map { id in here[id].map { record($0) } ?? ["id": id, "vendorId": 0, "productId": 0, "productName": "", "collections": [], "connected": false, "granted": true] })
        case "requestDevice": choose(args, reply: reply)
        case "open": reply(open(args["device"] as? String ?? ""))
        case "hidConnection":
            let c = connections[args["connection"] as? String ?? ""]
            reply(["open": c?.open ?? false, "connected": c.map { !$0.unplugged } ?? false])
        case "inputReports": reply(reports(args))
        case "sendReport", "sendFeatureReport":
            guard let c = connections[args["connection"] as? String ?? ""], c.open else { return reply(["error": "InvalidStateError"]) }
            guard let data = Data(base64Encoded: args["data"] as? String ?? "") else { return reply(["error": "TypeError"]) }
            let id = args["reportId"] as? Int ?? 0
            // The report id leads the bytes when the device uses report ids (IOHIDDeviceSetReport's contract).
            let bytes = id == 0 ? data : Data([UInt8(id)]) + data
            let type = name == "sendReport" ? kIOHIDReportTypeOutput : kIOHIDReportTypeFeature
            let status = bytes.withUnsafeBytes { IOHIDDeviceSetReport(c.shared.device, type, CFIndex(id), $0.bindMemory(to: UInt8.self).baseAddress!, bytes.count) }
            reply(status == kIOReturnSuccess ? [:] : ["error": "NetworkError", "message": String(format: "IOReturn 0x%08x", status)])
        case "receiveFeatureReport":
            guard let c = connections[args["connection"] as? String ?? ""], c.open else { return reply(["error": "InvalidStateError"]) }
            let id = args["reportId"] as? Int ?? 0
            var buffer = [UInt8](repeating: 0, count: 4096), length = CFIndex(buffer.count)
            let status = IOHIDDeviceGetReport(c.shared.device, kIOHIDReportTypeFeature, CFIndex(id), &buffer, &length)
            reply(status == kIOReturnSuccess ? ["data": Data(buffer[0..<length]).base64EncodedString()] : ["error": "NotAllowedError", "message": String(format: "IOReturn 0x%08x", status)])
        case "close":
            guard let c = connections[args["connection"] as? String ?? ""], c.open else { return reply(["error": "InvalidStateError"]) }
            release(c)
            reply([:])
        case "forget":
            let id = args["device"] as? String ?? ""
            remembered.removeAll { $0 == id }
            for c in connections.values where c.device == id && c.open { release(c) }
            context.changed("hid.getDevices")
            reply([:])
        default: reply(["error": "NotSupportedError"])
        }
    }

    /// The module's own sheet (D2): the matching devices by name; Cancel answers none, as WebHID's chooser does.
    private func choose(_ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let found = devices().filter { matches($0, args["filters"] as? [[String: Any]], args["exclusionFilters"] as? [[String: Any]]) }
        let alert = NSAlert()
        alert.messageText = "Choose a Device"
        alert.informativeText = found.isEmpty ? "No matching device is connected." : "The app can send reports to the device you choose and read its reports."
        let menu = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 300, height: 26), pullsDown: false)
        menu.addItems(withTitles: found.map { property($0, kIOHIDProductKey) ?? idOf($0) })
        if !found.isEmpty { alert.accessoryView = menu }
        alert.addButton(withTitle: "Connect").isEnabled = !found.isEmpty
        alert.addButton(withTitle: "Cancel")
        let done = { (response: NSApplication.ModalResponse) in
            guard response == .alertFirstButtonReturn, found.indices.contains(menu.indexOfSelectedItem) else { return reply(["exact:answer": []]) }
            let d = found[menu.indexOfSelectedItem], id = self.idOf(d)
            if !self.remembered.contains(id) { self.remembered.append(id) }
            self.context.changed("hid.getDevices")
            reply(["exact:answer": [self.record(d)]])
        }
        if let window = context.presentation?.window { alert.beginSheetModal(for: window, completionHandler: done) } else { done(alert.runModal()) }
    }

    private func open(_ id: String) -> [String: Any] {
        guard let d = devices().first(where: { idOf($0) == id }) else { return ["error": "NotFoundError"] }
        let shared: Shared
        if let s = opened[id] {
            shared = s
        } else {
            let status = IOHIDDeviceOpen(d, IOOptionBits(kIOHIDOptionsTypeNone))
            guard status == kIOReturnSuccess else { return ["error": "NetworkError", "message": String(format: "IOReturn 0x%08x", status)] }
            shared = Shared(d)
            opened[id] = shared
            let context = Unmanaged.passRetained(shared).toOpaque()
            // Input reports from every lease's first open to the device's close (D12).
            shared.buffer.withUnsafeMutableBufferPointer { buffer in
                IOHIDDeviceRegisterInputReportCallback(d, buffer.baseAddress!, buffer.count, { refcon, _, _, _, reportId, report, length in
                    guard let refcon else { return }
                    let s = Unmanaged<Shared>.fromOpaque(refcon).takeUnretainedValue()
                    s.sequence += 1
                    s.log.append((s.sequence, Int(reportId), Data(bytes: report, count: length), ProcessInfo.processInfo.systemUptime * 1000))
                    if s.log.count > HIDModule.kept { s.log.removeFirst(s.log.count - HIDModule.kept) }
                }, context)
            }
            IOHIDDeviceScheduleWithRunLoop(d, CFRunLoopGetMain(), CFRunLoopMode.defaultMode.rawValue)
        }
        shared.leases += 1
        nextConnection += 1
        let handle = "hid-\(nextConnection)"
        connections[handle] = Connection(id: handle, device: id, shared: shared)
        if !remembered.contains(id) { remembered.append(id) }
        context.changed("hid.getDevices")
        return ["connection": handle, "device": record(d)]
    }

    private func reports(_ args: [String: Any]) -> [String: Any] {
        guard let c = connections[args["connection"] as? String ?? ""] else { return ["error": "InvalidStateError"] }
        let wanted = args["reportId"] as? Int, history = min(1024, args["history"] as? Int ?? 16)
        let reader = "\(wanted ?? -1) \(history)"
        let log = c.shared.log.filter { wanted == nil || $0.reportId == wanted }
        var fresh = c.readers[reader].map { cursor in log.filter { $0.sequence > cursor } } ?? Array(log.suffix(1))
        var dropped = 0
        if fresh.count > history { dropped = fresh.count - history; fresh = Array(fresh.suffix(history)) }
        c.readers[reader] = c.shared.sequence
        return ["reports": fresh.map { ["reportId": $0.reportId, "data": $0.data.base64EncodedString(), "at": $0.at] }, "dropped": dropped, "interrupted": c.unplugged]
    }

    private func release(_ c: Connection) {
        c.open = false
        c.shared.leases -= 1
        if c.shared.leases == 0 {
            IOHIDDeviceClose(c.shared.device, IOOptionBits(kIOHIDOptionsTypeNone))
            opened[c.device] = nil
        }
        context.changed("hid.hidConnection")
    }

    public func destroy(_ done: @escaping () -> Void) {
        DispatchQueue.main.async {
            for c in self.connections.values where c.open { self.release(c) }
            IOHIDManagerClose(self.manager, IOOptionBits(kIOHIDOptionsTypeNone))
            done()
        }
    }
}
