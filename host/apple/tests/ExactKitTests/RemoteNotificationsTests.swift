#if !os(tvOS)
import Foundation
import XCTest
@testable import ExactKit

private final class RemoteReceipts: @unchecked Sendable {
    var opened = 0
    var tokens: [Data] = []
    var errors: [String] = []
}

final class RemoteNotificationsTests: XCTestCase {
    func testColdClickPersistsOpaqueJSONAndDuplicateCallbacksDeliverOnce() throws {
        let suite = "remote-click-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let center = NotificationCenter(), presenter = NotificationPresenter(), receipts = RemoteReceipts()
        let info: [AnyHashable: Any] = ["opaque": ["key": "value", "empty": NSNull()], "aps": ["alert": "Hello"]]
        let date = Date(timeIntervalSince1970: 100)
        // No module is observing when the launching click arrives.
        presenter.deliverRemote(id: "first", date: date, info: info, defaults: defaults, center: center)
        let data = try XCTUnwrap(defaults.data(forKey: ExactRemoteNotifications.pendingKey))
        let recovered = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertEqual((recovered["opaque"] as? [String: Any])?["key"] as? String, "value")
        let observer = center.addObserver(forName: ExactRemoteNotifications.openedEvent, object: nil, queue: nil) { _ in receipts.opened += 1 }
        defer { center.removeObserver(observer) }
        presenter.deliverRemote(id: "first", date: date, info: info, defaults: defaults, center: center)
        XCTAssertEqual(receipts.opened, 0)
        presenter.deliverRemote(id: "second", date: date, info: ["other": "payload"], defaults: defaults, center: center)
        XCTAssertEqual(receipts.opened, 1)
        let latest = try XCTUnwrap(defaults.data(forKey: ExactRemoteNotifications.pendingKey))
        let newer = try JSONSerialization.jsonObject(with: latest) as! [String: String]
        XCTAssertEqual(newer, ["other": "payload"])
        // An unencodable payload must not discard the unacknowledged click.
        ExactRemoteNotifications.opened(["bad": Date()], defaults: defaults, center: center)
        XCTAssertEqual(receipts.opened, 1)
        XCTAssertEqual(defaults.data(forKey: ExactRemoteNotifications.pendingKey), latest)
    }

    func testRegistrationRotationAndFailureAreForwardedWithoutInterpretingTokens() {
        let center = NotificationCenter.default, receipts = RemoteReceipts()
        let token = center.addObserver(forName: ExactRemoteNotifications.tokenEvent, object: nil, queue: nil) { note in
            if let data = note.object as? Data { receipts.tokens.append(data) }
        }
        let error = center.addObserver(forName: ExactRemoteNotifications.errorEvent, object: nil, queue: nil) { note in
            if let text = note.object as? String { receipts.errors.append(text) }
        }
        defer { center.removeObserver(token); center.removeObserver(error) }
        ExactRemoteNotifications.registered(Data([0, 255]))
        ExactRemoteNotifications.failed(NSError(domain: "PushTest", code: 1, userInfo: [NSLocalizedDescriptionKey: "Registration failed"]))
        ExactRemoteNotifications.registered(Data([1, 2]))
        XCTAssertEqual(receipts.tokens, [Data([0, 255]), Data([1, 2])])
        XCTAssertEqual(receipts.errors, ["Registration failed"])
    }
}
#endif
