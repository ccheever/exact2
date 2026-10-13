import Foundation

extension ExactSession {
    func relaunch() {
        #if os(macOS)
        guard ExactEnv.environment["EXACT_AGENT"] == nil else {
            log("relaunch refused: the agent owns this process; start a new drive on the same storage")
            return
        }
        delegate?.exactSession(self, command: "relaunch", args: [])
        #else
        log("relaunch refused: this platform does not allow an app to restart its process")
        #endif
    }
}

#if os(macOS)
/// The replacement waits outside AppKit until the old process has exited.
/// Start it only after the quit decision, storage drain and session teardown.
public enum ProcessRelaunch {
    private static let waitArgument = "--exact-relaunch-wait"

    public static func requestQuit(holding: @escaping () -> Bool, terminate: @escaping () -> Void) {
        RunLoop.main.perform(inModes: [.common]) {
            // beforeunload can request relaunch before its quit takes the storage hold.
            // AppKit may force a repeated terminate without asking the delegate.
            guard !holding() else { return }
            terminate()
        }
    }

    public static func start() throws {
        let process = Process()
        process.executableURL = Bundle.main.executableURL
        process.arguments = [waitArgument, String(getpid())] + CommandLine.arguments.dropFirst()
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        try process.run()
    }

    /// Called before the standalone host creates its application or sessions.
    public static func waitIfRequested() {
        let args = CommandLine.arguments
        guard args.count >= 3, args[1] == waitArgument, let parent = Int32(args[2]), parent > 0 else { return }
        let exited = DispatchSemaphore(value: 0)
        let source = DispatchSource.makeProcessSource(identifier: parent, eventMask: .exit, queue: .global())
        source.setEventHandler { exited.signal() }
        source.resume()
        if kill(parent, 0) == 0 { exited.wait() }
        source.cancel()
        let argv = ([args[0]] + args.dropFirst(3)).map { strdup($0) } + [nil]
        argv.withUnsafeBufferPointer { _ = execv(args[0], $0.baseAddress!) }
        fputs("exact: relaunch failed: \(String(cString: strerror(errno)))\n", stderr)
        exit(1)
    }
}
#endif
