#if os(iOS)
// Adapted from T3 Code (MIT), 365aa87982 T3NativeVideoPresentation.swift.
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
import AVKit
import UIKit

final class T3MobileMediaVideoPresentation: NSObject, AVPlayerViewControllerDelegate,
  UIAdaptivePresentationControllerDelegate {
  let identifier: String
  private let controller = AVPlayerViewController()
  private let completion: (Error?) -> Void
  private var itemObservation: NSKeyValueObservation?
  private var backgroundObserver: NSObjectProtocol?
  private var playbackError: Error?
  private var presented = false
  private var dismissRequested = false
  private var finished = false
  private struct AudioSessionConfiguration {
    let category: AVAudioSession.Category
    let mode: AVAudioSession.Mode
    let options: AVAudioSession.CategoryOptions

    init(_ session: AVAudioSession) {
      category = session.category
      mode = session.mode
      options = session.categoryOptions
    }
  }
  private var previousAudioSession: AudioSessionConfiguration?
  private weak var fullScreenController: UIViewController?

  init(identifier: String, url: URL, title: String, completion: @escaping (Error?) -> Void) {
    self.identifier = identifier
    self.completion = completion
    super.init()

    let item = AVPlayerItem(url: url)
    let metadata = AVMutableMetadataItem()
    metadata.identifier = .commonIdentifierTitle
    metadata.value = title as NSString
    item.externalMetadata = [metadata]
    controller.player = AVPlayer(playerItem: item)
    controller.delegate = self
    controller.overrideUserInterfaceStyle = .dark
    controller.allowsPictureInPicturePlayback = false

    itemObservation = item.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
      guard item.status == .failed else { return }
      DispatchQueue.main.async {
        guard let self else { return }
        self.playbackError = item.error ?? NSError(
          domain: "T3NativeVideo",
          code: 1,
          userInfo: [NSLocalizedDescriptionKey: "This video couldn't be played on this device."]
        )
        self.dismiss()
      }
    }
    backgroundObserver = NotificationCenter.default.addObserver(
      forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
    ) { [weak self] _ in self?.controller.player?.pause() }
  }

  func present(from presenter: UIViewController) {
    let audioSession = AVAudioSession.sharedInstance()
    previousAudioSession = AudioSessionConfiguration(audioSession)
    do {
      try audioSession.setCategory(.playback, mode: .moviePlayback)
    } catch {
      NSLog("T3 video audio session: %@", error.localizedDescription)
    }
    controller.modalPresentationStyle = .fullScreen
    presenter.present(controller, animated: !UIAccessibility.isReduceMotionEnabled) { [self] in
      presented = true
      if dismissRequested { dismiss() }
      else if UIApplication.shared.applicationState == .active { controller.player?.play() }
    }
    controller.presentationController?.delegate = self
  }

  func dismiss() {
    dismissRequested = true
    guard !finished else { return }
    guard presented else { return }
    (fullScreenController ?? controller).dismiss(animated: true) { [self] in finish() }
  }

  func playerViewController(
    _ playerViewController: AVPlayerViewController,
    willBeginFullScreenPresentationWithAnimationCoordinator coordinator: UIViewControllerTransitionCoordinator
  ) {
    fullScreenController = coordinator.viewController(forKey: .to)
    coordinator.animate(alongsideTransition: nil) { [weak self] context in
      guard let self else { return }
      if context.isCancelled {
        finish()
      } else {
        presented = true
        if dismissRequested { dismiss() }
      }
    }
  }

  func playerViewController(
    _ playerViewController: AVPlayerViewController,
    willEndFullScreenPresentationWithAnimationCoordinator coordinator: UIViewControllerTransitionCoordinator
  ) {
    coordinator.animate(alongsideTransition: nil) { [weak self] context in
      if !context.isCancelled { self?.finish() }
    }
  }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
    finish()
  }

  private func finish() {
    guard !finished else { return }
    finished = true
    controller.player?.pause()
    itemObservation = nil
    controller.player = nil
    if let backgroundObserver { NotificationCenter.default.removeObserver(backgroundObserver) }
    backgroundObserver = nil
    let audioSession = AVAudioSession.sharedInstance()
    if let previousAudioSession, audioSession.category == .playback,
      audioSession.mode == .moviePlayback, audioSession.categoryOptions.isEmpty {
      // AVPlayer owns activation. Deactivating the shared session here could
      // stop another player or recorder that was active before this preview.
      try? audioSession.setCategory(
        previousAudioSession.category,
        mode: previousAudioSession.mode,
        options: previousAudioSession.options
      )
    }
    completion(playbackError)
  }
}
#endif
