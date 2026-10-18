// T3Module's device ops (T3Module.swift routes them): input to a device screen
// and its screenshots, through the device hub access of the module's links (T3Link.swift).
import Foundation
import AppKit

extension T3Module {
    /// A device screen's input and screenshots (T3DeviceStream.swift).
    func deviceOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "deviceInput" { DispatchQueue.main.async { [weak self] in reply.send(self?.devices.input(request) ?? ["ok": false, "generation": 0]) }; return }
        if request["op"] as? String == "deviceScreenshot" { return T3DeviceStreams.screenshot(request, access: { [links] done in links.deviceHubAccess(done) }, exportsRoot: exportsRoot) { reply.send($0) } }
        next()
    }
}
