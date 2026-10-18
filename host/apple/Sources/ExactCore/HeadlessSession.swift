// @ref LLP 1126 D8 — the existing runtime and text engine, without a presenter.
import Foundation
import CoreText

/// A session that produces laid-out values and creates no platform views.
/// Calls are serialized on Exact's owner thread; Core Text objects stay there.
public final class HeadlessSession {
    /// One styled run of a text node's paragraph, with the font the kernel
    /// measured it in, so a renderer draws the glyphs layout counted.
    public struct Run {
        public let text: String
        public let size: CGFloat
        public let weight: Int
        public let family: Int
        public let italic: Bool
        public let letterSpacing: CGFloat
        /// CSS `font-variant-numeric` bits: 1 is `tabular-nums`.
        public let numeric: Int
        /// The run's own `color` as the wire carries it (a platform colour
        /// `{"sys": …, "c": …}`, a `[light, dark]` pair, or four channels);
        /// nil takes the node's `text_color`.
        public let color: BatchValue?
        public let background: BatchValue?
        public let decoration: String
        public let href: String
        public let hidden: Bool
        public let font: CTFont
        public init(text: String, size: CGFloat, weight: Int, family: Int, italic: Bool, letterSpacing: CGFloat = 0, numeric: Int = 0,
                    color: BatchValue? = nil, background: BatchValue? = nil, decoration: String = "", href: String = "", hidden: Bool = false, font: CTFont) {
            self.text = text; self.size = size; self.weight = weight; self.family = family; self.italic = italic
            self.letterSpacing = letterSpacing; self.numeric = numeric; self.color = color; self.background = background
            self.decoration = decoration; self.href = href; self.hidden = hidden; self.font = font
        }
    }
    /// A line's text in one run: `run` indexes the node's `runs`.
    public struct Segment {
        public let run: Int
        public let text: String
        /// Shaped UTF-16 coverage; a trailing ellipsis is not part of this range.
        public let range: NSRange
        public init(run: Int, text: String, range: NSRange = NSRange(location: 0, length: 0)) {
            self.run = run; self.text = text; self.range = range
        }
    }
    /// A laid-out line. Justification lives in Core Text glyph positions, not
    /// these strings: a renderer that cannot preserve it must refuse justified text.
    public struct Line {
        public let text: String
        /// UTF-16 range in the shaped text, after whitespace collapse, Markdown
        /// expansion and hyphenation; these are not offsets in the source prop.
        public let range: NSRange
        /// Horizontal origin and baseline, in points from the content box's
        /// origin (zero). Add `Node.contentBox.origin` once to place in the node.
        public let origin: CGPoint
        public let width: CGFloat
        public let ascent: CGFloat
        public let descent: CGFloat
        public let segments: [Segment]
        public init(text: String, range: NSRange, origin: CGPoint, width: CGFloat, ascent: CGFloat = 0, descent: CGFloat = 0, segments: [Segment] = []) {
            self.text = text; self.range = range; self.origin = origin; self.width = width
            self.ascent = ascent; self.descent = descent; self.segments = segments
        }
    }
    public struct Node {
        public let id: UInt32
        public let kind: String
        /// Border box in the parent's coordinates, in points.
        public fileprivate(set) var frame = CGRect.zero
        /// Content box in this node's border-box coordinates, in points.
        public var contentBox: CGRect { content ?? CGRect(origin: .zero, size: frame.size) }
        public fileprivate(set) var props: [String: String]
        public fileprivate(set) var style: NodeStyle
        public fileprivate(set) var children: [UInt32] = []
        /// Computed on first access for this node version, then shared by snapshots.
        public var lines: [Line] { textLines?.value.lines ?? [] }
        public var runs: [Run] { textLines?.value.runs ?? [] }
        public init(id: UInt32, kind: String, frame: CGRect = .zero, contentBox: CGRect? = nil, props: [String: String] = [:],
                    style: NodeStyle = [:], children: [UInt32] = [], lines: [Line] = [], runs: [Run] = []) {
            self.id = id; self.kind = kind; self.frame = frame; content = contentBox
            self.props = props; self.style = style; self.children = children
            if !lines.isEmpty || !runs.isEmpty { textLines = HeadlessLines(lines: lines, runs: runs) }
        }
        fileprivate var content: CGRect?
        fileprivate var inline: [InlineText] = []
        fileprivate var flow: [TextFlowShape] = []
        fileprivate var textLines: HeadlessLines?
        package var hasComputedLines: Bool { textLines?.computed ?? false }
    }
    /// Copy-on-write node/child/style storage. Taking a snapshot does not visit
    /// nodes or duplicate their strings, style rows or shaped paragraphs. Keeping
    /// older snapshots retains their changed node versions until they are released.
    public struct Snapshot {
        public let size: CGSize
        public let roots: [UInt32]
        public let nodes: [UInt32: Node]
        public init(size: CGSize, roots: [UInt32], nodes: [UInt32: Node]) { self.size = size; self.roots = roots; self.nodes = nodes }
    }
    public struct Failure: Error, CustomStringConvertible {
        public let description: String
    }

    package let core: SessionCore
    private var rootFontSize: Double
    private var nodes: [UInt32: Node] = [:]
    private var roots: [UInt32] = []
    private var size: CGSize
    private var language = ""

    /// Boots the supplied plan, or the linked app's baked plan when nil.
    public convenience init(plan: Data? = nil, entry: String? = nil, size: CGSize, facts: SessionFacts, assets: AssetResolver) throws {
        try self.init(plan: plan, entry: entry, size: size, facts: facts, assets: assets, presentation: nil)
    }

    /// A package composition may install Markdown expansion without a presenter.
    package init(plan: Data? = nil, entry: String? = nil, size: CGSize, facts: SessionFacts, assets: AssetResolver,
                 presentation: TextPresentation?) throws {
        self.size = size; rootFontSize = facts.rootFontSize
        let runtime = Runtime(hooks: .headless)
        let text = TextEngine(assets: { assets.url($0) }, read: { assets.bytes($0) }, bundled: { assets.bundledURL($0) },
                              coldTextTargetBytes: facts.textResidencyBytes, presentation: presentation)
        text.requiresMarkdownExpansion = true
        core = SessionCore(runtime: runtime, text: text)
        core.measure(with: text)
        runtime.setProfileResolver(assets)
        _ = try turn {
            try facts.prime(runtime)
            if let entry {
                guard let plan else { return Batch(ops: [], timers: false, motion: false, clock: nil, error: "an entry requires a plan") }
                return runtime.bootEntry(plan, kind: entry, width: size.width, height: size.height)
            }
            return plan.map { runtime.bootPlan($0, width: size.width, height: size.height) }
                ?? runtime.boot(width: size.width, height: size.height)
        }
        Owner.shared.sync { text.commitFonts() }
    }

    public convenience init(plan: Data? = nil, entry: String? = nil, size: CGSize, facts: SessionFacts,
                            assetRoot: URL = Bundle.main.bundleURL) throws {
        try self.init(plan: plan, entry: entry, size: size, facts: facts, assets: AssetResolver(root: assetRoot))
    }

    public static let defaultRootFontSize = SessionFacts.defaultRootFontSize

    /// A display-free convenience; a host with destination facts should supply them explicitly.
    public convenience init(plan: Data? = nil, entry: String? = nil, size: CGSize, rootFontSize: CGFloat? = nil,
                            assets: AssetResolver) throws {
        try self.init(plan: plan, entry: entry, size: size,
                      facts: SessionFacts(displayScale: 1, preferences: [], rootFontSize: Double(rootFontSize ?? Self.defaultRootFontSize)), assets: assets)
    }
    public convenience init(plan: Data? = nil, entry: String? = nil, size: CGSize, rootFontSize: CGFloat? = nil,
                            assetRoot: URL = Bundle.main.bundleURL) throws {
        try self.init(plan: plan, entry: entry, size: size, rootFontSize: rootFontSize, assets: AssetResolver(root: assetRoot))
    }

    /// The reader's body size, including Dynamic Type.
    @discardableResult
    public func setRootFontSize(_ px: CGFloat) throws -> Snapshot {
        try turn {
            let batch = core.runtime.setRootFontSize(Double(px))
            if batch.error == nil { rootFontSize = Double(px) }
            return batch
        }
    }

    public func close() {
        Owner.shared.sync {
            core.runtime.destroy()
            core.text.install(nil)
            // Snapshots hold a weak shaper; after close they shape on demand
            // with the saved catalog and no cold residency.
            core.text = TextEngine(assets: { _ in nil }, coldTextTargetBytes: 0)
            nodes.removeAll(); roots.removeAll()
        }
    }

    /// The tree after the most recent successful turn. Asynchronous replies need `refresh`.
    public func snapshot() throws -> Snapshot {
        try Owner.shared.sync { Result { try snapshotOnOwner() } }.get()
    }

    @discardableResult
    public func resize(to size: CGSize) throws -> Snapshot {
        try turn(size: size) { core.runtime.resize(width: size.width, height: size.height) }
    }

    /// Activate deferred data, drain ready replies and lay out their store changes.
    /// `now` is the runner clock in milliseconds; this session starts no timers.
    @discardableResult
    public func refresh(at now: Double = 0) throws -> Snapshot {
        _ = try turn { core.runtime.dataReady() }
        return try turn { core.runtime.pump(now: now) }
    }

    /// Run a node's press action and lay out the resulting store changes.
    @discardableResult
    public func press(_ node: UInt32, at now: Double = 0) throws -> Snapshot {
        try turn { core.runtime.press(node, now: now) }
    }

    private func turn(size: CGSize? = nil, _ body: () throws -> Batch) throws -> Snapshot {
        try Owner.shared.sync {
            Result {
                guard !core.runtime.destroyed else { throw Failure(description: "headless session is closed") }
                core.text.refusedMarkdown = nil
                let batch = try body()
                if let error = batch.error { throw Failure(description: error) }
                if let size { self.size = size }
                try apply(batch)
                return try snapshotOnOwner()
            }
        }.get()
    }

    private func apply(_ batch: Batch) throws {
        var changed = Set<UInt32>()
        for op in batch.ops {
            switch op.op {
            case .create: nodes[op.id] = Node(id: op.id, kind: op.kind, props: op.props, style: op.style)
            case .props:
                for key in op.clear { nodes[op.id]?.props[key] = nil }
                for (key, value) in op.props { nodes[op.id]?.props[key] = value }
            case .style: nodes[op.id]?.style = op.style
            case .children: nodes[op.id]?.children = op.ids
            case .paragraph: nodes[op.id]?.inline = op.runs
            case .frame: nodes[op.id]?.frame = CGRect(x: op.x, y: op.y, width: op.w, height: op.h)
            case .fieldContent:
                if let r = op.payload["rect"] as? [Double], r.count == 4 {
                    nodes[op.id]?.content = CGRect(x: r[0], y: r[1], width: r[2], height: r[3])
                } else { nodes[op.id]?.content = nil }
            case .flow:
                nodes[op.id]?.flow = (op.payload["shapes"] as? [[String: Any]] ?? []).compactMap(TextFlowShape.init)
            case .roots: roots = op.ids
            case .destroy: nodes[op.id] = nil
            case .language:
                language = op.payload["lang"] as? String ?? ""
                changed.formUnion(nodes.keys)
            default: continue
            }
            changed.insert(op.id)
        }
        // The callback refuses before shaping raw Markdown. Inspect unmeasured
        // nodes too (for example display:none) so boot never returns a partial tree.
        for id in changed {
            guard let node = nodes[id], node.kind == "text" else { continue }
            var markdown: [ExactCore.Run]?
            if node.props["markup"] == "markdown" {
                let source = TextStyle.run(node.props["text"] ?? node.inline.map(\.text).joined(),
                                           style: node.style, defaultSize: rootFontSize)
                markdown = core.text.presentation?.markdown(core.text, source: source)
                if markdown == nil { core.text.refusedMarkdown = id }
            }
            nodes[id]?.textLines = HeadlessLines(node: node, engine: core.text, language: language,
                                                 rootFontSize: rootFontSize, markdown: markdown)
        }
        if let id = core.text.refusedMarkdown {
            let name = nodes[id]?.props["testId"] ?? nodes[id]?.props["id"] ?? String(id)
            core.runtime.destroy()
            throw Failure(description: "headless node \(name) (#\(id)) requires a Markdown expansion through TextPresentation")
        }
    }

    private func snapshotOnOwner() throws -> Snapshot {
        guard !core.runtime.destroyed else { throw Failure(description: "headless session is closed") }
        return Snapshot(size: size, roots: roots, nodes: nodes)
    }
}

/// Each node version retains only the inputs until a consumer asks for its lines.
/// The live engine is borrowed weakly. After close, an uncomputed node uses a
/// temporary shaper with its saved catalog and no cold cache; computed nodes
/// retain only their exported lines and fonts, never the engine's residency.
fileprivate final class HeadlessLines {
    private var build: (() -> Value)?
    typealias Value = (lines: [HeadlessSession.Line], runs: [HeadlessSession.Run])
    private var cached: Value?
    var computed: Bool { Owner.shared.sync { cached != nil } }
    var value: Value {
        Owner.shared.sync {
            if let cached { return cached }
            let lines = build!(); cached = lines; build = nil; return lines
        }
    }
    init(lines: [HeadlessSession.Line], runs: [HeadlessSession.Run]) {
        cached = (lines, runs); build = nil
    }
    init(node: HeadlessSession.Node, engine: TextEngine, language: String, rootFontSize: Double, markdown: [Run]?) {
        // Capture individual COW values, not the node and its previous line cache.
        let props = node.props, style = node.style, inline = node.inline.filter(\.paints), flow = node.flow, box = node.contentBox
        let catalog = engine.catalog, resolve = engine.resolve, read = engine.read, bundled = engine.bundled
        build = { [weak engine] in
            let engine = engine ?? {
                let made = TextEngine(assets: resolve, read: read, bundled: bundled, coldTextTargetBytes: 0)
                made.catalog = catalog
                return made
            }()
            let runs = markdown ?? props["text"].map { [TextStyle.run($0, style: style, defaultSize: rootFontSize)] }
                ?? inline.map(\.lightRun)
            let spec = TextStyle.spec(runs: runs, style: style, props: props,
                strut: TextStyle.run("", style: style, defaultSize: rootFontSize), language: language)
            let paragraph = engine.paragraph(spec, width: box.width,
                flow: flow.map { $0.translated(CGPoint(x: -box.minX, y: -box.minY)) })
            let source = spec.runs.map(\.text).joined() as NSString
            let renderedRuns = spec.runs.enumerated().map { index, run in
                let input = markdown == nil && props["text"] == nil && index < inline.count ? inline[index] : nil
                return HeadlessSession.Run(text: run.text, size: run.size, weight: run.weight, family: run.family, italic: run.italic,
                    letterSpacing: run.letterSpacing, numeric: run.numeric,
                    color: Self.color(input?.colorRef, light: run.color, dark: input?.darkColor),
                    background: Self.color(input?.backgroundRef, light: run.background, dark: input?.darkBackground),
                    decoration: run.decoration, href: run.href, hidden: run.hidden, font: engine.font(run) as CTFont)
            }
            var ends: [Int] = [], offset = 0
            for run in spec.runs { offset += run.text.utf16.count; ends.append(offset) }
            let geometry = LineGeometry(paragraph)
            let lines = paragraph.lines.enumerated().map { index, line in
                let r = geometry.ranges[index], range = NSRange(location: r.location, length: r.length)
                let clamped = index == geometry.ranges.count - 1 && geometry.clamped != nil
                // A clamped line is drawn as the ellipsized line paints it: the
                // prefix its glyphs keep, then the token; `range` stays the broken range.
                let end = clamped ? Self.kept(line, range: range, source: source) : NSMaxRange(range)
                let suffix = clamped ? "…" : ""
                let shown = NSRange(location: range.location, length: end - range.location)
                let painted = clamped ? source.substring(with: shown) + suffix : Self.ink(source.substring(with: shown))
                var segments: [HeadlessSession.Segment] = [], from = range.location
                for (run, runEnd) in ends.enumerated() where runEnd > from && from < end {
                    let to = min(runEnd, end)
                    let part = NSRange(location: from, length: to - from)
                    segments.append(.init(run: run, text: source.substring(with: part), range: part))
                    from = to
                }
                if let tail = segments.popLast() {
                    segments.append(.init(run: tail.run, text: clamped ? tail.text + suffix : Self.ink(tail.text), range: tail.range))
                } else if clamped {
                    segments.append(.init(run: 0, text: suffix))
                }
                var ascent: CGFloat = 0, descent: CGFloat = 0
                let width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
                return HeadlessSession.Line(text: painted, range: range,
                    origin: CGPoint(x: paragraph.origin(index, align: spec.align, width: box.width), y: paragraph.baselines[index]),
                    width: width, ascent: ascent, descent: descent, segments: segments)
            }
            return (lines, renderedRuns)
        }
    }
    /// Where a clamped line's kept text ends: past the last glyph the ellipsized
    /// line keeps from the broken text (the token's glyphs sit at the stripped
    /// end, `TextEngine.clampedLine`), with trailing breaks and spaces removed.
    private static func kept(_ line: CTLine, range: NSRange, source: NSString) -> Int {
        let breaks: Set<Int> = [9, 10, 13, 32, 0x2028, 0x2029]
        var stripped = NSMaxRange(range)
        while stripped > range.location && breaks.contains(Int(source.character(at: stripped - 1))) { stripped -= 1 }
        var kept = range.location
        for run in CTLineGetGlyphRuns(line) as? [CTRun] ?? [] {
            let count = CTRunGetGlyphCount(run)
            guard count > 0 else { continue }
            var indices = [CFIndex](repeating: 0, count: count)
            CTRunGetStringIndices(run, CFRange(location: 0, length: count), &indices)
            let range = CTRunGetStringRange(run)
            for (i, index) in indices.enumerated() where index >= range.location && index < stripped {
                // A glyph's characters run to the next glyph's index, or the run's end.
                let next = i + 1 < count ? indices[i + 1] : range.location + range.length
                kept = max(kept, min(max(next, index + 1), stripped))
            }
        }
        while kept > range.location && breaks.contains(Int(source.character(at: kept - 1))) { kept -= 1 }
        return kept
    }
    private static func ink(_ text: String) -> String {
        text.last == "\u{AD}" ? String(text.dropLast()) + "-" : text
    }
    private static func color(_ ref: BatchValue?, light: [Double]?, dark: [Double]?) -> BatchValue? {
        if let ref { return ref }
        guard let light else { return nil }
        let value = BatchValue.array(light.map(BatchValue.number))
        return dark.flatMap { $0 != light ? .array([value, .array($0.map(BatchValue.number))]) : nil } ?? value
    }
}
