// @exact/usb on macOS (LLP 1133.037 D13, D14): IOUSBHost. The devices come from IOKit's `IOUSBHostDevice` services,
// their configurations from the configuration descriptors; `open` makes an `IOUSBHostDevice` the process shares among
// its connections (D5); `claimInterface` opens that interface's `IOUSBHostInterface`, never capturing one a kernel
// driver holds (a CDC port's `AppleUSBCDCACM`: `NetworkError`); transfers are the device's and the pipes' requests.
//
// - Filters are required and the attached list carries only the devices they match (D3).
// - WebUSB's protected interface classes cannot be claimed (`SecurityError`, D6).
// - `requestDevice` is a sheet listing the matching devices (D2); chosen ones are remembered in the app's defaults.
import AppKit
import ExactPackage
import Foundation
import IOKit
import IOKit.usb
import IOUSBHost

public final class USBModule: ExactPackage {
    private let context: ExactPackageContext
    private let queue = DispatchQueue(label: "exact.usb")
    private var opened: [String: Shared] = [:]
    private var connections: [String: Connection] = [:]
    private var nextConnection = 0
    private var notify: IONotificationPortRef?
    private static let rememberedKey = "exact.usb.remembered"
    /// WebUSB's protected interface classes (https://wicg.github.io/webusb/#protected-interface-classes).
    private static let protected: Set<Int> = [0x01, 0x03, 0x08, 0x09, 0x0B, 0x0E, 0x10, 0xE0]
    /// `kIOUSBPipeStalled` (`iokit_usb_err(0x4f)`, IOUSBHostFamily's `IOUSBHostFamily.h`), a C macro Swift does not import.
    private static let pipeStalled = Int(Int32(bitPattern: 0xE000_404F))

    private final class Shared {
        let device: IOUSBHostDevice
        var leases = 0, interfaces: [Int: IOUSBHostInterface] = [:], claims: [Int: String] = [:]
        init(_ device: IOUSBHostDevice) { self.device = device }
    }
    private final class Connection {
        let id: String, device: String, shared: Shared
        var open = true, claimed: Set<Int> = []
        init(id: String, device: String, shared: Shared) { self.id = id; self.device = device; self.shared = shared }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        guard let port = IONotificationPortCreate(kIOMainPortDefault) else { return }
        notify = port
        IONotificationPortSetDispatchQueue(port, queue)
        let me = Unmanaged.passUnretained(self).toOpaque()
        for kind in [kIOFirstMatchNotification, kIOTerminatedNotification] {
            var iterator: io_iterator_t = 0
            let callback: IOServiceMatchingCallback = { refcon, iterator in
                while case let s = IOIteratorNext(iterator), s != 0 { IOObjectRelease(s) }
                guard let refcon else { return }
                let m = Unmanaged<USBModule>.fromOpaque(refcon).takeUnretainedValue()
                for topic in ["usb.attachedDevices", "usb.getDevices", "usb.usbConnection"] { m.context.changed(topic) }
            }
            if IOServiceAddMatchingNotification(port, kind, IOServiceMatching("IOUSBHostDevice"), callback, me, &iterator) == KERN_SUCCESS {
                while case let s = IOIteratorNext(iterator), s != 0 { IOObjectRelease(s) }
            }
        }
    }

    // MARK: devices

    private struct Found { let id: String, record: [String: Any], service: io_service_t }

    private func number(_ s: io_service_t, _ key: String) -> Int? {
        IORegistryEntryCreateCFProperty(s, key as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue() as? Int
    }
    private func string(_ s: io_service_t, _ key: String) -> String? {
        IORegistryEntryCreateCFProperty(s, key as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue() as? String
    }

    /// Each USB device IOKit lists, with its record. The caller releases each service.
    private func found() -> [Found] {
        var out: [Found] = []
        var iterator: io_iterator_t = 0
        guard IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching("IOUSBHostDevice"), &iterator) == KERN_SUCCESS else { return [] }
        defer { IOObjectRelease(iterator) }
        while case let s = IOIteratorNext(iterator), s != 0 {
            let vendor = number(s, "idVendor") ?? 0, product = number(s, "idProduct") ?? 0, location = number(s, "locationID") ?? 0
            let id = String(format: "%04x:%04x@%08x", vendor, product, location)
            let bcd = number(s, "bcdDevice") ?? 0, usb = number(s, "bcdUSB") ?? 0
            let record: [String: Any] = [
                "id": id, "vendorId": vendor, "productId": product,
                "usbVersionMajor": usb >> 8, "usbVersionMinor": (usb >> 4) & 0xF, "usbVersionSubminor": usb & 0xF,
                "deviceClass": number(s, "bDeviceClass") ?? 0, "deviceSubclass": number(s, "bDeviceSubClass") ?? 0, "deviceProtocol": number(s, "bDeviceProtocol") ?? 0,
                "deviceVersionMajor": bcd >> 8, "deviceVersionMinor": (bcd >> 4) & 0xF, "deviceVersionSubminor": bcd & 0xF,
                "manufacturerName": string(s, "USB Vendor Name") ?? "", "productName": string(s, "USB Product Name") ?? "",
                "serialNumber": string(s, "USB Serial Number") ?? NSNull(), "configuration": number(s, "bConfigurationValue") ?? NSNull(), "configurations": configurations(s),
                "connected": true, "granted": true,
            ]
            out.append(Found(id: id, record: record, service: s))
        }
        return out
    }

    /// The configurations, read from the IORegistry's interfaces of the current configuration: each interface's
    /// alternates and endpoints as IOKit published them.
    private func configurations(_ device: io_service_t) -> [[String: Any]] {
        var interfaces: [Int: [[String: Any]]] = [:]
        var iterator: io_iterator_t = 0
        guard IORegistryEntryCreateIterator(device, kIOServicePlane, IOOptionBits(kIORegistryIterateRecursively), &iterator) == KERN_SUCCESS else { return [] }
        defer { IOObjectRelease(iterator) }
        var value = 1
        while case let s = IOIteratorNext(iterator), s != 0 {
            defer { IOObjectRelease(s) }
            guard IOObjectConformsTo(s, "IOUSBHostInterface") != 0, let n = number(s, "bInterfaceNumber") else { continue }
            value = number(s, "bConfigurationValue") ?? value
            let alternate: [String: Any] = [
                "alternateSetting": number(s, "bAlternateSetting") ?? 0, "interfaceClass": number(s, "bInterfaceClass") ?? 0,
                "interfaceSubclass": number(s, "bInterfaceSubClass") ?? 0, "interfaceProtocol": number(s, "bInterfaceProtocol") ?? 0,
                "interfaceName": string(s, "USB Interface Name") ?? NSNull(), "endpoints": [],
            ]
            interfaces[n, default: []].append(alternate)
        }
        let list = interfaces.keys.sorted().map { ["interfaceNumber": $0, "alternates": interfaces[$0]!] as [String: Any] }
        return list.isEmpty ? [] : [["configurationValue": value, "configurationName": NSNull(), "interfaces": list]]
    }

    /// A remembered device not plugged in: its id, every other field empty, `connected` false.
    private static func gone(_ id: String) -> [String: Any] {
        var r: [String: Any] = ["id": id, "manufacturerName": "", "productName": "", "serialNumber": NSNull(), "configuration": NSNull(), "configurations": [], "connected": false, "granted": true]
        for key in ["usbVersionMajor", "usbVersionMinor", "usbVersionSubminor", "deviceClass", "deviceSubclass", "deviceProtocol", "vendorId", "productId", "deviceVersionMajor", "deviceVersionMinor", "deviceVersionSubminor"] { r[key] = 0 }
        return r
    }

    private func matches(_ r: [String: Any], _ filters: [[String: Any]]?) -> Bool {
        guard let filters, !filters.isEmpty else { return false }
        let classes = ((r["configurations"] as? [[String: Any]])?.first?["interfaces"] as? [[String: Any]] ?? [])
            .flatMap { ($0["alternates"] as? [[String: Any]]) ?? [] }
            .map { ($0["interfaceClass"] as? Int ?? -1, $0["interfaceSubclass"] as? Int ?? -1, $0["interfaceProtocol"] as? Int ?? -1) }
            + [(r["deviceClass"] as? Int ?? -1, r["deviceSubclass"] as? Int ?? -1, r["deviceProtocol"] as? Int ?? -1)]
        return filters.contains { f in
            let eq = { (k: String, v: Any?) in (f[k] as? Int).map { $0 == v as? Int } ?? true }
            return eq("vendorId", r["vendorId"]) && eq("productId", r["productId"])
                && ((f["serialNumber"] as? String).map { $0 == r["serialNumber"] as? String } ?? true)
                && classes.contains { c in
                    (f["classCode"] as? Int).map { $0 == c.0 } ?? true && (f["subclassCode"] as? Int).map { $0 == c.1 } ?? true
                        && (f["protocolCode"] as? Int).map { $0 == c.2 } ?? true
                }
        }
    }

    private var remembered: [String] {
        get { UserDefaults.standard.stringArray(forKey: Self.rememberedKey) ?? [] }
        set { UserDefaults.standard.set(newValue, forKey: Self.rememberedKey) }
    }
    private func remember(_ id: String) { if !remembered.contains(id) { remembered.append(id) } }

    // MARK: names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        if name == "requestDevice" { return DispatchQueue.main.async { self.choose(args, reply: reply) } }
        queue.async { reply(self.answer(name, args)) }
    }

    private func answer(_ name: String, _ args: [String: Any]) -> [String: Any] {
        let all = found()
        defer { for f in all { IOObjectRelease(f.service) } }
        let list = { (value: [Any]) in ["exact:answer": value] as [String: Any] }
        switch name {
        case "usbStatus": return ["available": true, "attachedList": true, "state": "granted"]
        case "attachedDevices": return list(all.map(\.record).filter { matches($0, args["filters"] as? [[String: Any]]) })
        case "getDevices":
            return list(remembered.map { id in all.first { $0.id == id }?.record ?? Self.gone(id) })
        case "open":
            let id = args["device"] as? String ?? ""
            guard let f = all.first(where: { $0.id == id }) else { return ["error": "NotFoundError"] }
            let shared: Shared
            if let s = opened[id] { shared = s } else {
                do { shared = Shared(try IOUSBHostDevice(__ioService: f.service, options: [], queue: queue, interestHandler: nil)) } catch { return ["error": "NetworkError", "message": "\(error)"] }
                opened[id] = shared
            }
            shared.leases += 1
            nextConnection += 1
            let handle = "usb-\(nextConnection)"
            connections[handle] = Connection(id: handle, device: id, shared: shared)
            remember(id)
            context.changed("usb.getDevices")
            return ["connection": handle, "device": f.record]
        case "usbConnection":
            guard let c = connections[args["connection"] as? String ?? ""] else { return ["open": false, "connected": false, "claimed": []] }
            return ["open": c.open, "connected": all.contains { $0.id == c.device }, "claimed": Array(c.claimed).sorted()]
        case "forget":
            let id = args["device"] as? String ?? ""
            remembered.removeAll { $0 == id }
            for c in connections.values where c.device == id && c.open { _ = answer("close", ["connection": c.id]) }
            context.changed("usb.getDevices")
            return [:]
        default: break
        }
        guard let c = connections[args["connection"] as? String ?? ""], c.open else { return ["error": "InvalidStateError"] }
        do {
            switch name {
            case "selectConfiguration":
                try c.shared.device.__configure(withValue: args["configurationValue"] as? Int ?? 1, matchInterfaces: true)
            case "claimInterface":
                let n = args["interfaceNumber"] as? Int ?? 0
                if let other = c.shared.claims[n], other != c.id { return ["error": "NetworkError", "message": "claimed by another connection"] }
                guard let f = all.first(where: { $0.id == c.device }) else { return ["error": "NetworkError"] }
                let classes = ((f.record["configurations"] as? [[String: Any]])?.first?["interfaces"] as? [[String: Any]] ?? []).first { $0["interfaceNumber"] as? Int == n }
                if ((classes?["alternates"] as? [[String: Any]]) ?? []).contains(where: { Self.protected.contains($0["interfaceClass"] as? Int ?? -1) }) {
                    return ["error": "SecurityError"]
                }
                if c.shared.interfaces[n] == nil { c.shared.interfaces[n] = try interface(f.service, n) }
                c.shared.claims[n] = c.id
                c.claimed.insert(n)
            case "releaseInterface":
                let n = args["interfaceNumber"] as? Int ?? 0
                guard c.claimed.remove(n) != nil else { return ["error": "InvalidStateError"] }
                c.shared.claims[n] = nil
                c.shared.interfaces.removeValue(forKey: n)?.destroy()
            case "selectAlternateInterface":
                let n = args["interfaceNumber"] as? Int ?? 0
                guard c.claimed.contains(n), let i = c.shared.interfaces[n] else { return ["error": "InvalidStateError"] }
                try i.selectAlternateSetting(args["alternateSetting"] as? Int ?? 0)
            case "controlTransferIn", "controlTransferOut":
                return try control(c, args, in: name == "controlTransferIn")
            case "reset":
                try c.shared.device.reset()
            case "clearHalt", "transferIn", "transferOut":
                // Bulk and interrupt pipes: not built yet on macOS (Bench's updater uses control transfers only).
                return ["error": "NotSupportedError", "message": "\(name) is not built on macOS yet"]
            case "close":
                c.open = false
                for n in c.claimed { c.shared.claims[n] = nil; c.shared.interfaces.removeValue(forKey: n)?.destroy() }
                c.claimed.removeAll()
                c.shared.leases -= 1
                if c.shared.leases == 0 { c.shared.device.destroy(); opened[c.device] = nil }
                context.changed("usb.usbConnection")
            default: return ["error": "NotSupportedError"]
            }
            return [:]
        } catch {
            return ["error": "NetworkError", "message": "\(error)"]
        }
    }

    /// Interface `n` of the device, opened for this process; a kernel driver's interface refuses (D14).
    private func interface(_ device: io_service_t, _ n: Int) throws -> IOUSBHostInterface {
        var iterator: io_iterator_t = 0
        guard IORegistryEntryCreateIterator(device, kIOServicePlane, IOOptionBits(kIORegistryIterateRecursively), &iterator) == KERN_SUCCESS else { throw NSError(domain: "usb", code: 1) }
        defer { IOObjectRelease(iterator) }
        while case let s = IOIteratorNext(iterator), s != 0 {
            defer { IOObjectRelease(s) }
            if IOObjectConformsTo(s, "IOUSBHostInterface") != 0, number(s, "bInterfaceNumber") == n {
                return try IOUSBHostInterface(__ioService: s, options: [], queue: queue, interestHandler: nil)
            }
        }
        throw NSError(domain: "usb", code: 2, userInfo: [NSLocalizedDescriptionKey: "no interface \(n)"])
    }

    private func control(_ c: Connection, _ args: [String: Any], in incoming: Bool) throws -> [String: Any] {
        let setup = args["setup"] as? [String: Any] ?? [:]
        let types = ["standard": 0, "class": 1, "vendor": 2], recipients = ["device": 0, "interface": 1, "endpoint": 2, "other": 3]
        let index = setup["index"] as? Int ?? 0
        if setup["recipient"] as? String == "interface", !c.claimed.contains(index & 0xFF) {
            return ["error": "InvalidStateError", "message": "the interface is not claimed"]
        }
        let requestType = (incoming ? 0x80 : 0) | ((types[setup["requestType"] as? String ?? ""] ?? 0) << 5) | (recipients[setup["recipient"] as? String ?? ""] ?? 0)
        let data = incoming ? NSMutableData(length: args["length"] as? Int ?? 0)! : NSMutableData(data: Data(base64Encoded: args["data"] as? String ?? "") ?? Data())
        var request = IOUSBDeviceRequest()
        request.bmRequestType = UInt8(requestType)
        request.bRequest = UInt8(setup["request"] as? Int ?? 0)
        request.wValue = UInt16(setup["value"] as? Int ?? 0)
        request.wIndex = UInt16(index)
        request.wLength = UInt16(data.length)
        var transferred = 0
        do {
            try c.shared.device.__send(request, data: data, bytesTransferred: &transferred, completionTimeout: 5)
        } catch let e as NSError where e.code == Self.pipeStalled {
            return incoming ? ["status": "stall", "data": ""] : ["status": "stall", "bytesWritten": 0]
        }
        return incoming ? ["status": "ok", "data": Data(data.prefix(transferred)).base64EncodedString()] : ["status": "ok", "bytesWritten": transferred]
    }

    /// The module's own sheet (D2): the matching devices by name; Cancel fails `NotFoundError`, as WebUSB's does.
    private func choose(_ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let all = found()
        for f in all { IOObjectRelease(f.service) }
        let matching = all.map(\.record).filter { matches($0, args["filters"] as? [[String: Any]]) }
        let alert = NSAlert()
        alert.messageText = "Choose a USB Device"
        alert.informativeText = matching.isEmpty ? "No matching device is connected." : "The app can talk to the device you choose."
        let menu = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 300, height: 26), pullsDown: false)
        menu.addItems(withTitles: matching.map { ($0["productName"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? ($0["id"] as? String ?? "") })
        if !matching.isEmpty { alert.accessoryView = menu }
        alert.addButton(withTitle: "Connect").isEnabled = !matching.isEmpty
        alert.addButton(withTitle: "Cancel")
        let done = { (response: NSApplication.ModalResponse) in
            guard response == .alertFirstButtonReturn, matching.indices.contains(menu.indexOfSelectedItem) else { return reply(["error": "NotFoundError"]) }
            let r = matching[menu.indexOfSelectedItem]
            self.remember(r["id"] as? String ?? "")
            self.context.changed("usb.getDevices")
            reply(r)
        }
        if let window = context.presentation?.window { alert.beginSheetModal(for: window, completionHandler: done) } else { done(alert.runModal()) }
    }

    public func destroy(_ done: @escaping () -> Void) {
        queue.async {
            for c in self.connections.values where c.open { _ = self.answer("close", ["connection": c.id]) }
            if let n = self.notify { IONotificationPortDestroy(n) }
            done()
        }
    }
}
