#if os(macOS)
import AppKit
import Foundation

/// Lane r7-device: the hub's read-only feeds the Tools drawer shows (MIT reference, see LICENSE-T3:
/// apps/web/src/components/device/deviceHubApi.ts, DeviceStreamView.tsx's accessibility overlay,
/// useDeviceControls.ts's foreground subscription, DeviceToolsPanel.tsx EventLogSection). Device
/// changes go through `device.action` in TypeScript; nothing here writes to the device.
struct R7AxElement: Equatable {
    let id: String, label: String, role: String
    /// Normalized to the displayed screen: 0…1 on both axes.
    let x: Double, y: Double, width: Double, height: Double
}

enum R7DeviceAx {
    static let limit = 500

    private static func number(_ value: Any?, _ fallback: Double) -> Double {
        guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID() else { return fallback }
        let double = value.doubleValue
        return double.isFinite ? double : fallback
    }

    /// flattenIosAxTree: serve-sim's nested tree, skipping nodes that cover the root's frame.
    static func flattenIos(_ roots: [Any]) -> [R7AxElement] {
        let rootFrame = (roots.first as? [String: Any])?["frame"] as? [String: Any]
        let screenWidth = max(1, number(rootFrame?["width"], 1)), screenHeight = max(1, number(rootFrame?["height"], 1))
        var elements: [R7AxElement] = []
        func visit(_ node: Any, _ path: String) {
            guard elements.count < limit, let node = node as? [String: Any], let frame = node["frame"] as? [String: Any] else { return }
            let width = number(frame["width"], 0), height = number(frame["height"], 0)
            let covers = abs(width - screenWidth) < 0.5 && abs(height - screenHeight) < 0.5
            if !covers, width > 0, height > 0 {
                elements.append(R7AxElement(id: node["AXUniqueId"] as? String ?? path, label: node["AXLabel"] as? String ?? "", role: node["type"] as? String ?? "",
                                            x: number(frame["x"], 0) / screenWidth, y: number(frame["y"], 0) / screenHeight,
                                            width: width / screenWidth, height: height / screenHeight))
            }
            for (index, child) in ((node["children"] as? [Any]) ?? []).enumerated() { visit(child, "\(path).\(index)") }
        }
        for (index, root) in roots.enumerated() { visit(root, String(index)) }
        return elements
    }

    /// Android's uiautomator nodes: pixel bounds, the first node is the window; only labelled or
    /// clickable nodes that do not fill the window are drawn.
    static func flattenAndroid(_ payload: [String: Any]) -> [R7AxElement]? {
        guard let raw = payload["nodes"] as? [Any] else { return nil }
        let nodes = raw.compactMap { $0 as? [String: Any] }.filter { $0["bounds"] is [String: Any] }
        let root = nodes.first?["bounds"] as? [String: Any]
        let screenWidth = max(1, number(root?["right"], 1)), screenHeight = max(1, number(root?["bottom"], 1))
        return nodes.dropFirst().compactMap { node in
            let bounds = node["bounds"] as! [String: Any]
            let left = number(bounds["left"], 0), top = number(bounds["top"], 0)
            let width = (number(bounds["right"], left) - left) / screenWidth, height = (number(bounds["bottom"], top) - top) / screenHeight
            let text = node["text"] as? String ?? "", description = node["contentDescription"] as? String ?? ""
            let label = text.isEmpty ? description : text
            if width >= 0.95 && height >= 0.9 { return nil }
            if label.isEmpty && node["clickable"] as? Bool != true { return nil }
            let className = node["className"] as? String ?? ""
            let id: String = { if let value = node["id"] { return value is NSNull ? "" : "\(value)" } else { return "" } }()
            return R7AxElement(id: id, label: label, role: className.split(separator: ".").last.map(String.init) ?? "",
                               x: left / screenWidth, y: top / screenHeight, width: width, height: height)
        }
    }

    /// fetchDeviceAxTree's parse; nil for an unexpected payload (the overlay keeps its last tree).
    static func parse(platform: String, _ payload: Any) -> [R7AxElement]? {
        if platform == "ios" { return (payload as? [Any]).map(flattenIos) }
        return (payload as? [String: Any]).flatMap(flattenAndroid)
    }
}

/// The accessibility frames over the flat picture: `border border-info/80 bg-info/10`, the label
/// above each box (`bg-info px-1 text-3xs leading-3.5 text-white`, truncated to the box width).
final class R7DeviceAxOverlay: CALayer {
    static let info = CGColor(srgbRed: 0x2b / 255, green: 0x7f / 255, blue: 0xff / 255, alpha: 1)
    private(set) var elements: [R7AxElement] = []
    var count: Int { elements.count }

    override init() { super.init(); isGeometryFlipped = false; masksToBounds = false }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    func set(_ next: [R7AxElement]) {
        guard next != elements else { return }
        elements = next
        layoutElements()
    }

    func layoutElements() {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        sublayers?.forEach { $0.removeFromSuperlayer() }
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        for element in elements {
            let box = CALayer()
            box.frame = CGRect(x: element.x * Double(bounds.width), y: element.y * Double(bounds.height), width: element.width * Double(bounds.width), height: element.height * Double(bounds.height))
            box.borderWidth = 1
            box.borderColor = Self.info.copy(alpha: 0.8)
            box.backgroundColor = Self.info.copy(alpha: 0.1)
            addSublayer(box)
            guard !element.label.isEmpty else { continue }
            // `-top-3.5 left-0 max-w-full truncate rounded-sm bg-info px-1 text-3xs leading-3.5`.
            let font = NSFont.systemFont(ofSize: 10)
            let text = NSAttributedString(string: element.label, attributes: [.font: font, .foregroundColor: NSColor.white])
            let width = min(ceil(text.size().width) + 8, max(0, box.frame.width))
            let tag = CALayer()
            tag.backgroundColor = Self.info
            tag.cornerRadius = 6
            tag.masksToBounds = true
            tag.frame = CGRect(x: box.frame.minX, y: box.frame.minY - 14, width: width, height: 14)
            let label = CATextLayer()
            label.string = text
            label.contentsScale = scale
            label.truncationMode = .end
            label.isWrapped = false
            label.frame = CGRect(x: 4, y: (14 - ceil(font.ascender - font.descender)) / 2, width: max(0, width - 8), height: ceil(font.ascender - font.descender))
            tag.addSublayer(label)
            addSublayer(tag)
        }
        CATransaction.commit()
    }
}

/// The foreground-app and event-log server-sent events (iOS only) and accessibility reads, keyed
/// by device. A feed runs while the panel shows its device (foreground) or the drawer's event log
/// is open; its state is reported as `presentation.deviceTools[key]`.
final class R7DeviceToolFeeds: NSObject, URLSessionDataDelegate {
    static let eventLimit = 100
    private let access: R7DeviceClient.Access
    private let changed: (String) -> Void
    private lazy var session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: .main)
    private struct Feed { let key: String; let kind: String; var task: URLSessionDataTask?; var buffer = Data(); var retry = 0 }
    private var feeds: [String: Feed] = [:] // "\(kind)\u{1}\(key)"
    private var eventHosts: [ObjectIdentifier: String] = [:]
    private var foreground: [String: Any] = [:] // key → bundle id or NSNull
    private var events: [String: [[String: Any]]] = [:]

    init(access: @escaping R7DeviceClient.Access, changed: @escaping (String) -> Void) { self.access = access; self.changed = changed }

    var status: [String: Any] {
        var out: [String: [String: Any]] = [:]
        for (key, value) in foreground { out[key, default: [:]]["foreground"] = value }
        for (key, value) in events { out[key, default: [:]]["events"] = value }
        for feed in feeds.values where feed.kind == "events" { out[feed.key, default: [:]]["eventsOpen"] = true }
        return out
    }

    // MARK: Subscriptions

    /// The panels showing a device: iOS ones subscribe to `/appstate` (subscribeDeviceForeground).
    func watchForeground(panels: [(key: String, platform: String, deviceId: String, hostId: String)]) {
        let wanted = Set(panels.filter { $0.platform == "ios" }.map(\.key))
        for (id, feed) in feeds where feed.kind == "foreground" && !wanted.contains(feed.key) { feed.task?.cancel(); feeds[id] = nil; foreground[feed.key] = nil }
        for panel in panels where panel.platform == "ios" && feeds["foreground\u{1}\(panel.key)"] == nil {
            open(kind: "foreground", key: panel.key, path: "/appstate", deviceId: panel.deviceId, hostId: panel.hostId, extra: [])
        }
    }

    /// EventLogSection while open: `/api/event-log/events?device=&limit=100` (iOS only).
    func watchEvents(host: ObjectIdentifier, key: String, deviceId: String, hostId: String) {
        guard eventHosts[host] != key else { return }
        unwatchEvents(host: host)
        eventHosts[host] = key
        guard feeds["events\u{1}\(key)"] == nil else { return }
        events[key] = []
        open(kind: "events", key: key, path: "/api/event-log/events", deviceId: deviceId, hostId: hostId, extra: [URLQueryItem(name: "limit", value: String(Self.eventLimit))])
        changed("t3.status")
    }

    func unwatchEvents(host: ObjectIdentifier) {
        guard let key = eventHosts.removeValue(forKey: host), !eventHosts.values.contains(key) else { return }
        feeds["events\u{1}\(key)"]?.task?.cancel(); feeds["events\u{1}\(key)"] = nil
        events[key] = nil
        changed("t3.status")
    }

    func destroy() { feeds.values.forEach { $0.task?.cancel() }; feeds = [:]; session.invalidateAndCancel() }

    private func open(kind: String, key: String, path: String, deviceId: String, hostId: String, extra: [URLQueryItem]) {
        let id = "\(kind)\u{1}\(key)"
        var feed = feeds[id] ?? Feed(key: key, kind: kind)
        feeds[id] = feed
        access { [weak self] origin, ticket in
            DispatchQueue.main.async {
                guard let self, self.feeds[id] != nil else { return }
                guard let origin, let ticket, let url = R6DeviceWire.url(origin: origin, path: "/vendor/serve-sim" + path, query: [URLQueryItem(name: "device", value: deviceId)] + extra, ticket: ticket, hostId: hostId) else { return self.retry(id, kind: kind, key: key, path: path, deviceId: deviceId, hostId: hostId, extra: extra) }
                var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 3600)
                request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                let task = self.session.dataTask(with: request)
                task.taskDescription = "\(id)\u{2}\(deviceId)\u{2}\(hostId)\u{2}\(path)\u{2}\(extra.map { "\($0.name)=\($0.value ?? "")" }.joined(separator: "&"))"
                feed = self.feeds[id] ?? feed
                feed.task = task; feed.buffer = Data()
                self.feeds[id] = feed
                task.resume()
            }
        }
    }

    /// EventSource reconnects on its own; so do these, after 3 s.
    private func retry(_ id: String, kind: String, key: String, path: String, deviceId: String, hostId: String, extra: [URLQueryItem]) {
        guard var feed = feeds[id] else { return }
        feed.task = nil; feed.retry += 1; feeds[id] = feed
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
            guard let self, self.feeds[id] != nil, self.feeds[id]?.task == nil else { return }
            self.open(kind: kind, key: key, path: path, deviceId: deviceId, hostId: hostId, extra: extra)
        }
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard let id = dataTask.taskDescription?.components(separatedBy: "\u{2}").first, var feed = feeds[id], feed.task === dataTask else { return }
        feed.buffer.append(data)
        var messages: [String] = []
        while let range = feed.buffer.range(of: Data("\n\n".utf8)) ?? feed.buffer.range(of: Data("\r\n\r\n".utf8)) {
            let block = String(decoding: feed.buffer[feed.buffer.startIndex..<range.lowerBound], as: UTF8.self)
            feed.buffer.removeSubrange(feed.buffer.startIndex..<range.upperBound)
            let lines = block.split(whereSeparator: \.isNewline).filter { $0.hasPrefix("data:") }.map { $0.dropFirst(5).drop(while: { $0 == " " }) }
            if !lines.isEmpty { messages.append(lines.joined(separator: "\n")) }
        }
        feed.retry = 0
        feeds[id] = feed
        for message in messages { receive(feed, message) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let parts = task.taskDescription?.components(separatedBy: "\u{2}"), parts.count == 5, let feed = feeds[parts[0]], feed.task === task else { return }
        let extra = parts[4].split(separator: "&").compactMap { pair -> URLQueryItem? in
            let bits = pair.split(separator: "=", maxSplits: 1).map(String.init)
            return bits.count == 2 ? URLQueryItem(name: bits[0], value: bits[1]) : nil
        }
        retry(parts[0], kind: feed.kind, key: feed.key, path: parts[3], deviceId: parts[1], hostId: parts[2], extra: extra)
    }

    private func receive(_ feed: Feed, _ text: String) {
        guard let data = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { return }
        if feed.kind == "foreground" {
            if data["bundleId"] is NSNull || data["bundleId"] as? String == "" { foreground[feed.key] = NSNull() }
            else if let id = data["bundleId"] as? String { foreground[feed.key] = id } else { return }
        } else {
            if let list = data["events"] as? [Any] { events[feed.key] = Array(list.compactMap(Self.entry).suffix(Self.eventLimit)) }
            else if let entry = Self.entry(data["event"] as Any) { events[feed.key] = Array(((events[feed.key] ?? []) + [entry]).suffix(Self.eventLimit)) }
            else { return }
        }
        changed("t3.status")
    }

    /// toEventLogEntry, with the drawer's HH:MM:SS (timestamp.slice(11, 19)).
    static func entry(_ raw: Any) -> [String: Any]? {
        guard let raw = raw as? [String: Any], let id = (raw["id"] as? NSNumber)?.doubleValue else { return nil }
        let timestamp = raw["timestamp"] as? String ?? ""
        let chars = Array(timestamp)
        let time = chars.count > 11 ? String(chars[11..<min(19, chars.count)]) : ""
        return ["id": id, "time": time, "kind": raw["kind"] as? String ?? "", "summary": raw["summary"] as? String ?? raw["msg"] as? String ?? ""]
    }

    // MARK: Accessibility

    /// One fetchDeviceAxTree read; nil on failure (the caller keeps its last tree).
    func readAx(platform: String, deviceId: String, hostId: String, _ done: @escaping ([R7AxElement]?) -> Void) {
        access { origin, ticket in
            let vendor = R6DeviceWire.vendor(platform)
            let path = platform == "ios" ? "\(vendor)/helper/\(deviceId.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? deviceId)/ax" : "\(vendor)/api/accessibility"
            let query = platform == "ios" ? [] : [URLQueryItem(name: "device", value: deviceId)]
            guard let origin, let ticket, let url = R6DeviceWire.url(origin: origin, path: path, query: query, ticket: ticket, hostId: hostId) else { return done(nil) }
            URLSession(configuration: .ephemeral).dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)) { data, response, _ in
                guard let data, ((response as? HTTPURLResponse)?.statusCode ?? 0) / 100 == 2, let payload = try? JSONSerialization.jsonObject(with: data) else { return done(nil) }
                done(R7DeviceAx.parse(platform: platform, payload))
            }.resume()
        }
    }
}
#endif
