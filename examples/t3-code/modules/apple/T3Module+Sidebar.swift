// T3Module's sidebar ops (T3Module.swift routes them): the sidebar's native
// menus and reads, and the thread notifications, sound and Dock badge.
import Foundation
import AppKit

extension T3Module {
    /// The thread menu, modifier reads and jump hints (T3Sidebar.swift); thread notifications (T3Notifications.swift).
    func sidebarOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("sidebar") {
            // The thread menu pops up outside a main-queue block (T3MenuTurn, T3ContextMenu.swift).
            let work = { [weak self] in self?.sidebar.perform(request) { reply.send($0) }; return () }
            if op == "sidebarMenu" { T3MenuTurn.run(work) } else { DispatchQueue.main.async(execute: work) }
            return
        }
        if let op = request["op"] as? String, op.hasPrefix("notify") {
            DispatchQueue.main.async { [weak self] in self?.notifications.perform(request) { reply.send($0) } }
            return
        }
        next()
    }
}
