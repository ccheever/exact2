#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A node that mounts at one `opacity` and changes it in a later commit is
/// drawn at the new value: a row of buttons mounted disabled at 0.64 while a
/// save runs is drawn whole once the save ends, as the web draws it.
final class MountOpacityMacTests: XCTestCase {
    static let page = """
    shape Host
      id: string

    component App
      state busy = false
      state hosts: Host[] = []
      action save
        busy = true
        hosts = [Host(id="a")]
      state pressed = 0
      action done
        busy = false
        close("editor")
      action edit
        pressed = pressed + 1
      view
        column testId="page"
          button testId="open" commandfor="editor" command="show-modal"
            text "Add"
          for host in hosts key=host.id
            row
              Ghost(label="Edit", disabled=busy, press=edit, buttonId=`edit-${host.id}`)
          dialog id="editor" testId="editor" padding=20 width=300
            column gap=12
              button press=save testId="save"
                text "Save"
              button press=done testId="done"
                text "Done"

    component Ghost
      props
        label: string
        disabled: bool
        buttonId: string
        press: action
      state over = false
      action hover(value: bool)
        over = value
      derive lit = over and not disabled
      view
        button press=press hover=hover disabled=disabled testId=buttonId display="flex" height="1.75rem" border-width=0 background-color=(lit ? "#f4f4f5" : "#00000000") opacity=(disabled ? 0.64 : 1)
          :active
            background-color="#f4f4f5"
          text label font-size="0.875rem" white-space="nowrap"

    """

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    func testAButtonMountedDimmedIsDrawnWholeOnceItsOpacityReturnsToOne() throws {
        let f = try SheetFixture(page: Self.page, size: CGSize(width: 400, height: 300))
        fixture = f
        try f.press("open")
        try f.press("save")
        let edit = try f.node("edit-a")
        XCTAssertEqual(edit.alphaValue, 0.64, accuracy: 0.001)
        try f.press("done")
        XCTAssertTrue(edit === (try f.node("edit-a")), "the button stays mounted")
        XCTAssertEqual(edit.alphaValue, 1, accuracy: 0.001)
        XCTAssertEqual(edit.layer?.opacity ?? 0, 1, accuracy: 0.001)
    }
}
#endif
