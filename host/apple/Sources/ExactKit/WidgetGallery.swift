// @ref LLP 1126 D14 — development previews and agent reads use the same
// headless entry and snapshot view. The running app keeps its presenter.
#if EXACT_WIDGET_GALLERY && (os(iOS) || os(macOS))
import Foundation
import SwiftUI
import WidgetKit
import ImageIO
import UniformTypeIdentifiers
import ExactCore
import ExactSnapshot
#if os(iOS)
import UIKit
#else
import AppKit
#endif

struct WidgetEntry {
    let kind: String
    let families: [String]
    let preview: [String: Any]
}

struct WidgetVariant: Hashable, Identifiable {
    var entry: String
    var family: String
    var mode = "fullColor"
    var scheme = "light"
    var size = "large"
    var id: Self { self }
    var typeSize: DynamicTypeSize { size == "xxxl" ? .xxxLarge : .large }
    var displaySize: CGSize {
        switch family {
        case "systemSmall": return CGSize(width: 158, height: 158)
        case "systemMedium": return CGSize(width: 338, height: 158)
        case "systemLarge": return CGSize(width: 338, height: 354)
        case "systemExtraLarge": return CGSize(width: 720, height: 354)
        case "accessoryCircular", "accessoryCorner": return CGSize(width: 58, height: 58)
        case "accessoryInline": return CGSize(width: 225, height: 26)
        default: return CGSize(width: 160, height: 72)
        }
    }
    var margin: CGFloat { family.hasPrefix("system") ? 16 : 0 }
    var contentSize: CGSize { CGSize(width: displaySize.width - margin * 2, height: displaySize.height - margin * 2) }
    var renderingMode: WidgetRenderingMode { mode == "accented" ? .accented : mode == "vibrant" ? .vibrant : .fullColor }
}

@MainActor
final class WidgetPreview: ObservableObject {
    let variant: WidgetVariant
    let session: HeadlessSession
    let environment: SnapshotEnvironment
    let props: [String: Any]
    let refresh: String
    let refreshReason: String?
    init(_ variant: WidgetVariant, entry: WidgetEntry, app: ExactApp = .shared) throws {
        self.variant = variant
        guard let plan = app.resolver.bytes("plans/\(entry.kind).plan") else {
            throw HeadlessSession.Failure(description: "missing widget plan: \(entry.kind); rebuild the app")
        }
        let date = Date()
        let facts = SessionFacts(displayScale: 1, preferences: variant.scheme == "dark" ? [.dark] : [],
                                 rootFontSize: Double(SnapshotDriver.rootFontSize(for: variant.typeSize)), date: date)
        let session = try HeadlessSession(plan: plan, entry: entry.kind, size: variant.contentSize, facts: facts, assets: app.resolver)
        self.session = session
        do {
            let result = try session.refreshEntry(at: date)
            refresh = result["entryRefresh"] as? String ?? "preview"
            refreshReason = result["reason"] as? String
            let first = (result["timeline"] as? [[String: Any]])?.first
            props = first?["props"] as? [String: Any] ?? entry.preview
            let entryDate = (result["entryDate"] as? Double).map { Date(timeIntervalSince1970: $0 / 1000) } ?? date
            environment = SnapshotEnvironment(family: SnapshotEnvironment.Family(rawValue: variant.family)!,
                linkBase: URL(string: ExactEnv.appMetadata["ExactOrigin"] as? String ?? ""), imageRoots: [app.assetRoot],
                contentMargins: EdgeInsets(top: variant.margin, leading: variant.margin, bottom: variant.margin, trailing: variant.margin),
                rootFontSize: SnapshotDriver.rootFontSize(for: variant.typeSize), entryDate: entryDate)
        } catch { session.close(); throw error }
    }
    deinit { session.close() }
}

/// WidgetKit's container is supplied by the system only in an extension.
/// These are the same family dimensions and mode stand-ins as WP4's baseline.
/// The mode is also passed to the actual driver; archive parity is a separate test.
@MainActor
struct WidgetPreviewView: View {
    @ObservedObject var preview: WidgetPreview
    var body: some View {
        let variant = preview.variant
        ZStack {
            if variant.mode == "fullColor" { Color.clear.background(.background) }
            else { Color(white: 0.35) }
            SnapshotDriver.view(session: preview.session, displaySize: variant.displaySize, environment: preview.environment)
        }
        .frame(width: variant.displaySize.width, height: variant.displaySize.height)
        .saturation(variant.mode == "fullColor" ? 1 : 0)
        .environment(\.colorScheme, variant.mode == "fullColor" ? (variant.scheme == "dark" ? .dark : .light) : .dark)
        .environment(\.dynamicTypeSize, variant.typeSize)
        .environment(\.widgetRenderingMode, variant.renderingMode)
        .clipShape(RoundedRectangle(cornerRadius: variant.margin > 0 ? 22 : 0))
    }
}

@MainActor
private struct WidgetGalleryCell: View {
    let variant: WidgetVariant
    let entry: WidgetEntry
    @State private var preview: WidgetPreview?
    @State private var failure: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\(variant.family) · \(variant.mode) · \(variant.scheme) · \(variant.size)").font(.caption)
            if let preview {
                WidgetPreviewView(preview: preview)
                Text(preview.refresh == "timeline" ? "First timeline entry" : "Preview · \(preview.refresh)").font(.caption).foregroundStyle(.secondary)
            } else if let failure { Text(failure).foregroundStyle(.red) }
            else { ProgressView() }
        }
        .task {
            guard preview == nil, failure == nil else { return }
            do { preview = try WidgetPreview(variant, entry: entry) }
            catch { failure = String(describing: error) }
        }
    }
}

@MainActor
private struct WidgetGalleryView: View {
    let entries: [WidgetEntry]
    let close: () -> Void
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Widgets").font(.headline)
                Spacer()
                Button("Done", action: close)
            }.padding()
            SwiftUI.ScrollView([.vertical, .horizontal]) {
                LazyVStack(alignment: .leading, spacing: 24) {
                    ForEach(entries, id: \.kind) { entry in
                        Text(entry.kind).font(.title2)
                        ForEach(entry.families, id: \.self) { family in
                            ForEach(["fullColor", "accented", "vibrant"], id: \.self) { mode in
                                ForEach(["light", "dark"], id: \.self) { scheme in
                                    ForEach(["large", "xxxl"], id: \.self) { size in
                                        WidgetGalleryCell(variant: WidgetVariant(entry: entry.kind, family: family, mode: mode, scheme: scheme, size: size), entry: entry)
                                    }
                                }
                            }
                        }
                    }
                }.padding()
            }
        }
    }
}

enum WidgetGallery {
    static var entries: [WidgetEntry] {
        guard let data = try? Data(contentsOf: ExactApp.shared.assetRoot.appendingPathComponent("widget-gallery.json")),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        return values.compactMap { value in
            guard let kind = value["kind"] as? String, let families = value["families"] as? [String],
                  let preview = value["preview"] as? [String: Any] else { return nil }
            return WidgetEntry(kind: kind, families: families, preview: preview)
        }
    }
    static var available: Bool { !entries.isEmpty }
    #if os(macOS)
    @MainActor private static var window: NSWindow?
    #endif
    static func show() {
        MainActor.assumeIsolated {
            #if os(iOS)
            guard let controller = DevMenu.controller else { return }
            let host = UIHostingController(rootView: WidgetGalleryView(entries: entries) { [weak controller] in controller?.dismiss(animated: true) })
            // The action sheet finishes its dismissal before this presentation.
            let present = { controller.present(host, animated: true) }
            if controller.presentedViewController != nil { controller.dismiss(animated: true, completion: present) }
            else { present() }
            #else
            if let window { window.makeKeyAndOrderFront(nil); return }
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 700), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
            window.title = "Widgets"; window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: WidgetGalleryView(entries: entries) { self.window?.close() })
            self.window = window
            window.center(); window.makeKeyAndOrderFront(nil)
            #endif
        }
    }

    static func answer(_ op: String, request: [String: Any], agent: Agent) -> [String: Any] {
        MainActor.assumeIsolated {
            do {
                guard let address = request["widget"] as? [String: String], let kind = address["entry"],
                      let entry = entries.first(where: { $0.kind == kind }) else {
                    throw HeadlessSession.Failure(description: "unknown widget; available: \(entries.map(\.kind).joined(separator: ", "))")
                }
                let variant = WidgetVariant(entry: kind, family: address["family"] ?? entry.families.first ?? "systemSmall",
                    mode: address["mode"] ?? "fullColor", scheme: address["scheme"] ?? "light", size: address["size"] ?? "large")
                guard entry.families.contains(variant.family), SnapshotEnvironment.Family(rawValue: variant.family) != nil,
                      ["fullColor", "accented", "vibrant"].contains(variant.mode), ["light", "dark"].contains(variant.scheme),
                      ["large", "xxxl"].contains(variant.size) else {
                    throw HeadlessSession.Failure(description: "invalid widget variant; families: \(entry.families.joined(separator: ", ")); modes: fullColor, accented, vibrant; schemes: light, dark; sizes: large, xxxl")
                }
                guard ["tree", "screenshot", "state", "tags", "logs"].contains(op) else {
                    throw HeadlessSession.Failure(description: "widget gallery supports tree, screenshot and state")
                }
                if agent.widgetPreview?.variant != variant { agent.widgetPreview = try WidgetPreview(variant, entry: entry, app: agent.session.app) }
                let preview = agent.widgetPreview!
                var result: [String: Any] = ["widget": ["entry": variant.entry, "family": variant.family, "mode": variant.mode, "scheme": variant.scheme, "size": variant.size], "clock": 0, "epoch": 1, "incarnation": 1]
                switch op {
                case "tree":
                    let snapshot = try preview.session.snapshot()
                    var nodes: [[String: Any]] = []
                    func visit(_ id: UInt32, parent: UInt32?, depth: Int) {
                        guard let node = snapshot.nodes[id] else { return }
                        nodes.append(["id": id, "parent": parent.map { $0 as Any } ?? NSNull(), "depth": depth, "type": node.kind,
                            "props": node.props, "handlers": [], "children": node.children,
                            "frame": ["x": node.frame.minX, "y": node.frame.minY, "w": node.frame.width, "h": node.frame.height],
                            "lines": node.lines.map { ["text": $0.text, "width": $0.width] as [String: Any] }])
                        for child in node.children { visit(child, parent: id, depth: depth + 1) }
                    }
                    for root in snapshot.roots { visit(root, parent: nil, depth: 0) }
                    result["roots"] = snapshot.roots; result["nodes"] = nodes
                case "state":
                    result["props"] = preview.props; result["slots"] = ["snapshot_props": preview.props]
                    result["refresh"] = preview.refresh; result["refreshReason"] = preview.refreshReason
                case "screenshot":
                    guard let path = request["path"] as? String else { throw HeadlessSession.Failure(description: "screenshot needs a path") }
                    let renderer = ImageRenderer(content: WidgetPreviewView(preview: preview))
                    renderer.scale = 1; renderer.proposedSize = ProposedViewSize(variant.displaySize)
                    guard let image = renderer.cgImage,
                          let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil) else {
                        throw HeadlessSession.Failure(description: "could not render widget screenshot")
                    }
                    CGImageDestinationAddImage(destination, image, nil)
                    guard CGImageDestinationFinalize(destination) else { throw HeadlessSession.Failure(description: "could not write widget screenshot") }
                    result["screenshot"] = path; result["w"] = variant.displaySize.width; result["h"] = variant.displaySize.height; result["scale"] = 1
                    result["rendering"] = "in-process WidgetKit container stand-in"
                case "logs": result["lines"] = []; result["next"] = 0
                default: break
                }
                return result
            } catch { return ["error": String(describing: error)] }
        }
    }
}
#endif
