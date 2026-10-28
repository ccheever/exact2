// @exact/gamepad on iOS, iPadOS, macOS and tvOS (LLP 1133.022 D1-D8, D12):
// GameController's pads, numbered by the process's slot table
// (`ExactPads`), which a canvas's pads share, so an index names one pad to
// both.
//
// - The list: each slot's `GCController`, `mapping` `standard` when it has
//   the extended profile. `family` is by `productCategory`; `labels` are
//   each element's `localizedName`, which follows a person's remapping in
//   Settings, the family's word where an element has none; `apple.symbols`
//   each element's `sfSymbolsName`. `current` is `GCController.current`.
// - A reader with `interval` also reads buttons and axes, polled at that
//   interval and announced only when they changed. The profile's
//   `valueChangedHandler` is the canvas's (one closure per profile), never
//   this module's.
// - Effects and lights are claims the process keeps, whichever session made
//   them (D4): a new effect on a pad preempts the one playing there; a
//   disconnect ends every claim on the pad; a session's `destroy`, or its
//   `setLight()`, gives back only its own. `dual-rumble` plays the left
//   handle's Core Haptics engine at the strong magnitude and the right's at
//   the weak (one engine at the larger where the pad has only `handles`);
//   `trigger-rumble` the triggers'. Its span runs on the session's clock.
// - A light shows the newest surviving claim's colour through
//   `GCDeviceLight`, and the colour it read before its first write when
//   none survives or the app is in the background.
// - Under the agent (`ExactPads.driven`) the list is the drive's pads alone
//   (`EXACT_AGENT_PADS`): snapshot controllers in the slot table, announced
//   to no canvas, whose motors and light are a recorder journaled by
//   `context.log`. No pad attached to the machine is read or moved.
import CoreHaptics
import ExactPackage
import Foundation
import GameController

public final class GamepadModule: ExactPackage {
    /// The app's access hatch (D8): each pad's `GCController` when it takes a
    /// slot, before its first report, and every connected pad when set.
    public static var hatch: ((GCController) -> Void)? {
        didSet { if let hatch { Registry.shared?.pads.values.filter { $0.injected == nil }.forEach { hatch($0.controller) } } }
    }

    fileprivate let context: ExactPackageContext
    private let registry: Registry?
    /// The interval this session's reader asked for, ms; nil without one.
    fileprivate var interval: Double?
    fileprivate var shown = true

    public init(context: ExactPackageContext) {
        self.context = context
        registry = context.agent ? nil : context.pads.map { Registry.join($0, after: context.after) }
        registry?.clients.append(Weak(self))
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        // Under the agent the declared substitute answers; nothing here may reach a pad.
        guard !context.agent else { return reply(["error": "NotSupportedError"]) }
        guard let registry else {
            return reply(name == "getGamepads" ? ["available": false, "gamepads": [Any](), "current": NSNull()] : ["error": "NotSupportedError"])
        }
        // A send without `index` is for the pad in use when it arrives (D2).
        let index = (args["index"] as? Double).map(Int.init) ?? registry.current
        let named = index.flatMap { registry.pads[$0] != nil ? $0 : nil }
        switch name {
        case "getGamepads":
            if let ms = args["interval"] as? Double { interval = max(50, ms); registry.poll() }
            reply(registry.list(values: args["interval"] is Double, now: context.now()))
        case "playEffect":
            guard let named else { return reply(["error": "NotFoundError"]) }
            if let code = registry.play(self, named, args, done: { reply(["result": $0]) }) { reply(["error": code]) }
        case "resetEffects":
            guard let named else { return reply(["error": "NotFoundError"]) }
            registry.reset(named)
            reply(["result": "complete"])
        case "setLight":
            let rgb = (args["color"] as? [String: Any]).map { c in ["r", "g", "b"].map { (c[$0] as? Double ?? 0) / 255 } }
            // Without `index` or `color`: every light this session holds, given back.
            if rgb == nil && args["index"] == nil { registry.giveBack(self, nil); return reply([:]) }
            guard let named else { return reply(["error": "NotFoundError"]) }
            guard registry.pads[named]?.light == true else { return reply(["error": "NotSupportedError"]) }
            if let rgb { registry.light(self, named, rgb) } else { registry.giveBack(self, named) }
            reply([:])
        default:
            reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        guard !context.agent, name == "getGamepadsSync" else { return ["error": "NotSupportedError"] }
        guard let registry else { return ["available": false, "gamepads": [Any](), "current": NSNull()] }
        return registry.list(values: args["interval"] is Double, now: context.now())
    }

    public func release(_ name: String) {
        if name == "getGamepads" { interval = nil; registry?.poll() }
    }

    public func foreground(_ shown: Bool) {
        self.shown = shown
        registry?.shownChanged()
    }

    public func destroy(_ done: @escaping () -> Void) {
        registry?.leave(self)
        done()
    }
}

fileprivate struct Weak {
    weak var module: GamepadModule?
    init(_ module: GamepadModule) { self.module = module }
}

/// D2's labels by family, by standard index: the interface's `tables.labels`.
private let familyLabels: [String: [String]] = [
    "xbox": ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "View", "Menu", "LS", "RS", "Up", "Down", "Left", "Right", "Xbox"],
    "playstation": ["Cross", "Circle", "Square", "Triangle", "L1", "R1", "L2", "R2", "Create", "Options", "L3", "R3", "Up", "Down", "Left", "Right", "PS"],
    "nintendo": ["B", "A", "Y", "X", "L", "R", "ZL", "ZR", "−", "+", "LS", "RS", "Up", "Down", "Left", "Right", "Home"],
    "generic": ["A", "B", "X", "Y", "L1", "R1", "L2", "R2", "Select", "Start", "L3", "R3", "Up", "Down", "Left", "Right", "Home"],
]

/// The interface's `tables.apple` symbols by family: a drive's pad's, since a
/// snapshot controller names only the generic profile's.
private let familySymbols: [String: [String]] = [
    "xbox": ["a.circle", "b.circle", "x.circle", "y.circle", "lb.rectangle.roundedbottom", "rb.rectangle.roundedbottom", "lt.rectangle.roundedtop", "rt.rectangle.roundedtop", "rectangle.fill.on.rectangle.fill", "line.3.horizontal.circle", "l.joystick.press.down", "r.joystick.press.down", "dpad.up.filled", "dpad.down.filled", "dpad.left.filled", "dpad.right.filled", "logo.xbox"],
    "playstation": ["xmark.circle", "circle.circle", "square.circle", "triangle.circle", "l1.rectangle.roundedbottom", "r1.rectangle.roundedbottom", "l2.rectangle.roundedtop", "r2.rectangle.roundedtop", "", "line.3.horizontal.circle", "l.joystick.press.down", "r.joystick.press.down", "dpad.up.filled", "dpad.down.filled", "dpad.left.filled", "dpad.right.filled", "logo.playstation"],
    "nintendo": ["b.circle", "a.circle", "y.circle", "x.circle", "l.rectangle.roundedbottom", "r.rectangle.roundedbottom", "zl.rectangle.roundedtop", "zr.rectangle.roundedtop", "minus.circle", "plus.circle", "l.joystick.press.down", "r.joystick.press.down", "dpad.up.filled", "dpad.down.filled", "dpad.left.filled", "dpad.right.filled", "house.circle"],
    "generic": ["a.circle", "b.circle", "x.circle", "y.circle", "l1.rectangle.roundedbottom", "r1.rectangle.roundedbottom", "l2.rectangle.roundedtop", "r2.rectangle.roundedtop", "", "line.3.horizontal.circle", "l.joystick.press.down", "r.joystick.press.down", "dpad.up.filled", "dpad.down.filled", "dpad.left.filled", "dpad.right.filled", "house.circle"],
]

/// The face buttons' scheme from GameController's `productCategory`, or a
/// drive's USB vendor id.
private func familyOf(category: String, vendor: String) -> String {
    switch vendor.lowercased() {
    case "045e": return "xbox"
    case "054c": return "playstation"
    case "057e": return "nintendo"
    default: break
    }
    let c = category.lowercased()
    if c.contains("dualsense") || c.contains("dualshock") || c.contains("playstation") { return "playstation" }
    if c.contains("xbox") { return "xbox" }
    if c.contains("switch") || c.contains("joy-con") || c.contains("nintendo") { return "nintendo" }
    return "generic"
}

/// `%g`: 1 and 0.5, as the other hosts' recorders write them.
private func number(_ x: Double) -> String { String(format: "%g", x) }

/// One pad in a slot.
fileprivate final class Pad {
    let controller: GCController
    /// Which connection this is: a pad that returns is a new one.
    let connection: Int
    /// A drive's pad: its identity, and a recorder for its motors and light.
    let injected: [String: Any]?
    var engines: [GCHapticsLocality: CHHapticEngine] = [:]
    /// The light's colour before this module's first write.
    var platformColor: GCColor?

    init(_ controller: GCController, connection: Int, injected: [String: Any]?) {
        self.controller = controller; self.connection = connection; self.injected = injected
    }

    var profile: GCExtendedGamepad? { controller.extendedGamepad }
    var category: String { injected?["category"] as? String ?? controller.productCategory }
    var family: String { familyOf(category: category, vendor: injected?["vendor"] as? String ?? "") }
    var localities: Set<GCHapticsLocality> { controller.haptics?.supportedLocalities ?? [] }
    var effects: [String] {
        if injected != nil { return ["dual-rumble"] }
        let l = localities
        var out: [String] = []
        if l.contains(.leftHandle) && l.contains(.rightHandle) || l.contains(.handles) { out.append("dual-rumble") }
        if l.contains(.leftTrigger) && l.contains(.rightTrigger) { out.append("trigger-rumble") }
        return out
    }
    var light: Bool { injected != nil || controller.light != nil }

    /// The 17 standard elements, by index.
    var elements: [GCControllerButtonInput?] {
        guard let p = profile else { return Array(repeating: nil, count: 17) }
        return [p.buttonA, p.buttonB, p.buttonX, p.buttonY, p.leftShoulder, p.rightShoulder,
                p.leftTrigger, p.rightTrigger, p.buttonOptions, p.buttonMenu,
                p.leftThumbstickButton, p.rightThumbstickButton,
                p.dpad.up, p.dpad.down, p.dpad.left, p.dpad.right, p.buttonHome]
    }

    func shape(_ index: Int, values: Bool, now: Double) -> [String: Any] {
        let table = familyLabels[family] ?? familyLabels["generic"]!, elements = self.elements
        // A drive's pad says the family's words: a snapshot controller has none of a pad's own.
        let labels = elements.enumerated().map { i, e in injected == nil ? e?.localizedName ?? table[i] : table[i] }
        let name = injected?["id"] as? String ?? controller.vendorName ?? category
        var out: [String: Any] = [
            "index": index, "id": name, "mapping": profile == nil ? "" : "standard", "family": family,
            "labels": labels, "effects": effects, "light": light,
            "buttons": NSNull(), "axes": NSNull(), "timestamp": NSNull(),
            "apple": ["symbols": injected == nil ? elements.map { $0?.sfSymbolsName ?? "" } : familySymbols[family] ?? familySymbols["generic"]!,
                      "productCategory": category, "vendorName": name],
            "android": NSNull(), "windows": NSNull(), "linux": NSNull(),
        ]
        if values, let p = profile {
            out["buttons"] = elements.map { b -> [String: Any] in
                ["pressed": b?.isPressed ?? false, "touched": b?.isTouched ?? false, "value": Double(b?.value ?? 0)]
            }
            out["axes"] = [p.leftThumbstick.xAxis.value, -p.leftThumbstick.yAxis.value,
                           p.rightThumbstick.xAxis.value, -p.rightThumbstick.yAxis.value].map { Double($0) + 0 }
            out["timestamp"] = now
        }
        return out
    }

    /// The engine for `locality`, started; nil where the pad has none.
    func engine(_ locality: GCHapticsLocality) -> CHHapticEngine? {
        if let e = engines[locality] { return e }
        guard let e = controller.haptics?.createEngine(withLocality: locality) else { return nil }
        e.stoppedHandler = { [weak self] _ in DispatchQueue.main.async { self?.engines[locality] = nil } }
        e.resetHandler = { [weak self] in DispatchQueue.main.async { self?.engines[locality] = nil } }
        do { try e.start() } catch { return nil }
        engines[locality] = e
        return e
    }

    /// Each engine's level for an effect: `dual-rumble`'s strong and weak
    /// motors, or `trigger-rumble`'s left and right triggers.
    func motors(_ type: String, _ a: Double, _ b: Double) -> [(GCHapticsLocality, Double)] {
        let l = localities
        if type == "trigger-rumble" { return [(.leftTrigger, a), (.rightTrigger, b)] }
        if l.contains(.leftHandle) && l.contains(.rightHandle) { return [(.leftHandle, a), (.rightHandle, b)] }
        return [(.handles, max(a, b))]
    }
}

/// An effect a session plays on a pad.
fileprivate final class Effect {
    let client: ObjectIdentifier, index: Int, connection: Int
    var done: ((String) -> Void)?
    var timers: [() -> Void] = []
    var players: [CHHapticPatternPlayer] = []
    var started = false
    init(client: ObjectIdentifier, index: Int, connection: Int, done: @escaping (String) -> Void) {
        self.client = client; self.index = index; self.connection = connection; self.done = done
    }
}

/// A session's colour on a pad.
fileprivate struct Light {
    let client: ObjectIdentifier, index: Int, connection: Int
    let rgb: [Double]
    let order: Int
}

/// The process's pads and claims (D4): one, whichever sessions read them.
fileprivate final class Registry {
    static var shared: Registry?
    let slots: ExactPads
    var pads: [Int: Pad] = [:]
    var current: Int?
    var clients: [Weak] = []
    private var effects: [Effect] = []
    private var lights: [Light] = []
    private var connections = 0, order = 0
    private var hidden = false
    private var timer: Timer?
    private var lastValues: [String: String] = [:]

    /// The process's registry, made by its first session; `after` is that
    /// session's clock, on which a drive's pad leaves.
    static func join(_ slots: ExactPads, after: @escaping (Double, @escaping () -> Void) -> () -> Void) -> Registry {
        if let r = shared { return r }
        let r = Registry(slots, after: after)
        shared = r
        return r
    }

    private init(_ slots: ExactPads, after: (Double, @escaping () -> Void) -> () -> Void) {
        self.slots = slots
        if slots.driven {
            for pad in slots.injected { inject(pad, after: after) }
            return
        }
        let center = NotificationCenter.default
        center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
            (note.object as? GCController).map { self?.connect($0) }
        }
        center.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] note in
            (note.object as? GCController).map { self?.disconnect($0) }
        }
        center.addObserver(forName: .GCControllerDidBecomeCurrent, object: nil, queue: .main) { [weak self] _ in self?.followCurrent() }
        for c in GCController.controllers() { connect(c, quietly: true) }
        followCurrent(quietly: true)
    }

    private var modules: [GamepadModule] {
        clients.removeAll { $0.module == nil }
        return clients.compactMap(\.module)
    }

    func list(values: Bool, now: Double) -> [String: Any] {
        let listed = pads.keys.sorted().map { pads[$0]!.shape($0, values: values, now: now) }
        return ["available": true, "gamepads": listed, "current": current.map { $0 as Any } ?? NSNull()]
    }

    // MARK: pads coming and going

    private func connect(_ controller: GCController, quietly: Bool = false) {
        guard !pads.values.contains(where: { $0.controller === controller }), let index = slots.take(controller) else { return }
        connections += 1
        let pad = Pad(controller, connection: connections, injected: nil)
        pads[index] = pad
        GamepadModule.hatch?(controller)
        if !quietly { announce(came: [index]) }
    }

    private func disconnect(_ controller: GCController) {
        guard let index = pads.first(where: { $0.value.controller === controller })?.key, let pad = pads[index] else { return }
        let shape = pad.shape(index, values: false, now: 0)
        slots.free(controller)
        endDevice(index, pad)
        pads[index] = nil
        if current == index { current = nil }
        for m in modules { m.context.emit("gamepaddisconnected", [shape]) }
        for m in modules { m.context.changed("gamepad.getGamepads") }
    }

    /// A drive's pad: a snapshot controller in its slot, pressing its
    /// `pressed` buttons once, so the one pressed is current, and
    /// disconnecting `leaves` ms after the registry was made, when it says.
    private func inject(_ spec: [String: Any], after: (Double, @escaping () -> Void) -> () -> Void) {
        let controller = GCController.withExtendedGamepad()
        let wanted = (spec["index"] as? Double).map { min(Int($0), ExactPads.most - 1) }
        guard let index = slots.take(controller, at: wanted) else { return }
        connections += 1
        let pad = Pad(controller, connection: connections, injected: spec)
        pads[index] = pad
        let pressed = (spec["pressed"] as? [Double] ?? []).map(Int.init).filter { (0..<17).contains($0) }
        for b in pressed { pad.elements[b]?.setValue(1) }
        if !pressed.isEmpty { current = index }
        if let ms = spec["leaves"] as? Double { _ = after(ms) { [weak self] in self?.disconnect(controller) } }
    }

    private func announce(came: [Int]) {
        for m in modules { for i in came { if let p = pads[i] { m.context.emit("gamepadconnected", [p.shape(i, values: false, now: 0)]) } } }
        for m in modules { m.context.changed("gamepad.getGamepads") }
    }

    /// `current` changes only when another pad takes over (D2).
    private func followCurrent(quietly: Bool = false) {
        guard let c = GCController.current, let now = pads.first(where: { $0.value.controller === c })?.key, now != current else { return }
        current = now
        if !quietly { for m in modules { m.context.changed("gamepad.getGamepads") } }
    }

    // MARK: readings

    /// The poll for readers with `interval`: the shortest any asks for.
    func poll() {
        let shortest = modules.compactMap(\.interval).min()
        timer?.invalidate()
        timer = nil
        lastValues = [:]
        guard let ms = shortest else { return }
        timer = Timer.scheduledTimer(withTimeInterval: ms / 1000, repeats: true) { [weak self] _ in self?.tick() }
    }

    private func tick() {
        let readers = modules.filter { $0.interval != nil && $0.shown }
        guard !readers.isEmpty else { return }
        let seen = Dictionary(uniqueKeysWithValues: pads.map { i, p in
            let s = p.shape(i, values: true, now: 0)
            return (String(i), "\(s["buttons"] ?? "") \(s["axes"] ?? "")")
        })
        guard seen != lastValues else { return }
        lastValues = seen
        for m in readers { m.context.changed("gamepad.getGamepads") }
    }

    // MARK: effects

    /// `module` plays `args`'s effect on pad `index`, preempting the one
    /// playing there; `done` hears `complete` or `preempted`. A refusal's
    /// code when the pad cannot play it.
    func play(_ module: GamepadModule, _ index: Int, _ args: [String: Any], done: @escaping (String) -> Void) -> String? {
        guard let pad = pads[index] else { return "NotFoundError" }
        let type = args["type"] as? String ?? ""
        guard pad.effects.contains(type) else { return "NotSupportedError" }
        let d = { (k: String) in args[k] as? Double ?? 0 }
        var duration = d("duration")
        if duration > 5000 { module.context.log("gamepad.playEffect: duration \(number(duration)) capped at 5000"); duration = 5000 }
        let (a, b) = type == "trigger-rumble" ? (d("leftTrigger"), d("rightTrigger")) : (d("strongMagnitude"), d("weakMagnitude"))
        preempt(index)
        let effect = Effect(client: ObjectIdentifier(module), index: index, connection: pad.connection, done: done)
        effects.append(effect)
        let log = module.context.log
        let start = { [weak self, weak effect] in
            guard let self, let effect, self.effects.contains(where: { $0 === effect }) else { return }
            effect.started = true
            if pad.injected != nil { return log("pad \(index): rumble \(number(a)) \(number(b)) for \(number(duration)) ms") }
            effect.players = pad.motors(type, a, b).compactMap { locality, level in self.player(pad, locality, level, duration) }
        }
        let end = { [weak self, weak effect] in
            guard let self, let effect else { return }
            self.finish(effect, "complete", log: log)
        }
        let delay = max(0, d("startDelay"))
        if delay > 0 { effect.timers.append(module.context.after(delay, start)) } else { start() }
        effect.timers.append(module.context.after(delay + duration, end))
        return nil
    }

    private func player(_ pad: Pad, _ locality: GCHapticsLocality, _ level: Double, _ ms: Double) -> CHHapticPatternPlayer? {
        guard level > 0, ms > 0, let engine = pad.engine(locality) else { return nil }
        let event = CHHapticEvent(eventType: .hapticContinuous, parameters: [
            CHHapticEventParameter(parameterID: .hapticIntensity, value: Float(min(1, level))),
            CHHapticEventParameter(parameterID: .hapticSharpness, value: 0.5),
        ], relativeTime: 0, duration: ms / 1000)
        guard let pattern = try? CHHapticPattern(events: [event], parameters: []),
              let player = try? engine.makePlayer(with: pattern) else { return nil }
        do { try player.start(atTime: CHHapticTimeImmediate) } catch { return nil }
        return player
    }

    /// `effect` ends `how`: its motors stop and its sender hears it.
    private func finish(_ effect: Effect, _ how: String, log: (String) -> Void) {
        guard let i = effects.firstIndex(where: { $0 === effect }) else { return }
        effects.remove(at: i)
        effect.timers.forEach { $0() }
        if effect.started, let pad = pads[effect.index], pad.connection == effect.connection {
            if pad.injected != nil { log("pad \(effect.index): motors stopped") }
            for p in effect.players { try? p.stop(atTime: CHHapticTimeImmediate) }
        }
        let done = effect.done
        effect.done = nil
        done?(how)
    }

    private func logger(_ client: ObjectIdentifier) -> (String) -> Void {
        let m = modules.first { ObjectIdentifier($0) == client }
        return { m?.context.log($0) }
    }

    /// The effect playing on pad `index` ends, `preempted`.
    func reset(_ index: Int) { preempt(index) }

    private func preempt(_ index: Int) {
        for e in effects where e.index == index { finish(e, "preempted", log: logger(e.client)) }
    }

    /// Pad `index` went: every claim on it ends; its engines stop.
    private func endDevice(_ index: Int, _ pad: Pad) {
        for e in effects where e.connection == pad.connection { finish(e, "preempted", log: logger(e.client)) }
        lights.removeAll { $0.connection == pad.connection }
        pad.engines.values.forEach { $0.stop() }
        pad.engines = [:]
    }

    // MARK: lights

    func light(_ module: GamepadModule, _ index: Int, _ rgb: [Double]) {
        guard let pad = pads[index] else { return }
        let client = ObjectIdentifier(module)
        lights.removeAll { $0.client == client && $0.index == index }
        order += 1
        lights.append(Light(client: client, index: index, connection: pad.connection, rgb: rgb, order: order))
        show(index, log: module.context.log)
    }

    /// `module`'s light on pad `index`, or on every pad, given back.
    func giveBack(_ module: GamepadModule, _ index: Int?) {
        let client = ObjectIdentifier(module)
        let touched = Set(lights.filter { $0.client == client && (index == nil || $0.index == index) }.map(\.index))
        lights.removeAll { $0.client == client && touched.contains($0.index) }
        for i in touched.sorted() { show(i, log: module.context.log) }
    }

    /// Pad `index` shows its newest surviving claim, or the platform's colour.
    private func show(_ index: Int, log: (String) -> Void) {
        guard let pad = pads[index] else { return }
        let rgb = hidden ? nil : lights.filter { $0.index == index && $0.connection == pad.connection }.max { $0.order < $1.order }?.rgb
        if pad.injected != nil {
            let hex = rgb.map { "#" + $0.map { String(format: "%02x", Int(($0 * 255).rounded())) }.joined() }
            return log("pad \(index): light \(hex ?? "given back")")
        }
        guard let light = pad.controller.light else { return }
        if let rgb {
            if pad.platformColor == nil { pad.platformColor = light.color }
            light.color = GCColor(red: Float(rgb[0]), green: Float(rgb[1]), blue: Float(rgb[2]))
        } else if let was = pad.platformColor {
            light.color = was
            pad.platformColor = nil
        }
    }

    // MARK: sessions

    /// The app went to the background, or came back: every light is given
    /// back while no session shows, and its claims written again after.
    func shownChanged() {
        let hide = !modules.contains(where: \.shown)
        guard hide != hidden else { return }
        hidden = hide
        let log = modules.first.map { m in { (s: String) in m.context.log(s) } } ?? { _ in }
        for i in Set(lights.map(\.index)).sorted() { show(i, log: log) }
    }

    /// `module`'s session ended: its claims end, and it hears no more.
    func leave(_ module: GamepadModule) {
        let client = ObjectIdentifier(module)
        for e in effects where e.client == client { finish(e, "preempted", log: module.context.log) }
        giveBack(module, nil)
        clients.removeAll { $0.module == nil || $0.module === module }
        poll()
    }
}
