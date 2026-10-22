// @ref LLP 1126.001 WP14 — activity addresses use the development gallery.
#if EXACT_WIDGET_GALLERY && (os(iOS) || os(macOS))
import SwiftUI
import ImageIO
import UniformTypeIdentifiers
import ExactCore
import ExactSnapshot

@MainActor
struct ActivityAddressView: View {
    let preview: ActivityPreview
    let placement: String
    let width: CGFloat
    @ViewBuilder var body: some View {
        if placement == "expanded" { ActivityGalleryView(preview: preview, width: width).expanded }
        else if placement == "lockScreen" { ActivityGalleryView(preview: preview, width: width).lockScreen }
        else if let slot = ActivityPlacement(name: placement) {
            ActivityGalleryView(preview: preview, width: width).compactPlacement(slot)
        }
    }
}

enum ActivityAgent {
    static func answer(_ op: String, request: [String: Any], agent: Agent) -> [String: Any] {
        MainActor.assumeIsolated {
            do {
                let entries = WidgetGallery.activities(in: agent.session.app.resolver)
                guard let address = request["activity"] as? [String: String], let kind = address["entry"],
                      let entry = entries.first(where: { $0.kind == kind }) else {
                    throw HeadlessSession.Failure(description: "unknown activity; available: \(entries.map(\.kind).joined(separator: ", "))")
                }
                let placement = address["placement"] ?? "lockScreen", scheme = address["scheme"] ?? "light"
                // The stand-in's pixels per point: 1 unless asked, a device's 2 or 3.
                let scale = CGFloat(Int(address["scale"] ?? "1") ?? 0)
                guard [1, 2, 3].contains(scale) else { throw HeadlessSession.Failure(description: "activity --scale: 1, 2 or 3") }
                guard ["lockScreen", "compactLeading", "compactTrailing", "minimal", "expanded"].contains(placement), ["light", "dark"].contains(scheme) else {
                    throw HeadlessSession.Failure(description: "invalid activity placement or scheme")
                }
                guard ["tree", "state", "screenshot"].contains(op) else {
                    throw HeadlessSession.Failure(description: "activity gallery supports tree, screenshot, state and app clock")
                }
                if request["availability"] as? Bool == true { return ["available": true] }
                let given = try address["state"].map { bytes -> [String: Any] in
                    guard let value = try JSONSerialization.jsonObject(with: Data(bytes.utf8)) as? [String: Any] else {
                        throw HeadlessSession.Failure(description: "activity --state: expected a JSON object")
                    }
                    return value
                }
                let old = agent.activityPreview
                func encoded(_ value: [String: Any]?) throws -> Data? {
                    try value.map { try JSONSerialization.data(withJSONObject: $0, options: [.sortedKeys]) }
                }
                let stateChanged = try encoded(old?.suppliedState) != encoded(given)
                if old?.entry.kind != kind || old?.entry.assets.publicationID != entry.assets.publicationID || old?.scheme != scheme
                    || old?.clock != agent.session.now() || old?.epochAtZero != agent.session.dateNow().epoch
                    || stateChanged {
                    agent.activityPreview = try ActivityPreview(entry, appSession: agent.session, scheme: scheme, state: given)
                }
                let preview = agent.activityPreview!
                // The gallery receives this host's current viewport, including on macOS.
                let width = max(1, agent.session.viewportSize.width - 32)
                let gallery = ActivityGalleryView(preview: preview, width: width)
                let expanded: [ActivityPlacement] = [.expandedLeading, .expandedTrailing, .expandedCenter, .expandedBottom]
                let slots = placement == "expanded" ? expanded : [ActivityPlacement(name: placement)!]
                let frames = placement == "expanded" ? gallery.expandedFrames().all : []
                var result: [String: Any] = ["activity": ["entry": kind, "placement": placement, "scheme": scheme]]
                switch op {
                case "state":
                    result["state"] = preview.state; result["slots"] = ["snapshot_state": preview.state]
                case "tree":
                    var nodes: [[String: Any]] = [], roots: [UInt32] = []
                    for (index, slot) in slots.enumerated() {
                        let slotWidth = placement == "expanded" ? frames[index].width : gallery.regionWidth(slot)
                        guard let laid = preview.render.laidOut(slot, proposal: ProposedViewSize(width: slotWidth, height: nil)) else { continue }
                        roots.append(laid.root.id)
                        func visit(_ id: UInt32, parent: UInt32?, depth: Int) {
                            guard let node = laid.snapshot.nodes[id] else { return }
                            nodes.append(["id": id, "parent": parent.map { $0 as Any } ?? NSNull(), "depth": depth, "type": node.kind,
                                          "placement": slot.name, "props": node.props, "handlers": [], "children": node.children,
                                          "frame": ["x": node.frame.minX, "y": node.frame.minY, "w": node.frame.width, "h": node.frame.height],
                                          "lines": node.lines.map { ["text": $0.text, "width": $0.width] as [String: Any] }])
                            for child in node.children { visit(child, parent: id, depth: depth + 1) }
                        }
                        visit(laid.root.id, parent: nil, depth: 0)
                    }
                    result["roots"] = roots; result["nodes"] = nodes
                case "screenshot":
                    guard let path = request["path"] as? String else { throw HeadlessSession.Failure(description: "screenshot needs a path") }
                    let renderer = ImageRenderer(content: ActivityAddressView(preview: preview, placement: placement, width: width))
                    renderer.scale = scale; renderer.proposedSize = ProposedViewSize(width: width, height: nil)
                    guard let image = renderer.cgImage, let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil) else {
                        throw HeadlessSession.Failure(description: "could not render activity screenshot")
                    }
                    CGImageDestinationAddImage(destination, image, nil)
                    guard CGImageDestinationFinalize(destination) else { throw HeadlessSession.Failure(description: "could not write activity screenshot") }
                    result["screenshot"] = path; result["w"] = image.width; result["h"] = image.height; result["scale"] = scale
                    result["rendering"] = "in-process ActivityKit region stand-in"
                default: break
                }
                for (name, value) in preview.render.session.agentFacts() where ["clock", "epoch", "incarnation"].contains(name) { result[name] = value }
                return result
            } catch { return ["error": String(describing: error)] }
        }
    }
}
#endif
