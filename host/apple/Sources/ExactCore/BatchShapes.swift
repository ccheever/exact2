// The shapes emitted by host/apple/src/batch.rs. This is a consumer, not a
// second style schema; unknown rows/keys keep their existing meaning.
import Foundation

/// One ordinary task's native timer; generations reject delivery after cancellation.
public struct NativeTimerWake: Equatable, Sendable {
    public let index: UInt32
    public let generation: UInt64
    public let due: Double
    public let interval: UInt32
    public let tolerance: UInt32
    public let once: Bool
    public init(index: UInt32, generation: UInt64, due: Double, interval: UInt32, tolerance: UInt32, once: Bool) {
        self.index = index; self.generation = generation; self.due = due
        self.interval = interval; self.tolerance = tolerance; self.once = once
    }
}

extension BatchReader {
    /// A `canvas2d` op's lists, `[address, length]` pairs in the runtime's
    /// memory (LLP 1056 D4), copied out now: the runtime keeps them alive
    /// only until its next batch with lists.
    package static func canvasLists(_ value: Any?) -> [Data] {
        (value as? [Any] ?? []).compactMap { pair in
            guard let n = pair as? [Any], n.count == 2, let at = (n[0] as? NSNumber)?.uintValue,
                  let count = (n[1] as? NSNumber)?.intValue, count >= 0,
                  let from = UnsafeRawPointer(bitPattern: at) else { return nil }
            return Data(bytes: from, count: count)
        }
    }

    package mutating func batch() throws -> Batch {
        var ops: [BatchOp] = [], timers = false, motion = false, pending = false, spatial = false, canvas = false, frames = false, canvasOwed = false, controls = false
        var layoutProvisional = false, busy = false, requests = false
        var clock: Double?, due: Double?, error: String?
        var waits: [Double] = [], waitContinue = false
        var nativeTimers: [NativeTimerWake]?
        var images: [String] = []
        var seq: (UInt64, UInt64)?
        var alerts: [String] = []
        var journal: [String] = []
        var modes: [String: String]?
        var live: String?, unsupported = false, design: String?, facts = false, wakelock: Bool?, orientation: OrientationRequest?
        var unchanged = false, storeReadPending = false, storeActivationPending = false, landing = false
        var seen: Set<String> = []
        try object { r, key in
            guard seen.insert(key).inserted else { try r.skip(); return }
            if try r.null() { return }
            switch key {
            case "ops": ops = try r.array { try $0.batchOp() }
            case "timers": timers = try r.bool()
            case "motion": motion = try r.bool()
            case "canvas": canvas = try r.bool()
            case "nativeTimers": nativeTimers = try r.array { try $0.nativeTimer() }
            case "waits": waits = try r.array { try $0.number() }
            case "waitContinue": waitContinue = try r.bool()
            case "frames": frames = try r.bool()
            case "canvasOwed": canvasOwed = try r.bool()
            case "controls": controls = try r.bool()
            case "layoutProvisional": layoutProvisional = try r.bool()
            case "canvasImages": images = try r.array { try $0.string() }
            case "unchanged": unchanged = try r.bool()
            case "storeReadPending": storeReadPending = try r.bool()
            case "storeActivationPending": storeActivationPending = try r.bool()
            case "pending": pending = try r.bool()
            case "requests": requests = try r.bool()
            case "busy": busy = try r.bool()
            case "spatial": spatial = try r.bool()
            case "clock": clock = try r.number()
            case "timer_due_ms": due = try r.number()
            case "landing": landing = try r.bool()
            case "error": error = try r.string()
            case "modes": if case .object(let object) = try r.value() { modes = object.compactMapValues { $0.string } }
            case "live": live = try r.string()
            case "unsupported": unsupported = try r.bool()
            case "design": design = try r.value().string
            case "facts": facts = try r.bool()
            case "wakelock": wakelock = try r.bool()
            case "orientationlock": if case .object(let o) = try r.value() { orientation = OrientationRequest(o) }
            case "alerts": alerts = try r.array { try $0.string() }
            case "journal": journal = try r.array { try $0.string() }
            case "seq":
                let range = try r.array { try $0.number() }
                if range.count == 2, range.allSatisfy({ $0 >= 0 && $0 == $0.rounded() }) { seq = (UInt64(range[0]), UInt64(range[1])) }
            default: try r.skip()
            }
        }
        var batch = Batch(ops: ops, timers: timers, motion: motion, clock: clock, error: error, timerDueMs: due, pending: pending, busy: busy)
        batch.unchanged = unchanged
        batch.landing = landing
        batch.storeReadPending = storeReadPending
        batch.storeActivationPending = storeActivationPending
        batch.spatial = spatial
        batch.requests = requests
        batch.canvas = canvas
        batch.frames = frames
        batch.timerWaits = waits
        batch.nativeTimers = nativeTimers
        batch.waitContinue = waitContinue
        batch.canvasOwed = canvasOwed
        batch.controls = controls
        batch.layoutProvisional = layoutProvisional
        batch.canvasImages = images
        batch.seq = seq
        batch.alerts = alerts
        batch.journal = journal
        batch.modes = modes
        batch.live = live
        batch.unsupported = unsupported
        batch.design = design
        batch.facts = facts
        batch.wakelock = wakelock
        batch.orientation = orientation
        return batch
    }

    private mutating func nativeTimer() throws -> NativeTimerWake {
        var index: UInt32?, generation: UInt64?, due: Double?, interval: UInt32?, tolerance: UInt32?, once: Bool?
        try object { r, key in
            switch key {
            case "index": index = try r.id()
            case "generation": generation = UInt64(try r.string())
            case "due": due = try r.number()
            case "interval": interval = try r.id()
            case "tolerance": tolerance = try r.id()
            case "once": once = try r.bool()
            default: try r.skip()
            }
        }
        guard let index, let generation, let due, due >= 0, let interval, interval > 0, let tolerance, let once else { throw Invalid.wire }
        return NativeTimerWake(index: index, generation: generation, due: due, interval: interval, tolerance: tolerance, once: once)
    }

    package mutating func batchOp() throws -> BatchOp {
        try enter(123)
        // Rust writes the discriminator first. JSON object order is not part of
        // the contract: other writers take the typed-value path below, also in
        // one pass. Only the rare adapters finally bridge their payload to Any.
        let first = try string(); try expect(58)
        if first != "op" {
            var fields = [first: try value()]
            while !take(125) {
                try expect(44)
                let key = try string(); try expect(58)
                let value = try value()
                if fields[key] == nil { fields[key] = value }
            }
            depth -= 1
            return try BatchFields(fields).op(colorChannels: colorChannels)
        }
        let name = try string()
        var op = BatchOp(op: BatchOp.Kind(rawValue: name) ?? .unknown)
        if op.isAdapter {
            var payload: NodeStyle = ["op": .string(name)]
            while !take(125) {
                try expect(44)
                let key = try string(); try expect(58)
                payload[key] = try value()
            }
            depth -= 1
            op.nodeID = try BatchFields(payload).id("id")
            op.payload = payload.mapValues(\.any)
            if op.op == .canvas2d { op.payload["lists"] = BatchReader.canvasLists(op.payload["lists"]) }
            return op
        }
        while !take(125) {
            try expect(44)
            let key = try string(); try expect(58)
            if try null() { continue }
            switch (op.op, key) {
            case (_, "id"): op.nodeID = try id()
            case (.create, "kind"): op.kind = try string()
            case (.create, "props"), (.props, "set"): op.props = try strings()
            case (.create, "style"), (.style, "style"): op.style = try style()
            case (.roles, "roles"): op.style = try values()
            case (.create, "handlers"): op.handlers = Set(try array { try $0.string() })
            case (.props, "clear"): op.clear = try array { try $0.string() }
            case (.paragraph, "runs"): op.runs = try array { try $0.inline() }
            case (.children, "ids"), (.roots, "ids"): op.ids = try array { try $0.id() }
            case (.frame, "x"), (.content, "x"), (.present, "x"): op.x = try number()
            case (.frame, "y"), (.content, "y"), (.present, "y"): op.y = try number()
            case (.frame, "w"), (.content, "w"), (.present, "w"): op.w = try number()
            case (.frame, "h"), (.content, "h"), (.present, "h"): op.h = try number()
            case (.frame, "property"), (.content, "property"), (.present, "property"): op.property = try string()
            default: try skip()
            }
        }
        depth -= 1
        return op
    }

    package mutating func inline() throws -> InlineText {
        var id: UInt32?, parent: UInt32?, props: [String: String] = [:]
        var style = InlineStyle(), handlers: Set<String> = [], paints = false
        try object { r, key in
            switch key {
            case "id": id = try r.id()
            case "parent": parent = try r.id()
            default:
                if try r.null() { return }
                switch key {
                case "props": props = try r.strings()
                case "style": style = try r.inlineStyle()
                case "handlers": handlers = Set(try r.array { try $0.string() })
                case "paint": paints = try r.bool()
                default: try r.skip()
                }
            }
        }
        guard let id, let parent else { throw Invalid.wire }
        return InlineText(id: id, parent: parent, props: props, style: style, handlers: handlers, paints: paints)
    }
    package mutating func inlineStyle() throws -> InlineStyle {
        var style = InlineStyle(), height: BatchValue?
        try object { r, key in
            switch key {
            case "font_size", "font_weight", "font_family", "font_style", "font_stretch", "font_variation_settings", "letter_spacing", "text_color", "text_decoration_line", "background_color", "font_variant_numeric",
                 "text_shadow", "text_stroke_width", "text_stroke_color", "visibility", "user_select",
                 "border_radius_top_left", "border_radius_top_right", "border_radius_bottom_right", "border_radius_bottom_left":
                try style.set(key, r.value(), colorChannels: r.colorChannels)
            case "line_height": height = try r.value()
            default: try r.skip()
            }
        }
        try style.height(height)
        return style
    }
}

extension BatchOp {
    package var isAdapter: Bool {
        switch op {
        case .create, .props, .style, .children, .paragraph, .frame, .content, .present, .roots, .destroy, .roles: return false
        default: return true
        }
    }
}

// Type checks for the uncommon out-of-order op and heterogeneous inline rows.
// Unlike a Decoder, these select a known enum case; none probes by throwing.
package struct BatchFields {
    package let fields: NodeStyle
    package init(_ fields: NodeStyle) { self.fields = fields }
    package func value(_ key: String) -> BatchValue? {
        guard let value = fields[key], value != .null else { return nil }
        return value
    }
    package static func string(_ value: BatchValue) throws -> String {
        guard case .string(let s) = value else { throw BatchReader.Invalid.wire }; return s
    }
    package static func number(_ value: BatchValue) throws -> Double {
        guard case .number(let n) = value else { throw BatchReader.Invalid.wire }; return n
    }
    package static func bool(_ value: BatchValue) throws -> Bool {
        guard case .bool(let b) = value else { throw BatchReader.Invalid.wire }; return b
    }
    package static func id(_ value: BatchValue) throws -> UInt32 {
        guard let id = UInt32(exactly: try number(value)) else { throw BatchReader.Invalid.wire }; return id
    }
    package static func object(_ value: BatchValue) throws -> NodeStyle {
        guard case .object(let o) = value else { throw BatchReader.Invalid.wire }; return o
    }
    package static func array<T>(_ value: BatchValue, _ element: (BatchValue) throws -> T) throws -> [T] {
        guard case .array(let a) = value else { throw BatchReader.Invalid.wire }; return try a.map(element)
    }
    package func string(_ key: String) throws -> String? { try value(key).map(Self.string) }
    package func number(_ key: String) throws -> Double? { try value(key).map(Self.number) }
    package func id(_ key: String) throws -> UInt32? { try value(key).map(Self.id) }
    package func bool(_ key: String) throws -> Bool? { try value(key).map(Self.bool) }
    package func object(_ key: String) throws -> NodeStyle { try value(key).map(Self.object) ?? [:] }
    package func strings(_ key: String) throws -> [String: String] { try object(key).mapValues(Self.string) }
    package func array<T>(_ key: String, _ element: (BatchValue) throws -> T) throws -> [T] {
        try value(key).map { try Self.array($0, element) } ?? []
    }
    package func inline(colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil) throws -> InlineText {
        guard let id = try id("id"), let parent = try self.id("parent") else { throw BatchReader.Invalid.wire }
        var style = InlineStyle()
        let rows = try object("style")
        for (key, value) in rows where key != "line_height" { try style.set(key, value, colorChannels: colorChannels) }
        try style.height(rows["line_height"])
        return InlineText(id: id, parent: parent, props: try strings("props"), style: style,
                          handlers: Set(try array("handlers", Self.string)), paints: try bool("paint") ?? false)
    }
    package func op(colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil) throws -> BatchOp {
        guard let name = try string("op") else { throw BatchReader.Invalid.wire }
        var op = BatchOp(op: BatchOp.Kind(rawValue: name) ?? .unknown, nodeID: try id("id"))
        switch op.op {
        case .create:
            op.kind = try string("kind") ?? "view"
            op.props = try strings("props"); op.style = try CalculatedLength.decode(object("style"))
            op.handlers = Set(try array("handlers", Self.string))
        case .props: op.props = try strings("set"); op.clear = try array("clear", Self.string)
        case .style: op.style = try CalculatedLength.decode(object("style"))
        case .paragraph: op.runs = try array("runs") { try Self(Self.object($0)).inline(colorChannels: colorChannels) }
        case .children, .roots: op.ids = try array("ids", Self.id)
        case .frame, .content, .present:
            op.x = try number("x") ?? 0; op.y = try number("y") ?? 0
            op.w = try number("w") ?? 0; op.h = try number("h") ?? 0
            op.property = try string("property") ?? ""
        case .destroy: break
        case .roles: op.style = try object("roles")
        default: op.payload = fields.mapValues(\.any)
        }
        return op
    }
}

extension InlineStyle {
    package mutating func set(_ key: String, _ value: BatchValue, colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil) throws {
        if value == .null { return }
        let channels = colorChannels ?? { $0.fallbackChannels(dark: $1) }
        if key == "text_color" { fixedInk = value.fixesInk }
        switch key {
        case "font_size": run.size = CGFloat(Float(try BatchFields.number(value)))
        case "font_weight", "font_family":
            let n = try BatchFields.number(value)
            guard n >= Double(Int.min), n < Double(Int.max) else { throw BatchReader.Invalid.wire }
            if key == "font_weight" { run.weight = Int(n) } else { run.family = Int(n) }
        case "font_style": run.italic = try BatchFields.string(value) == "italic"
        case "font_stretch": run.stretch = CGFloat(Float(try BatchFields.number(value)))
        case "font_variation_settings": run.variations = FontAxis.settings(value, keeping: run.variations)
        case "font_optical_sizing": run.variations = FontAxis.opticalSizing(try BatchFields.string(value), run.variations)
        case "letter_spacing": run.letterSpacing = CGFloat(Float(try BatchFields.number(value)))
        case "font_variant_numeric": run.numeric = Int(try BatchFields.number(value)) & 0xff
        case "visibility": run.hidden = try BatchFields.string(value) == "hidden"
        case "user_select": selectsAll = try BatchFields.string(value) == "all"
        case "text_decoration_line": run.decoration = try BatchFields.string(value)
        case "text_color" where value.isWideColor, "background_color" where value.isWideColor:
            // @ref LLP 1100 D2
            guard let light = channels(value, false), let dark = channels(value, true) else { throw BatchReader.Invalid.wire }
            paired = value.isSchemeColor
            if key == "text_color" { run.color = light; darkColor = dark } else { run.background = light; darkBackground = dark }
        case "text_color" where value.isSystemColor, "background_color" where value.isSystemColor:
            // @ref LLP 1095 D5 — a platform colour, kept by name and resolved
            // as the run is read, in its paragraph owner's traits (an inline
            // run has no view), so a contrast or level change reaches it.
            guard channels(value, false) != nil, channels(value, true) != nil else { throw BatchReader.Invalid.wire }
            paired = true
            if key == "text_color" { colorRef = value } else { backgroundRef = value }
        case "text_color":
            guard case .array(let a) = value else { throw BatchReader.Invalid.wire }
            if a.count == 2 {
                paired = true
                run.color = try BatchFields.array(a[0], BatchFields.number)
                darkColor = try BatchFields.array(a[1], BatchFields.number)
            } else {
                run.color = try a.map(BatchFields.number); darkColor = run.color
            }
            guard run.color?.count == 4, darkColor?.count == 4 else { throw BatchReader.Invalid.wire }
        case "background_color":
            guard case .array(let a) = value else { throw BatchReader.Invalid.wire }
            if a.count == 2 {
                paired = true
                run.background = try BatchFields.array(a[0], BatchFields.number)
                darkBackground = try BatchFields.array(a[1], BatchFields.number)
            } else {
                run.background = try a.map(BatchFields.number); darkBackground = run.background
            }
            guard run.background?.count == 4, darkBackground?.count == 4 else { throw BatchReader.Invalid.wire }
        case "border_radius_top_left", "border_radius_top_right", "border_radius_bottom_right", "border_radius_bottom_left":
            let corners = ["border_radius_top_left", "border_radius_top_right", "border_radius_bottom_right", "border_radius_bottom_left"]
            if run.backgroundRadii.isEmpty { run.backgroundRadii = Array(repeating: InlineRadius(), count: 4) }
            let i = corners.firstIndex(of: key)!
            run.backgroundRadii[i] = InlineRadius(value)
        default: paint.set(key, value)
        }
    }
    package mutating func height(_ value: BatchValue?) throws {
        guard let value, value != .null else { return }
        switch value {
        case .number(let ratio): run.lineHeight = CGFloat(Float(ratio) * Float(run.size))
        case .string(let s):
            if s.hasSuffix("px"), let px = Float(s.dropLast(2)) { run.lineHeight = CGFloat(px) }
        default: throw BatchReader.Invalid.wire
        }
    }
}
