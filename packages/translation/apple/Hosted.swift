// A translator's hosted view (LLP 1133.027 D9): before iOS and macOS 26 a
// `TranslationSession` exists only inside SwiftUI's `translationTask`, and on
// every version a download's sheet comes from one. Each translator that needs
// a session hosts a zero-size view carrying the task with its fixed
// configuration, under the session's own view (D17 item 3): on iOS a
// `UIHostingController` made a child of the window's root view controller,
// so it gets the appearance calls that start the task; on macOS an
// `NSHostingView`, which runs its tasks once it is in a window. Inside the
// task the session serves the translator's work, one job at a time, until the
// view disappears, which marks it dead before anything can call it.
#if canImport(Translation) && !os(tvOS)
import ExactPackage
import SwiftUI
import Translation

@available(iOS 18.0, macOS 15.0, *)
@MainActor
final class Hosted {
    /// One piece of work on the session; `failed` runs instead when the session died first.
    struct Job {
        let run: (TranslationSession) async -> Void
        let failed: () -> Void
        fileprivate var number = 0
        init(run: @escaping (TranslationSession) async -> Void, failed: @escaping () -> Void) { self.run = run; self.failed = failed }
    }

    private let stream: AsyncStream<Job>
    private let continuation: AsyncStream<Job>.Continuation
    private var waiting: [Job] = [], submitted = 0
    private(set) var dead = false
    #if os(macOS)
    private var host: NSHostingView<TaskView>?
    #else
    private var host: UIHostingController<TaskView>?
    #endif

    /// A view for `source` to `target` under `context`'s session view, or nil while that is in no window.
    init?(source: Locale.Language, target: Locale.Language, context: ExactPackageContext) {
        (stream, continuation) = AsyncStream.makeStream(of: Job.self)
        guard let view = context.view, let presentation = context.presentation else { return nil }
        let task = TaskView(configuration: TranslationSession.Configuration(source: source, target: target), hosted: self)
        #if os(macOS)
        _ = presentation
        let host = NSHostingView(rootView: task)
        host.frame = .zero
        view.addSubview(host)
        #else
        let host = UIHostingController(rootView: task)
        host.view.frame = .zero
        host.view.isUserInteractionEnabled = false
        host.view.backgroundColor = .clear
        presentation.rootViewController.addChild(host)
        view.addSubview(host.view)
        host.didMove(toParent: presentation.rootViewController)
        #endif
        self.host = host
    }

    /// Run `job` on the session, after the jobs before it.
    func submit(_ job: Job) {
        guard !dead else { return job.failed() }
        var job = job
        submitted += 1
        job.number = submitted
        waiting.append(job)
        continuation.yield(job)
    }

    /// The task's body: each job in order, while the view is shown.
    fileprivate func serve(_ session: TranslationSession) async {
        for await job in stream {
            guard !dead else { break }
            waiting.removeAll { $0.number == job.number }
            await job.run(session)
        }
    }

    /// The view left its window: the session must not be called again.
    fileprivate func disappeared() {
        guard !dead else { return }
        dead = true
        continuation.finish()
        let jobs = waiting
        waiting = []
        for job in jobs { job.failed() }
    }

    /// Take the view out; the session dies with it.
    func remove() {
        disappeared()
        #if os(macOS)
        host?.removeFromSuperview()
        #else
        host?.willMove(toParent: nil)
        host?.view.removeFromSuperview()
        host?.removeFromParent()
        #endif
        host = nil
    }
}

@available(iOS 18.0, macOS 15.0, *)
struct TaskView: View {
    let configuration: TranslationSession.Configuration
    let hosted: Hosted

    var body: some View {
        Color.clear
            .frame(width: 0, height: 0)
            .translationTask(configuration) { session in await hosted.serve(session) }
            .onDisappear { hosted.disappeared() }
    }
}
#endif
