// @ref LLP 1042 §9.1. The agent and Exact's picture-in-picture windows: each
// floating image's row in `tree` carries its window (its kind, title, frame on
// screen and controls), `tap <image> pictureinpicture <control>` presses one of
// that window's controls or moves the pointer over it (`hover`), and
// `screenshot <png> window pictureinpicture [<image>]` is the window server's
// picture of it.
#if os(macOS)
import AppKit

extension Agent {
    /// The images floating in this session in Exact's windows, each with its window.
    private func floatingWindows() -> [(node: NodeView, window: PictureInPictureWindow)] {
        PictureInPictureHost.floating.compactMap { floating in
            guard presenter.views[floating.node.id] === floating.node, let window = floating.host.window else { return nil }
            return (floating.node, window)
        }
    }

    /// `tree`: each floating image's row gains `pictureInPicture`.
    func decoratePictureInPicture(_ tree: [String: Any]) -> [String: Any] {
        let floating = floatingWindows()
        guard !floating.isEmpty, var nodes = tree["nodes"] as? [[String: Any]] else { return tree }
        for index in nodes.indices {
            guard let id = (nodes[index]["id"] as? NSNumber)?.uint32Value, let window = floating.first(where: { $0.node.id == id })?.window else { continue }
            nodes[index]["pictureInPicture"] = window.report
        }
        var out = tree
        out["nodes"] = nodes
        return out
    }

    /// The views that float, for a refusal that names them.
    private func floatingIds(_ floating: [(node: NodeView, window: PictureInPictureWindow)]) -> String {
        floating.map { "view \($0.node.id)" }.joined(separator: ", ")
    }

    /// `tap <image> pictureinpicture <control>`: the control's own action, as its button runs it.
    func pictureInPictureTap(_ req: [String: Any], control: String) -> [String: Any] {
        guard let v = view(req) else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let floating = floatingWindows()
        guard !floating.isEmpty else {
            return ["error": "pictureinpicture: no image floats in Exact's window (`requestPictureInPicture(id, presentation=\"window\")`)"]
        }
        guard let window = floating.first(where: { $0.node === v })?.window else {
            return ["error": "pictureinpicture: view \(v.id) is not a floating image; \(floatingIds(floating)) floats"]
        }
        if let refused = window.press(control) { return ["error": "pictureinpicture: \(refused)"] }
        presenter.settlePump()
        return ["tapped": Int(v.id), "pictureInPicture": control, "delivery": "activation"]
    }

    /// `screenshot <png> window pictureinpicture [<image>]`: the window server's picture of the
    /// window the image floats in; with one window floating, the image may go unsaid.
    func pictureInPictureShot(_ path: String, of id: UInt32?) -> [String: Any] {
        let floating = floatingWindows()
        guard !floating.isEmpty else { return ["error": "screenshot window pictureinpicture: no image floats in Exact's window"] }
        let chosen: PictureInPictureWindow
        if let id {
            guard let window = floating.first(where: { $0.node.id == id })?.window else {
                return ["error": "screenshot window pictureinpicture: view \(id) is not a floating image; \(floatingIds(floating)) floats"]
            }
            chosen = window
        } else {
            guard floating.count == 1 else {
                return ["error": "screenshot window pictureinpicture: \(floatingIds(floating)) float; name the image, as screenshot <png> window pictureinpicture <image>"]
            }
            chosen = floating[0].window
        }
        let panel = chosen.panel
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 2 / 60 + 0.004))
        guard let image = Self.ownWindowImage(panel.windowNumber) else { return ["error": "the window server gave no picture of window \(panel.windowNumber)"] }
        guard let png = NSBitmapImageRep(cgImage: image).converting(to: .sRGB, renderingIntent: .default)?.representation(using: .png, properties: [:]) else { return ["error": "no PNG"] }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        return ["screenshot": path, "window": "pictureinpicture", "w": Agent.r2(panel.frame.width), "h": Agent.r2(panel.frame.height), "scale": Agent.r2(panel.backingScaleFactor)]
    }
}
#endif
