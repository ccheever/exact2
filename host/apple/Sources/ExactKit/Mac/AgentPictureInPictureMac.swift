// @ref LLP 1042 §9.1. The agent and Exact's picture-in-picture window: the
// floating image's row in `tree` carries the window (its kind, title, frame on
// screen and controls), `tap <image> pictureinpicture <control>` presses one of
// its controls or moves the pointer over it (`hover`), and `screenshot <png> window pictureinpicture` is the window
// server's picture of it.
#if os(macOS)
import AppKit

extension Agent {
    /// The window of the image floating in this session, if it is Exact's.
    private func floatingWindow() -> (node: NodeView, window: PictureInPictureWindow)? {
        guard let floating = PictureInPictureHost.floating, presenter.views[floating.node.id] === floating.node,
              let window = floating.host.window else { return nil }
        return (floating.node, window)
    }

    /// `tree`: the floating image's row gains `pictureInPicture`.
    func decoratePictureInPicture(_ tree: [String: Any]) -> [String: Any] {
        guard let floating = floatingWindow(), var nodes = tree["nodes"] as? [[String: Any]] else { return tree }
        for index in nodes.indices where (nodes[index]["id"] as? NSNumber)?.uint32Value == floating.node.id {
            nodes[index]["pictureInPicture"] = floating.window.report
        }
        var out = tree
        out["nodes"] = nodes
        return out
    }

    /// `tap <image> pictureinpicture <control>`: the control's own action, as its button runs it.
    func pictureInPictureTap(_ req: [String: Any], control: String) -> [String: Any] {
        guard let v = view(req) else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard let floating = floatingWindow() else {
            return ["error": "pictureinpicture: no image floats in Exact's window (`requestPictureInPicture(id, presentation=\"window\")`)"]
        }
        guard floating.node === v else { return ["error": "pictureinpicture: view \(v.id) is not the floating image; view \(floating.node.id) is"] }
        if let refused = floating.window.press(control) { return ["error": "pictureinpicture: \(refused)"] }
        presenter.settlePump()
        return ["tapped": Int(v.id), "pictureInPicture": control, "delivery": "activation"]
    }

    /// `screenshot <png> window pictureinpicture`: the window server's picture of the window.
    func pictureInPictureShot(_ path: String) -> [String: Any] {
        guard let floating = floatingWindow() else { return ["error": "screenshot window pictureinpicture: no image floats in Exact's window"] }
        let panel = floating.window.panel
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 2 / 60 + 0.004))
        guard let image = Self.ownWindowImage(panel.windowNumber) else { return ["error": "the window server gave no picture of window \(panel.windowNumber)"] }
        guard let png = NSBitmapImageRep(cgImage: image).converting(to: .sRGB, renderingIntent: .default)?.representation(using: .png, properties: [:]) else { return ["error": "no PNG"] }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        return ["screenshot": path, "window": "pictureinpicture", "w": Agent.r2(panel.frame.width), "h": Agent.r2(panel.frame.height), "scale": Agent.r2(panel.backingScaleFactor)]
    }
}
#endif
