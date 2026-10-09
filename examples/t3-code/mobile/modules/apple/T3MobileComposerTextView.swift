#if os(iOS)
// Adapted from T3 Code (MIT), pinned365aa87982 T3ComposerEditorView.swift.
// Copyright T3 Code contributors. MIT license: examples/t3-code/mobile/LICENSE-T3.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-focus-restoration
import UIKit

struct ComposerTokenPayload: Decodable {
  let type: String
  let source: String
  let label: String
  let iconUri: String?
  let accent: String?
  let symbol: String?
  let detail: String?
  let start: Int
  let end: Int
}

struct ComposerSelectionPayload: Decodable {
  let start: Int
  let end: Int
}

struct ComposerControlledDocumentPayload: Decodable {
  let value: String
  let selection: ComposerSelectionPayload?
  let tokensJson: String
  let mostRecentEventCount: Int
  let isNativeEcho: Bool
}

struct ComposerThemePayload: Decodable {
  let text: String
  let placeholder: String
  let chipBackground: String
  let chipBorder: String
  let chipText: String
  let skillBackground: String
  let skillBorder: String
  let skillText: String
  let fileTint: String
}

struct ComposerChipStyle {
  let tint: UIColor
  let backgroundColor: UIColor
  let borderColor: UIColor
  let textColor: UIColor
}

enum ComposerEnterBehavior: String {
  case send
  case newline
}

final class ComposerTextAttachment: NSTextAttachment {
  let source: String
  let label: String

  init(source: String, label: String, image: UIImage, size: CGSize, baselineOffset: CGFloat) {
    self.source = source
    self.label = label
    super.init(data: nil, ofType: nil)
    self.image = image
    bounds = CGRect(x: 0, y: baselineOffset, width: size.width, height: size.height)
  }

  required init?(coder: NSCoder) {
    nil
  }
}

final class ComposerContextAccessibilityElement: UIAccessibilityElement {
  var activate: (() -> Bool)?

  override func accessibilityActivate() -> Bool {
    activate?() ?? false
  }
}

final class ComposerTextView: UITextView {
  private static let pastedImageDirectoryName = "t3-composer-paste"
  private static let stalePastedImageAge: TimeInterval = 60 * 60
  private static let readOnlyActions = Set([
    "cut:",
    "delete:",
    "paste:",
    "redo:",
    "toggleBoldface:",
    "toggleItalics:",
    "toggleUnderline:",
    "undo:",
  ])

  var pasteAllowed: () -> Bool = { false }
  var onCompositionChanged: (() -> Void)?
  private var ownedAlive = true
  private var pasteSerial = 0
  private var pasteProgress: [Progress] = []
  private var ownedPasteURIs = Set<String>()
  override func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
    super.setMarkedText(markedText, selectedRange: selectedRange)
    onCompositionChanged?()
  }
  override func unmarkText() {
    super.unmarkText()
    onCompositionChanged?()
  }
  func adoptPasteURIs(_ uris: [String]) { for uri in uris { ownedPasteURIs.remove(uri) } }
  func invalidatePendingPaste() {
    pasteSerial += 1
    for progress in pasteProgress { progress.cancel() }
    pasteProgress.removeAll()
  }
  func cancelOwnedWork() {
    ownedAlive = false; pasteSerial += 1
    for progress in pasteProgress { progress.cancel() }
    pasteProgress.removeAll()
    for uri in ownedPasteURIs { if let url = URL(string: uri) { try? FileManager.default.removeItem(at: url) } }
    ownedPasteURIs.removeAll()
  }
  private func writeOwnedImage(_ image: UIImage) -> String? {
    guard ownedAlive, let uri = Self.writeTemporaryImage(image) else { return nil }
    ownedPasteURIs.insert(uri); return uri
  }

  var onPasteImages: (([String]) -> Void)?
  var onPasteContext: (([String: String]) -> Void)?
  var onPasteText: ((String, NSRange) -> Void)?
  var clipboardFragment = ""
  var onAttributedMutation: (() -> Void)?
  var onSubmit: ((Bool) -> Void)?
  var isReadOnly = false
  var textPasteThresholdBytes = 0
  var maxInputChars = Int.max
  var enterBehavior: ComposerEnterBehavior = .send
  /// Shortcut HUD titles. JS supplies what the two sends actually do right now
  /// ("Queue Message" / "Steer Message"), so the iPad Command-hold list names
  /// the outcome rather than a generic "Send".
  var submitTitle = "Send Message"
  var alternateSubmitTitle = "Send Message"
  private var bypassTextPasteInterception = false

  override var keyCommands: [UIKeyCommand]? {
    var commands = super.keyCommands ?? []
    guard !isReadOnly, markedTextRange == nil else { return commands }
    // The plainer chord always performs the configured follow-up behavior and
    // the more-modified one performs its opposite, so Command is the "other
    // way" modifier whichever Return behavior is configured.
    if enterBehavior == .send {
      let submitOnReturn = UIKeyCommand(
        input: "\r",
        modifierFlags: [],
        action: #selector(submitMessage(_:))
      )
      submitOnReturn.discoverabilityTitle = submitTitle
      submitOnReturn.wantsPriorityOverSystemBehavior = true
      commands.append(submitOnReturn)

      let submitAlternate = UIKeyCommand(
        input: "\r",
        modifierFlags: .command,
        action: #selector(submitMessageAlternate(_:))
      )
      submitAlternate.discoverabilityTitle = alternateSubmitTitle
      submitAlternate.wantsPriorityOverSystemBehavior = true
      commands.append(submitAlternate)

      let newline = UIKeyCommand(
        input: "\r",
        modifierFlags: .shift,
        action: #selector(insertNewline(_:))
      )
      newline.discoverabilityTitle = "New Line"
      newline.wantsPriorityOverSystemBehavior = true
      commands.append(newline)
    } else {
      let submit = UIKeyCommand(
        input: "\r",
        modifierFlags: .command,
        action: #selector(submitMessage(_:))
      )
      submit.discoverabilityTitle = submitTitle
      submit.wantsPriorityOverSystemBehavior = true
      commands.append(submit)

      let submitAlternate = UIKeyCommand(
        input: "\r",
        modifierFlags: [.command, .shift],
        action: #selector(submitMessageAlternate(_:))
      )
      submitAlternate.discoverabilityTitle = alternateSubmitTitle
      submitAlternate.wantsPriorityOverSystemBehavior = true
      commands.append(submitAlternate)
    }
    if textPasteThresholdBytes > 0 {
      let pasteAsText = UIKeyCommand(
        input: "v",
        modifierFlags: [.command, .shift],
        action: #selector(pasteInline(_:))
      )
      pasteAsText.discoverabilityTitle = "Paste as Text"
      pasteAsText.wantsPriorityOverSystemBehavior = true
      commands.append(pasteAsText)
    }
    return commands
  }

  @objc private func submitMessage(_ sender: UIKeyCommand) {
    guard isEditable, !isReadOnly, markedTextRange == nil else { return }
    onSubmit?(false)
  }

  @objc private func submitMessageAlternate(_ sender: UIKeyCommand) {
    guard isEditable, !isReadOnly, markedTextRange == nil else { return }
    onSubmit?(true)
  }

  @objc private func insertNewline(_ sender: UIKeyCommand) {
    guard isEditable, !isReadOnly, markedTextRange == nil else { return }
    insertText("\n")
  }

  @objc private func pasteInline(_ sender: UIKeyCommand) {
    guard !isReadOnly else {
      return
    }
    bypassTextPasteInterception = true
    defer { bypassTextPasteInterception = false }
    paste(sender)
  }

  override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
    if action == #selector(submitMessage(_:)) || action == #selector(submitMessageAlternate(_:)) || action == #selector(insertNewline(_:)) {
      return isEditable && !isReadOnly && markedTextRange == nil
    }
    if isReadOnly && Self.readOnlyActions.contains(NSStringFromSelector(action)) {
      return false
    }
    if action == #selector(paste(_:)) {
      let pasteboard = UIPasteboard.general
      if pasteboard.hasImages ||
        pasteboard.itemProviders.contains(where: {
          $0.canLoadObject(ofClass: UIImage.self)
        }) {
        return true
      }
    }
    return super.canPerformAction(action, withSender: sender)
  }

  override func paste(_ sender: Any?) {
    guard !isReadOnly, ownedAlive, pasteAllowed(), markedTextRange == nil else {
      return
    }
    let pasteboard = UIPasteboard.general
    let context = T3ComposerClipboard.read()
    if !context["fragment", default: ""].isEmpty || context["html", default: ""].contains("data-t3-context-fragment=") {
      onPasteContext?(context)
      return
    }
    let imageProviders = pasteboard.itemProviders.filter {
      $0.canLoadObject(ofClass: UIImage.self)
    }
    if !imageProviders.isEmpty {
      loadImages(from: imageProviders)
      return
    }

    let images = pasteboard.images ?? []
    if !images.isEmpty {
      let urls = images.compactMap(writeOwnedImage)
      if !urls.isEmpty {
        onPasteImages?(urls)
        return
      }
    }
    if !bypassTextPasteInterception,
       let text = pasteboard.string, shouldInterceptTextPaste(text) {
      onPasteText?(text, selectedRange)
      return
    }
    super.paste(sender)
  }

  private func shouldInterceptTextPaste(_ text: String) -> Bool {
    guard textPasteThresholdBytes > 0, !text.isEmpty else { return false }
    let pastedLength = (text as NSString).length
    if pastedLength >= textPasteThresholdBytes || text.utf8.count >= textPasteThresholdBytes {
      return true
    }
    // Chips occupy one display character but expand to their source in the
    // submitted message. Measure that source, including the replaced selection.
    let sourceLength = sourceOffset(forDisplayOffset: attributedText.length)
    let selectedLength = sourceOffset(forDisplayOffset: NSMaxRange(selectedRange)) -
      sourceOffset(forDisplayOffset: selectedRange.location)
    return sourceLength - selectedLength + pastedLength > maxInputChars
  }

  override func deleteBackward() {
    guard !isReadOnly else {
      return
    }
    guard selectedRange.length == 0, selectedRange.location > 0 else {
      super.deleteBackward()
      return
    }

    let previousOffset = selectedRange.location - 1
    if textStorage.attribute(.attachment, at: previousOffset, effectiveRange: nil)
      is ComposerTextAttachment {
      replaceDisplayRange(NSRange(location: previousOffset, length: 1))
      return
    }

    super.deleteBackward()
  }

  private func replaceDisplayRange(_ range: NSRange) {
    guard let start = position(from: beginningOfDocument, offset: range.location),
          let end = position(from: start, offset: range.length),
          let textRange = textRange(from: start, to: end) else {
      return
    }
    replace(textRange, withText: "")
  }

  func loadImages(from providers: [NSItemProvider]) {
    guard ownedAlive, !isReadOnly, markedTextRange == nil, pasteAllowed() else { return }
    pasteSerial += 1
    let serial = pasteSerial, source = serializedText(), selection = selectedRange
    for progress in pasteProgress { progress.cancel() }
    pasteProgress.removeAll()
    let group = DispatchGroup()
    let lock = NSLock()
    var images = [UIImage?](repeating: nil, count: providers.count)

    for (index, provider) in providers.enumerated() {
      group.enter()
      let progress = provider.loadObject(ofClass: UIImage.self) { object, _ in
        defer { group.leave() }
        guard let image = object as? UIImage else {
          return
        }
        lock.lock()
        images[index] = image
        lock.unlock()
      }
      pasteProgress.append(progress)
    }

    group.notify(queue: .main) { [weak self] in
      guard let self, self.ownedAlive, !self.isReadOnly, self.markedTextRange == nil, self.pasteSerial == serial, self.pasteAllowed(),
            self.serializedText() == source, NSEqualRanges(self.selectedRange, selection) else {
        return
      }
      self.pasteProgress.removeAll()
      let urls = images.compactMap { $0 }.compactMap(self.writeOwnedImage)
      if !urls.isEmpty {
        self.onPasteImages?(urls)
      }
    }
  }

  override func copy(_ sender: Any?) {
    guard selectedRange.length > 0 else {
      return super.copy(sender)
    }
    T3ComposerClipboard.write(text: serializedText(in: selectedRange), fragment: clipboardFragment)
  }

  override func cut(_ sender: Any?) {
    guard !isReadOnly else {
      return
    }
    guard isEditable, selectedRange.length > 0 else {
      return super.cut(sender)
    }
    copy(sender)
    textStorage.replaceCharacters(in: selectedRange, with: "")
    selectedRange = NSRange(location: selectedRange.location, length: 0)
    onAttributedMutation?()
  }

  func serializedText() -> String {
    serializedText(in: NSRange(location: 0, length: attributedText.length))
  }

  func serializedText(in range: NSRange) -> String {
    guard range.length > 0 else {
      return ""
    }

    let source = NSMutableString()
    let nsString = attributedText.string as NSString
    var cursor = range.location
    let end = NSMaxRange(range)
    attributedText.enumerateAttribute(.attachment, in: range) { value, attachmentRange, _ in
      if attachmentRange.location > cursor {
        source.append(
          nsString.substring(
            with: NSRange(location: cursor, length: attachmentRange.location - cursor)
          )
        )
      }
      if let attachment = value as? ComposerTextAttachment {
        source.append(attachment.source)
      } else {
        source.append(nsString.substring(with: attachmentRange))
      }
      cursor = NSMaxRange(attachmentRange)
    }
    if cursor < end {
      source.append(nsString.substring(with: NSRange(location: cursor, length: end - cursor)))
    }
    return source as String
  }

  func sourceOffset(forDisplayOffset displayOffset: Int) -> Int {
    let boundedOffset = max(0, min(attributedText.length, displayOffset))
    if boundedOffset == 0 {
      return 0
    }

    var sourceOffset = 0
    let range = NSRange(location: 0, length: boundedOffset)
    attributedText.enumerateAttribute(.attachment, in: range) { value, attributeRange, _ in
      if let attachment = value as? ComposerTextAttachment {
        sourceOffset += (attachment.source as NSString).length
      } else {
        sourceOffset += attributeRange.length
      }
    }
    return sourceOffset
  }

  private static func writeTemporaryImage(_ image: UIImage) -> String? {
    guard let data = image.pngData() else {
      return nil
    }
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent(pastedImageDirectoryName, isDirectory: true)
    do {
      try FileManager.default.createDirectory(
        at: directory,
        withIntermediateDirectories: true
      )
      // Only this editor's unadopted leases are removed on teardown.
      let url = directory.appendingPathComponent("\(UUID().uuidString).png")
      try data.write(to: url, options: .atomic)
      return url.absoluteString
    } catch {
      return nil
    }
  }

  private static func removeStaleTemporaryImages(in directory: URL) {
    let cutoff = Date().addingTimeInterval(-stalePastedImageAge)
    guard let urls = try? FileManager.default.contentsOfDirectory(
      at: directory,
      includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey],
      options: [.skipsHiddenFiles]
    ) else {
      return
    }

    for url in urls {
      guard
        let values = try? url.resourceValues(
          forKeys: [.contentModificationDateKey, .isRegularFileKey]
        ),
        values.isRegularFile == true,
        let modifiedAt = values.contentModificationDate,
        modifiedAt < cutoff
      else {
        continue
      }
      try? FileManager.default.removeItem(at: url)
    }
  }
}

enum T3ComposerClipboard {
  static let fragmentType = "app.t3.context-fragment"

  static func write(text: String, fragment: String) {
    var items: [String: Any] = ["public.utf8-plain-text": text]
    if let data = fragment.data(using: .utf8),
       var payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
       let records = payload["records"] as? [[String: Any]] {
      var selected = records.filter { record in
        guard let id = record["contextId"] as? String else { return false }
        return text.contains("/\(id))")
      }
      let screenshots = Set(selected.compactMap { $0["screenshotContextId"] as? String })
      selected.append(contentsOf: records.filter { screenshots.contains($0["contextId"] as? String ?? "") && !text.contains("/\($0["contextId"] as? String ?? ""))") })
      payload["records"] = selected
      if !selected.isEmpty, let encoded = try? JSONSerialization.data(withJSONObject: payload), let raw = String(data: encoded, encoding: .utf8) {
        let attribute = raw.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
        let escaped = text.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;")
        items[fragmentType] = encoded
        items["public.html"] = Data("<pre data-t3-context-fragment=\"\(attribute)\">\(escaped)</pre>".utf8)
      }
    }
    UIPasteboard.general.items = [items]
  }

  static func read() -> [String: String] {
    let board = UIPasteboard.general
    return [
      "text": board.string ?? "",
      "fragment": board.data(forPasteboardType: fragmentType).flatMap { String(data: $0, encoding: .utf8) } ?? "",
      "html": board.data(forPasteboardType: "public.html").flatMap { String(data: $0, encoding: .utf8) } ?? "",
    ]
  }
}


#endif
