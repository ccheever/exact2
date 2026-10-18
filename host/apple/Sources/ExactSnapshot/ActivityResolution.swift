// @ref LLP 1126.001 WP10, LLP 1126 D9 — what an activity renders from: the
// generation its attributes name, the plan of its kind there, and the state
// that plan decodes (the envelope's, else the last decodable one kept for
// the activity, else the preview as the placeholder). The extension's
// configuration is a thin adapter over this; the tests drive it directly.
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore

/// What the envelope says about one activity (`ExactActivityAttributes` and
/// its `ContentState`), with the state as the JSON bytes the plan decodes.
public struct ActivityRequest: Hashable, Sendable {
    public var activityID: String
    public var kind: String
    public var generation: String
    /// The shape digest the attributes were started under.
    public var shape: String
    /// The content state's shape digest and its state, encoded with sorted keys.
    public var stateShape: String
    public var state: Data
    public init(activityID: String, kind: String, generation: String, shape: String, stateShape: String, state: Data) {
        self.activityID = activityID; self.kind = kind; self.generation = generation; self.shape = shape; self.stateShape = stateShape; self.state = state
    }
}

/// The facts a session reads that shape its layout: everything the render
/// is keyed by beside the request. Two renders of one request differ when
/// any of these differ (a text size, Bold Text, the scheme, the scale).
public struct ActivityFacts: Hashable, Sendable {
    public var displayScale: Double
    public var preferences: UInt32
    public var rootFontSize: Double
    public var locale: String
    public var timeZone: String
    public init(displayScale: Double, preferences: Preferences, rootFontSize: Double, locale: String = Locale.preferredLanguages.first ?? Locale.current.identifier(.bcp47), timeZone: TimeZone = .current) {
        self.displayScale = displayScale; self.preferences = preferences.rawValue; self.rootFontSize = rootFontSize; self.locale = locale; self.timeZone = timeZone.identifier
    }
    public var dark: Bool { Preferences(rawValue: preferences).contains(.dark) }
    public func session(at date: Date = Date()) -> SessionFacts {
        SessionFacts(displayScale: displayScale, preferences: Preferences(rawValue: preferences), rootFontSize: rootFontSize, date: date,
                     timeZone: TimeZone(identifier: timeZone) ?? .current, locale: locale)
    }
}

/// Where an activity's plans come from: the app group's published
/// generations, and the bundle's embedded plans with the generation their
/// identity makes (`ExactWidgetGeneration`), which stand in only for that
/// generation. The compatibility id gates both (`WidgetContainer.pin`).
public struct ActivitySources {
    public var groupRoot: URL?
    public var embeddedRoot: URL
    public var embeddedGeneration: String?
    public var compatibility: String
    public init(groupRoot: URL?, embeddedRoot: URL, embeddedGeneration: String?, compatibility: String) {
        self.groupRoot = groupRoot; self.embeddedRoot = embeddedRoot; self.embeddedGeneration = embeddedGeneration; self.compatibility = compatibility
    }
    /// The extension's: the app group and the bundle it runs from.
    public static func main() -> ActivitySources {
        ActivitySources(groupRoot: WidgetContainer.root, embeddedRoot: Bundle.main.bundleURL,
                        embeddedGeneration: Bundle.main.object(forInfoDictionaryKey: "ExactWidgetGeneration") as? String,
                        compatibility: WidgetContainer.compatibility)
    }
    /// The directory holding generation `name`'s plans: the published one of
    /// that name in this cohort, or the embedded plans when their identity
    /// is that generation. Never another generation (D9: an activity renders
    /// from the generation it was started under).
    public func root(for name: String) throws -> URL {
        if let groupRoot, let pinned = try? WidgetContainer.pin(root: groupRoot, generation: name, compatibility: compatibility) { return pinned }
        if let embeddedGeneration, WidgetContainer.generationDigest(compatibility: compatibility, publicationID: "embedded:" + embeddedGeneration) == name {
            return embeddedRoot
        }
        throw ActivityResolver.Failure.generationUnavailable(name)
    }
}

/// One activity rendered: the session over the plan of its kind in its
/// generation, with the state the plan decoded, and what that state was.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public final class ActivityRendering {
    public let request: ActivityRequest
    public let facts: ActivityFacts
    public let render: ActivityRender
    /// The directory the plan and its images came from.
    public let root: URL
    /// The kept last-good state is shown, not the envelope's (D9).
    public let outdated: Bool
    /// The preview stands in: nothing decodable was ever kept.
    public let placeholder: Bool
    public var kind: String { request.kind }
    init(request: ActivityRequest, facts: ActivityFacts, render: ActivityRender, root: URL, outdated: Bool, placeholder: Bool) {
        self.request = request; self.facts = facts; self.render = render; self.root = root; self.outdated = outdated; self.placeholder = placeholder
    }
}

@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public enum ActivityResolver {
    public enum Failure: Error, CustomStringConvertible, Equatable {
        /// The named generation is neither published in this cohort nor the
        /// embedded one: the activity shows a degraded presentation until
        /// the app publishes it again, and nothing else is rendered in its
        /// place.
        case generationUnavailable(String)
        case planUnavailable(kind: String, generation: String)
        case stateUndecodable(kind: String)
        public var description: String {
            switch self {
            case .generationUnavailable(let name): return "activity generation \(name.prefix(12))… is not published in this cohort"
            case .planUnavailable(let kind, let generation): return "activity \(kind): no plan in generation \(generation.prefix(12))…"
            case .stateUndecodable(let kind): return "activity \(kind): neither the state nor a kept one decodes, and the preview is missing"
            }
        }
    }
    struct Registry: Decodable {
        let kind: String
        let shape: String
        let preview: ActivityJSON
    }

    /// Resolves and boots: the generation's plan, then the envelope's state
    /// when its shape is the registry's and the plan decodes it (kept as
    /// the activity's last good state), else the last good state kept for
    /// the activity id (`outdated`), else the preview (`placeholder`).
    /// `stateCache` is the group's per-id store; nil keeps nothing.
    public static func resolve(_ request: ActivityRequest, facts: ActivityFacts, sources: ActivitySources, stateCache: ActivityStateCache?, at date: Date = Date()) throws -> ActivityRendering {
        let root = try sources.root(for: request.generation)
        guard let plan = try? Data(contentsOf: root.appendingPathComponent("plans/\(request.kind).plan")) else {
            throw Failure.planUnavailable(kind: request.kind, generation: request.generation)
        }
        let registry = (try? JSONDecoder().decode([Registry].self, from: Data(contentsOf: root.appendingPathComponent("plans/activities.json"))))?.first { $0.kind == request.kind }
        let session = facts.session(at: date)
        func boot(_ state: Data?) throws -> ActivityRender {
            try ActivityRender(plan: plan, kind: request.kind, state: state, facts: session, assetRoot: root)
        }
        // A state the plan cannot decode (another shape's digest, fields it
        // does not know) is refused by the boot; a boot that succeeds is the render.
        var booted: [Data: ActivityRender] = [:]
        func accepts(_ bytes: Data) -> Bool {
            if booted[bytes] == nil, let render = try? boot(bytes) { booted[bytes] = render }
            return booted[bytes] != nil
        }
        let envelope = request.stateShape == registry?.shape ? request.state : nil
        var resolved: (state: Data?, outdated: Bool)
        do { resolved = try stateCache?.resolve(envelope, accepts: accepts) ?? (nil, false) }
        catch { resolved = (nil, false) }
        if resolved.state == nil, let envelope, accepts(envelope) { resolved = (envelope, false) }
        if let state = resolved.state, let render = booted[state] ?? (try? boot(state)) {
            return ActivityRendering(request: request, facts: facts, render: render, root: root, outdated: resolved.outdated, placeholder: false)
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let registry, let preview = try? encoder.encode(registry.preview), let render = try? boot(preview) else {
            throw Failure.stateUndecodable(kind: request.kind)
        }
        return ActivityRendering(request: request, facts: facts, render: render, root: root, outdated: false, placeholder: true)
    }
}

/// The renders alive in a process, keyed by the request and the facts: the
/// eight placements of one push share one session (one session per
/// activity, D9), and a state the system asks for again (another region)
/// is not booted twice; any fact that shapes the layout keys a new one.
/// Bounded: an activity's older renders leave as new ones come; a failed
/// resolution is kept by nobody.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public final class ActivityRenders {
    public static let shared = ActivityRenders()
    private struct Key: Hashable { let request: ActivityRequest; let facts: ActivityFacts }
    private let lock = NSLock()
    private var kept: [(key: Key, rendering: ActivityRendering)] = []
    public let limit: Int
    public init(limit: Int = 8) { self.limit = limit }

    public func rendering(for request: ActivityRequest, facts: ActivityFacts, sources: ActivitySources, stateCache: ActivityStateCache?, at date: Date = Date()) throws -> ActivityRendering {
        let key = Key(request: request, facts: facts)
        lock.lock()
        if let found = kept.first(where: { $0.key == key }) { lock.unlock(); return found.rendering }
        lock.unlock()
        let made = try ActivityResolver.resolve(request, facts: facts, sources: sources, stateCache: stateCache, at: date)
        lock.lock(); defer { lock.unlock() }
        kept.removeAll { $0.key.request.activityID == request.activityID && $0.key.request == request && $0.key.facts != facts }
        kept.append((key, made))
        if kept.count > limit { kept.removeFirst(kept.count - limit) }
        return made
    }
    public var count: Int { lock.lock(); defer { lock.unlock() }; return kept.count }
}

/// One placement of a rendering, as the system archives it: the driver's
/// placement view, the preview redacted as a placeholder, a kept state in
/// the driver's outdated presentation (dimmed; the system's `isStale` is
/// the system's own fact and presentation, never dimmed again here).
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
public struct ActivityPresentation: View {
    public let rendering: ActivityRendering
    public let placement: ActivityPlacement
    public let environment: SnapshotEnvironment
    public static let outdatedOpacity: Double = 0.55
    public init(rendering: ActivityRendering, placement: ActivityPlacement, environment: SnapshotEnvironment) {
        self.rendering = rendering; self.placement = placement; self.environment = environment
    }
    public var body: some View {
        SnapshotActivity.view(rendering.render, placement: placement, environment: environment)
            .redacted(reason: rendering.placeholder ? .placeholder : [])
            .opacity(rendering.outdated ? Self.outdatedOpacity : 1)
    }
}
#endif
