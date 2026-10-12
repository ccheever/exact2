import Foundation

private final class QuestionFaultDisk: T3QuestionAttachmentDisk {
    enum Failure { case none, beforeReplace, afterReplace }
    let files: T3QuestionAttachmentFiles
    private let lock = NSLock()
    private var failure: Failure = .none
    init(_ root: URL) { files = .init(dataRoot: root) }
    func arm(_ failure: Failure) { lock.lock(); self.failure = failure; lock.unlock() }
    private func take() -> Failure { lock.lock(); defer { lock.unlock() }; let value = failure; failure = .none; return value }
    func load() throws -> Data? { try files.load() }
    func replace(_ data: Data) throws {
        let failure = take()
        if failure == .beforeReplace { throw CocoaError(.fileWriteOutOfSpace) }
        try files.replace(data)
        if failure == .afterReplace { throw CocoaError(.fileWriteUnknown) }
    }
    func write(_ id: String, bytes: Data) throws { try files.write(id, bytes: bytes) }
    func read(_ id: String, limit: Int) throws -> Data { try files.read(id, limit: limit) }
    func remove(_ id: String) throws { try files.remove(id) }
}

@main struct QuestionAttachmentInventoryTests {
    private struct Check {
        let name: String
        let passed: Bool
        let detail: String
    }
    private static var checks: [Check] = []
    private static var roots: [URL] = []
    private static let scope = try! T3QuestionScope(origin: "https://fixture.test", environment: "environment",
                                                    thread: "thread", request: "pending-request")
    private static let questions = ["one": "signature-one", "two": "signature-two"]

    private static func check(_ value: Bool, _ name: String, _ detail: String = "") {
        checks.append(.init(name: name, passed: value, detail: detail))
    }
    private static func run(_ name: String, _ body: () throws -> Void) {
        do { try body() } catch { checks.append(.init(name: name, passed: false, detail: String(describing: error))) }
    }
    private static func refuses(_ name: String, _ body: () throws -> Void) {
        do { try body(); check(false, name, "unexpected success") }
        catch { check(true, name) }
    }
    private static func root(_ label: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("question-attachment-\(label)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        roots.append(url)
        return url
    }
    private static func admission(_ owner: T3QuestionAttachments, _ question: String = "one",
                                  generation: Int = 1, route: String = "/questions", mount: UUID = UUID()) throws -> (String, T3QuestionAdmission) {
        let request = try owner.register(scope, questions: questions)
        let value = try owner.admit(request: request, question: question, signature: questions[question]!,
                                    generation: generation, route: route, mount: mount, fileLimit: 50 * 1024 * 1024)
        return (request, value)
    }
    private static func stage(_ owner: T3QuestionAttachments, _ operation: T3QuestionOperation,
                              ordinal: Int, bytes: Data) throws -> T3QuestionReservation {
        let ticket = try owner.reserve(operation, ordinal: ordinal)
        let metadata = T3QuestionAttachmentMetadata(kind: .file, name: "file-\(ordinal).bin",
                                                     mimeType: "application/octet-stream")
        try owner.stage(ticket, metadata: metadata) { bytes }
        return ticket
    }
    private static func add(_ owner: T3QuestionAttachments, _ admission: T3QuestionAdmission,
                            bytes: Data, ordinal: Int = 0) throws -> (T3QuestionOperation, T3QuestionAttachment) {
        let operation = try owner.open(admission, kind: "files")
        _ = try stage(owner, operation, ordinal: ordinal, bytes: bytes)
        return (operation, try owner.adopt(operation).first!)
    }
    private static func path(_ root: URL, _ id: String) -> URL {
        root.appendingPathComponent("question-attachments", isDirectory: true).appendingPathComponent(id)
    }

    private static func child(_ mode: String, root: URL) throws {
        let owner = T3QuestionAttachments.shared(dataRoot: root)
        let (request, admitted) = try admission(owner)
        let (operation, item) = try add(owner, admitted, bytes: Data("restart-adopted".utf8))
        if mode == "child-unadopted" {
            let second = try owner.open(admitted, kind: "files")
            _ = try stage(owner, second, ordinal: 0, bytes: Data("restart-unadopted".utf8))
            print("{\"request\":\"\(request)\",\"operation\":\"\(second.id)\",\"fingerprint\":\"\(second.fingerprint)\"}")
        } else {
            print("{\"request\":\"\(request)\",\"operation\":\"\(operation.id)\",\"fingerprint\":\"\(operation.fingerprint)\",\"item\":\"\(item.id)\"}")
        }
    }
    private static func launchChild(_ mode: String, root: URL) throws -> [String: String] {
        let process = Process(), pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: CommandLine.arguments[0])
        process.arguments = [mode, root.path]
        process.standardOutput = pipe; process.standardError = pipe
        try process.run(); process.waitUntilExit()
        let output = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        guard process.terminationStatus == 0,
              let line = output.split(separator: "\n").last,
              let value = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: String] else {
            throw T3QuestionAttachmentFailure("Restart child failed: \(output)")
        }
        return value
    }

    private static func lazyAndIdentity() throws {
        let parent = try root("lazy"), dataRoot = parent.appendingPathComponent("not-created", isDirectory: true)
        let owner = T3QuestionAttachments.shared(dataRoot: dataRoot)
        check(!FileManager.default.fileExists(atPath: dataRoot.path), "Construction performs no storage read or creation")
        try FileManager.default.createDirectory(at: dataRoot, withIntermediateDirectories: true)
        let first = try owner.register(scope, questions: questions)
        check(try owner.register(scope, questions: questions) == first, "Register replays stable live request incarnation")
        refuses("Changed live question signature is refused") {
            _ = try owner.register(scope, questions: ["one": "changed", "two": "signature-two"])
        }
        let admitted = try owner.admit(request: first, question: "one", signature: "signature-one",
                                       generation: 1, route: "/a", mount: UUID(), fileLimit: 1024)
        let opened = try owner.open(admitted, kind: "files")
        refuses("Unknown operation kind is refused") { _ = try owner.open(admitted, kind: "upload") }
        refuses("Malformed replay ID is refused") { _ = try owner.replay(request: first, operation: "bad", fingerprint: opened.fingerprint) }
        refuses("Changed operation fingerprint is refused") {
            _ = try owner.replay(request: first, operation: opened.id, fingerprint: String(repeating: "0", count: 64))
        }
        try owner.cancel(opened)
        check(try owner.replay(request: first, operation: opened.id, fingerprint: opened.fingerprint) == [],
              "Cancelled operation result replays")
        try owner.retire(request: first)
        let next = try owner.register(scope, questions: questions)
        check(next != first, "Retired reused request identity receives a fresh native incarnation")
    }

    private static func capacityAndOrder() throws {
        let dataRoot = try root("capacity"), owner = T3QuestionAttachments.shared(dataRoot: dataRoot)
        let (request, one) = try admission(owner), two = try owner.admit(
            request: try owner.register(scope, questions: questions), question: "two", signature: "signature-two",
            generation: 1, route: "/questions", mount: UUID(), fileLimit: 50 * 1024 * 1024)
        let bulk = try owner.open(one, kind: "files")
        for index in 0..<99 { _ = try stage(owner, bulk, ordinal: index, bytes: Data("item-\(index)".utf8)) }
        let adopted = try owner.adopt(bulk), remaining = try owner.remaining(one)
        check(adopted.count == 99 && remaining == 1, "Ninety-nine items share one request capacity across questions")

        let a = try owner.open(one, kind: "paste"), b = try owner.open(two, kind: "photos")
        let resultLock = NSLock(), group = DispatchGroup()
        var winners: [(T3QuestionReservation, T3QuestionOperation)] = [], failures = 0
        for operation in [a, b] {
            group.enter()
            DispatchQueue.global().async {
                do {
                    let ticket = try owner.reserve(operation, ordinal: 0)
                    resultLock.lock(); winners.append((ticket, operation)); resultLock.unlock()
                } catch {
                    resultLock.lock(); failures += 1; resultLock.unlock()
                }
                group.leave()
            }
        }
        group.wait()
        check(winners.count == 1 && failures == 1, "Overlapping picker and paste can reserve only the hundredth slot")
        if let winner = winners.first {
            let metadata = T3QuestionAttachmentMetadata(kind: .file, name: "last.bin", mimeType: "application/octet-stream")
            try owner.stage(winner.0, metadata: metadata) { Data("last".utf8) }
            _ = try owner.adopt(winner.1)
        }
        check((try owner.items(request: request, question: "one").count
               + owner.items(request: request, question: "two").count) == 100,
              "Capacity counts adopted and reserved slots exactly once")

        let orderRoot = try root("order"), orderOwner = T3QuestionAttachments.shared(dataRoot: orderRoot)
        let (_, admitted) = try admission(orderOwner)
        let operation = try orderOwner.open(admitted, kind: "paste")
        let tickets = try (0..<4).map { try orderOwner.reserve(operation, ordinal: $0) }
        try orderOwner.discard(tickets[1])
        for index in [3, 2, 0] {
            let metadata = T3QuestionAttachmentMetadata(kind: .file, name: "\(index).bin", mimeType: "application/octet-stream")
            try orderOwner.stage(tickets[index], metadata: metadata) { Data("\(index)".utf8) }
        }
        let ordered = try orderOwner.adopt(operation)
        check(ordered.map(\.metadata.name) == ["0.bin", "2.bin", "3.bin"],
              "Reverse provider completion preserves ordinals and skips unreadable member")
        check(try orderOwner.replay(request: ordered[0].request, operation: operation.id,
                                    fingerprint: operation.fingerprint) == ordered,
              "Lost adoption reply replays immutable ordered result")
    }

    private static func cancellationAndAdmission() throws {
        let dataRoot = try root("cancel"), owner = T3QuestionAttachments.shared(dataRoot: dataRoot)
        let (_, admitted) = try admission(owner)
        let operation = try owner.open(admitted, kind: "files"), ticket = try owner.reserve(operation, ordinal: 0)
        let started = DispatchSemaphore(value: 0), resume = DispatchSemaphore(value: 0), done = DispatchGroup()
        var stageFailed = false
        done.enter()
        DispatchQueue.global().async {
            do {
                try owner.stage(ticket, metadata: .init(kind: .file, name: "held.bin", mimeType: "application/octet-stream")) {
                    started.signal(); resume.wait(); return Data("held".utf8)
                }
            } catch { stageFailed = true }
            done.leave()
        }
        started.wait()
        try owner.cancel(operation)
        check(!FileManager.default.fileExists(atPath: path(dataRoot, ticket.id).path),
              "Cancellation takes logical effect while writer hold remains")
        resume.signal(); done.wait()
        check(stageFailed && !FileManager.default.fileExists(atPath: path(dataRoot, ticket.id).path),
              "Cancelled writer cannot adopt and cleanup waits for writer completion")

        let old = try owner.open(admitted, kind: "files"), oldTicket = try owner.reserve(old, ordinal: 0)
        let replacement = try owner.admit(request: admitted.request, question: "one", signature: "signature-one",
                                          generation: 2, route: "/replacement", mount: UUID(), fileLimit: 50 * 1024 * 1024)
        check(try owner.remaining(replacement) == 100, "Replacement admission durably releases old reservations")
        refuses("Old admission cannot stage after replacement") {
            try owner.stage(oldTicket, metadata: .init(kind: .file, name: "old.bin", mimeType: "application/octet-stream")) { Data("old".utf8) }
        }
        check(try owner.replay(request: admitted.request, operation: old.id, fingerprint: old.fingerprint) == [],
              "Replacement admission records old cancellation before admitting new work")

        let adopted = try add(owner, replacement, bytes: Data("adopted".utf8))
        try owner.cancel(adopted.0)
        check(try owner.items(request: replacement.request, question: "one").contains(adopted.1),
              "Cancellation after durable adoption cannot retract an escaped item")
    }

    private static func stagedRemoval() throws {
        let dataRoot = try root("staged-remove"), owner = T3QuestionAttachments.shared(dataRoot: dataRoot)
        let (_, admitted) = try admission(owner)
        let staged = try owner.open(admitted, kind: "files")
        let stagedTicket = try stage(owner, staged, ordinal: 0, bytes: Data("staged-remove".utf8))
        try owner.remove(admitted, id: stagedTicket.id)
        let stagedAdoption = try owner.adopt(staged)
        let stagedRemaining = try owner.remaining(admitted)
        check(stagedAdoption.isEmpty && stagedRemaining == 100
              && !FileManager.default.fileExists(atPath: path(dataRoot, stagedTicket.id).path),
              "Removing a staged slot durably frees capacity and prevents later adoption")
        try owner.remove(admitted, id: stagedTicket.id)
        check(try owner.replay(request: admitted.request, operation: staged.id, fingerprint: staged.fingerprint) == [],
              "Repeating staged removal remains absent after operation adoption")

        let producing = try owner.open(admitted, kind: "files")
        let producingTicket = try owner.reserve(producing, ordinal: 0)
        let started = DispatchSemaphore(value: 0), resume = DispatchSemaphore(value: 0), done = DispatchGroup()
        var producerFailed = false
        done.enter()
        DispatchQueue.global().async {
            do {
                try owner.stage(producingTicket, metadata: .init(kind: .file, name: "producing.bin", mimeType: "application/octet-stream")) {
                    started.signal(); resume.wait(); return Data("late-producer".utf8)
                }
            } catch { producerFailed = true }
            done.leave()
        }
        started.wait()
        try owner.remove(admitted, id: producingTicket.id)
        check(try owner.remaining(admitted) == 100, "Removal frees a producer-held reservation logically before its callback ends")
        resume.signal(); done.wait()
        let producingAdoption = try owner.adopt(producing)
        check(producerFailed && producingAdoption.isEmpty
              && !FileManager.default.fileExists(atPath: path(dataRoot, producingTicket.id).path),
              "A removed producer-held slot cannot write late bytes or publish")

        let lostRoot = try root("staged-remove-lost-write"), disk = QuestionFaultDisk(lostRoot)
        let lostOwner = T3QuestionAttachments(disk: disk), (_, lostAdmission) = try admission(lostOwner)
        let lost = try lostOwner.open(lostAdmission, kind: "files")
        let lostTicket = try lostOwner.reserve(lost, ordinal: 0)
        disk.arm(.afterReplace)
        refuses("Lost staged-write acknowledgement is surfaced before removal") {
            try lostOwner.stage(lostTicket, metadata: .init(kind: .file, name: "lost.bin", mimeType: "application/octet-stream")) {
                Data("lost-write".utf8)
            }
        }
        try lostOwner.remove(lostAdmission, id: lostTicket.id)
        let lostAdoption = try lostOwner.adopt(lost), lostRemaining = try lostOwner.remaining(lostAdmission)
        check(lostAdoption.isEmpty && lostRemaining == 100
              && !FileManager.default.fileExists(atPath: path(lostRoot, lostTicket.id).path),
              "Removal reconciles a durably staged write whose acknowledgement was lost")
    }

    private static func leasesAndIsolation() throws {
        let dataRoot = try root("lease"), owner = T3QuestionAttachments.shared(dataRoot: dataRoot)
        let (request, admitted) = try admission(owner)
        let (_, item) = try add(owner, admitted, bytes: Data("leased bytes".utf8))
        let lease = try owner.lease(request: request, question: "one", id: item.id)
        try owner.remove(admitted, id: item.id)
        check(try owner.items(request: request, question: "one").isEmpty, "Removal frees logical capacity immediately")
        let leasedBytes = try lease.read()
        check(FileManager.default.fileExists(atPath: path(dataRoot, item.id).path)
              && leasedBytes == Data("leased bytes".utf8), "Original bytes survive logical removal under a real lease")
        try owner.remove(admitted, id: item.id)
        check(FileManager.default.fileExists(atPath: path(dataRoot, item.id).path), "Repeated removal cannot release another owner's lease")
        lease.release(); lease.release()
        check(!FileManager.default.fileExists(atPath: path(dataRoot, item.id).path), "Last idempotent lease release permits physical deletion")
        refuses("Released lease cannot read") { _ = try lease.read() }

        let (_, another) = try add(owner, admitted, bytes: Data("changed".utf8))
        let changedLease = try owner.lease(request: request, question: "one", id: another.id)
        try Data("mutated".utf8).write(to: path(dataRoot, another.id))
        refuses("Mutated original bytes fail fingerprint verification") { _ = try changedLease.read() }
        changedLease.release()
        refuses("Question-local lookup does not cross question namespace") {
            _ = try owner.lease(request: request, question: "two", id: another.id)
        }
        let otherScope = try T3QuestionScope(origin: "https://other.test", environment: "environment",
                                             thread: "thread", request: "pending-request")
        let otherRequest = try owner.register(otherScope, questions: questions)
        refuses("Question-local lookup does not cross request namespace") {
            _ = try owner.lease(request: otherRequest, question: "one", id: another.id)
        }
    }

    private static func restart() throws {
        let unadoptedRoot = try root("restart-open"), childOpen = try launchChild("child-unadopted", root: unadoptedRoot)
        let reopened = T3QuestionAttachments.shared(dataRoot: unadoptedRoot)
        let (request, admitted) = try admission(reopened), remaining = try reopened.remaining(admitted)
        check(request == childOpen["request"] && remaining == 99,
              "Actual restart cancels unadopted work but preserves separately adopted item")
        check(try reopened.replay(request: request, operation: childOpen["operation"]!,
                                  fingerprint: childOpen["fingerprint"]!) == [],
              "Actual restart durably replays old-process cancellation")

        let adoptedRoot = try root("restart-adopted"), childAdopted = try launchChild("child-adopted", root: adoptedRoot)
        let adoptedOwner = T3QuestionAttachments.shared(dataRoot: adoptedRoot)
        let adoptedRequest = try adoptedOwner.register(scope, questions: questions)
        let items = try adoptedOwner.items(request: adoptedRequest, question: "one")
        check(items.count == 1 && items[0].id == childAdopted["item"],
              "Actual restart preserves adopted result after lost caller reply")
        check(try adoptedOwner.replay(request: adoptedRequest, operation: childAdopted["operation"]!,
                                      fingerprint: childAdopted["fingerprint"]!) == items,
              "Actual restart replays adopted operation result")
    }

    private static func persistenceFailures() throws {
        for failure in [QuestionFaultDisk.Failure.beforeReplace, .afterReplace] {
            let dataRoot = try root("initial-\(failure)"), disk = QuestionFaultDisk(dataRoot)
            let owner = T3QuestionAttachments(disk: disk)
            disk.arm(failure)
            refuses("Initial journal \(failure) failure is surfaced") { _ = try owner.register(scope, questions: questions) }
            let request = try owner.register(scope, questions: questions)
            check(!request.isEmpty, "Empty first-save \(failure) state remains recoverable")
        }

        let dataRoot = try root("transition-failures"), disk = QuestionFaultDisk(dataRoot)
        let owner = T3QuestionAttachments(disk: disk), (request, admitted) = try admission(owner)
        let operation = try owner.open(admitted, kind: "files"), ticket = try owner.reserve(operation, ordinal: 0)
        disk.arm(.afterReplace)
        refuses("Lost stage journal acknowledgement is surfaced") {
            try owner.stage(ticket, metadata: .init(kind: .file, name: "fault.bin", mimeType: "application/octet-stream")) { Data("fault".utf8) }
        }
        var replayedProducer = false
        try owner.stage(ticket, metadata: .init(kind: .file, name: "fault.bin", mimeType: "application/octet-stream")) {
            replayedProducer = true
            return Data("fault".utf8)
        }
        check(!replayedProducer, "Staged operation replay verifies durable bytes without repeating conversion")
        disk.arm(.afterReplace)
        refuses("Lost adoption acknowledgement is surfaced") { _ = try owner.adopt(operation) }
        let adopted = try owner.adopt(operation)
        check(adopted.count == 1, "Adoption reconciles ambiguous successful replacement")
        disk.arm(.afterReplace)
        refuses("Lost removal acknowledgement is surfaced") { try owner.remove(admitted, id: adopted[0].id) }
        try owner.remove(admitted, id: adopted[0].id)
        check(try owner.items(request: request, question: "one").isEmpty,
              "Removal reconciles ambiguous successful replacement idempotently")

        let cancel = try owner.open(admitted, kind: "files")
        _ = try owner.reserve(cancel, ordinal: 0)
        disk.arm(.afterReplace)
        refuses("Lost cancellation acknowledgement is surfaced") { try owner.cancel(cancel) }
        try owner.cancel(cancel)
        check(try owner.replay(request: request, operation: cancel.id, fingerprint: cancel.fingerprint) == [],
              "Cancellation reconciles ambiguous successful replacement")
    }

    private static func corruptAndPaths() throws {
        let missingRoot = try root("missing")
        let missingDirectory = missingRoot.appendingPathComponent("question-attachments", isDirectory: true)
        try FileManager.default.createDirectory(at: missingDirectory, withIntermediateDirectories: true)
        let unknown = missingDirectory.appendingPathComponent(UUID().uuidString.lowercased())
        try Data("unknown".utf8).write(to: unknown)
        refuses("Nonempty store missing inventory fails closed") {
            _ = try T3QuestionAttachments(disk: T3QuestionAttachmentFiles(dataRoot: missingRoot)).register(scope, questions: questions)
        }
        check(try Data(contentsOf: unknown) == Data("unknown".utf8), "Missing inventory retains unknown bytes")

        let corruptRoot = try root("corrupt"), owner = T3QuestionAttachments.shared(dataRoot: corruptRoot)
        _ = try owner.register(scope, questions: questions)
        let inventory = corruptRoot.appendingPathComponent("question-attachments/inventory.json")
        var object = try JSONSerialization.jsonObject(with: Data(contentsOf: inventory)) as! [String: Any]
        object["unknown"] = true
        try JSONSerialization.data(withJSONObject: object).write(to: inventory)
        refuses("Unknown persisted record field fails strict decoding") {
            _ = try T3QuestionAttachments(disk: T3QuestionAttachmentFiles(dataRoot: corruptRoot)).register(scope, questions: questions)
        }
        check(FileManager.default.fileExists(atPath: inventory.path), "Corrupt inventory remains for recovery")

        let realRoot = try root("symlink-real"), linkedRoot = realRoot.deletingLastPathComponent()
            .appendingPathComponent("question-symlink-\(UUID().uuidString)")
        try FileManager.default.createSymbolicLink(at: linkedRoot, withDestinationURL: realRoot)
        defer { try? FileManager.default.removeItem(at: linkedRoot) }
        refuses("Symlink data root is refused") {
            _ = try T3QuestionAttachments(disk: T3QuestionAttachmentFiles(dataRoot: linkedRoot)).register(scope, questions: questions)
        }

        let itemRoot = try root("symlink-item"), itemOwner = T3QuestionAttachments.shared(dataRoot: itemRoot)
        let (request, admitted) = try admission(itemOwner)
        let (_, item) = try add(itemOwner, admitted, bytes: Data("inside".utf8))
        let outside = itemRoot.appendingPathComponent("outside")
        try Data("outside".utf8).write(to: outside)
        try FileManager.default.removeItem(at: path(itemRoot, item.id))
        try FileManager.default.createSymbolicLink(at: path(itemRoot, item.id), withDestinationURL: outside)
        let lease = try itemOwner.lease(request: request, question: "one", id: item.id)
        refuses("Symlink-owned item is refused without reading target") { _ = try lease.read() }
        check(try Data(contentsOf: outside) == Data("outside".utf8), "Symlink target remains unchanged")
        lease.release()
    }

    private static func mainBatch() {
        for (name, body) in [
            ("lazy/identity", lazyAndIdentity),
            ("capacity/order", capacityAndOrder),
            ("cancellation/admission", cancellationAndAdmission),
            ("staged removal", stagedRemoval),
            ("leases/isolation", leasesAndIsolation),
            ("actual restart", restart),
            ("persistence failures", persistenceFailures),
            ("corrupt/path integrity", corruptAndPaths),
        ] { run(name, body) }
    }

    static func main() throws {
        if CommandLine.arguments.count == 3, CommandLine.arguments[1].hasPrefix("child-") {
            try child(CommandLine.arguments[1], root: URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true))
            return
        }
        mainBatch()
        let failed = checks.filter { !$0.passed }
        let result: [String: Any] = [
            "checks": checks.count,
            "passed": checks.count - failed.count,
            "failed": failed.map { ["name": $0.name, "detail": $0.detail] },
            "pid": ProcessInfo.processInfo.processIdentifier,
            "method": "Production question owner and production POSIX filesystem; isolated roots; actual child-process restart.",
        ]
        print(String(decoding: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), as: UTF8.self))
        for root in roots { try? FileManager.default.removeItem(at: root) }
        if !failed.isEmpty { throw T3QuestionAttachmentFailure("\(failed.count) question attachment checks failed") }
    }
}
