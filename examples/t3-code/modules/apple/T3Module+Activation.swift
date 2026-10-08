// T3Module's `t3 app <dir>` ops (T3Module.swift routes them; 20261005-app-activation): the window's
// half of the reference's IPC (apps/desktop/src/ipc/methods/appActivation.ts and the request
// channel). The socket and the broker are one per app (T3AppControl.swift); each session's module
// is a window that attaches at init and detaches at destroy.
import AppKit
import Foundation

extension T3Module {
    /// `activationReady {ready, token}`: setReady. `activationStatus`: the request handed to this
    /// window (`dispatched`, '' when none) and the socket's state; `t3.activation` announces a handed
    /// request. `activationRequest {requestId}`: that request while it still waits for this window.
    /// `activationComplete {response}`: the window's answer, sent back to the command line.
    func activationOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let control = T3AppControl.shared
        switch request["op"] as? String {
        case "activationReady":
            control.setReady(self, ready: request["ready"] as? Bool == true, token: request["token"] as? String ?? "")
            reply.send(["ok": true, "generation": generation, "value": [String: Any]()])
        case "activationStatus":
            reply.send(["ok": true, "generation": generation, "value": control.status(for: self)])
        case "activationRequest":
            if let handed = control.handedRequest(for: self, requestId: request["requestId"] as? String ?? "") {
                reply.send(["ok": true, "generation": generation, "value": handed])
            } else {
                reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "Activation", message: "That request is no longer waiting for this window.").json])
            }
        case "activationComplete":
            if let failure = control.complete(request["response"]) {
                reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "Arguments", message: failure).json])
            } else {
                reply.send(["ok": true, "generation": generation, "value": [String: Any]()])
            }
        default: next()
        }
    }

    func attachAppControl(_ context: ExactModuleContext) {
        T3AppControl.shared.attach(self, dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), changed: context.changed)
    }

    func detachAppControl() { T3AppControl.shared.detach(self) }

    /// The first of this module's views to be in a window names the window whose closing ends its requests.
    func activationElement(_ element: ExactElement) {
        if let window = element.view?.window { T3AppControl.shared.observeWindow(self, window) }
    }
}
