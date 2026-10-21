// @ref LLP 1115.002 D7.2 — a `dialog` that is not alert-shaped is a sheet.
// Every case of `DialogMacTests` again, with the dialog in AppKit's sheet,
// as a person gets it: what a dialog is to the app (the modal focus, its
// Tab, Escape, `closedby`, the default button's Return, what a batch does
// to an open one) holds in either presentation. The cases below are the
// sheet's own.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class DialogSheetMacTests: DialogMacTests {
    override var painted: Bool { false }
}
#endif
