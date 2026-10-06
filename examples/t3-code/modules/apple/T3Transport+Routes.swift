// T3Transport's route ops (T3Transport.perform routes them; lane environment-routes):
// the app's route edits for one saved environment. The walk itself is T3Routes.swift's
// T3RouteState, driven from T3Transport.start. Runs on the transport's queue.
import Foundation

extension T3Transport {
    /// Route edits; false for an op that is not one of them.
    func routeOps(_ request: [String: Any], completion: @escaping Completion) throws -> Bool {
        switch request["op"] as? String {
        case "setRoutes": try setRoutes(request, completion: completion)
        default: return false
        }
        return true
    }

    /// The app's route edits (connection-routes.ts): the saved order, removals and learned routes. A removed
    /// paired route's token goes with it unless another route still borrows it; removing the route in use reconnects.
    func setRoutes(_ request: [String: Any], completion: @escaping Completion) throws {
        guard let environment = request["environmentId"] as? String, !environment.isEmpty, let list = request["routes"] as? [[String: Any]] else {
            throw arguments("setRoutes requires environmentId and routes.")
        }
        let removed = try savedEnvironments.setRoutes(environment: environment, routes: list)
        let kept = savedEnvironments.entry(environment: environment)
        let owners = Set(T3SavedEnvironments.credentialOrigins(kept) + (kept?["routes"] as? [[String: Any]] ?? []).compactMap { $0["credential"] as? String })
        for route in removed where route["learned"] as? Bool != true {
            let owner = T3SavedEnvironments.trimmed(route["credential"] as? String ?? route["origin"] as? String ?? "")
            if !owner.isEmpty, !owners.contains(owner) { try? credentials.forget(origin: owner, environment: environment) }
        }
        if routes.environmentId == environment {
            let active = routes.activeId
            routes.reload(saved: savedEnvironments)
            if !active.isEmpty, removed.contains(where: { $0["id"] as? String == active }), wantsConnection {
                retire(T3Failure(kind: "Replaced", message: "The route in use was removed.", uncertain: true))
                failures = 0; start()
            } else if state == "connected" { routes.schedule { [weak self] in self?.lookForBetterRoute() } }
        }
        finish(completion, value: ["saved": savedEnvironments.all])
    }
}
