import ExactCore
import CExact
import Foundation

extension Runtime {
    convenience init() {
        self.init(hooks: RuntimeHooks(
            decodeBatch: { Batch.decode($0, resolver: $1) },
            prepareBatch: { $0.prepare() }, symbolMeasure: SymbolMeasure.measure,
            prewarmIslands: { svg in
                if svg & 1 != 0 { SvgRasterModule.prewarm() }
                #if os(iOS) || os(tvOS)
                if svg & 2 != 0 { SvgFilterMetal.prewarm() }
                #endif
            }))
    }

    func holdBegin(_ view: UInt32, property: UInt32, now: Double) -> (NativeHold?, Batch) {
        if holds() { return (nil, Runtime.heldBatch) } // (`on(busy:)` doesn't hold: ResetGate)
        return on(busy: (nil, Runtime.busy)) {
            let batch = read(exact_hold_begin(rt, view, property, now))
            let start = batch.ops.first { $0.op == .hold }.flatMap { NativeHold($0.payload) }
            return (start, batch)
        }
    }
    package func heightDragBegin(_ handleKey: UInt64, targetKey: UInt64, now: Double) -> (NativeHold?, Batch) {
        if holds() { return (nil, Runtime.heldBatch) }
        return on(busy: (nil, Runtime.busy)) {
            let batch = read(exact_height_drag_begin(rt, handleKey, targetKey, now))
            return (batch.ops.first { $0.op == .hold }.flatMap { NativeHold($0.payload) }, batch)
        }
    }
    /// A `change`; a text field's carries its selection as the edit left
    /// it (kind 41, x2apps codeedit #2), else the value alone (kind 1).
    func change(_ view: UInt32, _ value: String, selection: FieldSelection? = nil, now: Double) -> Batch {
        return on {
            let n = write(selection?.payload(value) ?? value)
            return read(exact_dispatch(rt, view, selection == nil ? 1 : 41, n, now))
        }
    }
    /// A text field's value as it moves: HTML's `input` (LLP 1069.001 D4),
    /// with its selection when the host has one (kind 40, else 23).
    func input(_ view: UInt32, _ value: String, selection: FieldSelection? = nil, now: Double) -> Batch {
        return on {
            let n = write(selection?.payload(value) ?? value)
            return read(exact_dispatch(rt, view, selection == nil ? 23 : 40, n, now))
        }
    }
    /// The scripted replacement's one input, with its DOM inputType (kind 48).
    func replacementInput(_ view: UInt32, _ value: String, _ selection: FieldSelection, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 48, write(selection.payload(value)), now)) }
    }
    /// A text field's `select` (kind 42, x2apps codeedit #2): its value and
    /// the selection it now has.
    func fieldSelect(_ view: UInt32, _ value: String, _ selection: FieldSelection, now: Double) -> Batch {
        on { read(exact_dispatch(rt, view, 42, write(selection.payload(value)), now)) }
    }
    /// The wall clock owns time, or the agent's clock does (the timers RFC, D1).
    func liveClock(_ yes: Bool) { setLiveClock(yes ? { ExactEnv.wall() } : nil) }
    /// The posture and the viewport segments (LLP 1078 D4): the rects as `x y w h` floats in the input buffer, none for one segment.
    func segments(_ fold: ViewportFold) -> Batch {
        on {
            var data = Data(capacity: fold.rects.count * 16)
            for r in fold.rects { for v in [Float(r.minX), Float(r.minY), Float(r.width), Float(r.height)] { withUnsafeBytes(of: v.bitPattern.littleEndian) { data.append(contentsOf: $0) } } }
            _ = write(data)
            return read(exact_segments(rt, fold.posture == "folded" ? 1 : 0, UInt32(fold.cols), UInt32(fold.rows), UInt32(fold.rects.count)))
        }
    }
    /// @ref LLP 1095 D1 — every colour reference the kernel resolves itself
    /// (paint motion, gradients, SVG scenes), resolved by the platform in
    /// both appearances under the current contrast and reported, so none of
    /// them is a frozen fallback. A name the platform lacks is left out: the
    /// kernel keeps its fallback pair. `tint` is the app's tint,
    /// light then dark (`SystemColor.tintPair`), read on main: the owner
    /// thread has no window to read it from; without one `@tint` is not
    /// reported, and the kernel keeps what it had.
    func reportColors(tint: [[Double]]?) -> Batch {
        on {
            let len = exact_color_references(rt)
            let json = Data(bytes: exact_out(rt), count: Int(len))
            let refs = (try? JSONSerialization.jsonObject(with: json)) as? [[Any]] ?? []
            var bytes = Data(capacity: refs.count * 16)
            for r in refs {
                guard r.count == 3, let kind = r[0] as? Int, let id = r[1] as? Int, let name = r[2] as? String,
                      tint != nil || !name.hasPrefix("@tint") else { continue }
                for dark in [false, true] {
                    guard let c = SystemColor.channels(name, dark: dark, tint: tint?[dark ? 1 : 0], fallback: nil), c.count == 4 else { continue }
                    bytes.append(UInt8(kind)); bytes.append(dark ? 1 : 0)
                    withUnsafeBytes(of: UInt16(id).littleEndian) { bytes.append(contentsOf: $0) }
                    for v in c { bytes.append(UInt8(max(0, min(255, v.rounded())))) }
                }
            }
            return read(exact_colors(rt, write(bytes)))
        }
    }
    /// A button's face, custom or native (LLP 1069.011.000 D1).
    func buttonFace(_ view: UInt32) -> ButtonFace {
        return on(busy: ButtonFace()) {
            let len = exact_press_face(rt, view)
            return ButtonFace(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    /// A select's options and the one it shows (LLP 1069.001 D5).
    func selectOptions(_ view: UInt32) -> SelectMenu {
        return on(busy: SelectMenu(json: Data())) {
            let len = exact_select_options(rt, view)
            return SelectMenu(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    /// A radio's group and the radios its arrows move to (x2apps survey #2).
    func radioGroup(_ view: UInt32) -> RadioGroup {
        return on(busy: RadioGroup(json: Data())) {
            let len = exact_radio_group(rt, view)
            return RadioGroup(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    /// A navigation root's tabs by the kernel's rule (LLP 1115.001 D11); nil for one stack.
    func rootTabs(_ view: UInt32) -> RootTabs? {
        return on(busy: nil) {
            let len = exact_root_tabs(rt, view)
            return RootTabs(json: Data(bytes: exact_out(rt), count: Int(len)))
        }
    }
    /// What native containers cover of boxes (LLP 1075.003 §3.5), under one
    /// layout: `edges` add to a box's padding; `whole` lays it out as
    /// `display: none`; nil clears.
    func covers(_ covers: [(UInt32, HostCover?)], owners: [(UInt32, OwnerValues?)] = []) -> Batch {
        return on {
            var bytes = Data(capacity: covers.count * 24 + owners.count * 188)
            func append(_ words: [UInt32]) {
                for word in words { withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) } }
            }
            func floats(_ values: [CGFloat]) -> [UInt32] { values.map { Float($0).bitPattern } }
            for (view, cover) in covers {
                let (kind, edges): (UInt32, [CGFloat]) = switch cover {
                case .none: (0, [0, 0, 0, 0])
                case .edges(let e)?: (1, [e.top, e.right, e.bottom, e.left])
                case .whole?: (2, [0, 0, 0, 0])
                case .inset(let top, let bottom)?: (3, [top, 0, bottom, 0])
                case .gutter(let right, let bottom)?: (12, [0, right, bottom, 0])
                }
                append([view, kind] + floats(edges))
            }
            // Kind 16, a content owner's values; 17 takes them back (exact.h).
            for (view, values) in owners {
                guard let v = values else { append([view, 17, 0, 0, 0, 0]); continue }
                let rects = v.fold.cols * v.fold.rows > 1 ? v.fold.rects : []
                let segments = rects.prefix(4).flatMap { [$0.minX, $0.minY, $0.width, $0.height] }
                let regions = v.regions.flatMap { $0.count == 20 ? $0 : nil }
                append([view, 16] + floats(v.safe) + floats([v.size.width, v.size.height]) + [UInt32(v.fold.cols), UInt32(v.fold.rows)]
                       + floats(segments + Array(repeating: 0, count: 16 - segments.count))
                       + [regions == nil ? 0 : 1] + floats(regions ?? Array(repeating: 0, count: 20)))
            }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
    /// The margins UIKit's own list gives each grouped list's cards and
    /// cells, under one layout: the card's left and right insets, then the
    /// cell's left and right margins; nil clears them.
    package func groupedMargins(_ lists: [(UInt32, [CGFloat]?)]) -> Batch {
        return on {
            var bytes = Data(capacity: lists.count * 24)
            for (view, margins) in lists {
                let lengths = (margins ?? [0, 0, 0, 0]).map { Float($0).bitPattern }
                for word in [view, margins == nil ? 9 : 8] + lengths {
                    withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
                }
            }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
    /// The margin UIKit's cell gives each cells list's rows at its left and
    /// right, under one layout (LLP 1138); nil clears it.
    package func listMargins(_ lists: [(UInt32, [CGFloat]?)]) -> Batch {
        return on {
            var bytes = Data(capacity: lists.count * 24)
            for (view, margins) in lists {
                let lengths = ((margins ?? [0, 0]) + [0, 0]).map { Float($0).bitPattern }
                for word in [view, margins == nil ? 11 : 10] + lengths {
                    withUnsafeBytes(of: word.littleEndian) { bytes.append(contentsOf: $0) }
                }
            }
            return read(exact_host_covers(rt, write(bytes)))
        }
    }
    func regionComplete(_ artifact: RegionArtifact) -> Batch {
        return on {
            let p = artifact.metadata
            let retained = Unmanaged.passRetained(artifact).toOpaque()
            return read(exact_region_complete(rt, artifact.id,
                ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline)),
                retained, { pointer in
                    if let pointer { Unmanaged<RegionArtifact>.fromOpaque(pointer).release() }
                }))
        }
    }
}
