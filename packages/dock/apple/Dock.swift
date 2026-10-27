// @exact/dock on macOS (LLP 1133.025 D5): `requestAttention` bounces the
// Dock icon, `NSApplication.requestUserAttention(_:)`: once for an
// informational request, until the app is activated for a critical one.
// Apple: "Sending requestUserAttention(_:) to an app that is already active
// has no effect", and an accessory app has no Dock tile; the journal says
// which.
import AppKit
import ExactPackage

public final class DockModule: ExactPackage {
    private let context: ExactPackageContext
    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            guard name == "requestAttention" else { return reply(["error": "NotSupportedError"]) }
            let critical = args["critical"] as? Bool == true
            if NSApp.activationPolicy() != .regular {
                context.log("dock.requestAttention: no Dock tile (an accessory app)")
            } else if NSApp.isActive {
                context.log("dock.requestAttention: the app is in front")
            } else {
                NSApp.requestUserAttention(critical ? .criticalRequest : .informationalRequest)
                context.log("dock.requestAttention: \(critical ? "critical" : "informational")")
            }
            reply([:])
        }
    }
}
