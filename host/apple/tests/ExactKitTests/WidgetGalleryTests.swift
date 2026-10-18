#if EXACT_WIDGET_GALLERY && os(macOS)
import XCTest
import AppKit
import SwiftUI
import ExactCore
@testable import ExactKit

@MainActor
final class WidgetGalleryTests: XCTestCase {
    func testGalleryOpensWithTheBakedEntryAndLeavesTheAppAssetsIntact() throws {
        _ = NSApplication.shared
        let plan = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SNAPSHOT_TRAINS_PLAN"]))
        let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: plan.deletingLastPathComponent().appendingPathComponent("entries/caltrain/app.json"))) as! [String: Any]
        let directory = plan.deletingLastPathComponent().appendingPathComponent("widget-gallery-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("plans"), withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: plan, to: directory.appendingPathComponent("plans/next-trains.plan"))
        try JSONSerialization.data(withJSONObject: try XCTUnwrap(manifest["widgets"])).write(to: directory.appendingPathComponent("widget-gallery.json"))
        let app = ExactApp.shared, previous = ExactApp.shared.assetRoot
        defer { app.assetRoot = previous; try? FileManager.default.removeItem(at: directory) }
        app.assetRoot = directory
        XCTAssertTrue(WidgetGallery.available)
        let entry = try XCTUnwrap(WidgetGallery.entries.first)
        let preview = try WidgetPreview(WidgetVariant(entry: entry.kind, family: "systemMedium", mode: "accented", scheme: "dark", size: "xxxl"), entry: entry)
        XCTAssertEqual(preview.props["station"] as? String, "Mountain View")
        XCTAssertEqual(try preview.session.snapshot().nodes.count, 8)
        WidgetGallery.show()
        let window = try XCTUnwrap(NSApp.windows.first { $0.title == "Widgets" })
        defer { window.close() }
        XCTAssertTrue(window.isVisible)
        let view = try XCTUnwrap(window.contentView)
        view.layoutSubtreeIfNeeded()
        XCTAssertGreaterThan(view.bounds.height, 600)
        if let output = ProcessInfo.processInfo.environment["EXACT_WP6_OUT"],
           let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
            view.cacheDisplay(in: view.bounds, to: bitmap)
            try XCTUnwrap(bitmap.representation(using: .png, properties: [:])).write(to: URL(fileURLWithPath: output).appendingPathComponent("gallery-macos.png"))
        }
    }
}
#endif
