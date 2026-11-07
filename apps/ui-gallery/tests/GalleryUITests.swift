import XCTest
import UIKit

/// Run the full suite on an open Duo; the header and video checks also run closed.
final class GalleryUITests: XCTestCase {
    override func setUp() {
        super.setUp()
        continueAfterFailure = false
    }

    func testSidebarSelectionFinishesWithoutAnotherTouch() throws {
        let app = XCUIApplication(bundleIdentifier: "com.exact.uigallery")
        app.launch()
        let sidebar = app.tables["gallery-sidebar"]
        XCTAssertTrue(sidebar.waitForExistence(timeout: 20))
        XCTAssertGreaterThan(app.windows.firstMatch.frame.width, 800, "Run this check on the open Duo so selection leaves the sidebar visible")
        for section in ["css", "profiles", "textcolors"] {
            app.cells["sidebar-panel-" + section].tap()
            let before = try sidebarPixels(app, sidebar)
            sidebar.staticTexts["Text"].firstMatch.tap()
            let after = try sidebarPixels(app, sidebar)
            XCTAssertTrue(before == after, "Sidebar selection must finish drawing without another touch. Keep existing cells when only selection changes.")
        }
    }

    func testSidebarSelectionPreservesVisibleCells() throws {
        let tabs = (0..<9).map { index in
            GalleryTestTab(name: "Section \(index)", controller: UINavigationController(), item: UITabBarItem(title: "Section \(index)", image: nil, tag: index))
        }
        let contents = ExactTabContents(tabs: tabs)
        let sidebar = GallerySidebar(contents)
        sidebar.loadViewIfNeeded()
        let table = try XCTUnwrap(sidebar.tableView)
        table.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        let source = GalleryDataSourceSpy(sidebar)
        table.dataSource = source
        table.reloadData()
        table.layoutIfNeeded()
        XCTAssertGreaterThan(source.configurations, 0, "The table must configure visible rows before testing reuse")
        let configurations = source.configurations
        let cells = table.visibleCells
        for selection in [2, 3, 1, 1] {
            contents.selected = selection
            sidebar.refreshSelection()
            table.layoutIfNeeded()
            XCTAssertEqual(source.configurations, configurations, "Changing selection must not rebuild unchanged rows")
            XCTAssertEqual(table.visibleCells.map(ObjectIdentifier.init), cells.map(ObjectIdentifier.init))
        }
        XCTAssertEqual(table.indexPathForSelectedRow, IndexPath(row: 1, section: 0))
    }

    func testHeaderCollapsesAndTheRangeBarFollowsIt() throws {
        let app = XCUIApplication(bundleIdentifier: "com.exact.uigallery")
        app.launch()
        let sidebar = app.tables["gallery-sidebar"]
        XCTAssertTrue(sidebar.waitForExistence(timeout: 20))
        let cell = app.cells["sidebar-panel-css"]
        for _ in 0..<4 where !cell.isHittable { sidebar.swipeDown() }
        cell.tap()
        let bar = app.navigationBars["CSS color spaces"]
        let range = app.segmentedControls["range-css"]
        XCTAssertTrue(range.waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["range-css-title"].label, "Dynamic range limit")
        XCTAssertFalse(app.staticTexts["range-css-note"].exists)
        let scroll = app.otherElements["gallery"].firstMatch
        var readings: [[String: Any]] = []
        func record(_ step: String) throws {
            let title = bar.staticTexts["CSS color spaces"].firstMatch
            let rangeTitle = app.staticTexts["range-css-title"]
            let rangeFrame = app.otherElements["range-bar-css"].firstMatch.frame
            let image = try screenImage(app), window = app.windows.firstMatch.frame
            let headerColor = try backgroundChannels(image, window: window, at: CGPoint(x: rangeFrame.minX + 4, y: rangeFrame.minY - 4))
            let rangeColor = try backgroundChannels(image, window: window, at: CGPoint(x: rangeFrame.minX + 4, y: rangeFrame.minY + 4))
            let difference = zip(headerColor, rangeColor).map { abs($0 - $1) }.max() ?? 0
            readings.append(["step": step, "headerBottom": bar.frame.maxY,
                             "headerTitleBottom": title.exists ? title.frame.maxY : -1,
                             "rangeTitleTop": rangeTitle.frame.minY, "rangeTitleBottom": rangeTitle.frame.maxY,
                             "titleControlGap": range.frame.minX - rangeTitle.frame.maxX,
                             "titleCenter": rangeTitle.frame.midY, "controlCenter": range.frame.midY,
                             "rangeTop": range.frame.minY, "gap": range.frame.minY - bar.frame.maxY,
                             "barTop": app.otherElements["range-bar-css"].firstMatch.frame.minY,
                             "titleVisible": title.exists && title.isHittable,
                             "titleTop": title.exists ? title.frame.minY : -1,
                             "headerColor": headerColor, "rangeColor": rangeColor,
                             "backgroundDifference": difference,
                             "hittable": range.isHittable])
            if step == "initial" || step == "fast-up", let data = UIImage(cgImage: image).pngData() {
                try? data.write(to: output.appendingPathComponent("header-\(step).png"))
            }
        }
        try record("initial")
        scroll.swipeUp(velocity: .slow)
        try record("slow-up")
        scroll.swipeUp(velocity: .fast)
        try record("fast-up")
        scroll.swipeDown(velocity: .fast)
        try record("fast-down")
        scroll.swipeUp(velocity: .fast)
        try record("fast-up-again")
        try save(readings, as: "header-range.json")
        // UIKit retains the bar's accessibility frame after hiding its title
        XCTAssertEqual(readings.first?["titleVisible"] as? Bool, true)
        XCTAssertEqual(readings[2]["titleVisible"] as? Bool, false, "Scrolling must hide the native title")
        XCTAssertEqual(readings[3]["titleVisible"] as? Bool, true, "Scrolling back restores the native title")
        for reading in readings where reading["titleVisible"] as? Bool == true {
            XCTAssertEqual(try XCTUnwrap(reading["barTop"] as? CGFloat),
                           try XCTUnwrap(reading["headerBottom"] as? CGFloat), accuracy: 1,
                           "The sticky bar starts at the bottom of the visible header")
        }
        let expanded = readings.filter { $0["titleVisible"] as? Bool == true }.compactMap { $0["rangeTop"] as? CGFloat }
        let collapsed = readings.filter { $0["titleVisible"] as? Bool == false }.compactMap { $0["rangeTop"] as? CGFloat }
        XCTAssertLessThan(try XCTUnwrap(collapsed.first), try XCTUnwrap(expanded.first) - 20, "The range bar follows the native header")
        for positions in [expanded, collapsed] {
            XCTAssertEqual(try XCTUnwrap(positions.max()), try XCTUnwrap(positions.min()), accuracy: 1, "Swipe velocity does not change spacing within a header state")
        }
        for reading in readings {
            XCTAssertLessThanOrEqual(try XCTUnwrap(reading["backgroundDifference"] as? Int), 3,
                                     "The header and range controls share a continuous background: \(reading)")
            XCTAssertEqual(reading["hittable"] as? Bool, true)
            XCTAssertEqual(try XCTUnwrap(reading["titleControlGap"] as? CGFloat), 16, accuracy: 0.5,
                           "The title and segmented control use system horizontal spacing")
            XCTAssertEqual(try XCTUnwrap(reading["titleCenter"] as? CGFloat),
                           try XCTUnwrap(reading["controlCenter"] as? CGFloat), accuracy: 0.5,
                           "The label is vertically centered beside the control")
        }
        range.buttons["standard"].tap()
        XCTAssertTrue(range.buttons["standard"].isSelected)
        if app.windows.firstMatch.frame.width < 500 {
            let back = app.buttons["BackButton"].firstMatch
            XCTAssertTrue(back.waitForExistence(timeout: 5))
            back.tap()
            XCTAssertTrue(sidebar.waitForExistence(timeout: 10))
        }
    }

    func testRangeControlsRemainInteractiveOverScrollingContent() throws {
        let app = XCUIApplication(bundleIdentifier: "com.exact.uigallery")
        app.launch()
        let sidebar = app.tables["gallery-sidebar"]
        XCTAssertTrue(sidebar.waitForExistence(timeout: 20))
        for section in ["css", "images", "css"] {
            let cell = app.cells["sidebar-panel-" + section]
            for _ in 0..<4 where !cell.isHittable { sidebar.swipeDown() }
            cell.tap()
            let range = app.segmentedControls["range-" + section]
            XCTAssertTrue(range.waitForExistence(timeout: 10))
            range.buttons["standard"].tap()
            let scroll = app.otherElements["gallery"].firstMatch
            let intro = app.staticTexts["intro-" + section]
            for _ in 0..<6 where !intro.isHittable { scroll.swipeDown(velocity: .fast) }
            scroll.swipeUp(velocity: .fast)
            let bar = app.otherElements["range-bar-" + section].firstMatch
            let before = range.frame
            _ = try rangeBarPixels(app, bar, name: section + "-before")
            scroll.swipeUp(velocity: .fast)
            _ = try rangeBarPixels(app, bar, name: section + "-after")
            XCTAssertEqual(range.frame, before, "Scrolling keeps the control in place")
            XCTAssertTrue(range.isHittable)
            range.buttons["constrained"].tap()
            XCTAssertTrue(range.buttons["constrained"].isSelected)
        }
    }

    func testVideoFormatAndPlaybackControls() throws {
        let app = XCUIApplication(bundleIdentifier: "com.exact.uigallery")
        app.launch()
        let sidebar = app.tables["gallery-sidebar"]
        XCTAssertTrue(sidebar.waitForExistence(timeout: 20))
        let media = app.cells["sidebar-panel-media"]
        for _ in 0..<4 where !media.isHittable { sidebar.swipeUp() }
        media.tap()
        let formats = app.segmentedControls["video-formats"]
        XCTAssertTrue(formats.waitForExistence(timeout: 10))
        let scroll = app.otherElements["gallery"].firstMatch
        for _ in 0..<4 where !formats.isHittable { scroll.swipeUp() }
        for label in ["PQ", "HLG", "P3 SDR", "SDR"] {
            let format = formats.buttons[label]
            format.tap()
            XCTAssertTrue(format.isSelected)
            let toggle = app.buttons["video-toggle"]
            XCTAssertEqual(toggle.label, "Pause videos")
            toggle.tap()
            XCTAssertEqual(toggle.label, "Play videos")
            toggle.tap()
            XCTAssertEqual(toggle.label, "Pause videos")
            XCTAssertEqual(app.state, .runningForeground)
        }
        let image = try screenImage(app)
        let data = try XCTUnwrap(UIImage(cgImage: image).pngData())
        try data.write(to: output.appendingPathComponent("video-controls.png"))
    }

    func testPageSwitchesKeepContentBelowTheRangeBar() throws {
        let app = XCUIApplication(bundleIdentifier: "com.exact.uigallery")
        app.launch()
        let sidebar = app.tables["gallery-sidebar"]
        XCTAssertTrue(sidebar.waitForExistence(timeout: 20))
        XCTAssertGreaterThan(app.windows.firstMatch.frame.width, 800)
        var readings: [[String: Any]] = []
        for section in ["typography", "css", "profiles", "images", "typography", "textcolors", "media", "css", "images", "profiles", "textcolors", "typography", "media"] {
            // Read the initial page before a repeated selection can scroll it back to the top
            if !readings.isEmpty {
                let cell = app.cells["sidebar-panel-" + section]
                for _ in 0..<4 where !cell.isHittable {
                    if section == "media" || section == "images" { sidebar.swipeUp() } else { sidebar.swipeDown() }
                }
                cell.tap()
            }
            let bar = app.otherElements["range-bar-" + section].firstMatch
            let intro = app.staticTexts["intro-" + section]
            XCTAssertTrue(intro.waitForExistence(timeout: 10))
            readings.append(["section": section, "barBottom": bar.frame.maxY, "contentTop": intro.frame.minY])
            try save(readings, as: "page-switch-insets.json")
        }
        for reading in readings {
            XCTAssertGreaterThanOrEqual(try XCTUnwrap(reading["contentTop"] as? CGFloat),
                                       try XCTUnwrap(reading["barBottom"] as? CGFloat) + 10,
                                       reading["section"] as? String ?? "")
        }
    }

    private var output: URL {
        URL(fileURLWithPath: ProcessInfo.processInfo.environment["REACH_OUT"]!)
    }

    private func save(_ value: Any, as name: String) throws {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: output.appendingPathComponent(name))
    }

    private func rangeBarPixels(_ app: XCUIApplication, _ bar: XCUIElement, name: String) throws -> Data {
        let window = app.windows.firstMatch.frame
        let image = try screenImage(app)
        let scale = CGFloat(image.width) / window.width
        let frame = bar.frame
        let box = CGRect(x: (frame.minX - window.minX + 16) * scale,
                         y: (frame.minY - window.minY) * scale,
                         width: (frame.width - 32) * scale, height: (frame.height - 12) * scale).integral
        let cropped = try XCTUnwrap(image.cropping(to: box))
        try XCTUnwrap(UIImage(cgImage: cropped).pngData()).write(to: output.appendingPathComponent("range-body-" + name + ".png"))
        return try pixels(cropped)
    }

    private func backgroundChannels(_ image: CGImage, window: CGRect, at point: CGPoint) throws -> [Int] {
        let scale = CGFloat(image.width) / window.width
        let box = CGRect(x: (point.x - window.minX) * scale, y: (point.y - window.minY) * scale,
                         width: 2 * scale, height: 2 * scale).integral
        let data = try pixels(XCTUnwrap(image.cropping(to: box)))
        return (0..<3).map { channel in
            stride(from: channel, to: data.count, by: 4).reduce(0) { $0 + Int(data[$1]) } / (data.count / 4)
        }
    }

    private func sidebarPixels(_ app: XCUIApplication, _ sidebar: XCUIElement) throws -> Data {
        let window = app.windows.firstMatch.frame
        let image = try screenImage(app)
        let scale = CGFloat(image.width) / window.width
        let first = sidebar.cells.firstMatch.frame.minY
        var box = sidebar.frame.insetBy(dx: 24, dy: 0)
        box.size.height = box.maxY - first
        box.origin.y = first
        box = box.offsetBy(dx: -window.minX, dy: -window.minY)
        box = CGRect(x: box.minX * scale, y: box.minY * scale,
                     width: box.width * scale, height: box.height * scale).integral
        return try pixels(XCTUnwrap(image.cropping(to: box)))
    }

    private func screenImage(_ app: XCUIApplication) throws -> CGImage {
        let window = app.windows.firstMatch.frame
        for (index, screen) in XCUIScreen.screens.enumerated() {
            let raw = screen.screenshot().pngRepresentation
            try raw.write(to: output.appendingPathComponent("screen-\(index).png"))
            guard let source = UIImage(data: raw) else { continue }
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            let normalized = UIGraphicsImageRenderer(size: source.size, format: format).image { _ in
                source.draw(in: CGRect(origin: .zero, size: source.size))
            }
            guard let image = normalized.cgImage,
                  abs(CGFloat(image.width) / CGFloat(image.height) - window.width / window.height) < 0.01 else { continue }
            let data = try pixels(image)
            if data.enumerated().contains(where: { $0.offset % 4 < 3 && $0.element != 0 }) { return image }
        }
        throw NSError(domain: "GalleryUITests", code: 1, userInfo: [NSLocalizedDescriptionKey: "No active display matches the app window. Set the Duo pose before starting the UI test and retry."])
    }

    private func pixels(_ image: CGImage) throws -> Data {
        var data = Data(count: image.width * image.height * 4)
        try data.withUnsafeMutableBytes { bytes in
            let context = try XCTUnwrap(CGContext(data: bytes.baseAddress, width: image.width, height: image.height,
                                                bitsPerComponent: 8, bytesPerRow: image.width * 4,
                                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        return data
    }
}

// Test the UIKit container independently of Exact's router
struct GalleryTestTab {
    let name: String
    let controller: UINavigationController
    let item: UITabBarItem
}

final class ExactTabContents {
    let tabs: [GalleryTestTab]
    var selected = 0
    var onSelect: ((Int) -> Void)?

    init(tabs: [GalleryTestTab]) { self.tabs = tabs }
    func select(_ index: Int) { selected = index; onSelect?(index) }
}

private final class GalleryDataSourceSpy: NSObject, UITableViewDataSource {
    let source: UITableViewDataSource
    private(set) var configurations = 0

    init(_ source: UITableViewDataSource) { self.source = source }
    func numberOfSections(in tableView: UITableView) -> Int { source.numberOfSections?(in: tableView) ?? 1 }
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        source.tableView(tableView, numberOfRowsInSection: section)
    }
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        configurations += 1
        return source.tableView(tableView, cellForRowAt: indexPath)
    }
    func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        source.tableView?(tableView, titleForHeaderInSection: section)
    }
}
