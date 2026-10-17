// @ref LLP 1126 D8 — the existing runtime and text engine, without a presenter.
import Foundation
import CoreText

/// A session that produces laid-out values and creates no platform views.
/// Calls are serialized on Exact's owner thread; Core Text objects stay there.
public final class HeadlessSession {
    public struct Line {
        public let text: String
        public let range: NSRange
        /// Origin and baseline in the text node's content coordinates.
        public let origin: CGPoint
        public let width: CGFloat
    }
    public struct Node {
        public let id: UInt32
        public let kind: String
        public let frame: CGRect
        public let contentBox: CGRect
        public let props: [String: String]
        public let style: [String: Any]
        public let children: [UInt32]
        public let lines: [Line]
    }
    public struct Snapshot {
        public let size: CGSize
        public let roots: [UInt32]
        public let nodes: [UInt32: Node]
    }
    public struct Failure: Error, CustomStringConvertible {
        public let description: String
    }

    package let core: SessionCore
    private struct Record {
        var kind: String
        var props: [String: String]
        var style: NodeStyle
        var children: [UInt32] = []
        var frame = CGRect.zero
        var contentBox: CGRect?
        var runs: [InlineText] = []
        var flow: [TextFlowShape] = []
    }
    private var records: [UInt32: Record] = [:]
    private var roots: [UInt32] = []
    private var size: CGSize
    private var language = ""

    /// Boots the supplied plan, or the linked app's baked plan when nil.
    public init(plan: Data? = nil, size: CGSize, assets: AssetResolver) throws {
        self.size = size
        let runtime = Runtime(headless: ())
        let text = TextEngine(assets: { assets.url($0) }, read: { assets.bytes($0) }, bundled: { assets.bundledURL($0) })
        core = SessionCore(runtime: runtime, text: text)
        core.measure(with: text)
        runtime.setProfileResolver(assets)
        _ = try turn {
            plan.map { runtime.bootPlan($0, width: size.width, height: size.height) }
                ?? runtime.boot(width: size.width, height: size.height)
        }
        Owner.shared.sync { text.commitFonts() }
    }

    public convenience init(plan: Data? = nil, size: CGSize, assetRoot: URL = Bundle.main.bundleURL) throws {
        try self.init(plan: plan, size: size, assets: AssetResolver(root: assetRoot))
    }

    deinit { core.runtime.destroy() }

    public func close() { Owner.shared.sync { core.runtime.destroy() } }

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

    private func turn(size: CGSize? = nil, _ body: () -> Batch) throws -> Snapshot {
        try Owner.shared.sync {
            Result {
                guard !core.runtime.destroyed else { throw Failure(description: "headless session is closed") }
                let batch = body()
                if let error = batch.error { throw Failure(description: error) }
                if let size { self.size = size }
                apply(batch)
                return try snapshotOnOwner()
            }
        }.get()
    }

    private func apply(_ batch: Batch) {
        for op in batch.ops {
            switch op.op {
            case .create: records[op.id] = Record(kind: op.kind, props: op.props, style: op.style)
            case .props:
                for key in op.clear { records[op.id]?.props[key] = nil }
                for (key, value) in op.props { records[op.id]?.props[key] = value }
            case .style: records[op.id]?.style = op.style
            case .children: records[op.id]?.children = op.ids
            case .paragraph: records[op.id]?.runs = op.runs
            case .frame: records[op.id]?.frame = CGRect(x: op.x, y: op.y, width: op.w, height: op.h)
            case .fieldContent:
                if let r = op.payload["rect"] as? [Double], r.count == 4 {
                    records[op.id]?.contentBox = CGRect(x: r[0], y: r[1], width: r[2], height: r[3])
                } else { records[op.id]?.contentBox = nil }
            case .flow:
                records[op.id]?.flow = (op.payload["shapes"] as? [[String: Any]] ?? []).compactMap(TextFlowShape.init)
            case .roots: roots = op.ids
            case .destroy: records[op.id] = nil
            case .language: language = op.payload["lang"] as? String ?? ""
            default: break
            }
        }
    }

    private func snapshotOnOwner() throws -> Snapshot {
        var nodes: [UInt32: Node] = [:]
        for (id, node) in records {
            let box = node.contentBox ?? CGRect(origin: .zero, size: node.frame.size)
            var lines: [Line] = []
            if node.kind == "text" {
                guard node.props["markup"] != "markdown" else {
                    throw Failure(description: "headless text does not install the ExactKit Markdown adapter")
                }
                let runs = node.props["text"].map { [TextStyle.run($0, style: node.style)] }
                    ?? node.runs.filter(\.paints).map(\.lightRun)
                let spec = TextStyle.spec(runs: runs, style: node.style, props: node.props,
                                          strut: TextStyle.run("", style: node.style), language: language)
                let paragraph = core.text.paragraph(spec, width: box.width,
                    flow: node.flow.map { $0.translated(CGPoint(x: -box.minX, y: -box.minY)) })
                let source = spec.runs.map(\.text).joined() as NSString
                lines = paragraph.lines.enumerated().map { index, line in
                    let r = CTLineGetStringRange(line)
                    let range = NSRange(location: r.location, length: r.length)
                    return Line(text: source.substring(with: range), range: range,
                                origin: CGPoint(x: box.minX + paragraph.origin(index, align: spec.align, width: box.width),
                                                y: box.minY + paragraph.baselines[index]),
                                width: CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                }
            }
            nodes[id] = Node(id: id, kind: node.kind, frame: node.frame, contentBox: box,
                             props: node.props, style: node.style.mapValues(\.any), children: node.children, lines: lines)
        }
        return Snapshot(size: size, roots: roots, nodes: nodes)
    }
}
