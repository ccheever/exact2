// @ref LLP 1012#summary — bounded physical-device carrier admission before authentication.
import XCTest
import Darwin
@testable import ExactKit

final class AgentSocketStartupTests: XCTestCase {
    private func reservedLoopback() throws -> (Int32, sockaddr_in) {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(.ENFILE) }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        }
        guard bound == 0 else { close(fd); throw POSIXError(.EADDRINUSE) }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        let named = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) }
        }
        guard named == 0 else { close(fd); throw POSIXError(.EINVAL) }
        return (fd, address)
    }

    func testDelayedListenerRecoversAndHandsOffOneBlockingConnection() throws {
        let (reservation, address) = try reservedLoopback()
        // Darwin leaves a connect to a bound, unlistened TCP port pending.
        // A genuinely absent listener refuses immediately.
        close(reservation)
        var listener: Int32 = -1
        defer { if listener >= 0 { close(listener) } }
        let startup = AgentSocketStartup(active: true, timeout: 2)
        var refused = 0
        let client = try startup.connect(to: address, failedAttempt: { error in
            XCTAssertEqual(error, ECONNREFUSED)
            refused += 1
            // Advance on an actual refused socket, rather than a scheduling delay.
            listener = socket(AF_INET, SOCK_STREAM, 0)
            XCTAssertGreaterThanOrEqual(listener, 0)
            var destination = address
            let bound = withUnsafePointer(to: &destination) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(listener, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            }
            XCTAssertEqual(bound, 0)
            XCTAssertEqual(listen(listener, 1), 0)
        })
        defer { close(client) }
        XCTAssertEqual(refused, 1)
        XCTAssertEqual(fcntl(client, F_GETFL) & O_NONBLOCK, 0)
        let peer = accept(listener, nil, nil)
        XCTAssertGreaterThanOrEqual(peer, 0)
        guard peer >= 0 else { return }
        defer { close(peer) }
        startup.cancel() // Ownership already transferred; cancellation cannot close it.
        var byte: UInt8 = 42
        XCTAssertEqual(write(client, &byte, 1), 1)
        var received: UInt8 = 0
        XCTAssertEqual(read(peer, &received, 1), 1)
        XCTAssertEqual(received, 42)
    }

    func testInactiveStartupDoesNotConnectAndUsesTheSameDeadline() throws {
        let (listener, address) = try reservedLoopback()
        defer { close(listener) }
        XCTAssertEqual(listen(listener, 1), 0)
        let startup = AgentSocketStartup(active: false, timeout: 0.1)
        XCTAssertThrowsError(try startup.connect(to: address)) { error in
            guard case AgentSocketStartup.Failure.timedOut(let last) = error else { return XCTFail("unexpected \(error)") }
            XCTAssertEqual(last, 0)
        }
        XCTAssertEqual(fcntl(listener, F_SETFL, O_NONBLOCK), 0)
        XCTAssertEqual(accept(listener, nil, nil), -1)
        XCTAssertEqual(errno, EWOULDBLOCK)
        startup.setActive(true)
        XCTAssertThrowsError(try startup.connect(to: address)) // Activating cannot restart the deadline.
    }

    func testForegroundAdmissionWakesAnInactiveStartup() throws {
        let (listener, address) = try reservedLoopback()
        defer { close(listener) }
        XCTAssertEqual(listen(listener, 1), 0)
        let startup = AgentSocketStartup(active: false, timeout: 2)
        let waiting = expectation(description: "connect invoked")
        let connected = expectation(description: "foreground connect returned")
        DispatchQueue.global().async {
            do { let fd = try startup.connect(to: address, waitingForForeground: { waiting.fulfill() }); close(fd) }
            catch { XCTFail("unexpected \(error)") }
            connected.fulfill()
        }
        wait(for: [waiting], timeout: 1)
        startup.setActive(true)
        wait(for: [connected], timeout: 2)
        let peer = accept(listener, nil, nil)
        XCTAssertGreaterThanOrEqual(peer, 0)
        if peer >= 0 { close(peer) }
    }

    func testCancellationWakesInactiveStartupWithoutAConnection() throws {
        let (listener, address) = try reservedLoopback()
        defer { close(listener) }
        XCTAssertEqual(listen(listener, 1), 0)
        let startup = AgentSocketStartup(active: false, timeout: 10)
        let waiting = expectation(description: "connect invoked")
        let cancelled = expectation(description: "cancel returned before original deadline")
        DispatchQueue.global().async {
            do {
                let fd = try startup.connect(to: address, waitingForForeground: { waiting.fulfill() })
                close(fd)
                XCTFail("cancelled startup connected")
            } catch {
                guard case AgentSocketStartup.Failure.cancelled = error else { return XCTFail("unexpected \(error)") }
            }
            cancelled.fulfill()
        }
        wait(for: [waiting], timeout: 1)
        startup.cancel()
        wait(for: [cancelled], timeout: 1)
        XCTAssertEqual(fcntl(listener, F_SETFL, O_NONBLOCK), 0)
        XCTAssertEqual(accept(listener, nil, nil), -1)
        XCTAssertEqual(errno, EWOULDBLOCK)
    }

    func testUnreachableListenerHasOneBoundedDeadline() throws {
        for blackholed in [false, true] {
            let (reservation, address) = try reservedLoopback()
            if !blackholed { close(reservation) }
            defer { if blackholed { close(reservation) } }
            let startup = AgentSocketStartup(active: true, timeout: 0.25)
            var refusals = 0
            let began = DispatchTime.now().uptimeNanoseconds
            XCTAssertThrowsError(try startup.connect(to: address, failedAttempt: { error in
                XCTAssertEqual(error, ECONNREFUSED)
                refusals += 1
            })) { error in
                guard case AgentSocketStartup.Failure.timedOut(let last) = error else { return XCTFail("unexpected \(error)") }
                XCTAssertEqual(last, blackholed ? EINPROGRESS : ECONNREFUSED)
            }
            if blackholed { XCTAssertEqual(refusals, 0) }
            else { XCTAssertGreaterThan(refusals, 0) }
            XCTAssertLessThan(DispatchTime.now().uptimeNanoseconds - began, 2_000_000_000)
        }
    }
}
