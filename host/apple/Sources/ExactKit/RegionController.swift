#if os(macOS)
import AppKit

private final class WeakRegionArtifact {
    weak var value: RegionArtifact?
    init(_ value: RegionArtifact) { self.value = value }
}
private final class WeakRegionSource {
    weak var value: RegionTextSource?
    init(_ value: RegionTextSource) { self.value = value }
}
struct RegionPresentation {
    let snapshot: RegionSnapshot
    let artifacts: [UInt64: RegionArtifact]
    var rows: [RegionPaintRow] {
        snapshot.frames.filter { $0.artifact != 0 }.map { RegionPaintRow(artifact: $0.artifact, box: $0.box) }
    }
    var extent: CGSize {
        snapshot.frames.first(where: { $0.kind == "scroll" })?.extent
            ?? CGSize(width: 0, height: 0)
    }
}
/// All calls are on the session/UI executor. Worker answers revalidate generation,
/// request and publication on delivery, after the presenter's outermost batch.
final class RegionController {
    private weak var session: ExactSession?
    private var lifetime: RegionServiceLifetime?
    private var service: RegionService? { lifetime?.service }
    private var snapshot: RegionSnapshot?
    private var pendingAnswer: RegionAnswer?
    private var sources: [UInt64: WeakRegionSource] = [:]
    private var artifacts: [UInt64: WeakRegionArtifact] = [:]
    private var busy = false
    private var submitted: UInt64 = 0
    private var submittedRaster: UInt64 = 0
    private var serial: UInt64 = 0
    private(set) var candidate: RegionPresentation?
    private(set) var accepted: RegionPresentation?
    private(set) var surface: RegionSurfaceMac?
    private(set) var failure: String?
    private var generation = -1
    var currentGeneration: Int { generation }
    // The latest wire, not candidate.snapshot (which may describe older A).
    var currentSourcePublication: UInt64? { snapshot?.current == true ? snapshot?.publication : nil }
    private var shapeCompletions = 0
    private var rasterCompletions = 0
    private var sourceCaptures = 0
    private var sourceBytes = 0
    private var desiredRaster: RegionRasterRequest?
    private var capturedAppearance: String?
    private var members = Set<UInt32>()
    init(_ session: ExactSession) { self.session = session }
    deinit { service?.close() }
    func reset() {
        if session?.state == .destroyed { service?.close() } else { lifetime?.reset() }
        snapshot = nil; pendingAnswer = nil; sources.removeAll(); artifacts.removeAll()
        busy = false; submitted = 0; submittedRaster = 0; candidate = nil; accepted = nil
        surface?.suspend(); surface?.removeFromSuperview(); surface = nil; failure = nil; desiredRaster = nil; members.removeAll()
        generation = -1; capturedAppearance = nil
    }
    func owns(_ node: NodeView) -> Bool {
        guard let snapshot else { return false }
        if members.contains(node.id) { return true }
        var view: NSView? = node
        while let next = view {
            if let n = next as? NodeView, n.id == snapshot.content { return true }
            view = next.superview
        }
        return false
    }
    /// Before live create/style/children can trigger native painting or callbacks.
    func prepare(_ batch: Batch) {
        guard let session else { return }
        let regionOps = batch.ops.filter { $0.op == .region }
        // Ordinary apps have no retained region pixels to invalidate. Still
        // inspect incoming ops so a first registration or refusal is processed.
        guard snapshot != nil || !regionOps.isEmpty else { return }
        // Read old ancestry before any create/children/style operation can hide
        // its provenance. Include incoming members even when not mounted yet.
        var protected = members
        func protect(_ value: RegionSnapshot) {
            protected.insert(value.owner); protected.insert(value.content)
            var view: NSView? = session.presenter.views[value.owner]
            while let node = view {
                if let node = node as? NodeView { protected.insert(node.id) }
                view = node.superview
            }
        }
        if let snapshot { protect(snapshot) }
        // Presenter pageBackground is supplied by the first root even when the
        // region belongs to another root. Its paint changes are not disjoint.
        if let page = session.presenter.root.subviews.first as? NodeView { protected.insert(page.id) }
        var identityChanged = false
        for op in regionOps {
            guard let next = RegionSnapshot(op.payload), let incoming = op.payload["members"] as? [UInt32] else {
                identityChanged = true; continue
            }
            protect(next); protected.formUnion(incoming)
            if let snapshot {
                identityChanged = identityChanged || snapshot.incarnation != next.incarnation ||
                    snapshot.owner != next.owner || snapshot.content != next.content || Set(incoming) != members
            }
        }
        if batch.error != nil || identityChanged || RegionRetentionInvalidation.required(ops: batch.ops,protected: protected) {
            // Do not refund the running worker slot. Its old serial cannot rearm
            // A after source/style ABA, even if its output later matches again.
            desiredRaster = nil
            surface?.invalidateRetainedSource()
        }
        for op in regionOps {
            // @ref LLP 1043.000 §3 D7 — retire opaque raster before native flow.
            if op.payload["disabled"] as? String != nil { reset(); continue }
            guard let next = RegionSnapshot(op.payload) else { refuse("invalid region wire"); continue }
            if generation != session.generation || snapshot?.incarnation != next.incarnation {
                reset(); generation = session.generation
                if lifetime == nil { lifetime = RegionServiceLifetime { [weak self] answer in self?.receive(answer) } }
            }
            snapshot = next
            if batch.error == nil { service?.updateShapeRequest(next.request, generation: generation) }
            members = Set((op.payload["members"] as? [UInt32]) ?? [])
            if next.publication == 0 { candidate = nil }
            else if candidate?.snapshot.publication != next.publication {
                var retained: [UInt64: RegionArtifact] = [:]
                for frame in next.frames where frame.artifact != 0 {
                    guard let value = artifacts[frame.artifact]?.value else {
                        refuse("accepted publication has no native artifact"); return
                    }
                    retained[frame.artifact] = value
                }
                candidate = RegionPresentation(snapshot: next, artifacts: retained)
            }
        }
    }
    func flush() {
        guard let session, !session.isApplyingPresentation, let snapshot,
              session.generation == generation, session.state != .destroyed else { return }
        if let pendingAnswer { self.pendingAnswer = nil; receive(pendingAnswer); return }
        if let owner = session.presenter.views[snapshot.owner] {
            if surface == nil {
                let view = RegionSurfaceMac(controller: self)
                surface = view; owner.addSubview(view, positioned: .above, relativeTo: nil)
            }
            if let surface {
                surface.frame = owner.bounds; surface.autoresizingMask = [.width, .height]
                surface.isHidden = candidate == nil && accepted == nil
                surface.needsLayout = true; surface.needsDisplay = true
            }
        }
        schedule()
    }
    private func schedule() {
        guard !busy, validateAppearance(), let session, let snapshot, let service else { return }
        if snapshot.request != 0 && submitted != snapshot.request {
            let existing = sources[snapshot.source]?.value
            guard let wire = session.runtime.regionRequest(snapshot.request, knownSource: existing == nil ? 0 : snapshot.source),
                  RegionSnapshot.uint(wire["request"]) == snapshot.request,
                  let width = wire["width"] as? Double, let height = wire["height"] as? Double,
                  width.isFinite, height.isFinite else { refuse("stale or malformed region request"); return }
            let source: RegionTextSource
            if let existing { source = existing }
            else {
                guard let owner = session.presenter.views[snapshot.owner] else { refuse("region source owner missing"); return }
                capturedAppearance = appearance(of: owner)
                let dark = owner.drawsDark
                guard let captured = RegionTextSource.capture(wire: wire, engine: session.text, dark: dark) else {
                    refuse("region font/source capture refused"); return
                }
                source = captured
                sources = sources.filter { $0.value.value != nil }
                guard sources.count < 64 else { refuse("region source cap"); return }
                sources[snapshot.source] = WeakRegionSource(source)
                sourceCaptures += 1; sourceBytes += source.sourceUTF8Bytes
            }
            busy = true; submitted = snapshot.request
            service.submit(.shape(RegionShapeRequest(id: snapshot.request, sourceID: snapshot.source,
                source: source, width: width, height: height, generation: generation)))
        } else if let desiredRaster, submittedRaster != desiredRaster.serial {
            busy = true; submittedRaster = desiredRaster.serial; service.submit(.raster(desiredRaster))
        }
    }
    private func receive(_ answer: RegionAnswer) {
        precondition(Thread.isMainThread)
        guard let session, session.state != .destroyed, session.generation == generation else { return }
        guard validateAppearance() else { return }
        if case .abandoned(let id, let answerGeneration) = answer {
            // Check before touching busy or the deferred-answer slot: a late
            // terminal never releases/replaces a different active owner.
            guard answerGeneration == generation, busy, submitted == id else { return }
        }
        if session.isApplyingPresentation { pendingAnswer = answer; return }
        busy = false
        switch answer {
        case .abandoned:
            submitted = 0
        case .shape(let value):
            if submitted == value.id { submitted = 0 }
            guard value.generation == generation, snapshot?.request == value.id else { schedule(); return }
            artifacts = artifacts.filter { $0.value.value != nil }
            guard artifacts.count < 64 else { refuse("region native artifact cap"); return }
            artifacts[value.id] = WeakRegionArtifact(value)
            shapeCompletions += 1
            let batch = session.runtime.regionComplete(value)
            if let error = batch.error { refuse(error); return }
            session.apply(batch)
        case .raster(let value):
            if submittedRaster == value.request.serial { submittedRaster = 0 }
            guard desiredRaster?.serial == value.request.serial,
                  candidate?.snapshot.publication == value.request.publication,
                  value.request.generation == generation else {
                // A rejected old publication must not make its own desire
                // runnable again. A newer intent still owns its scheduling.
                if let desired = desiredRaster, desired.serial == value.request.serial,
                   desired.generation == value.request.generation,
                   desired.publication == value.request.publication { desiredRaster = nil }
                schedule(); return
            }
            // Bounds notifications need not have reached updateInk yet. Sample
            // actual geometry/palette/selection again at the delivery boundary.
            guard surface?.accepts(value.intent) == true else {
                if desiredRaster?.serial == value.request.serial { desiredRaster = nil }
                surface?.invalidatePhase(); schedule(); return
            }
            guard let image = MainActor.assumeIsolated({ value.image() }) else { refuse("region CGImage publication refused"); return }
            accepted = candidate; rasterCompletions += 1
            surface?.publish(value, image: image, presentation: accepted!)
            // Native extent/scroll notifications may establish a successor.
            if desiredRaster?.serial == value.request.serial { desiredRaster = nil }
        case .refused(let job, let reason):
            switch job {
            case .shape(let request):
                if submitted == request.id { submitted = 0 }
                if snapshot?.request == request.id && request.generation == generation { refuse(reason) }
            case .raster(let request):
                if submittedRaster == request.serial { submittedRaster = 0 }
                if desiredRaster?.serial == request.serial && request.generation == generation {
                    if surface?.accepts(request) == true { refuse(reason) }
                    else { desiredRaster = nil; surface?.invalidatePhase() }
                }
            }
        }
        schedule()
    }
    /// Geometry/profile/selection are sampled by the actual stationary view.
    func requestRaster(size: CGSize, scale: Int, profile: NativeProfile, format: UInt32,
                       background: [CGFloat], selectionColor: [CGFloat], scroll: CGPoint,
                       selections: [UInt64: NSRange], interaction: RegionPointRequest? = nil) {
        guard let candidate, validateAppearance() else { return }
        var rows = candidate.rows
        for i in rows.indices { rows[i].selection = selections[rows[i].artifact] ?? NSRange(location: 0, length: 0) }
        if let old = desiredRaster, old.publication == candidate.snapshot.publication,
           old.rows == rows, old.scroll == scroll, old.size == size, old.scale == scale,
           old.profile == profile, old.format == format, old.background == background,
           old.selectionColor == selectionColor, old.interaction == interaction { return }
        if let old = surface?.raster?.request, old.publication == candidate.snapshot.publication,
           old.rows == rows, old.scroll == scroll, old.size == size, old.scale == scale,
           old.profile == profile, old.format == format, old.background == background,
           old.selectionColor == selectionColor, interaction == nil {
            // A cache hit is still a new latest intent. It supersedes any B
            // already occupying the serial worker; do not release that slot.
            desiredRaster = nil
            return
        }
        guard serial < UInt64.max else { refuse("region raster serial exhausted"); return }
        serial += 1
        let next = RegionRasterRequest(serial: serial, publication: candidate.snapshot.publication, generation: generation,
            rows: rows, scroll: scroll, size: size, scale: scale, profile: profile, format: format,
            background: background, selectionColor: selectionColor, interaction: interaction)
        guard next.bytes != nil else { refuse("region surface pixel admission refused"); return }
        desiredRaster = next
        schedule()
    }
    private func appearance(of owner: NodeView) -> String {
        owner.effectiveAppearance.name.rawValue + (owner.drawsDark ? ":dark" : ":light")
    }
    /// Source CTLines contain resolved colors. This trial supports one fixed
    /// effective appearance per registration, never mixed old-ink/new-background.
    @discardableResult func validateAppearance() -> Bool {
        guard failure == nil else { return false }
        guard let capturedAppearance else { return true }
        guard let snapshot, let owner = session?.presenter.views[snapshot.owner],
              appearance(of: owner) == capturedAppearance else {
            refuse("region effective appearance changed; restart registration required")
            return false
        }
        return true
    }
    func geometryChanged() {
        guard validateAppearance() else { return }
        surface?.invalidatePhase()
    }
    func refuse(_ reason: String) {
        guard failure == nil else { return }
        failure = reason; desiredRaster = nil; pendingAnswer = nil
        surface?.suspend()
        session?.runtime.log("content-region: \(reason)")
        surface?.needsDisplay = true
    }
    var diagnostics: [String: Any] {
        let p = service?.pixels.stats, i = service?.ink.stats
        let acceptedSources: [[String: Any]] = accepted?.snapshot.frames.compactMap { frame in
            guard let artifact = accepted?.artifacts[frame.artifact] else { return nil }
            let source = artifact.metadata.source
            return ["key": String(frame.key), "id": frame.id, "artifact": String(artifact.id),
                "sourceID": String(artifact.sourceID), "utf16": source.utf16Count,
                "utf8": source.sourceUTF8Bytes, "sha256": artifact.metadata.sourceSHA256,
                "lines": artifact.metadata.lines.count]
        } ?? []
        return ["registered": snapshot != nil, "acceptedSources": acceptedSources, "generation": generation,
            "request": String(snapshot?.request ?? 0), "publication": String(accepted?.snapshot.publication ?? 0),
            "candidate": String(candidate?.snapshot.publication ?? 0), "shapeCompletions": shapeCompletions,
            "rasterCompletions": rasterCompletions, "sourceCaptures": sourceCaptures, "capturedUTF8Bytes": sourceBytes,
            "pixels": p?.bytes ?? 0, "pixelPeak": p?.peak ?? 0, "indexBytes": i?.bytes ?? 0, "indexPeak": i?.peak ?? 0,
            "surface": surface?.diagnostics ?? [:],
            "refused": failure as Any? ?? NSNull(), "scrollY": surface?.scrollOffset.y ?? 0,
            "rasterScrollY": surface?.raster?.request.scroll.y ?? 0,
            "acceptedUTF16": accepted?.artifacts.values.reduce(0) { $0 + $1.metadata.source.utf16Count } ?? 0]
    }
}
/// A deliberately small pre-apply classifier: only geometry and nonpainting
/// motion-binding metadata are neutral. Protected full props/style dictionaries
/// invalidate even if an apparent change might be geometry-only.
enum RegionRetentionInvalidation {
    static func required(ops: [BatchOp], protected: Set<UInt32>) -> Bool {
        for op in ops {
            let kind = op.op
            switch kind {
            case .region, .frame, .content, .hold, .heightDrag, .transformDrag, .retireMotion:
                continue
            case .create, .props, .style, .destroy, .present, .surface:
                guard let id = op.nodeID else { return true }
                if protected.contains(id) { return true }
            case .children:
                guard let id = op.nodeID else { return true }
                let children = op.ids
                if protected.contains(id) || children.contains(where: protected.contains) { return true }
            default:
                // Roots, routing/commands, collection ownership and unknown
                // operations have effects outside a single node dictionary.
                return true
            }
        }
        return false
    }
}
#endif
