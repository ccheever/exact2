#if os(macOS)
import AppKit

/// App-owned image lifecycle for ToolActivityIconView (T3 Code 1e2ecbd975,
/// MIT, LICENSE-T3). Contract keeps the fallback glyph in this hook's host;
/// Contract toggles its fallback through the hook button's documented click API.
/// The hook draws only its own subview, including transparent logos.
final class T3ToolActivityIcon {
    private var entries: [ObjectIdentifier: IconView] = [:]

    func install(_ element: ExactElement) {
        guard element.hatch == .t3ToolIcon, let host = element.view else { return }
        let key = ObjectIdentifier(host)
        let view = entries[key] ?? IconView(frame: host.bounds)
        entries[key] = view
        view.autoresizingMask = [.width, .height]
        if view.superview !== host { host.addSubview(view) }
        view.changed = { [weak element] in element?.click() }
        view.update(light: element.data[.toolIconLight] ?? "", dark: element.data[.toolIconDark] ?? "")
    }

    func remove(_ element: ExactElement) {
        guard element.hatch == .t3ToolIcon, let host = element.view else { return }
        entries.removeValue(forKey: ObjectIdentifier(host))?.detach()
    }

    func destroy() {
        for view in entries.values { view.detach() }
        entries.removeAll()
    }

    private final class IconView: NSView {
        private static let cache = NSCache<NSString, NSImage>()
        private var light = "", dark = "", source = ""
        private var image: NSImage?
        private var task: URLSessionDataTask?
        var changed: (() -> Void)?
        private var reported = false
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
        override func viewDidChangeEffectiveAppearance() { super.viewDidChangeEffectiveAppearance(); resolve() }
        func update(light: String, dark: String) { self.light = light; self.dark = dark; resolve() }
        func updateFallback() {
            let loaded = image != nil
            guard loaded != reported else { return }
            reported = loaded
            // A click toggles Contract state; dispatch after the current element batch.
            let notify = changed
            DispatchQueue.main.async { notify?() }
        }
        func detach() {
            task?.cancel(); changed = nil; removeFromSuperview()
        }
        private func resolve() {
            let darkMode = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            let next = darkMode ? dark : light
            guard next != source else { return }
            task?.cancel(); task = nil; source = next
            image = Self.cache.object(forKey: next as NSString)
            updateFallback(); needsDisplay = true
            guard image == nil, let url = URL(string: next), ["http", "https", "data"].contains(url.scheme ?? "") else { return }
            if url.scheme == "data" {
                if let bytes = Data(contentsOfDataURL: next) { receive(bytes, source: next) }
                return
            }
            var request = URLRequest(url: url)
            request.timeoutInterval = 15
            // Match the reference's no-referrer image requests.
            request.setValue(nil, forHTTPHeaderField: "Referer")
            task = URLSession.shared.dataTask(with: request) { [weak self] data, response, _ in
                guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode), let data else { return }
                DispatchQueue.main.async { self?.receive(data, source: next) }
            }
            task?.resume()
        }
        private func receive(_ data: Data, source: String) {
            guard self.source == source, data.count <= 5 * 1024 * 1024, let loaded = NSImage(data: data), loaded.isValid else { return }
            Self.cache.countLimit = 128
            Self.cache.setObject(loaded, forKey: source as NSString)
            image = loaded; updateFallback(); needsDisplay = true
        }
        override func draw(_ dirtyRect: NSRect) {
            guard let image, image.size.width > 0, image.size.height > 0 else { return }
            let scale = min(bounds.width / image.size.width, bounds.height / image.size.height)
            let size = NSSize(width: image.size.width * scale, height: image.size.height * scale)
            image.draw(in: NSRect(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2, width: size.width, height: size.height))
        }
    }
}

private extension Data {
    init?(contentsOfDataURL raw: String) {
        guard let comma = raw.firstIndex(of: ","), raw.count <= 7 * 1024 * 1024 else { return nil }
        let header = raw[..<comma], body = String(raw[raw.index(after: comma)...])
        if header.hasSuffix(";base64") { self.init(base64Encoded: body) }
        else if let decoded = body.removingPercentEncoding { self = Data(decoded.utf8) }
        else { return nil }
    }
}
#endif
