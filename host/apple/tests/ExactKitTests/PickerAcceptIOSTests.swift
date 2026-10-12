import XCTest
import UniformTypeIdentifiers
@testable import ExactKit

final class PickerAcceptIOSTests: XCTestCase {
    func testUnrestrictedFileInputUsesDocumentItem() {
        XCTAssertEqual(Picker.types([]), [.item])
        XCTAssertEqual(Picker.types(["*/*"]), [.item])
        XCTAssertEqual(Picker.types(["image/*"]), [.image])
        XCTAssertFalse(Picker.converts("image/heic", accept: ["image/jpeg", "*/*"]))
        XCTAssertTrue(Picker.converts("image/heic", accept: ["image/jpeg"]))
    }

    func testUnknownFileIsCopied() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("archive.xyz")
        let target = directory.appendingPathComponent("copied.xyz")
        try Data("any bytes".utf8).write(to: source)
        try Picker.copy(source, to: target, accept: [])
        XCTAssertEqual(try Data(contentsOf: target), Data("any bytes".utf8))
    }
}
