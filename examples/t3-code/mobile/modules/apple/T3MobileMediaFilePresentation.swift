#if os(iOS)
// Adapted from T3 Code (MIT), 365aa87982 T3NativeFilePresentation.swift.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import ImageIO
import QuickLook
import UIKit
import UniformTypeIdentifiers

private final class FilePreviewItem: NSObject, QLPreviewItem {
  var previewItemURL: URL?
  var previewItemTitle: String?
}

private final class FilePreviewController: QLPreviewController {
  var onAppear: (() -> Void)?

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    onAppear?()
  }
}

/// Quick Look owns image and document controls, zooming, and source-view transitions.
final class T3MobileMediaFilePresentation: NSObject, QLPreviewControllerDataSource,
  QLPreviewControllerDelegate, UIAdaptivePresentationControllerDelegate {
  let identifier: String
  private var controller: UIViewController?
  private let completion: (Error?) -> Void
  private let item = FilePreviewItem()
  private var loading: Task<Void, Never>?
  private var dismissRequested = false
  private var finished = false

  init(identifier: String, completion: @escaping (Error?) -> Void) {
    self.identifier = identifier
    self.completion = completion
    super.init()
  }

  func present(source: T3MobileMediaSource, files: T3MobileMediaFiles, from presenter: UIViewController) {
    loading = Task { @MainActor [self] in
      do {
        let file = try await files.prepare(source)
        guard !finished, !Task.isCancelled else {
          try? FileManager.default.removeItem(at: file.deletingLastPathComponent())
          return
        }
        // Quick Look knows which formats it renders; refuse before presenting so the caller
        // can fall back instead of showing an "unsupported format" page.
        guard QLPreviewController.canPreview(file as NSURL) else {
          try? FileManager.default.removeItem(at: file.deletingLastPathComponent())
          throw NSError(
            domain: "T3NativePresentation",
            code: 3,
            userInfo: [NSLocalizedDescriptionKey: "This file type cannot be previewed on this device."]
          )
        }
        item.previewItemURL = file
        item.previewItemTitle = source.name
        let preview = FilePreviewController()
        preview.delegate = self
        preview.dataSource = self
        preview.onAppear = { [weak self] in self?.resumePendingDismissal() }
        controller = preview
        presenter.present(preview, animated: !UIAccessibility.isReduceMotionEnabled) { [self] in
          resumePendingDismissal()
        }
        preview.presentationController?.delegate = self
      } catch {
        finish(error: error)
      }
    }
  }

  func dismiss() {
    dismissRequested = true
    loading?.cancel()
    guard !finished else { return }
    guard let controller else { finish(); return }
    // Drain Close from viewDidAppear after opening or cancelling an interactive dismissal.
    // Starting a second modal transition while UIKit is settling the first can strand it.
    guard !controller.isBeingPresented, !controller.isBeingDismissed else { return }
    controller.dismiss(animated: !UIAccessibility.isReduceMotionEnabled) { [self] in finish() }
  }

  private func resumePendingDismissal() {
    // Appearance callbacks run before UIKit has cleared the current transition.
    DispatchQueue.main.async { [weak self] in
      if self?.dismissRequested == true { self?.dismiss() }
    }
  }

  func numberOfPreviewItems(in controller: QLPreviewController) -> Int { item.previewItemURL == nil ? 0 : 1 }

  func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
    item
  }

  func previewControllerDidDismiss(_ controller: QLPreviewController) { finish() }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { finish() }

  private func finish(error: Error? = nil) {
    guard !finished else { return }
    finished = true
    loading?.cancel()
    loading = nil
    if let file = item.previewItemURL {
      try? FileManager.default.removeItem(at: file.deletingLastPathComponent())
    }
    item.previewItemURL = nil
    DispatchQueue.main.async { [completion] in completion(error) }
  }

}
#endif
