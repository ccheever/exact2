#if os(iOS)
// upstream 365aa87982 apps/mobile/src/connection/{platform,app-state-wakeups}.ts.
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
import Foundation
import UIKit
import Network

/// One observer per transport, including background environments. All state lives on its queue.
final class T3Signals: @unchecked Sendable {
    private let queue: DispatchQueue
    private var observers: [NSObjectProtocol] = []
    private let monitor = NWPathMonitor()
    private var online: Bool?
    private var networkType: NWInterface.InterfaceType?
    private var backgroundedAt: TimeInterval?
    private var alive = true

    init(queue: DispatchQueue, active: @escaping (Bool) -> Void,
         network: @escaping (Bool) -> Void, pathChanged: @escaping () -> Void) {
        self.queue = queue
        // UIKit state is read on main, including when a fleet creates its transport from an I/O queue.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let initialBackground = UIApplication.shared.applicationState == .background
            self.queue.async { [weak self] in
                guard let self, self.alive else { return }
                if initialBackground { self.backgroundedAt = ProcessInfo.processInfo.systemUptime }
            }
        }
        observers.append(NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification,
                                                                object: nil, queue: .main) { [weak self] _ in
            let now = ProcessInfo.processInfo.systemUptime
            self?.queue.async { [weak self] in
                guard let self, self.alive else { return }
                self.backgroundedAt = now
            }
        })
        observers.append(NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification,
                                                                object: nil, queue: .main) { [weak self] _ in
            let now = ProcessInfo.processInfo.systemUptime
            self?.queue.async { [weak self] in
                guard let self, self.alive else { return }
                let reconnect = self.backgroundedAt.map { now - $0 >= 10 } ?? false
                self.backgroundedAt = nil
                active(reconnect)
            }
        })
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self, self.alive else { return }
            let now = path.status == .satisfied
            if let before = self.online, before != now { network(now) }
            self.online = now
            // Keep the last online type across offline periods, as upstream does.
            if now, let type = [NWInterface.InterfaceType.wifi, .cellular, .wiredEthernet, .loopback, .other]
                .first(where: { path.usesInterfaceType($0) }) {
                if let previous = self.networkType, previous != type { pathChanged() }
                self.networkType = type
            }
        }
        monitor.start(queue: queue)
    }

    func cancel() {
        alive = false
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        monitor.cancel()
    }
}

#endif
