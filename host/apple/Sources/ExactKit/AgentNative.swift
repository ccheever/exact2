// @ref LLP 1080.001 — the two inspection forms of `layout`, the part both
// Apple presenters share: the request's validation, the kernel's frames
// (the private `frames` message), and the agreement report's envelope.
// `layout <target> native` dumps the platform views under a node, as
// observation; `layout agree` walks what the session mounted and reports
// only the disagreements the code lets it judge soundly (D2): `stray`,
// `parked-visible`, `leaving-interactive`, `frame`, `hidden`. The walks are
// the platform's (`AgentNativeIOS.swift`, `AgentNativeMac.swift`).
import Foundation
import CoreFoundation
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

/// One kernel node from `frames`: the parent-relative frame the host is sent.
struct KernelFrame {
    let id: UInt32
    let parent: UInt32?
    let rect: CGRect
    let bits: Int
    var displayNone: Bool { bits & 1 != 0 }
    var transformed: Bool { bits & 2 != 0 }
    var hostOwned: Bool { bits & 4 != 0 }
    var inlineRun: Bool { bits & 8 != 0 }
}

/// The kernel's frames by id and in preorder, with the epoch they were read
/// at and whether `frames` stopped at its cap.
struct KernelFrames {
    var frames: [UInt32: KernelFrame] = [:]
    var order: [UInt32] = []
    var epoch = -1
    var complete = true
    init() {}
    init(_ rows: [KernelFrame]) { for f in rows { frames[f.id] = f; order.append(f.id) } }
}

/// What `layout agree` found, accumulated by a walk and answered bounded.
final class AgreementReport {
    static let kinds = ["stray", "parked-visible", "leaving-interactive", "frame", "hidden"]
    static let walkCap = 20_000
    var incomplete: [String] = []
    var walked: [String] = [], excluded: [String] = []
    var judged = 0, opaque = 0, views = 0
    /// Of the opaque, those inside a view a hatch's region binds, by its sentence (LLP 1075.003.000.001 §3.4).
    var opaqueBy: [String: Int] = [:]
    var parkedRoots = 0, leaving = 0
    var compared = 0, sizeOnly = 0
    var skipped: [String: Int] = [:]
    var hiddenCompared = 0
    var claimed: [String: Int] = [:]
    var counts: [String: Int] = [:]
    var found: [[String: Any]] = []
    let tolerance: CGFloat

    init(tolerance: CGFloat) { self.tolerance = tolerance }

    func skip(_ reason: String) { skipped[reason, default: 0] += 1 }
    func claim(_ hider: String) { claimed[hider, default: 0] += 1 }
    func add(_ kind: String, _ fields: [String: Any]) {
        counts[kind, default: 0] += 1
        var entry = fields
        entry["kind"] = kind
        found.append(entry)
    }
    func incomplete(_ reason: String) { if !incomplete.contains(reason) { incomplete.append(reason) } }

    static func rect(_ r: CGRect) -> [String: Any] { ["x": Agent.r2(r.minX), "y": Agent.r2(r.minY), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }

    /// A model frame against the kernel's: every edge (or, size only, the
    /// width and height) within one physical pixel.
    func compare(_ id: UInt32, kernel: CGRect, native: CGRect, sizeOnly: Bool) {
        if sizeOnly { self.sizeOnly += 1 } else { compared += 1 }
        let t = tolerance + 0.001
        let off = sizeOnly
            ? abs(native.width - kernel.width) > t || abs(native.height - kernel.height) > t
            : abs(native.minX - kernel.minX) > t || abs(native.minY - kernel.minY) > t
                || abs(native.maxX - kernel.maxX) > t || abs(native.maxY - kernel.maxY) > t
        guard off else { return }
        var fields: [String: Any] = ["id": Int(id), "kernel": Self.rect(kernel), "native": Self.rect(native),
                                     "delta": ["x": Agent.r2(native.minX - kernel.minX), "y": Agent.r2(native.minY - kernel.minY),
                                               "w": Agent.r2(native.width - kernel.width), "h": Agent.r2(native.height - kernel.height)]]
        if sizeOnly { fields["sizeOnly"] = true }
        add("frame", fields)
    }

    func json(limit: Int) -> [String: Any] {
        // The table's order, then id: a reader sees the same list twice.
        let order = Dictionary(uniqueKeysWithValues: Self.kinds.enumerated().map { ($1, $0) })
        let sorted = found.sorted {
            let a = order[$0["kind"] as? String ?? ""] ?? 99, b = order[$1["kind"] as? String ?? ""] ?? 99
            if a != b { return a < b }
            return ($0["id"] as? Int ?? $0["under"] as? Int ?? 0) < ($1["id"] as? Int ?? $1["under"] as? Int ?? 0)
        }
        var out: [String: Any] = [
            "complete": incomplete.isEmpty,
            "incomplete": incomplete,
            "roots": ["walked": walked, "excluded": excluded],
            "coverage": [
                "stray": opaqueBy.isEmpty ? ["judged": judged, "opaque": opaque] : ["judged": judged, "opaque": opaque, "opaqueBy": ["hatch": opaqueBy]],
                "kept": ["parkedRoots": parkedRoots, "leaving": leaving],
                "frame": ["compared": compared, "sizeOnly": sizeOnly, "skipped": skipped],
                "hidden": ["compared": hiddenCompared, "claimed": claimed],
            ] as [String: Any],
            "counts": counts,
            "disagreements": Array(sorted.prefix(limit)),
            "tolerance": ["px": Agent.r2(tolerance)],
        ]
        if sorted.count > limit { out["truncated"] = ["disagreements"] }
        return out
    }
}

extension Agent {
    /// A JSON integer: a number with no fraction, never a Boolean.
    static func integer(_ value: Any?) -> Int? {
        guard let n = value as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID() else { return nil }
        let d = n.doubleValue
        guard d.isFinite, d.rounded() == d, abs(d) < 1e9 else { return nil }
        return Int(d)
    }

    /// The inspection forms of `layout` (LLP 1080.001 D1, D2), validated
    /// strictly; nil for an ordinary `layout` request.
    func inspectLayout(_ req: [String: Any]) -> [String: Any]? {
        let pick = req["world"] != nil || req["x"] != nil || req["y"] != nil
        if let native = req["native"] {
            guard let spec = native as? [String: Any] else { return ["error": "layout native: `native` must be an object"] }
            if let extra = spec.keys.first(where: { $0 != "depth" && $0 != "limit" }) { return ["error": "layout native: unknown field \(extra)"] }
            guard let id = Agent.integer(req["id"]), id >= 0 else { return ["error": "layout native needs a target"] }
            if req["agree"] != nil || pick { return ["error": "layout native: refused with agree, at or world"] }
            let depth = spec["depth"] == nil ? 3 : Agent.integer(spec["depth"]), limit = spec["limit"] == nil ? 200 : Agent.integer(spec["limit"])
            guard let depth, (1...12).contains(depth) else { return ["error": "layout native: depth must be an integer in 1...12"] }
            guard let limit, (1...2000).contains(limit) else { return ["error": "layout native: limit must be an integer in 1...2000"] }
            return nativeSubviews(UInt32(id), depth: depth, limit: limit, plan: req["plan"] as? Bool == true)
        }
        if let agree = req["agree"] {
            guard let flag = agree as? NSNumber, CFGetTypeID(flag) == CFBooleanGetTypeID(), flag.boolValue else { return ["error": "layout agree: `agree` must be true"] }
            if req["id"] != nil || pick { return ["error": "layout agree: refused with a target, at or world (no scoped form)"] }
            let limit = req["limit"] == nil ? 32 : Agent.integer(req["limit"])
            guard let limit, (1...500).contains(limit) else { return ["error": "layout agree: limit must be an integer in 1...500"] }
            return agreement(limit: limit)
        }
        return nil
    }

    /// The kernel's frames (the private `frames` message), with the tags
    /// they were read at.
    func kernelFrames() -> KernelFrames? {
        guard let d = session.agent("{\"op\":\"frames\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
              let rows = o["nodes"] as? [[Any]] else { return nil }
        var out = KernelFrames(rows.compactMap { row -> KernelFrame? in
            guard row.count == 7, let id = Agent.integer(row[0]) else { return nil }
            let n = row.dropFirst(2).map { ($0 as? NSNumber)?.doubleValue ?? 0 }
            return KernelFrame(id: UInt32(id), parent: Agent.integer(row[1]).map(UInt32.init), rect: CGRect(x: n[0], y: n[1], width: n[2], height: n[3]), bits: Int(n[4]))
        })
        out.epoch = Agent.integer(o["epoch"]) ?? -1
        out.complete = o["complete"] as? Bool ?? false
        return out
    }

    /// The runner's epoch now: a walk that spans a commit says so (`spanned`).
    func currentEpoch() -> Int {
        guard let d = session.agent("{\"op\":\"tags\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return -2 }
        return Agent.integer(o["epoch"]) ?? -2
    }

    /// The runner's half of `layout <id>`, or its refusal: liveness first,
    /// so only a retired id is ever called stale (D1).
    func runnerNode(_ id: UInt32, plan: Bool = false) -> [String: Any] {
        guard let d = session.agent("{\"op\":\"node\",\"id\":\(id),\"plan\":\(plan)}").data(using: .utf8),
              let node = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] else { return ["error": "node #\(id): unreadable"] }
        return node
    }
}
