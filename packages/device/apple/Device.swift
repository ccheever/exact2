// @exact/device on Apple (LLP 1133.031 D1–D5, D9): the battery from UIDevice
// on iOS and the IOKit power sources on macOS, the network from one
// `NWPathMonitor`, and the power state from `ProcessInfo`, in the
// interface's shapes. The app's composed module calls it outside the agent;
// under the agent the runner's substitute answers, unless the drive binds it
// (`--bind device`).
//
// - One observer per source per process (D5 item 1), kept by `DeviceHub`:
//   the first read in any session starts it, and it stops when no session
//   reads the source (a release, or the session's end).
// - A source announces its topic to each reading session only when an
//   answered field changes: `level` is rounded to hundredths first (D1), so
//   a finer change announces nothing.
// - The foreground reads every observed source again, since a suspended iOS
//   app receives none of these notifications (D5 item 4).
// - No battery to report (a Mac with no internal battery, an Apple TV, the
//   simulator's unknown state) answers the specification's full battery,
//   charging, with `apple.batteryState` `unknown` (D1).
// - `UIDevice.isBatteryMonitoringEnabled` is the process's: set at the first
//   read and at each foreground, never cleared (D1).
import ExactPackage
import Foundation
import Network
#if os(iOS)
import UIKit
#elseif os(macOS)
import IOKit.ps
#endif

/// The answers, from the platform's raw values. Pure, so the suite can feed
/// it recorded readings.
enum DeviceAnswers {
    /// A level in hundredths (D1's precision).
    static func hundredths(_ level: Double) -> Double { (level * 100).rounded() / 100 }

    /// The specification's no-battery values (D1).
    static let noBattery: [String: Any] = ["charging": true, "level": 1.0, "apple": ["batteryState": "unknown"]]

    /// `state` is `UIDevice.BatteryState`'s word; a level below zero is unknown.
    static func battery(level: Double, state: String) -> [String: Any] {
        guard state != "unknown", level >= 0 else { return noBattery }
        return ["charging": state != "unplugged", "level": hundredths(level), "apple": ["batteryState": state]]
    }

    /// An interface whose link the platform does not name: a VPN's or a
    /// tunnel's (`utun`, `ipsec`, `ppp`, …), which a path lists before the
    /// Wi-Fi or Ethernet it leaves by.
    static func tunnel(_ name: String) -> Bool {
        ["utun", "ipsec", "ppp", "tun", "tap", "gif", "stf"].contains { name.hasPrefix($0) }
    }

    /// `kinds` are the path's interfaces in its order, as (name, type word).
    static func network(satisfied: Bool, kinds: [(String, String)], constrained: Bool, expensive: Bool) -> [String: Any] {
        let type: String
        if !satisfied {
            type = "none"
        } else {
            type = kinds.first { !tunnel($0.0) }?.1 ?? "other"
        }
        return ["type": type, "saveData": constrained, "apple": ["expensive": expensive]]
    }

    static func thermal(_ state: ProcessInfo.ThermalState) -> String {
        switch state {
        case .nominal: return "nominal"
        case .fair: return "fair"
        case .serious: return "serious"
        case .critical: return "critical"
        @unknown default: return "critical"
        }
    }

    static func power(lowPower: Bool, thermal: String) -> [String: Any] {
        ["lowPowerMode": lowPower, "thermalState": thermal]
    }
}

/// The process's sources: who reads each, by session, and the last answer
/// each announced. Main thread.
final class DeviceHub {
    static let shared = DeviceHub(platform: DevicePlatform().platform)

    /// What observes the device; the suite gives a recorded one.
    struct Platform {
        /// Start observing `source`; `heard` is called on the main thread with each new answer.
        var start: (_ source: String, _ heard: @escaping ([String: Any]) -> Void) -> Void
        var stop: (_ source: String) -> Void
        /// The answer now, or nil before the platform's first (the network's path).
        var read: (_ source: String) -> [String: Any]?
        /// The process's foreground preparation (the battery monitoring flag).
        var foreground: () -> Void
    }

    private let platform: Platform
    private var readers: [String: [String: (String) -> Void]] = [:]
    private var last: [String: [String: Any]] = [:]
    /// Reads waiting for the platform's first answer.
    private var waiting: [String: [([String: Any]) -> Void]] = [:]

    init(platform: Platform) { self.platform = platform }

    /// `session` reads `source`; `changed` announces to that session.
    func read(_ source: String, session: String, changed: @escaping (String) -> Void, reply: @escaping ([String: Any]) -> Void) {
        if readers[source] == nil {
            readers[source] = [:]
            platform.start(source) { [weak self] answer in self?.heard(source, answer) }
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in self?.settle(source) }
        }
        readers[source]?[session] = changed
        if let answer = last[source] ?? platform.read(source) {
            last[source] = answer
            reply(answer)
        } else {
            waiting[source, default: []].append(reply)
        }
    }

    /// No reader of `source` is left in `session` (`nil`: any source).
    func leave(_ source: String?, session: String) {
        for name in source.map({ [$0] }) ?? Array(readers.keys) {
            guard readers[name]?.removeValue(forKey: session) != nil, readers[name]?.isEmpty == true else { continue }
            readers[name] = nil
            last[name] = nil
            platform.stop(name)
            for reply in waiting.removeValue(forKey: name) ?? [] { reply(["error": "AbortError"]) }
        }
    }

    /// Every observed source read again (D5 item 4).
    func foreground() {
        platform.foreground()
        for source in readers.keys { if let answer = platform.read(source) { heard(source, answer) } }
    }

    /// A new answer: replies that waited for one, and an announcement when it differs.
    func heard(_ source: String, _ answer: [String: Any]) {
        guard readers[source] != nil else { return }
        for reply in waiting.removeValue(forKey: source) ?? [] { reply(answer) }
        if let before = last[source], NSDictionary(dictionary: before).isEqual(to: answer) { return }
        let first = last[source] == nil
        last[source] = answer
        if !first { for changed in (readers[source] ?? [:]).values { changed("device.\(source)") } }
    }

    /// A read still waiting after a second answers what the platform has not yet said: `unknown` (D2).
    private func settle(_ source: String) {
        guard let replies = waiting.removeValue(forKey: source), !replies.isEmpty else { return }
        let answer: [String: Any] = source == "connection" ? ["type": "unknown", "saveData": false] : ["error": "NotReadableError"]
        for reply in replies { reply(answer) }
    }
}

/// The platform's observers. Main thread, except where a callback says.
final class DevicePlatform {
    private var monitor: NWPathMonitor?
    private var path: NWPath?
    private var observers: [String: [NSObjectProtocol]] = [:]
    #if os(macOS)
    private var powerSource: CFRunLoopSource?
    #endif

    var platform: DeviceHub.Platform {
        DeviceHub.Platform(start: start, stop: stop, read: read, foreground: Self.monitorBattery)
    }

    private static func monitorBattery() {
        #if os(iOS)
        UIDevice.current.isBatteryMonitoringEnabled = true
        #endif
    }

    private func start(_ source: String, _ heard: @escaping ([String: Any]) -> Void) {
        let center = NotificationCenter.default
        let again = { [weak self] (_: Notification) in
            DispatchQueue.main.async { if let answer = self?.read(source) { heard(answer) } }
        }
        switch source {
        case "getBattery":
            Self.monitorBattery()
            #if os(iOS)
            observers[source] = [UIDevice.batteryLevelDidChangeNotification, UIDevice.batteryStateDidChangeNotification]
                .map { center.addObserver(forName: $0, object: nil, queue: nil, using: again) }
            #elseif os(macOS)
            // The callback is a C function: it reaches the hub through the shared instance.
            DevicePlatform.batteryHeard = { heard($0) }
            if let source = IOPSNotificationCreateRunLoopSource({ _ in
                DispatchQueue.main.async { if let b = DevicePlatform.macBattery() { DevicePlatform.batteryHeard?(b) } }
            }, nil)?.takeRetainedValue() {
                CFRunLoopAddSource(CFRunLoopGetMain(), source, .defaultMode)
                powerSource = source
            }
            #endif
        case "connection":
            let monitor = NWPathMonitor()
            monitor.pathUpdateHandler = { [weak self] path in
                DispatchQueue.main.async {
                    guard let self, self.monitor === monitor else { return }
                    self.path = path
                    heard(Self.network(path))
                }
            }
            self.monitor = monitor
            monitor.start(queue: DispatchQueue(label: "exact.device.path"))
        case "powerState":
            // Apple: "to receive this notification, you must access the thermalState property prior to registering".
            _ = ProcessInfo.processInfo.thermalState
            var names = [ProcessInfo.thermalStateDidChangeNotification]
            if #available(macOS 12.0, *) { names.append(.NSProcessInfoPowerStateDidChange) }
            observers[source] = names.map { center.addObserver(forName: $0, object: nil, queue: nil, using: again) }
        default: break
        }
    }

    private func stop(_ source: String) {
        for observer in observers.removeValue(forKey: source) ?? [] { NotificationCenter.default.removeObserver(observer) }
        switch source {
        case "connection":
            monitor?.cancel()
            monitor = nil
            path = nil
        case "getBattery":
            #if os(macOS)
            if let source = powerSource { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .defaultMode) }
            powerSource = nil
            DevicePlatform.batteryHeard = nil
            #endif
        default: break
        }
    }

    private func read(_ source: String) -> [String: Any]? {
        switch source {
        case "getBattery":
            #if os(iOS)
            let device = UIDevice.current
            let state: String
            switch device.batteryState {
            case .unplugged: state = "unplugged"
            case .charging: state = "charging"
            case .full: state = "full"
            default: state = "unknown"
            }
            return DeviceAnswers.battery(level: Double(device.batteryLevel), state: state)
            #elseif os(macOS)
            return Self.macBattery() ?? DeviceAnswers.noBattery
            #else
            return DeviceAnswers.noBattery
            #endif
        case "connection":
            return path.map(Self.network)
        case "powerState":
            let info = ProcessInfo.processInfo
            var low = false
            if #available(macOS 12.0, *) { low = info.isLowPowerModeEnabled }
            return DeviceAnswers.power(lowPower: low, thermal: DeviceAnswers.thermal(info.thermalState))
        default:
            return nil
        }
    }

    private static func network(_ path: NWPath) -> [String: Any] {
        let kinds = path.availableInterfaces.map { i -> (String, String) in
            switch i.type {
            case .wifi: return (i.name, "wifi")
            case .cellular: return (i.name, "cellular")
            case .wiredEthernet: return (i.name, "ethernet")
            default: return (i.name, "other")
            }
        }
        return DeviceAnswers.network(satisfied: path.status == .satisfied, kinds: kinds, constrained: path.isConstrained, expensive: path.isExpensive)
    }

    #if os(macOS)
    static var batteryHeard: (([String: Any]) -> Void)?

    /// The internal battery's answer, or nil when the Mac has none.
    static func macBattery() -> [String: Any]? {
        guard let info = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let list = IOPSCopyPowerSourcesList(info)?.takeRetainedValue() as? [CFTypeRef] else { return nil }
        for item in list {
            guard let d = IOPSGetPowerSourceDescription(info, item)?.takeUnretainedValue() as? [String: Any],
                  d[kIOPSTypeKey] as? String == kIOPSInternalBatteryType,
                  let current = d[kIOPSCurrentCapacityKey] as? Double, let most = d[kIOPSMaxCapacityKey] as? Double, most > 0 else { continue }
            let onAC = d[kIOPSPowerSourceStateKey] as? String == kIOPSACPowerValue
            let state = !onAC ? "unplugged" : d[kIOPSIsChargedKey] as? Bool == true ? "full" : "charging"
            return DeviceAnswers.battery(level: current / most, state: state)
        }
        return nil
    }
    #endif
}

public final class DeviceModule: ExactPackage {
    private let context: ExactPackageContext
    private let hub: DeviceHub

    public init(context: ExactPackageContext) {
        self.context = context
        hub = DeviceHub.shared
    }

    /// The suite's: an instance over a recorded hub.
    init(context: ExactPackageContext, hub: DeviceHub) {
        self.context = context
        self.hub = hub
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "getBattery", "connection", "powerState":
                hub.read(name, session: context.session, changed: context.changed, reply: reply)
            default:
                reply(["error": "NotSupportedError"])
            }
        }
    }

    public func release(_ name: String) { hub.leave(name, session: context.session) }

    public func foreground(_ shown: Bool) { if shown { hub.foreground() } }

    public func destroy(_ done: @escaping () -> Void) {
        hub.leave(nil, session: context.session)
        done()
    }
}
