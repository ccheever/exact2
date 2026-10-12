// T3Module's device ops (T3Module.swift routes them): input to a device screen
// and its screenshots, through the transport's device hub access.
import Foundation
import AppKit

extension T3Module {
    /// A device screen's input and screenshots (R6DeviceStream.swift, lane r6-media).
    func deviceOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "r6DeviceInput" { DispatchQueue.main.async { [weak self] in reply.send(self?.devices.input(request) ?? ["ok": false, "generation": 0]) }; return } // lane r6-media
        if request["op"] as? String == "r6DeviceScreenshot" { return R6DeviceStreams.screenshot(request, access: { [transport] done in transport.deviceHubAccess(done) }, exportsRoot: exportsRoot) { reply.send($0) } }
        next()
    }
}
