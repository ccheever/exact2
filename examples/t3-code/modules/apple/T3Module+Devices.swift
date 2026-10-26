// T3Module's device ops (T3Module.swift routes them): input to a device screen
// and its screenshots, with the hub access the data module sends along.
import Foundation
import AppKit

extension T3Module {
    /// A device screen's input and screenshots (T3DeviceStream.swift).
    func deviceOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "deviceInput" { DispatchQueue.main.async { [weak self] in reply.send(self?.devices.input(request) ?? ["ok": false, "generation": 0]) }; return }
        if request["op"] as? String == "deviceScreenshot" {
            let origin = (request["origin"] as? String).flatMap(URL.init(string:)), ticket = (request["ticket"] as? String).flatMap { $0.isEmpty ? nil : $0 }
            return T3DeviceStreams.screenshot(request, access: { done in done(origin, ticket) }, exportsRoot: exportsRoot) { reply.send($0) }
        }
        next()
    }
}
