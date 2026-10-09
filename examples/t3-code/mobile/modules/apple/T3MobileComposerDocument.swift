#if os(iOS)
// Adapted from T3 Code (MIT), pinned365aa87982 T3ComposerEditorView.swift.
// Copyright T3 Code contributors. MIT license: examples/t3-code/mobile/LICENSE-T3.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-focus-restoration
import UIKit

final class T3MobileOwnedComposerView: UIView, UITextViewDelegate, UITextDropDelegate, UIGestureRecognizerDelegate {
  let textView = ComposerTextView()
  private var alive = true
  private var deferredRebuild = false
  private var iconTasks: [String: URLSessionDataTask] = [:]
  var ownedEventCount: Int { nativeEventCount }
  private let placeholderLabel = UILabel()
  private var value = ""
  private var tokensJson = "[]"
  private var tokens: [ComposerTokenPayload] = []
  private var requestedSelection: ComposerSelectionPayload?
  private var theme = ComposerThemePayload(
    text: "#262626",
    placeholder: "#8e8e93",
    chipBackground: "#f2f2f7",
    chipBorder: "#dedee3",
    chipText: "#262626",
    skillBackground: "#f9e8fb",
    skillBorder: "#e5a6eb",
    skillText: "#a21caf",
    fileTint: "#737373"
  )
  private var fontFamily = "DMSans-Regular"
  private var fontSize: CGFloat = 14
  private var lineHeight: CGFloat = 20
  private var contentInsetVertical: CGFloat = 0
  private var shouldAutoFocus = false
  private var didAutoFocus = false
  private var isReadOnly = false
  private var isApplyingControlledValue = false
  private var nativeEventCount = 0
  private var lastContentSize = CGSize.zero
  private var iconImages: [String: UIImage] = [:]
  private var pendingIconUris = Set<String>()
  private var tokensNeedRebuild = false
  private var chipsNeedMeasuredWidth = false

  var onComposerChange: ([String: Any]) -> Void = { _ in }
  var onComposerSelectionChange: ([String: Any]) -> Void = { _ in }
  var onComposerFocus: ([String: Any]) -> Void = { _ in }
  var onComposerBlur: ([String: Any]) -> Void = { _ in }
  var onComposerSubmit: ([String: Any]) -> Void = { _ in }
  var onComposerPasteImages: ([String: Any]) -> Void = { _ in }
  var onComposerContextPress: ([String: Any]) -> Void = { _ in }
  var onComposerPasteContext: ([String: Any]) -> Void = { _ in }
  var onComposerPasteText: ([String: Any]) -> Void = { _ in }
  var onComposerContentSizeChange: ([String: Any]) -> Void = { _ in }

  override init(frame: CGRect) {
    super.init(frame: frame)

    clipsToBounds = false
    textView.delegate = self
    textView.onCompositionChanged = { [weak self] in self?.compositionChanged() }
    textView.textDropDelegate = self
    textView.backgroundColor = .clear
    textView.textContainerInset = .zero
    textView.textContainer.lineFragmentPadding = 0
    textView.keyboardDismissMode = .interactive
    textView.alwaysBounceVertical = false
    textView.showsVerticalScrollIndicator = true
    textView.adjustsFontForContentSizeCategory = true
    textView.onPasteImages = { [weak self] urls in
      self?.onComposerPasteImages(["uris": urls])
    }
    textView.onPasteContext = { [weak self] context in
      guard let self else { return }
      let selection = self.sourceSelection()
      self.nativeEventCount += 1
      var payload: [String: Any] = context
      payload["value"] = self.textView.serializedText()
      payload["eventCount"] = self.nativeEventCount
      payload["selection"] = ["start": selection.start, "end": selection.end]
      self.onComposerPasteContext(payload)
    }
    textView.onPasteText = { [weak self] text, _ in
      guard let self else { return }
      let selection = self.sourceSelection()
      self.nativeEventCount += 1
      self.onComposerPasteText([
        "value": self.textView.serializedText(),
        "eventCount": self.nativeEventCount,
        "text": text,
        "selection": ["start": selection.start, "end": selection.end],
      ])
    }
    textView.onAttributedMutation = { [weak self] in
      self?.emitTextChange()
    }
    textView.onSubmit = { [weak self] alternate in
      self?.onComposerSubmit(["alternate": alternate])
    }
    let contextTap = UITapGestureRecognizer(target: self, action: #selector(openContext(_:)))
    contextTap.cancelsTouchesInView = false
    contextTap.delegate = self
    textView.addGestureRecognizer(contextTap)
    addSubview(textView)

    placeholderLabel.numberOfLines = 0
    placeholderLabel.adjustsFontForContentSizeCategory = true
    addSubview(placeholderLabel)
    applyTypography()
    applyTheme()
  }

  func sourceSnapshot() -> T3ComposerSnapshot {
    let selection = sourceSelection()
    return .init(value: textView.serializedText(), selection: .init(start: selection.start, end: selection.end),
                 composing: textView.markedTextRange != nil, focused: textView.isFirstResponder)
  }

  private func compositionChanged() {
    guard alive, !isApplyingControlledValue else { return }
    // Unmarking may not mutate text; always publish the end of composition.
    value = textView.serializedText()
    rebaseTokensFromDisplay()
    requestedSelection = nil
    if textView.markedTextRange == nil, deferredRebuild {
      deferredRebuild = false
      applyTypography(); applyTheme(); applyControlledDocument(force: true)
    }
    emitSelection()
  }

  func applyReplacement(value next: String, selection: T3ComposerSelection, tokensJson nextTokens: String) {
    guard alive, textView.markedTextRange == nil else { return }
    let previous = sourceSnapshot(), previousTokens = tokensJson
    textView.undoManager?.registerUndo(withTarget: self) { target in
      target.applyReplacement(value: previous.value, selection: previous.selection, tokensJson: previousTokens)
      target.emitTextChange()
    }
    textView.undoManager?.setActionName("Composer replacement")
    let payload: [String: Any] = ["value": next, "selection": ["start": selection.start, "end": selection.end],
      "tokensJson": nextTokens, "mostRecentEventCount": nativeEventCount, "isNativeEcho": false]
    if let bytes = try? JSONSerialization.data(withJSONObject: payload) {
      setControlledDocumentJson(String(decoding: bytes, as: UTF8.self))
    }
  }

  func destroyOwned() {
    guard alive else { return }
    alive = false; shouldAutoFocus = false
    textView.cancelOwnedWork(); textView.undoManager?.removeAllActions()
    textView.delegate = nil; textView.textDropDelegate = nil
    textView.onCompositionChanged = nil; textView.onPasteImages = nil
    textView.onPasteContext = nil; textView.onPasteText = nil
    textView.onAttributedMutation = nil; textView.onSubmit = nil
    textView.clipboardFragment = ""
    for task in iconTasks.values { task.cancel() }
    iconTasks.removeAll(); pendingIconUris.removeAll(); iconImages.removeAll()
    tokens.removeAll(); tokensJson = "[]"; requestedSelection = nil
    onComposerChange = { _ in }; onComposerSelectionChange = { _ in }
    onComposerFocus = { _ in }; onComposerBlur = { _ in }; onComposerSubmit = { _ in }
    onComposerPasteImages = { _ in }; onComposerPasteContext = { _ in }
    onComposerPasteText = { _ in }; onComposerContextPress = { _ in }; onComposerContentSizeChange = { _ in }
    textView.resignFirstResponder()
  }

  required init?(coder: NSCoder) { nil }

  @objc private func openContext(_ recognizer: UITapGestureRecognizer) {
    guard let (index, attachment) = contextAttachment(at: recognizer.location(in: textView)) else {
      textView.becomeFirstResponder()
      return
    }
    openContext(index: index, attachment: attachment)
  }

  private func openContext(index: Int, attachment: ComposerTextAttachment) {
    let start = textView.sourceOffset(forDisplayOffset: index)
    onComposerContextPress(["source": attachment.source, "start": start, "end": start + (attachment.source as NSString).length])
  }

  override var accessibilityElements: [Any]? {
    get {
      var elements: [Any] = [textView]
      let layout = textView.layoutManager
      textView.textStorage.enumerateAttribute(.attachment, in: NSRange(location: 0, length: textView.textStorage.length)) { value, range, _ in
        guard let attachment = value as? ComposerTextAttachment else { return }
        let glyphRange = layout.glyphRange(forCharacterRange: range, actualCharacterRange: nil)
        let rect = layout.boundingRect(forGlyphRange: glyphRange, in: textView.textContainer)
          .offsetBy(dx: textView.textContainerInset.left, dy: textView.textContainerInset.top)
        guard rect.intersects(textView.bounds) else { return }
        let element = ComposerContextAccessibilityElement(accessibilityContainer: self)
        element.accessibilityLabel = attachment.label
        element.accessibilityTraits = .button
        element.accessibilityFrameInContainerSpace = textView.convert(rect, to: self)
        element.activate = { [weak self] in
          guard let self, range.location < self.textView.textStorage.length,
                self.textView.textStorage.attribute(.attachment, at: range.location, effectiveRange: nil) as? ComposerTextAttachment === attachment else { return false }
          self.openContext(index: range.location, attachment: attachment)
          return true
        }
        elements.append(element)
      }
      return elements
    }
    set { super.accessibilityElements = newValue }
  }

  func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
    true
  }

  func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
    true
  }

  func textView(_ textView: UITextView, shouldInteractWith textAttachment: NSTextAttachment, in characterRange: NSRange, interaction: UITextItemInteraction) -> Bool {
    // A chip is text context, not an image to save to the camera roll.
    !(textAttachment is ComposerTextAttachment)
  }

  private func contextAttachment(at point: CGPoint) -> (Int, ComposerTextAttachment)? {
    let containerPoint = CGPoint(x: point.x - textView.textContainerInset.left, y: point.y - textView.textContainerInset.top)
    let layout = textView.layoutManager
    let index = layout.characterIndex(for: containerPoint, in: textView.textContainer, fractionOfDistanceBetweenInsertionPoints: nil)
    guard index < textView.textStorage.length,
          let attachment = textView.textStorage.attribute(.attachment, at: index, effectiveRange: nil) as? ComposerTextAttachment else { return nil }
    let glyphRange = layout.glyphRange(forCharacterRange: NSRange(location: index, length: 1), actualCharacterRange: nil)
    guard layout.boundingRect(forGlyphRange: glyphRange, in: textView.textContainer).contains(containerPoint) else { return nil }
    return (index, attachment)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    textView.frame = bounds
    if chipsNeedMeasuredWidth, bounds.width > 0 {
      chipsNeedMeasuredWidth = false
      applyControlledDocument(force: true)
    }
    let placeholderX = textView.textContainerInset.left + textView.textContainer.lineFragmentPadding
    let placeholderY = textView.textContainerInset.top
    let placeholderWidth = max(
      0,
      bounds.width - placeholderX - textView.textContainerInset.right -
        textView.textContainer.lineFragmentPadding
    )
    placeholderLabel.frame = CGRect(
      x: placeholderX,
      y: placeholderY,
      width: placeholderWidth,
      height: max(lineHeight, placeholderLabel.font.lineHeight)
    )
    emitContentSizeIfNeeded()
  }

  func setClipboardFragment(_ fragment: String) {
    textView.clipboardFragment = fragment
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    guard alive, window != nil, shouldAutoFocus, !didAutoFocus else {
      return
    }
    didAutoFocus = true
    DispatchQueue.main.async { [weak self] in
      guard let self, self.alive, self.window != nil, self.shouldAutoFocus else { return }
      self.textView.becomeFirstResponder()
    }
  }

  func setControlledDocumentJson(_ documentJson: String) {
    guard alive, textView.markedTextRange == nil else { return }
    guard let document = decode(ComposerControlledDocumentPayload.self, from: documentJson),
          document.mostRecentEventCount >= nativeEventCount else {
      return
    }
    if document.isNativeEcho && textView.serializedText() != document.value {
      return
    }
    if tokensJson != document.tokensJson {
      tokensJson = document.tokensJson
      tokens = decode([ComposerTokenPayload].self, from: document.tokensJson) ?? []
      tokensNeedRebuild = true
    }
    value = document.value
    requestedSelection = document.selection
    applyControlledDocument(force: tokensNeedRebuild)
    applyRequestedSelection()
    if tokensMatchCurrentValue() {
      tokensNeedRebuild = false
    }
  }

  func setThemeJson(_ themeJson: String) {
    guard let nextTheme = decode(ComposerThemePayload.self, from: themeJson) else {
      return
    }
    theme = nextTheme
    applyTheme()
    applyControlledDocument(force: true)
  }

  func setPlaceholder(_ placeholder: String) {
    placeholderLabel.text = placeholder
    setNeedsLayout()
  }

  func setFontFamily(_ fontFamily: String) {
    self.fontFamily = fontFamily
    applyTypography()
    applyControlledDocument(force: true)
  }

  func setFontSize(_ fontSize: CGFloat) {
    self.fontSize = fontSize
    applyTypography()
    applyControlledDocument(force: true)
  }

  func setLineHeight(_ lineHeight: CGFloat) {
    self.lineHeight = lineHeight
    applyTypography()
    applyControlledDocument(force: true)
  }

  func setContentInsetVertical(_ contentInsetVertical: CGFloat) {
    self.contentInsetVertical = contentInsetVertical
    textView.textContainerInset = UIEdgeInsets(
      top: contentInsetVertical,
      left: 0,
      bottom: contentInsetVertical,
      right: 0
    )
    setNeedsLayout()
  }

  func setEditable(_ editable: Bool) {
    textView.isEditable = editable
  }

  func setReadOnly(_ readOnly: Bool) {
    isReadOnly = readOnly
    textView.isReadOnly = readOnly
  }

  func setScrollEnabled(_ scrollEnabled: Bool) {
    textView.isScrollEnabled = scrollEnabled
  }

  func setAutoFocus(_ autoFocus: Bool) {
    shouldAutoFocus = autoFocus
  }

  func setAutoCorrect(_ autoCorrect: Bool) {
    textView.autocorrectionType = autoCorrect ? .yes : .no
  }

  func setSpellCheck(_ spellCheck: Bool) {
    textView.spellCheckingType = spellCheck ? .yes : .no
  }

  func setEnterBehavior(_ behavior: String) {
    textView.enterBehavior = ComposerEnterBehavior(rawValue: behavior) ?? .send
  }

  func setSubmitTitle(_ title: String) {
    textView.submitTitle = title
  }

  func setAlternateSubmitTitle(_ title: String) {
    textView.alternateSubmitTitle = title
  }

  func setTextPasteThresholdBytes(_ threshold: Int) {
    textView.textPasteThresholdBytes = threshold
  }

  func setMaxInputChars(_ maxInputChars: Int) {
    textView.maxInputChars = maxInputChars
  }

  func focusEditor() {
    textView.becomeFirstResponder()
  }

  func blurEditor() {
    textView.resignFirstResponder()
  }

  func setSelection(start: Int, end: Int) {
    requestedSelection = ComposerSelectionPayload(start: start, end: end)
    applyRequestedSelection()
  }

  func textViewDidChange(_ textView: UITextView) {
    emitTextChange()
  }

  func textViewDidChangeSelection(_ textView: UITextView) {
    guard alive, !isApplyingControlledValue else {
      return
    }
    restoreBaseTypingAttributes()
    // UIKit moves the selection before textViewDidChange runs. Emitting here
    // would pair the post-edit text with a pre-edit revision counter, so let
    // the change event that follows carry both; only pure caret moves emit.
    guard self.textView.serializedText() == value else {
      return
    }
    emitSelection()
  }

  func textView(
    _ textView: UITextView,
    shouldChangeTextIn range: NSRange,
    replacementText text: String
  ) -> Bool {
    restoreBaseTypingAttributes()
    return !isReadOnly
  }

  func textDroppableView(
    _ textDroppableView: UIView & UITextDroppable,
    proposalForDrop drop: UITextDropRequest
  ) -> UITextDropProposal {
    guard !isReadOnly else {
      return UITextDropProposal(operation: .cancel)
    }
    guard droppedImageProviders(in: drop) != nil else {
      return drop.suggestedProposal
    }

    // The composer owns image drops so UIKit does not insert NSTextAttachments
    // that the controlled plain-text value cannot represent.
    let proposal = UITextDropProposal(operation: .copy)
    proposal.dropAction = .insert
    proposal.dropPerformer = .delegate
    return proposal
  }

  func textDroppableView(
    _ textDroppableView: UIView & UITextDroppable,
    willPerformDrop drop: UITextDropRequest
  ) {
    guard !isReadOnly else {
      return
    }
    guard let imageProviders = droppedImageProviders(in: drop) else {
      return
    }
    textView.loadImages(from: imageProviders)
  }

  private func droppedImageProviders(in drop: UITextDropRequest) -> [NSItemProvider]? {
    let providers = drop.dropSession.items.map(\.itemProvider)
    guard !providers.isEmpty,
          providers.allSatisfy({ $0.canLoadObject(ofClass: UIImage.self) }) else {
      return nil
    }
    return providers
  }

  func textViewDidBeginEditing(_ textView: UITextView) {
    onComposerFocus([:])
  }

  func textViewDidEndEditing(_ textView: UITextView) {
    onComposerBlur([:])
  }

  private func applyControlledDocument(force: Bool = false) {
    guard alive else { return }
    guard textView.markedTextRange == nil else { deferredRebuild = true; return }
    let currentSource = textView.serializedText()
    guard force || currentSource != value || !documentMatchesExpectedTokens() else {
      updatePlaceholderVisibility()
      return
    }

    let previousSelection = sourceSelection()
    if currentSource != value { textView.invalidatePendingPaste() }
    isApplyingControlledValue = true
    textView.attributedText = makeAttributedDocument()
    let targetSelection = requestedSelection ?? previousSelection
    requestedSelection = nil
    textView.selectedRange = displayRange(for: targetSelection)
    restoreBaseTypingAttributes()
    isApplyingControlledValue = false
    updatePlaceholderVisibility()
    emitContentSizeIfNeeded()
  }

  private func rebaseTokensFromDisplay() {
    // Native typing moves attachment positions before controlled token props catch up.
    // Keep source coordinates derived from the actual displayed attachments during that gap.
    var rebased: [ComposerTokenPayload] = []
    textView.textStorage.enumerateAttribute(.attachment, in: NSRange(location: 0, length: textView.textStorage.length)) { value, range, _ in
      guard let attachment = value as? ComposerTextAttachment,
            let token = tokens.first(where: { $0.source == attachment.source && $0.label == attachment.label }) else { return }
      let start = textView.sourceOffset(forDisplayOffset: range.location)
      rebased.append(ComposerTokenPayload(type: token.type, source: token.source, label: token.label, iconUri: token.iconUri,
        accent: token.accent, symbol: token.symbol, detail: token.detail, start: start, end: start + (token.source as NSString).length))
    }
    tokens = rebased
  }

  private func makeAttributedDocument() -> NSAttributedString {
    let result = NSMutableAttributedString()
    let source = value as NSString
    var cursor = 0
    let validTokens = tokens.filter {
      $0.start >= cursor &&
        $0.end > $0.start &&
        $0.end <= source.length &&
        source.substring(with: NSRange(location: $0.start, length: $0.end - $0.start)) == $0.source
    }

    for token in validTokens {
      if token.start < cursor {
        continue
      }
      if token.start > cursor {
        appendPlainText(
          source.substring(with: NSRange(location: cursor, length: token.start - cursor)),
          to: result
        )
      }
      result.append(makeAttachmentString(token))
      cursor = token.end
    }
    if cursor < source.length {
      appendPlainText(
        source.substring(with: NSRange(location: cursor, length: source.length - cursor)),
        to: result
      )
    }
    return result
  }

  private func appendPlainText(_ text: String, to result: NSMutableAttributedString) {
    result.append(NSAttributedString(string: text, attributes: baseAttributes()))
  }

  private func makeAttachmentString(_ token: ComposerTokenPayload) -> NSAttributedString {
    let isSkill = token.type == "skill"
    let accent = token.accent.flatMap { UIColor(composerHex: $0) }
    let foreground = UIColor(composerHex: theme.chipText) ?? .label
    let border = UIColor(composerHex: theme.chipBorder) ?? .separator
    let tint = accent.map { blend($0, over: foreground, weight: 0.22) }
      ?? UIColor(composerHex: isSkill ? theme.skillText : theme.fileTint) ?? .secondaryLabel
    let iconName = token.symbol ?? (isSkill ? "cube" : "doc")
    let iconImage = token.iconUri.flatMap(iconImage(for:))
    let style = ComposerChipStyle(
      tint: tint,
      backgroundColor: accent?.withAlphaComponent(0.11) ?? UIColor(
        composerHex: isSkill ? theme.skillBackground : theme.chipBackground
      ) ?? .secondarySystemFill,
      borderColor: accent.map { blend($0, over: border, weight: 0.34) } ?? UIColor(
        composerHex: isSkill ? theme.skillBorder : theme.chipBorder
      ) ?? .separator,
      textColor: tint
    )
    let image = renderChip(
      label: token.label,
      detail: token.detail,
      iconName: iconName,
      iconImage: iconImage,
      style: style
    )
    let font = UIFont(name: fontFamily, size: fontSize)
      ?? UIFont.systemFont(ofSize: fontSize)
    let baselineOffset = floor((font.capHeight - image.size.height) / 2)
    let attachment = ComposerTextAttachment(
      source: token.source,
      label: token.label,
      image: image,
      size: image.size,
      baselineOffset: baselineOffset
    )
    let attributedAttachment = NSMutableAttributedString(attachment: attachment)
    attributedAttachment.addAttributes(
      baseAttributes(),
      range: NSRange(location: 0, length: attributedAttachment.length)
    )
    return attributedAttachment
  }

  /// Chip glyphs with no SF Symbol that reads correctly. A pull request would otherwise land on
  /// `arrow.triangle.branch`, a road-sign fork that says "branch", not "pull request", so it is
  /// drawn from the same lucide geometry web and Android use.
  private static func vectorIcon(named name: String, size: CGFloat, color: UIColor) -> UIImage? {
    guard name == "git-pull-request" else { return nil }
    return UIGraphicsImageRenderer(size: CGSize(width: size, height: size)).image { _ in
      let s = size / 24  // lucide authors on a 24pt grid.
      let path = UIBezierPath()
      for centre in [CGPoint(x: 18 * s, y: 18 * s), CGPoint(x: 6 * s, y: 6 * s)] {
        path.append(UIBezierPath(arcCenter: centre, radius: 3 * s, startAngle: 0,
                                 endAngle: .pi * 2, clockwise: true))
      }
      path.move(to: CGPoint(x: 13 * s, y: 6 * s))
      path.addLine(to: CGPoint(x: 16 * s, y: 6 * s))
      path.addCurve(to: CGPoint(x: 18 * s, y: 8 * s),
                    controlPoint1: CGPoint(x: 17.1 * s, y: 6 * s),
                    controlPoint2: CGPoint(x: 18 * s, y: 6.9 * s))
      path.addLine(to: CGPoint(x: 18 * s, y: 15 * s))
      path.move(to: CGPoint(x: 6 * s, y: 9 * s))
      path.addLine(to: CGPoint(x: 6 * s, y: 21 * s))
      path.lineWidth = 2 * s
      path.lineCapStyle = .round
      path.lineJoinStyle = .round
      color.setStroke()
      path.stroke()
    }
  }

  private func renderChip(
    label: String,
    detail: String?,
    iconName: String,
    iconImage: UIImage?,
    style: ComposerChipStyle
  ) -> UIImage {
    // Kept in step with `T3ContextChipVectorIcon` in the markdown module: a chip drawn here and
    // the same chip drawn in a sent message have to be the same picture.
    let chipFontSize = fontSize * 0.86
    let font = UIFont(name: "DMSans-Medium", size: chipFontSize)
      ?? UIFont.systemFont(ofSize: chipFontSize, weight: .medium)
    let fallbackIcon = Self.vectorIcon(named: iconName, size: 14, color: style.textColor)
      ?? UIImage(
        systemName: iconName,
        withConfiguration: UIImage.SymbolConfiguration(pointSize: 12, weight: .medium)
      )
    let icon = iconImage ?? fallbackIcon
    // The size reads as metadata, not part of the name, so it renders a step down from the
    // label the way the web chip does.
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = .left
    let detailFont = UIFont(name: "DMSans-Medium", size: chipFontSize * 0.84)
      ?? UIFont.systemFont(ofSize: chipFontSize * 0.84, weight: .medium)
    let attributedLabel = NSMutableAttributedString(
      string: label,
      attributes: [.font: font, .foregroundColor: style.textColor, .paragraphStyle: paragraph]
    )
    if let detail, !detail.isEmpty {
      attributedLabel.append(
        NSAttributedString(
          string: " \(detail)",
          attributes: [
            .font: detailFont,
            .foregroundColor: style.textColor,
            .paragraphStyle: paragraph,
          ]
        )
      )
    }
    let iconWidth: CGFloat = icon == nil ? 0 : chipFontSize * 1.17
    let iconGap: CGFloat = icon == nil ? 0 : chipFontSize * 0.33
    let padding = chipFontSize * 0.5
    let height = ceil(chipFontSize * 1.41)
    // A long path would otherwise draw a chip wider than the composer and clip. Cap the label
    // to the text the editor can actually show and truncate inside it, as Android's
    // `maximumWidth` does, so the chip always fits the line it sits on.
    let availableWidth = textView.textContainer.size.width > 0
      ? textView.textContainer.size.width - textView.textContainer.lineFragmentPadding * 2
      : (textView.window?.bounds.width ?? bounds.width)
    if availableWidth <= 0 {
      chipsNeedMeasuredWidth = true
    }
    let maximumLabelWidth = max(chipFontSize * 3, availableWidth - padding * 2 - iconWidth - iconGap)
    paragraph.lineBreakMode = .byTruncatingMiddle
    attributedLabel.addAttribute(
      .paragraphStyle,
      value: paragraph,
      range: NSRange(location: 0, length: attributedLabel.length)
    )
    let measured = attributedLabel.size()
    let textSize = CGSize(width: min(measured.width, maximumLabelWidth), height: measured.height)
    let width = ceil(padding * 2 + iconWidth + iconGap + textSize.width)
    let format = UIGraphicsImageRendererFormat.preferred()
    format.opaque = false
    let renderer = UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format)
    return renderer.image { context in
      let rect = CGRect(origin: .zero, size: CGSize(width: width, height: height))
      let path = UIBezierPath(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), cornerRadius: chipFontSize * 0.5)
      style.backgroundColor.setFill()
      path.fill()
      style.borderColor.setStroke()
      path.lineWidth = 1
      path.stroke()

      var x = padding
      if let icon {
        let renderedIcon = iconImage == nil
          ? icon.withTintColor(style.tint, renderingMode: .alwaysOriginal)
          : icon
        renderedIcon.draw(
          in: CGRect(x: x, y: (height - iconWidth) / 2, width: iconWidth, height: iconWidth)
        )
        x += iconWidth + iconGap
      }
      // Exactly the measured width: a spare pixel here would let a capped label draw past
      // the cap instead of truncating inside it.
      attributedLabel.draw(
        in: CGRect(x: x, y: (height - textSize.height) / 2, width: textSize.width, height: textSize.height)
      )
      context.cgContext.setAllowsAntialiasing(true)
    }
  }

  private func blend(_ accent: UIColor, over base: UIColor, weight: CGFloat) -> UIColor {
    var ar: CGFloat = 0, ag: CGFloat = 0, ab: CGFloat = 0, aa: CGFloat = 0
    var br: CGFloat = 0, bg: CGFloat = 0, bb: CGFloat = 0, ba: CGFloat = 0
    accent.getRed(&ar, green: &ag, blue: &ab, alpha: &aa)
    base.getRed(&br, green: &bg, blue: &bb, alpha: &ba)
    return UIColor(
      red: ar * weight + br * (1 - weight),
      green: ag * weight + bg * (1 - weight),
      blue: ab * weight + bb * (1 - weight),
      alpha: aa * weight + ba * (1 - weight)
    )
  }

  private func iconImage(for uri: String) -> UIImage? {
    if let image = iconImages[uri] {
      return image
    }
    if uri.hasPrefix(T3ComposerBundledIcon.prefix) {
      // The pinned app icon family is local-only. Invalid/missing identifiers
      // must not fall through to URLSession as if they were remote URLs.
      guard let image = T3ComposerBundledIcon.image(uri) else { return nil }
      iconImages[uri] = image
      return image
    }
    guard !pendingIconUris.contains(uri), let url = URL(string: uri) else {
      return nil
    }

    if url.isFileURL, let image = UIImage(contentsOfFile: url.path) {
      iconImages[uri] = image
      return image
    }

    pendingIconUris.insert(uri)
    let task = URLSession.shared.dataTask(with: url) { [weak self] data, _, _ in
      guard let self, let data, let image = UIImage(data: data) else {
        DispatchQueue.main.async {
          self?.iconTasks.removeValue(forKey: uri)
          self?.pendingIconUris.remove(uri)
        }
        return
      }
      DispatchQueue.main.async {
        self.iconTasks.removeValue(forKey: uri)
        self.pendingIconUris.remove(uri)
        guard self.alive, self.tokens.contains(where: { $0.iconUri == uri }) else { return }
        self.iconImages[uri] = image
        self.applyControlledDocument(force: true)
      }
    }
    iconTasks[uri] = task; task.resume()
    return nil
  }

  private func baseAttributes() -> [NSAttributedString.Key: Any] {
    let font = UIFont(name: fontFamily, size: fontSize)
      ?? UIFont.systemFont(ofSize: fontSize)
    let paragraph = NSMutableParagraphStyle()
    paragraph.minimumLineHeight = lineHeight
    paragraph.maximumLineHeight = lineHeight
    return [
      .font: font,
      .foregroundColor: UIColor(composerHex: theme.text) ?? .label,
      .paragraphStyle: paragraph,
    ]
  }

  private func applyTypography() {
    guard textView.markedTextRange == nil else { deferredRebuild = true; return }
    let font = UIFont(name: fontFamily, size: fontSize)
      ?? UIFont.systemFont(ofSize: fontSize)
    textView.font = font
    restoreBaseTypingAttributes()
    placeholderLabel.font = font
    setNeedsLayout()
  }

  private func restoreBaseTypingAttributes() {
    guard textView.markedTextRange == nil else {
      return
    }
    textView.typingAttributes = baseAttributes()
  }

  private func applyTheme() {
    guard textView.markedTextRange == nil else { deferredRebuild = true; return }
    textView.textColor = UIColor(composerHex: theme.text) ?? .label
    placeholderLabel.textColor = UIColor(composerHex: theme.placeholder) ?? .placeholderText
    tintColor = UIColor.systemBlue
  }

  private func emitTextChange() {
    guard alive, !isApplyingControlledValue else {
      return
    }
    textView.invalidatePendingPaste()
    value = textView.serializedText()
    rebaseTokensFromDisplay()
    let selection = sourceSelection()
    nativeEventCount += 1
    onComposerChange([
      "value": value,
      "selection": ["start": selection.start, "end": selection.end],
      "eventCount": nativeEventCount,
    ])
    updatePlaceholderVisibility()
    emitContentSizeIfNeeded()
  }

  private func emitSelection() {
    // Caret moves advance the revision counter like text edits do: a
    // controlled payload computed before this move is stale and must fail the
    // revision guard instead of yanking the caret back mid-typing.
    textView.invalidatePendingPaste()
    let currentValue = textView.serializedText()
    let selection = sourceSelection()
    nativeEventCount += 1
    onComposerSelectionChange([
      "value": currentValue,
      "selection": ["start": selection.start, "end": selection.end],
      "eventCount": nativeEventCount,
    ])
  }

  func sourceSelection() -> ComposerSelectionPayload {
    ComposerSelectionPayload(
      start: textView.sourceOffset(forDisplayOffset: textView.selectedRange.location),
      end: textView.sourceOffset(forDisplayOffset: NSMaxRange(textView.selectedRange))
    )
  }

  private func displayRange(for selection: ComposerSelectionPayload) -> NSRange {
    let start = displayOffset(forSourceOffset: selection.start)
    let end = displayOffset(forSourceOffset: selection.end)
    return NSRange(location: start, length: max(0, end - start))
  }

  private func displayOffset(forSourceOffset sourceOffset: Int) -> Int {
    let boundedOffset = max(0, min((value as NSString).length, sourceOffset))
    var collapsedLength = 0
    for token in tokens where token.end <= boundedOffset {
      collapsedLength += max(0, token.end - token.start - 1)
    }
    if let token = tokens.first(where: { $0.start < boundedOffset && boundedOffset < $0.end }) {
      return token.start - collapsedLength + 1
    }
    return boundedOffset - collapsedLength
  }

  private func applyRequestedSelection() {
    guard textView.markedTextRange == nil else { return }
    guard let requestedSelection else {
      return
    }
    let nextRange = displayRange(for: requestedSelection)
    guard nextRange.location <= textView.attributedText.length,
          NSMaxRange(nextRange) <= textView.attributedText.length else {
      return
    }
    self.requestedSelection = nil
    // Programmatically assigning selectedRange resets the keyboard's
    // autocorrect and predictive-text context even when the range is
    // unchanged, so a no-op assignment must be skipped.
    guard !NSEqualRanges(nextRange, textView.selectedRange) else {
      return
    }
    textView.invalidatePendingPaste()
    isApplyingControlledValue = true
    textView.selectedRange = nextRange
    isApplyingControlledValue = false
  }

  private func updatePlaceholderVisibility() {
    placeholderLabel.isHidden = !value.isEmpty
  }

  private func emitContentSizeIfNeeded() {
    let nextSize = textView.contentSize
    guard abs(nextSize.width - lastContentSize.width) > 0.5 ||
      abs(nextSize.height - lastContentSize.height) > 0.5 else {
      return
    }
    lastContentSize = nextSize
    onComposerContentSizeChange(["width": nextSize.width, "height": nextSize.height])
  }

  private func decode<T: Decodable>(_ type: T.Type, from json: String) -> T? {
    guard let data = json.data(using: .utf8) else {
      return nil
    }
    return try? JSONDecoder().decode(type, from: data)
  }

  private func tokensMatchCurrentValue() -> Bool {
    let source = value as NSString
    return tokens.allSatisfy {
      $0.start >= 0 &&
        $0.end > $0.start &&
        $0.end <= source.length &&
        source.substring(with: NSRange(location: $0.start, length: $0.end - $0.start)) == $0.source
    }
  }

  private func documentMatchesExpectedTokens() -> Bool {
    let source = value as NSString
    let expectedSources = tokens.compactMap { token -> String? in
      guard token.start >= 0,
            token.end > token.start,
            token.end <= source.length,
            source.substring(
              with: NSRange(location: token.start, length: token.end - token.start)
            ) == token.source else {
        return nil
      }
      return token.source
    }
    var renderedSources: [String] = []
    textView.attributedText.enumerateAttribute(
      .attachment,
      in: NSRange(location: 0, length: textView.attributedText.length)
    ) { value, _, _ in
      if let attachment = value as? ComposerTextAttachment {
        renderedSources.append(attachment.source)
      }
    }
    return renderedSources == expectedSources
  }
}

/// Restricted app-owned static assets, matching the browser renderer's dev-root
/// then bundle convention. This does not reach private Exact asset generations.
enum T3ComposerBundledIcon {
  static let prefix = "t3-bundled-icon:"
  static var roots: [URL] {
    [ProcessInfo.processInfo.environment["EXACT_ASSETS"].map { URL(fileURLWithPath: $0, isDirectory: true) },
     Bundle.main.resourceURL].compactMap { $0 }
  }
  static func relativePath(_ identifier: String) -> String? {
    guard identifier.hasPrefix(prefix) else { return nil }
    let key = identifier.dropFirst(prefix.count)
    guard !key.isEmpty, key.utf8.count <= 128, key.unicodeScalars.allSatisfy({ scalar in
      (97...122).contains(scalar.value) || (48...57).contains(scalar.value) || scalar == "_" || scalar == "-"
    }) else { return nil }
    return "assets/file-icons/pierre_\(key).png"
  }
  static func image(_ identifier: String, roots: [URL] = roots) -> UIImage? {
    guard let path = relativePath(identifier) else { return nil }
    for root in roots where root.isFileURL {
      if let image = UIImage(contentsOfFile: root.appendingPathComponent(path).path) { return image }
    }
    return nil
  }
}

private extension UIColor {
  convenience init?(composerHex hex: String?) {
    guard var value = hex?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else {
      return nil
    }
    if value.hasPrefix("#") {
      value.removeFirst()
    }
    guard value.count == 6 || value.count == 8,
          let raw = UInt64(value, radix: 16) else {
      return nil
    }
    if value.count == 8 {
      self.init(
        red: CGFloat((raw >> 24) & 0xff) / 255,
        green: CGFloat((raw >> 16) & 0xff) / 255,
        blue: CGFloat((raw >> 8) & 0xff) / 255,
        alpha: CGFloat(raw & 0xff) / 255
      )
    } else {
      self.init(
        red: CGFloat((raw >> 16) & 0xff) / 255,
        green: CGFloat((raw >> 8) & 0xff) / 255,
        blue: CGFloat(raw & 0xff) / 255,
        alpha: 1
      )
    }
  }
}

#endif
