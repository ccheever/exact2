// The Surfaces capability under test (LLP 1047.001 D4): its tests install it
// before they make a session, and reach its implementation through it.
@testable import ExactKit
@testable import ExactSurfaces

extension ExactSession {
    /// This session's GPU surfaces, as the capability made them.
    var surfaceHost: CanvasesHost {
        guard let host = canvases as? CanvasesHost else { fatalError("ExactSurfaces.install() before the session is made") }
        return host
    }
}
