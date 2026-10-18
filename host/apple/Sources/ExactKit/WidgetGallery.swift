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
    let assets: AssetResolver
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
    let clock: Double
    let epochAtZero: Double
    let suppliedProps: String?
    let generation: String
    init(_ variant: WidgetVariant, entry: WidgetEntry, appSession: ExactSession, props suppliedProps: String? = nil) throws {
        self.variant = variant; self.suppliedProps = suppliedProps
        generation = entry.assets.publicationID
        clock = appSession.now(); epochAtZero = appSession.dateNow().epoch
        guard let family = SnapshotEnvironment.Family(rawValue: variant.family) else {
            throw HeadlessSession.Failure(description: "invalid widget family: \(variant.family)")
        }
        guard let plan = entry.assets.bytes("plans/\(entry.kind).plan") else {
            throw HeadlessSession.Failure(description: "missing widget plan: \(entry.kind); rebuild the app")
        }
        let place = appSession.launchPlace
        let date = Date(timeIntervalSince1970: (epochAtZero + clock) / 1000)
        let facts = SessionFacts(displayScale: 1, preferences: variant.scheme == "dark" ? [.dark] : [],
            rootFontSize: Double(SnapshotDriver.bodySize(for: variant.typeSize)),
            date: date, clock: clock,
            timeZone: TimeZone(identifier: place.timeZone) ?? TimeZone(secondsFromGMT: 0)!, locale: place.locale, seed: place.seed)
        var result: [String: Any] = [:]
        if suppliedProps == nil {
            let initial = try HeadlessSession(plan: plan, entry: entry.kind, size: variant.contentSize, facts: facts, assets: entry.assets)
            defer { initial.close() }
            result = try initial.previewTimeline(at: date)
        }
        let first = (result["timeline"] as? [[String: Any]])?.first
        var given: [String: Any]?
        if let suppliedProps {
            guard let object = try JSONSerialization.jsonObject(with: Data(suppliedProps.utf8)) as? [String: Any] else {
                throw HeadlessSession.Failure(description: "widget --props: expected a JSON object")
            }
            given = object
        }
        props = given ?? (first?["props"] as? [String: Any]) ?? entry.preview
        refresh = suppliedProps != nil ? "props" : first != nil ? "timeline" : result["entryRefresh"] as? String == "timeline" ? "empty" : result["entryRefresh"] as? String ?? "preview"
        refreshReason = result["reason"] as? String
        let bytes = try JSONSerialization.data(withJSONObject: props, options: [.sortedKeys])
        let rendered = try HeadlessSession(plan: plan, entry: entry.kind, props: bytes, size: variant.contentSize, facts: facts, assets: entry.assets)
        do { try rendered.advance(to: clock) } catch { rendered.close(); throw error }
        session = rendered
        let entryDate = (first?["date"] as? Double).map { Date(timeIntervalSince1970: $0 / 1000) } ?? date
        environment = SnapshotEnvironment(family: family,
            linkBase: URL(string: ExactEnv.appMetadata["ExactOrigin"] as? String ?? ""), imageRoots: [entry.assets.root],
            contentMargins: EdgeInsets(top: variant.margin, leading: variant.margin, bottom: variant.margin, trailing: variant.margin),
            rootFontSize: SnapshotDriver.bodySize(for: variant.typeSize), entryDate: entryDate,
            frozenDate: appSession.clock == nil ? nil : date)
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
            if variant.mode == "fullColor" {
                if variant.margin > 0 { RoundedRectangle(cornerRadius: 22, style: .continuous).fill(.background) }
                else { Rectangle().fill(.background) }
            }
            else { Color(white: 0.35) }
            SnapshotDriver.view(session: preview.session, displaySize: variant.displaySize, environment: preview.environment)
        }
        .frame(width: variant.displaySize.width, height: variant.displaySize.height)
        .saturation(variant.mode == "fullColor" ? 1 : 0)
        .environment(\.colorScheme, variant.mode == "fullColor" ? (variant.scheme == "dark" ? .dark : .light) : .dark)
        .environment(\.dynamicTypeSize, variant.typeSize)
        .environment(\.widgetRenderingMode, variant.renderingMode)
    }
}

@MainActor
private struct WidgetGalleryCell: View {
    let variant: WidgetVariant
    let entry: WidgetEntry
    let appSession: ExactSession
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
            do { preview = try WidgetPreview(variant, entry: entry, appSession: appSession) }
            catch { failure = String(describing: error) }
        }
    }
}

/// A Live Activity entry (LLP 1126.001 WP10): its kind and preview state,
/// from the bake's `plans/activities.json`.
struct ActivityEntry {
    let kind: String
    let preview: [String: Any]
    let assets: AssetResolver
}

/// One activity's render for the gallery: the entry's plan booted with
/// its preview state (or a state the agent supplies), at the app's clock
/// and place, in a scheme; the eight placements are laid out from it as
/// the system lays them out, at the widths the gallery proposes.
@MainActor
final class ActivityPreview {
    let entry: ActivityEntry
    let render: ActivityRender
    let state: [String: Any]
    let scheme: String
    let environment: (ActivityPlacement) -> SnapshotEnvironment
    init(_ entry: ActivityEntry, appSession: ExactSession, scheme: String = "light", state supplied: [String: Any]? = nil) throws {
        self.entry = entry; self.scheme = scheme
        guard let plan = entry.assets.bytes("plans/\(entry.kind).plan") else {
            throw HeadlessSession.Failure(description: "missing activity plan: \(entry.kind); rebuild the app")
        }
        let place = appSession.launchPlace, clock = appSession.now()
        let date = Date(timeIntervalSince1970: (appSession.dateNow().epoch + clock) / 1000)
        let facts = SessionFacts(displayScale: 1, preferences: scheme == "dark" ? [.dark] : [], rootFontSize: Double(SnapshotDriver.bodySize(for: .large)),
            date: date, clock: clock, timeZone: TimeZone(identifier: place.timeZone) ?? TimeZone(secondsFromGMT: 0)!, locale: place.locale, seed: place.seed)
        state = supplied ?? entry.preview
        render = try ActivityRender(plan: plan, kind: entry.kind, state: try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]), facts: facts, assets: entry.assets)
        let linkBase = URL(string: ExactEnv.appMetadata["ExactOrigin"] as? String ?? ""), roots = [entry.assets.root]
        let frozen: Date? = appSession.clock == nil ? nil : date
        environment = { placement in
            SnapshotEnvironment(family: .liveActivity, placement: placement, linkBase: linkBase, imageRoots: roots,
                                rootFontSize: SnapshotDriver.bodySize(for: .large), entryDate: date, frozenDate: frozen)
        }
    }
    func view(_ placement: ActivityPlacement) -> some View {
        SnapshotActivity.view(render, placement: placement, environment: environment(placement))
    }
}

/// The eight placements as the system presents them, in the gallery's own
/// width: the Lock Screen card at the width the gallery has, the expanded
/// Island as its regions (leading, centre and trailing on one row, the
/// bottom under them) in a container of that width, the compact Island as
/// its two regions around the sensor's obstruction, the minimal view in its
/// circle. The regions' sizes are what the driver answers to the stand-ins'
/// proposals, as they are in the extension; no region width is a number
/// the gallery holds.
@MainActor
struct ActivityGalleryView: View {
    let preview: ActivityPreview
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Lock Screen · \(preview.scheme)").font(.caption).foregroundStyle(.secondary)
            // The tint stands in for the system's `activityBackgroundTint` (iOS only) over its material.
            preview.view(.lockScreen)
                .background(SnapshotActivity.tint(preview.render, dark: preview.scheme == "dark") ?? Color.black.opacity(0.85))
                .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
                .environment(\.colorScheme, preview.scheme == "dark" ? .dark : .light)
            Text("Dynamic Island · expanded").font(.caption).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .top, spacing: 0) {
                    preview.view(.expandedLeading)
                    Spacer(minLength: 8)
                    preview.view(.expandedCenter)
                    Spacer(minLength: 8)
                    preview.view(.expandedTrailing)
                }
                preview.view(.expandedBottom)
            }
            .padding(EdgeInsets(top: 18, leading: 18, bottom: 18, trailing: 18))
            .background(Color.black)
            .clipShape(RoundedRectangle(cornerRadius: 44, style: .continuous))
            .environment(\.colorScheme, .dark)
            Text("Dynamic Island · compact and minimal").font(.caption).foregroundStyle(.secondary)
            HStack(spacing: 16) {
                HStack(spacing: 0) {
                    preview.view(.compactLeading).padding(EdgeInsets(top: 7, leading: 7, bottom: 7, trailing: 0))
                    Color.clear.frame(width: 125, height: 37)
                    preview.view(.compactTrailing).padding(EdgeInsets(top: 7, leading: 0, bottom: 7, trailing: 7))
                }
                .background(Color.black).clipShape(Capsule())
                preview.view(.minimal)
                    .padding(7).frame(minWidth: 37, minHeight: 37)
                    .background(Color.black).clipShape(Capsule())
            }
            .environment(\.colorScheme, .dark)
        }
    }
}

@MainActor
private struct ActivityGalleryCell: View {
    let entry: ActivityEntry
    let scheme: String
    let appSession: ExactSession
    @State private var preview: ActivityPreview?
    @State private var failure: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let preview { ActivityGalleryView(preview: preview) }
            else if let failure { Text(failure).foregroundStyle(.red) }
            else { ProgressView() }
        }
        .task {
            guard preview == nil, failure == nil else { return }
            do { preview = try ActivityPreview(entry, appSession: appSession, scheme: scheme) }
            catch { failure = String(describing: error) }
        }
    }
}

@MainActor
private struct WidgetGalleryView: View {
    let appSession: ExactSession
    @State private var revision = 0
    private var entries: [WidgetEntry] { WidgetGallery.entries(in: appSession.app.resolver) }
    private var activities: [ActivityEntry] { WidgetGallery.activities(in: appSession.app.resolver) }
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
                                        WidgetGalleryCell(variant: WidgetVariant(entry: entry.kind, family: family, mode: mode, scheme: scheme, size: size), entry: entry, appSession: appSession)
                                    }
                                }
                            }
                        }
                    }
                    ForEach(activities, id: \.kind) { entry in
                        Text("\(entry.kind) (Live Activity)").font(.title2)
                        ForEach(["light", "dark"], id: \.self) { scheme in
                            ActivityGalleryCell(entry: entry, scheme: scheme, appSession: appSession)
                        }
                    }
                }.padding().id(appSession.app.resolver.publicationID + String(revision))
            }
        }.onReceive(NotificationCenter.default.publisher(for: Notification.Name("ExactWidgetGeneration"))) { _ in revision += 1 }
    }
}

enum WidgetGallery {
    static var entries: [WidgetEntry] { entries(in: ExactApp.shared.resolver) }
    static func entries(in assets: AssetResolver) -> [WidgetEntry] {
        guard let data = assets.bytes("plans/widgets.json"),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        return values.compactMap { value in
            guard let kind = value["kind"] as? String, let families = value["families"] as? [String],
                  let preview = value["preview"] as? [String: Any] else { return nil }
            return WidgetEntry(kind: kind, families: families, preview: preview, assets: assets)
        }
    }
    static func activities(in assets: AssetResolver) -> [ActivityEntry] {
        guard let data = assets.bytes("plans/activities.json"),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        return values.compactMap { value in
            guard let kind = value["kind"] as? String, let preview = value["preview"] as? [String: Any] else { return nil }
            return ActivityEntry(kind: kind, preview: preview, assets: assets)
        }
    }
    static var available: Bool { !entries.isEmpty || !activities(in: ExactApp.shared.resolver).isEmpty }
    #if os(macOS)
    @MainActor private static var window: NSWindow?
    #endif
    static func show() {
        MainActor.assumeIsolated {
            guard let appSession = DevMenu.session else { return }
            #if os(iOS)
            guard let controller = DevMenu.controller else { return }
            let host = UIHostingController(rootView: WidgetGalleryView(appSession: appSession) { [weak controller] in controller?.dismiss(animated: true) })
            // The action sheet finishes its dismissal before this presentation.
            let present = { controller.present(host, animated: true) }
            if controller.presentedViewController != nil { controller.dismiss(animated: true, completion: present) }
            else { present() }
            #else
            window?.close()
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 700), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
            window.title = "Widgets"; window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: WidgetGalleryView(appSession: appSession) { self.window?.close() })
            self.window = window
            window.center(); window.makeKeyAndOrderFront(nil)
            #endif
        }
    }

    static func answer(_ op: String, request: [String: Any], agent: Agent) -> [String: Any] {
        MainActor.assumeIsolated {
            do {
                let entries = entries(in: agent.session.app.resolver)
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
                guard ["tree", "screenshot", "state"].contains(op) else {
                    throw HeadlessSession.Failure(description: "widget gallery supports tree, screenshot, state and app clock")
                }
                if request["availability"] as? Bool == true { return ["available": true] }
                let old = agent.widgetPreview
                if old?.variant != variant || old?.clock != agent.session.now() || old?.epochAtZero != agent.session.dateNow().epoch
                    || old?.generation != entry.assets.publicationID || old?.suppliedProps != address["props"] {
                    agent.widgetPreview = try WidgetPreview(variant, entry: entry, appSession: agent.session, props: address["props"])
                }
                let preview = agent.widgetPreview!
                var result: [String: Any] = ["widget": ["entry": variant.entry, "family": variant.family, "mode": variant.mode, "scheme": variant.scheme, "size": variant.size]]
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
                default: break
                }
                let facts = preview.session.agentFacts()
                for name in ["clock", "epoch", "incarnation"] { result[name] = facts[name] }
                return result
            } catch { return ["error": String(describing: error)] }
        }
    }
}
#endif
