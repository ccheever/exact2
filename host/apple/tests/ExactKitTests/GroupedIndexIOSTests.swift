#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// A list whose sections are each headed by one letter (Contacts' A to Z)
/// has UIKit's section index, as a hand-built list of them does; any other
/// list has none (contacts, round 29).
final class GroupedIndexIOSTests: GroupedListFixture {
    func lettered(_ headers: [String?]) -> GroupedListModel {
        var m = model()
        for (i, h) in headers.enumerated() { m.sections[i].header = h }
        return m
    }

    func titles(_ p: Presenter) throws -> [String]? {
        let c = try list(p).collection
        return c.dataSource?.indexTitles?(for: c)
    }

    func testLetteredSectionsHaveAnIndex() throws {
        let p = presenter { self.lettered(["A", "B"]) }
        defer { window.isHidden = true }
        XCTAssertEqual(try titles(p), ["A", "B"])
        let c = try list(p).collection
        XCTAssertEqual(c.dataSource?.collectionView?(c, indexPathForIndexTitle: "B", at: 1), IndexPath(item: 0, section: 1),
                       "a jump lands on the section's first row")
    }

    /// Favorites above the letters (bake-off r28's contacts): the letters keep
    /// their index, and a jump to one lands past the sections above them.
    func testSectionsAboveTheLettersAreLeftOutOfTheIndex() throws {
        let p = presenter {
            var m = self.lettered(["Favorites", "A"])
            m.sections.append(.init(view: 40, header: "B", footer: nil, rows: [Row(view: 30, title: "Ada", pressable: true)]))
            return m
        }
        defer { window.isHidden = true }
        XCTAssertEqual(try titles(p), ["A", "B"])
        let c = try list(p).collection
        XCTAssertEqual(c.dataSource?.collectionView?(c, indexPathForIndexTitle: "B", at: 1), IndexPath(item: 0, section: 2))
    }

    func testOtherSectionsHaveNone() throws {
        for headers in [["Account", nil], ["A", nil], ["A", "Bo"], ["A", "Favorites"]] as [[String?]] {
            let p = presenter { self.lettered(headers) }
            XCTAssertNil(try titles(p), "\(headers)")
            window.isHidden = true
        }
        let p = presenter {
            var m = self.lettered(["A", "B"])
            m.sections[1].rows = []
            return m
        }
        defer { window.isHidden = true }
        XCTAssertNil(try titles(p), "a section with no row has no place to jump to")
    }
}
#endif
