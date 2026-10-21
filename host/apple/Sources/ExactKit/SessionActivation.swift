// What follows first pixel: the app module, the data source, native views.
// @ref LLP 1027 D4 — app logic loads after first pixel.
// @ref LLP 1024 D3 — native views attach after the paint gate.
import Foundation
import ExactCore

extension ExactSession {
    func firstDrawn(generation drawnGeneration: Int, token: UInt64) {
        guard state != .destroyed, generation == drawnGeneration else { return }
        if firstDrawMs == nil { firstDrawMs = ExactEnv.wall() }
        app.sharedStore.firstPixel()
        guard activatedGeneration != drawnGeneration else { return }
        activatedGeneration = drawnGeneration
        DispatchQueue.main.async { [weak self] in
            guard let self, state != .destroyed, generation == drawnGeneration else { return }
            // Prepare before any source calls the module (LLP 1067.000 D8).
            natives.prepareAppModule(); presenter.releaseLaunchAutofocus() // the next turn, never waiting on a loading source
            restoreKeptDocuments() // before any answer reads a kept document (LLP 1069.010.000 D4)
            if preparedGeneration != drawnGeneration { prepareData(drawnGeneration, token: token) } else { activateData(drawnGeneration, token: token) }
        }
    }

    /// Whether activation still has work queued: the agent's settle waits for it.
    var activationInFlight: Bool { natives.activationQueued || dataPreparing }

    /// The data source activates on the owner thread, where a TypeScript
    /// module evaluates its top level, while this thread goes on: images
    /// land, native views attach and input is handled. Waiting here instead
    /// held this thread for the whole evaluation (325 ms for a 2 MB catalog
    /// on an iPhone 17 Pro Max). A call this thread makes to the runtime
    /// meanwhile still waits its turn behind it: on a cold launch, the
    /// page's facts when the app becomes active do.
    private func prepareData(_ drawnGeneration: Int, token: UInt64) {
        preparedGeneration = drawnGeneration
        dataPreparing = true
        openNativeViews(drawnGeneration)
        let runtime = runtime
        Owner.shared.post {
            runtime.prepareData()
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                dataPreparing = false
                guard state != .destroyed, generation == drawnGeneration else { return }
                activateData(drawnGeneration, token: token)
            }
        }
    }

    private func activateData(_ drawnGeneration: Int, token: UInt64) {
        let batch = runtime.dataReady()
        if batch.pending {
            pendingActivation = (drawnGeneration, token) // the generation stays activated: only the source's wake retries
            return
        }
        appFileRoots = AppFiles.learn(runtime) // the roots storage configured
        apply(batch)
        dataGeneration = drawnGeneration
        activityOutput?.drain()
        storeRefresh.activated = { [weak self] in
            guard let self, state != .destroyed, generation == drawnGeneration else { return }
            presenter.collections.dataReady(); app.firstPixel(token)
        }
        storeRefresh.record(batch)
        retryStoreActivation()
        canvases.loadIfNeeded()
        openNativeViews(drawnGeneration)
        drainSurfaceWorkNow()
        frames.run(frames.motion || canvases.wantsFrames)
        frames.run(frames.motion || frames.timerSoon || canvases.wantsFrames)
        Owner.shared.releaseIdleStack() // after the data's batch: the deepest the boot goes
    }

    /// Native views attach after this turn's commit, once per generation;
    /// they read no data, so they do not wait for it.
    private func openNativeViews(_ drawnGeneration: Int) {
        guard nativesOpened != drawnGeneration else { return }
        nativesOpened = drawnGeneration
        natives.activateAfterCommit { [weak self] in self.map { $0.state != .destroyed && $0.generation == drawnGeneration } ?? false }
    }
}
