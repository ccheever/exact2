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
    /// Pixels per point of the rendered picture: 1 unless asked, a device's 2 or 3.
    var scale: CGFloat = 1
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
        let facts = SessionFacts(displayScale: Double(variant.scale), preferences: variant.scheme == "dark" ? [.dark] : [],
            rootFontSize: Double(SnapshotDriver.bodySize(for: variant.typeSize)),
            date: date, clock: clock,
            timeZone: TimeZone(identifier: place.timeZone) ?? TimeZone(secondsFromGMT: 0)!, locale: place.locale, formatLocale: place.formatLocale, seed: place.seed)
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
        environment = SnapshotEnvironment(family: family, entryName: entry.kind,
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
    let clock: Double
    let epochAtZero: Double
    let suppliedState: [String: Any]?
    let environment: (ActivityPlacement) -> SnapshotEnvironment
    init(_ entry: ActivityEntry, appSession: ExactSession, scheme: String = "light", state supplied: [String: Any]? = nil) throws {
        self.entry = entry; self.scheme = scheme
        clock = appSession.now(); epochAtZero = appSession.dateNow().epoch; suppliedState = supplied
        guard let plan = entry.assets.bytes("plans/\(entry.kind).plan") else {
            throw HeadlessSession.Failure(description: "missing activity plan: \(entry.kind); rebuild the app")
        }
        let place = appSession.launchPlace
        let date = Date(timeIntervalSince1970: (epochAtZero + clock) / 1000)
        let facts = SessionFacts(displayScale: 1, preferences: scheme == "dark" ? [.dark] : [], rootFontSize: Double(SnapshotDriver.bodySize(for: .large)),
            date: date, clock: clock, timeZone: TimeZone(identifier: place.timeZone) ?? TimeZone(secondsFromGMT: 0)!, locale: place.locale, formatLocale: place.formatLocale, seed: place.seed)
        state = supplied ?? entry.preview
        render = try ActivityRender(plan: plan, kind: entry.kind, state: try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]), facts: facts, assets: entry.assets)
        _ = try render.session.advance(to: clock)
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

/// The Dynamic Island's geometry the gallery stands in for: the camera's
/// obstruction, the compact regions' insets and the expanded presentation's
/// insets. No API gives them in-process and the extension never reads them,
/// so they are one device's measurements: the iPhone 17 Pro's, from
/// chronod's metrics (`obstructionSize (125.33, 36.67)`, compact `edgeInsets
/// 7`) and the expanded container's accessibility frames (its content at
/// 17.7 pt from the sides and 18 pt from the top of a 374 pt container).
struct IslandGeometry {
    let obstruction: CGSize
    let compactInset: CGFloat
    let expandedInsets: EdgeInsets
    static let iPhone17Pro = IslandGeometry(obstruction: CGSize(width: 125.33, height: 36.67), compactInset: 7,
                                            expandedInsets: EdgeInsets(top: 18, leading: 17.7, bottom: 18, trailing: 17.7))
}

/// The expanded presentation's regions, allocated together as ActivityKit
/// arranges them ("the system first determines the width of the center
/// content while taking into account the minimal width of the leading and
/// trailing content ... By default, leading and trailing positions receive
/// an equal amount of horizontal space"): the sides take their content's
/// width at least, the centre what is left of the inner width at most, the
/// sides equal space capped at the camera, the centre below it, the bottom below
/// them all. The frames never overlap.
enum ExpandedArrangement {
    struct Frames: Equatable {
        var leading: CGRect, trailing: CGRect, center: CGRect, bottom: CGRect
        var height: CGFloat
        var all: [CGRect] { [leading, trailing, center, bottom] }
    }
    static func allocate(inner: CGFloat, geometry: IslandGeometry, leading: CGSize, trailing: CGSize, center: CGSize, bottom: CGSize) -> Frames {
        let sideLimit = max(0, (inner - geometry.obstruction.width) / 2)
        let sideMinimum = min(max(leading.width, trailing.width), sideLimit)
        let centerWidth = max(0, min(center.width, inner - 2 * sideMinimum))
        let side = min(sideLimit, max(sideMinimum, (inner - centerWidth) / 2))
        let leadingFrame = CGRect(x: 0, y: 0, width: side, height: leading.height)
        let trailingFrame = CGRect(x: inner - side, y: 0, width: side, height: trailing.height)
        let centerFrame = CGRect(x: (inner - centerWidth) / 2, y: geometry.obstruction.height, width: centerWidth, height: center.height)
        let below = max(leadingFrame.maxY, trailingFrame.maxY, centerFrame.maxY)
        let bottomFrame = CGRect(x: 0, y: below, width: inner, height: bottom.height)
        return Frames(leading: leadingFrame, trailing: trailingFrame, center: centerFrame, bottom: bottomFrame, height: bottomFrame.maxY)
    }
    static func measure(inner: CGFloat, geometry: IslandGeometry, size: (Int, ProposedViewSize) -> CGSize) -> Frames {
        let ideal = (0..<4).map { size($0, .unspecified) }
        var frames = allocate(inner: inner, geometry: geometry, leading: ideal[0], trailing: ideal[1], center: ideal[2], bottom: ideal[3])
        let heights = frames.all.enumerated().map { size($0.offset, ProposedViewSize(width: $0.element.width, height: nil)).height }
        frames.leading.size.height = heights[0]; frames.trailing.size.height = heights[1]; frames.center.size.height = heights[2]
        frames.bottom.origin.y = max(frames.leading.maxY, frames.trailing.maxY, frames.center.maxY)
        frames.bottom.size.height = heights[3]; frames.height = frames.bottom.maxY
        return frames
    }
}

/// The expanded regions placed by `ExpandedArrangement`: four subviews,
/// leading, trailing, centre, bottom, each measured at its ideal size.
struct ExpandedIslandLayout: Layout {
    let geometry: IslandGeometry
    private func frames(_ proposal: ProposedViewSize, _ subviews: Subviews) -> ExpandedArrangement.Frames? {
        guard subviews.count == 4, let width = proposal.width, width.isFinite else { return nil }
        return ExpandedArrangement.measure(inner: width, geometry: geometry) { subviews[$0].sizeThatFits($1) }
    }
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let frames = frames(proposal, subviews) else { return .zero }
        return CGSize(width: proposal.width ?? 0, height: frames.height)
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard let frames = frames(ProposedViewSize(bounds.size), subviews) else { return }
        for (index, frame) in frames.all.enumerated() {
            let trailing = index == 1
            subviews[index].place(at: CGPoint(x: bounds.minX + (trailing ? frame.maxX : frame.minX), y: bounds.minY + frame.minY),
                                  anchor: trailing ? .topTrailing : .topLeading, proposal: ProposedViewSize(width: frame.width, height: nil))
        }
    }
}

/// The eight placements as the system presents them, in a given width:
/// the Lock Screen card as wide as the gallery; the expanded Island as its
/// regions, allocated together (`ExpandedArrangement`); the compact Island
/// as its two regions around the camera's obstruction; the minimal view in
/// its circle. The regions' sizes are what the driver answers to the
/// stand-ins' proposals, as in the extension.
@MainActor
struct ActivityGalleryView: View {
    let preview: ActivityPreview
    let width: CGFloat
    var geometry = IslandGeometry.iPhone17Pro

    private var dark: Bool { preview.scheme == "dark" }
    /// The expanded regions' inner width at this gallery width.
    var expandedInner: CGFloat { width - geometry.expandedInsets.leading - geometry.expandedInsets.trailing }
    func regionWidth(_ placement: ActivityPlacement) -> CGFloat {
        let margins = ActivityPlacementView.platformMargins
        switch placement {
        case .lockScreen: return max(1, width - margins.leading - margins.trailing)
        case .minimal: return max(1, geometry.obstruction.height - 2 * geometry.compactInset)
        case .compactLeading, .compactTrailing:
            return max(1, (width - geometry.obstruction.width - geometry.obstruction.height - 16 - 2 * geometry.compactInset) / 2)
        default: return expandedFrames().all[placement.rawValue - ActivityPlacement.expandedLeading.rawValue].width
        }
    }
    func compactRegion(_ placement: ActivityPlacement) -> some View {
        preview.view(placement).frame(width: regionWidth(placement), alignment: placement == .compactTrailing ? .trailing : .leading)
    }
    func compactPlacement(_ placement: ActivityPlacement) -> some View {
        compactRegion(placement).padding(geometry.compactInset)
            .background(Color.black).clipShape(Capsule()).environment(\.colorScheme, .dark)
    }
    /// The frames the expanded regions take at this width, from the
    /// driver's ideal sizes: what the stand-in places, for a test to read.
    func expandedFrames() -> ExpandedArrangement.Frames {
        let placements: [ActivityPlacement] = [.expandedLeading, .expandedTrailing, .expandedCenter, .expandedBottom]
        return ExpandedArrangement.measure(inner: expandedInner, geometry: geometry) { preview.render.size(placements[$0], proposal: $1) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Lock Screen · \(preview.scheme) · \(Int(width)) pt").font(.caption).foregroundStyle(.secondary)
            lockScreen
            Text("Dynamic Island · expanded").font(.caption).foregroundStyle(.secondary)
            expanded
            Text("Dynamic Island · compact and minimal").font(.caption).foregroundStyle(.secondary)
            compact
        }
        .frame(width: width, alignment: .leading)
    }

    /// The card: the root's colour as the tint stands in for the system's
    /// `activityBackgroundTint` (iOS only), its gradients the view paints,
    /// and, unsaid, the scheme's material.
    var lockScreen: some View {
        let laid = try? preview.render.layout(.lockScreen, width: regionWidth(.lockScreen), height: nil)
        let tint = laid.flatMap { SnapshotActivity.background(of: $0, dark: dark).tint }
        return preview.view(.lockScreen)
            .frame(width: width)
            .background { if let tint { tint } else { Rectangle().fill(.regularMaterial) } }
            .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
            .environment(\.colorScheme, dark ? .dark : .light)
    }

    var expanded: some View {
        ExpandedIslandLayout(geometry: geometry) {
            preview.view(.expandedLeading)
            preview.view(.expandedTrailing)
            preview.view(.expandedCenter)
            preview.view(.expandedBottom)
        }
        .padding(geometry.expandedInsets)
        .frame(width: width)
        .background(Color.black)
        .clipShape(RoundedRectangle(cornerRadius: 44, style: .continuous))
        .environment(\.colorScheme, .dark)
    }

    private var compact: some View {
        let inset = geometry.compactInset
        return HStack(spacing: 16) {
            HStack(spacing: 0) {
                compactRegion(.compactLeading).padding(EdgeInsets(top: inset, leading: inset, bottom: inset, trailing: 0))
                Color.clear.frame(width: geometry.obstruction.width, height: geometry.obstruction.height)
                compactRegion(.compactTrailing).padding(EdgeInsets(top: inset, leading: 0, bottom: inset, trailing: inset))
            }
            .background(Color.black).clipShape(Capsule())
            compactRegion(.minimal)
                .padding(inset).frame(minWidth: geometry.obstruction.height, minHeight: geometry.obstruction.height)
                .background(Color.black).clipShape(Capsule())
        }
        .environment(\.colorScheme, .dark)
    }
}

@MainActor
private struct ActivityGalleryCell: View {
    let entry: ActivityEntry
    let scheme: String
    let width: CGFloat
    let appSession: ExactSession
    @State private var preview: ActivityPreview?
    @State private var failure: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let preview { ActivityGalleryView(preview: preview, width: width) }
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
            // The gallery's own width, finite before the scroll view, is what
            // an activity's placements are proposed (a horizontal scroll
            // would leave it unbounded); the widget variants keep their
            // family frames and scroll both ways.
            GeometryReader { proxy in
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
                                ActivityGalleryCell(entry: entry, scheme: scheme, width: max(IslandGeometry.iPhone17Pro.obstruction.width + 2 * 60, proxy.size.width - 32), appSession: appSession)
                            }
                        }
                    }.padding().id(appSession.app.resolver.publicationID + String(revision))
                }
            }
        }.onReceive(NotificationCenter.default.publisher(for: Notification.Name("ExactWidgetGeneration"))) { _ in revision += 1 }
    }
}

enum WidgetGallery {
    static var entries: [WidgetEntry] { entries(in: ExactApp.shared.resolver) }
    static func entries(in assets: AssetResolver) -> [WidgetEntry] {
        guard let data = assets.bytes("plans/widgets.json"),
              let values = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        #if os(watchOS)
        let familyKey = "watchFamilies"
        #else
        let familyKey = "families"
        #endif
        return values.compactMap { value in
            guard let kind = value["kind"] as? String, let families = value[familyKey] as? [String],
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
                    mode: address["mode"] ?? "fullColor", scheme: address["scheme"] ?? "light", size: address["size"] ?? "large",
                    scale: CGFloat(Int(address["scale"] ?? "1") ?? 0))
                guard entry.families.contains(variant.family), SnapshotEnvironment.Family(rawValue: variant.family) != nil,
                      ["fullColor", "accented", "vibrant"].contains(variant.mode), ["light", "dark"].contains(variant.scheme),
                      ["large", "xxxl"].contains(variant.size), [1, 2, 3].contains(variant.scale) else {
                    throw HeadlessSession.Failure(description: "invalid widget variant; families: \(entry.families.joined(separator: ", ")); modes: fullColor, accented, vibrant; schemes: light, dark; sizes: large, xxxl; scales: 1, 2, 3")
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
                    renderer.scale = variant.scale; renderer.proposedSize = ProposedViewSize(variant.displaySize)
                    guard let image = renderer.cgImage,
                          let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil) else {
                        throw HeadlessSession.Failure(description: "could not render widget screenshot")
                    }
                    CGImageDestinationAddImage(destination, image, nil)
                    guard CGImageDestinationFinalize(destination) else { throw HeadlessSession.Failure(description: "could not write widget screenshot") }
                    result["screenshot"] = path; result["w"] = variant.displaySize.width; result["h"] = variant.displaySize.height; result["scale"] = variant.scale
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
