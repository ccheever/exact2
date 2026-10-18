#if os(macOS) || os(iOS) || os(tvOS)
import Foundation
import GameController
import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Gamepads for world canvases (LLP 1127.003 D2) through GameController's
/// extended gamepad profile, in the W3C Gamepad API's standard mapping
/// (https://w3c.github.io/gamepad/#remapping): buttons by position, so
/// `buttonA` is the bottom face button, standard button 0, on every family.
/// A change to any element sends the focused canvas the pad's whole state; a
/// change, not a sample per frame. The pads follow the focus: when a canvas
/// takes it, or the app becomes active again, the canvas that had them hears
/// each pad go and the focused one hears each pad as it is now. A controller
/// without the extended profile (the Siri Remote) is reported connected and
/// sends nothing more.
final class CanvasPads {
    static let shared = CanvasPads()
    /// A pad's index is its slot; the next pad to connect takes the lowest free
    /// one, and the world treats it as a fresh device.
    private var slots: [GCController?] = []
    private var sent: [Int: [Double]] = [:]
    private weak var sink: CanvasInputHost?
    private var hosts: [Weak] = []
    private struct Weak { weak var host: CanvasInputHost? }
    private static let maxPads = 16, step = 256.0

    private init() {
        let center = NotificationCenter.default
        _ = center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
            self?.connect(note.object as? GCController)
        }
        _ = center.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] note in
            self?.disconnect(note.object as? GCController)
        }
        #if os(macOS)
        let active = NSApplication.didBecomeActiveNotification
        #else
        let active = UIApplication.didBecomeActiveNotification
        #endif
        _ = center.addObserver(forName: active, object: nil, queue: .main) { [weak self] _ in self?.resync() }
        for controller in GCController.controllers() { connect(controller) }
    }
    /// A canvas that takes input hears the pads while it has focus; it takes
    /// its first focus after this turn of the run loop.
    func add(_ host: CanvasInputHost) {
        hosts.removeAll { $0.host == nil }
        hosts.append(Weak(host: host))
        DispatchQueue.main.async { [weak self] in self?.resync() }
    }
    /// The canvas's world released every pad (a blur): the pads have no
    /// canvas until one takes the focus, and then report again.
    func blurred(_ host: CanvasInputHost) {
        if sink === host { sink = nil; sent.removeAll() }
    }
    /// Move the pads to the focused canvas and send it every connected pad.
    func resync() {
        let host = hosts.lazy.compactMap(\.host).first(where: \.padFocus)
        if host !== sink { move(to: host) }
        for index in slots.indices where slots[index] != nil { report(index) }
    }
    /// The canvas that had the pads hears each one go.
    private func move(to host: CanvasInputHost?) {
        if let old = sink, let view = old.view {
            for index in sent.keys.sorted() { send(view, index, nil) }
        }
        sink = host
        sent.removeAll()
    }
    private func connect(_ controller: GCController?) {
        guard let controller, !slots.contains(where: { $0 === controller }) else { return }
        let index = slots.firstIndex(where: { $0 == nil }) ?? slots.endIndex
        guard index < Self.maxPads else { return }
        if index == slots.endIndex { slots.append(controller) } else { slots[index] = controller }
        controller.extendedGamepad?.valueChangedHandler = { [weak self] _, _ in self?.report(index) }
        report(index)
    }
    private func disconnect(_ controller: GCController?) {
        guard let controller, let index = slots.firstIndex(where: { $0 === controller }) else { return }
        controller.extendedGamepad?.valueChangedHandler = nil
        slots[index] = nil
        report(index)
    }
    private static func quantize(_ v: Float, low: Double) -> Double {
        min(1, max(low, (Double(v) * step).rounded() / step)) + 0 // + 0: no -0 on the wire
    }
    /// Standard flag, then 17 buttons and 4 axes; positive y is down, as on the web.
    private func reading(_ controller: GCController) -> [Double] {
        guard let p = controller.extendedGamepad else { return [0] + Array(repeating: 0, count: 21) }
        let buttons: [GCControllerButtonInput?] = [
            p.buttonA, p.buttonB, p.buttonX, p.buttonY, p.leftShoulder, p.rightShoulder,
            p.leftTrigger, p.rightTrigger, p.buttonOptions, p.buttonMenu,
            p.leftThumbstickButton, p.rightThumbstickButton,
            p.dpad.up, p.dpad.down, p.dpad.left, p.dpad.right, p.buttonHome,
        ]
        let axes = [p.leftThumbstick.xAxis.value, -p.leftThumbstick.yAxis.value,
                    p.rightThumbstick.xAxis.value, -p.rightThumbstick.yAxis.value]
        return [1] + buttons.map { Self.quantize($0?.value ?? 0, low: 0) } + axes.map { Self.quantize($0, low: -1) }
    }
    private func report(_ index: Int) {
        let host = hosts.lazy.compactMap(\.host).first(where: \.padFocus)
        if host !== sink {
            // The focus moved without telling us: the whole set moves with it.
            move(to: host)
            if host != nil { return resync() }
        }
        guard let host, let view = host.view else { return }
        let controller = index < slots.count ? slots[index] : nil
        let value = controller.map(reading)
        guard sent[index] != value else { return }
        sent[index] = value
        send(view, index, value)
    }
    private func send(_ view: NodeView, _ index: Int, _ value: [Double]?) {
        let r = value ?? [1] + Array(repeating: 0, count: 21)
        _ = view.canvases?.input(view, ["t": "pad", "index": index, "connected": value != nil, "standard": r[0] == 1,
                                        "buttons": Array(r[1...17]), "axes": Array(r[18...21])])
    }
}
#endif
