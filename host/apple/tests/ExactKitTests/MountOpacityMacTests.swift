#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A node that mounts at one `opacity` and changes it in a later commit is
/// drawn at the new value: a row of buttons mounted disabled at 0.64 while a
/// save runs is drawn whole once the save ends, as the web draws it. The
/// buttons are the T3 Code clone's `EasGhostButton` as it was before
/// b9e83df3a5, every property, and the save runs in its editor's overlay
/// (`settings-eas-hosts.contract`).
final class MountOpacityMacTests: XCTestCase {
    static let page = """
    shape Host
      id: string

    component App
      state busy = false
      state editing = false
      state hosts: Host[] = []
      state pressed = 0
      action open
        editing = true
      action save
        busy = true
        hosts = [Host(id="a")]
      action done
        busy = false
        editing = false
      action edit
        pressed = pressed + 1
      view
        column testId="page" position="relative" width="100%" height="100%"
          button press=open testId="open"
            text "Add"
          button press=done testId="done"
            text "Done"
          for host in hosts key=host.id
            row flex-shrink=0 gap="0.25rem"
              EasGhostButton(label="Edit", destructive=false, disabled=busy, press=edit, buttonId=`eas-host-edit-${host.id}`)
              EasGhostButton(label="Remove", destructive=true, disabled=busy, press=edit, buttonId=`eas-host-remove-${host.id}`)
          if editing
            column position="absolute" left=0 right=0 top=0 bottom=100 align-items="center" justify-content="center" background-color="light-dark(#fcfcfc99, #090909a3)" z-index=30 testId="overlay"
              button press=save disabled=busy testId="save" gap="0.375rem" height="2rem" padding-left="0.6875rem" padding-right="0.6875rem" border-radius="0.5rem" border-width=0 background-color="light-dark(#1b4ed8, #346bf1)" display="flex" flex-direction="row" align-items="center" opacity=(busy ? 0.64 : 1) data-focus-stop=(busy ? "" : "enabled")
                if busy
                  box width="0.75rem" height="0.75rem" flex-shrink=0 animation="spin 1s linear infinite" aria-hidden=true
                text "Save host" font-size="0.875rem" font-weight=500 line-height="1.25rem" color="#ffffff"

    keyframes spin
      from rotate=0
      to rotate=360

    component EasGhostButton
      props
        label: string
        destructive: bool
        disabled: bool
        press: action
        buttonId: string
      state over = false
      action hover(value: bool)
        over = value
      derive lit = over and not disabled
      view
        button id=buttonId press=press hover=hover disabled=disabled testId=buttonId display="flex" flex-direction="row" align-items="center" height="1.75rem" padding-left="0.5625rem" padding-right="0.5625rem" box-sizing="border-box" border-radius="0.5rem" border-width=0 background-color=(lit ? "light-dark(#f4f4f5, #ffffff0a)" : "#00000000") opacity=(disabled ? 0.64 : 1) data-focus-stop=(disabled ? "" : "enabled page")
          :active
            background-color="light-dark(#f4f4f5, #ffffff0a)"
          text label font-size="0.875rem" font-weight=500 line-height="1.25rem" color=(destructive ? (lit ? "light-dark(#fb2c36, #fb414a)" : "light-dark(#71717b, #a1a1a1)") : "light-dark(#18181b, #f5f5f5)") white-space="nowrap"

    """

    /// The save's end in the batch that mounts the row: the row's first
    /// layout raises a `resize` round whose commit lifts `busy`, presented
    /// after the mount's `present opacity 0.64` in one batch.
    static let round = """
    shape Host
      id: string

    component App
      state busy = false
      state hosts: Host[] = []
      action save
        busy = true
        hosts = [Host(id="a")]
      action laidOut(width: number, height: number)
        busy = false
      view
        column testId="page"
          button press=save testId="save"
            text "Save"
          for host in hosts key=host.id
            row flex-shrink=0 gap="0.25rem"
              EasGhostButton(label="Edit", destructive=false, disabled=busy, buttonId=`eas-host-edit-${host.id}`)
              box resize=laidOut width=1 height=1

    component EasGhostButton
      props
        label: string
        destructive: bool
        disabled: bool
        buttonId: string
      state over = false
      action hover(value: bool)
        over = value
      derive lit = over and not disabled
      view
        button id=buttonId hover=hover disabled=disabled testId=buttonId display="flex" flex-direction="row" align-items="center" height="1.75rem" padding-left="0.5625rem" padding-right="0.5625rem" box-sizing="border-box" border-radius="0.5rem" border-width=0 background-color=(lit ? "light-dark(#f4f4f5, #ffffff0a)" : "#00000000") opacity=(disabled ? 0.64 : 1) data-focus-stop=(disabled ? "" : "enabled page")
          :active
            background-color="light-dark(#f4f4f5, #ffffff0a)"
          text label font-size="0.875rem" font-weight=500 line-height="1.25rem" color=(destructive ? (lit ? "light-dark(#fb2c36, #fb414a)" : "light-dark(#71717b, #a1a1a1)") : "light-dark(#18181b, #f5f5f5)") white-space="nowrap"

    """

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    /// What the view draws at: its alpha, its layer's model and presented
    /// opacity, and no opacity animation held on it (a press's or a look's
    /// additive hold, `PressPaint.swift`).
    private func assertDrawn(_ view: NodeView, at alpha: Double, _ name: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(view.alphaValue, alpha, accuracy: 0.001, "\(name) alphaValue", file: file, line: line)
        let layer = view.layer
        XCTAssertEqual(Double(layer?.opacity ?? -1), alpha, accuracy: 0.001, "\(name) layer opacity", file: file, line: line)
        XCTAssertEqual(Double((layer?.presentation() ?? layer)?.opacity ?? -1), alpha, accuracy: 0.001, "\(name) presented opacity", file: file, line: line)
        let held = (layer?.animationKeys() ?? []).filter { (layer?.animation(forKey: $0) as? CAPropertyAnimation)?.keyPath == "opacity" }
        XCTAssertEqual(held, [], "\(name) opacity animations", file: file, line: line)
    }

    func testAButtonMountedDimmedIsDrawnWholeOnceItsOpacityReturnsToOne() throws {
        let f = try SheetFixture(page: Self.page, size: CGSize(width: 400, height: 300), data: ["focus-stop"])
        fixture = f
        try f.press("open")
        try f.press("save")
        let edit = try f.node("eas-host-edit-a"), remove = try f.node("eas-host-remove-a")
        assertDrawn(edit, at: 0.64, "Edit, mounted")
        assertDrawn(remove, at: 0.64, "Remove, mounted")
        try f.press("done")
        XCTAssertTrue(edit === (try f.node("eas-host-edit-a")), "the button stays mounted")
        XCTAssertFalse(f.presenter.views.values.contains { $0.props["testId"] == "overlay" }, "the overlay is gone")
        assertDrawn(edit, at: 1, "Edit, the save ended")
        assertDrawn(remove, at: 1, "Remove, the save ended")
    }

    func testAButtonMountedDimmedAndLiftedInTheSameBatchIsDrawnWhole() throws {
        let f = try SheetFixture(page: Self.round, size: CGSize(width: 400, height: 300), data: ["focus-stop"])
        fixture = f
        try f.press("save")
        assertDrawn(try f.node("eas-host-edit-a"), at: 1, "Edit, the save ended in its mount's batch")
    }
}
#endif
