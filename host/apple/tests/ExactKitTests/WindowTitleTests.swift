// What a window is called once something is open.
//
// The folder rather than the file: a reader shows the filename in its own
// chrome, and it is the *project* you have open that you pick out of a
// window list. @ref LLP 1033 D7
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class WindowTitleTests: XCTestCase {
    /// A corpus directory is titled by the folder it lives in — which is the
    /// repository, and the reason this rule was chosen.
    func testACorpusIsTitledByTheProjectAroundIt() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/llp"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// A file is titled by the folder that holds it, for the same reason.
    func testAFileIsTitledByItsFolder() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/README.md"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// A trailing slash is not a folder called "".
    func testATrailingSlashIsNotAName() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/llp/"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// Nothing above it is just the app: a title with a dangling dash reads
    /// like a bug, and at the root there is no project to name.
    func testTheRootIsJustTheApp() {
        XCTAssertEqual(ExactDocuments.windowTitle(for: "/"), ExactEnv.appName)
        XCTAssertEqual(ExactDocuments.windowTitle(for: ""), ExactEnv.appName)
    }
}

final class MaterialPresentationTests: XCTestCase {
    private func fixture() -> (Presenter, NodeView, NodeView) {
        _ = NSApplication.shared
        let presenter = Presenter()
        let parent = NodeView(id: 1, kind: "view", presenter: presenter)
        let child = NodeView(id: 2, kind: "view", presenter: presenter)
        parent.frame = NSRect(x: 0, y: 0, width: 240, height: 160)
        child.frame = NSRect(x: 13, y: 19, width: 50, height: 30)
        parent.addSubview(child)
        return (presenter, parent, child)
    }

    func testMaterialChangesPreserveChildrenFramesAndHitTargets() {
        let (presenter, parent, child) = fixture()
        let host = FlippedView(frame: parent.frame)
        host.addSubview(parent)
        let frame = child.frame
        for material in ["glass", "ultra-thin", "glass"] {
            parent.applyProps(set: ["backgroundMaterial": material], clear: [])
            parent.layoutSubtreeIfNeeded()
            XCTAssertTrue(child.superview === parent.container)
            XCTAssertEqual(child.frame, frame)
            XCTAssertTrue(parent.hitTest(NSPoint(x: 20, y: 25)) === child)
            XCTAssertTrue(parent.hitTest(NSPoint(x: 200, y: 100)) === parent)
            XCTAssertEqual(parent.materialView?.frame, parent.bounds)
        }
        parent.applyProps(set: [:], clear: ["backgroundMaterial"])
        XCTAssertTrue(child.superview === parent)
        XCTAssertEqual(child.frame, frame)
        XCTAssertNil(parent.materialView)
        XCTAssertEqual(parent.appliedMaterial, "none")
        withExtendedLifetime(presenter) {}
    }

    func testMaterialReportsMountedClassAndUpdatesRadius() {
        let (presenter, parent, _) = fixture()
        parent.applyProps(set: ["backgroundMaterial": "glass"], clear: [])
        parent.applyStyle(["border_radius": 12.0])
        if #available(macOS 26.0, *) {
            XCTAssertEqual(parent.appliedMaterial, "NSGlassEffectView(.regular)")
            XCTAssertEqual((parent.materialView as? NSGlassEffectView)?.cornerRadius, 12)
            XCTAssertEqual((parent.materialView as? NSGlassEffectView)?.style, .regular)
        } else {
            XCTAssertEqual(parent.appliedMaterial, "NSVisualEffectView(.popover)")
        }
        parent.applyProps(set: ["backgroundMaterial": "ultra-thin"], clear: [])
        let blur = parent.materialView as? NSVisualEffectView
        XCTAssertEqual(blur?.material, .popover)
        XCTAssertEqual(blur?.blendingMode, .withinWindow)
        XCTAssertEqual(blur?.state, .followsWindowActiveState)
        XCTAssertNil(blur?.appearance)
        XCTAssertEqual(blur?.layer?.cornerRadius, 12)
        // A computed name the table lacks draws ultra-thin (LLP 1053.000 D4).
        parent.applyProps(set: ["backgroundMaterial": "unknown"], clear: [])
        XCTAssertEqual((parent.materialView as? NSVisualEffectView)?.material, .popover)
        XCTAssertEqual(parent.appliedMaterial, "NSVisualEffectView(.popover)")
        // Every AppKit material by its name.
        parent.applyProps(set: ["backgroundMaterial": "sidebar"], clear: [])
        XCTAssertEqual((parent.materialView as? NSVisualEffectView)?.material, .sidebar)
        XCTAssertEqual(parent.appliedMaterial, "NSVisualEffectView(.sidebar)")
        parent.applyProps(set: ["backgroundMaterial": "hud-window"], clear: [])
        XCTAssertEqual((parent.materialView as? NSVisualEffectView)?.material, .hudWindow)
        withExtendedLifetime(presenter) {}
    }

    func testMaterialAndScrollChangesKeepTheSameChild() {
        let (presenter, parent, child) = fixture()
        parent.applyProps(set: ["backgroundMaterial": "glass"], clear: [])
        parent.applyStyle(["overflow_y": "scroll"])
        XCTAssertTrue(child.superview === parent.scroll?.documentView)
        let scroller = parent.scroll
        parent.applyProps(set: ["backgroundMaterial": "ultra-thin"], clear: [])
        XCTAssertTrue(parent.scroll === scroller)
        XCTAssertTrue(child.superview === scroller?.documentView)
        parent.applyStyle([:])
        XCTAssertNil(parent.scroll)
        XCTAssertTrue(child.superview === parent.container)
        parent.applyProps(set: [:], clear: ["backgroundMaterial"])
        XCTAssertTrue(child.superview === parent)
        withExtendedLifetime(presenter) {}
    }

    func testChangingMaterialDoesNotResignFocusedChild() {
        let (presenter, parent, child) = fixture()
        child.handlers = ["press"]
        let window = NSWindow(contentRect: parent.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = parent
        XCTAssertTrue(window.makeFirstResponder(child))
        for material in ["glass", "ultra-thin", "glass"] {
            parent.applyProps(set: ["backgroundMaterial": material], clear: [])
            XCTAssertTrue(window.firstResponder === child)
        }
        parent.applyProps(set: [:], clear: ["backgroundMaterial"])
        XCTAssertTrue(window.firstResponder === child)
        window.makeFirstResponder(nil)
        withExtendedLifetime(presenter) {}
    }

    func testChangingMaterialPreservesNativeFieldEditorAndSelection() {
        let (presenter, parent, _) = fixture()
        let input = NodeView(id: 3, kind: "input", presenter: presenter)
        input.frame = NSRect(x: 10, y: 60, width: 180, height: 24)
        input.field?.frame = input.bounds
        input.applyProps(set: ["value": "A retained draft"], clear: [])
        parent.addSubview(input)
        let window = NSWindow(contentRect: parent.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = parent
        XCTAssertTrue(window.makeFirstResponder(input.field))
        let editor = input.field?.currentEditor()
        XCTAssertNotNil(editor)
        editor?.selectedRange = NSRange(location: 2, length: 8)
        for material in ["glass", "ultra-thin", "glass"] {
            parent.applyProps(set: ["backgroundMaterial": material], clear: [])
            XCTAssertTrue(input.field?.currentEditor() === editor)
            XCTAssertEqual(editor?.string, "A retained draft")
            XCTAssertEqual(editor?.selectedRange, NSRange(location: 2, length: 8))
        }
        parent.applyProps(set: [:], clear: ["backgroundMaterial"])
        XCTAssertTrue(input.field?.currentEditor() === editor)
        window.makeFirstResponder(nil)
        withExtendedLifetime(presenter) {}
    }
}
#endif
