#if os(iOS)
// @ref llp/1106.008-mobile-voice.decision.md#native-transcription
// Adapted from pinned @react-native-ai/apple0.12.0 plus T3 Code365aa87982 patch.
// AppleTranscriptionImpl.swift by Mike Grabowski, 2025. MIT: ../../voice-APPLE-LICENSE.
import Foundation
import Speech
import AVFoundation

@available(iOS 26, *)
enum T3MobileVoiceTranscription {
    static func transcriber(_ locale: Locale) -> SpeechTranscriber {
        let preset = SpeechTranscriber.Preset.timeIndexedTranscriptionWithAlternatives
        return SpeechTranscriber(locale: locale, transcriptionOptions: preset.transcriptionOptions,
            reportingOptions: preset.reportingOptions.subtracting([.alternativeTranscriptions]), attributeOptions: preset.attributeOptions)
    }
    static func prepare(_ language: String) async throws -> String {
        guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: language)) else {
            throw VoiceFailure("unsupported-locale", "Voice transcription is not available for this language.")
        }
        try Task.checkCancellation()
        let module = transcriber(locale)
        switch await AssetInventory.status(forModules: [module]) {
        case .installed: break
        case .supported, .downloading:
            if let request = try await AssetInventory.assetInstallationRequest(supporting: [module]) { try await request.downloadAndInstall() }
        case .unsupported: throw VoiceFailure("unsupported-locale", "Voice transcription is not available for this language.")
        @unknown default: throw VoiceFailure("preparation-failed", "Could not prepare voice transcription.")
        }
        try Task.checkCancellation()
        return locale.identifier
    }
    static func transcribe(_ file: URL, locale: String) async throws -> String {
        let audio = try AVAudioFile(forReading: file)
        let module = transcriber(Locale(identifier: locale))
        let analyzer = SpeechAnalyzer(modules: [module])
        let collector = Task { () throws -> [String] in
            var segments: [String] = []
            for try await result in module.results {
                try Task.checkCancellation()
                if result.isFinal { segments.append(String(result.text.characters)) }
            }
            return segments
        }
        return try await withTaskCancellationHandler(operation: {
            do {
                let end = try await analyzer.analyzeSequence(from: audio)
                try Task.checkCancellation()
                if let end { try await analyzer.finalizeAndFinish(through: end) }
                else { await analyzer.cancelAndFinishNow() }
                let segments = try await collector.value
                try Task.checkCancellation()
                return segments.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
            } catch {
                collector.cancel(); await analyzer.cancelAndFinishNow(); _ = try? await collector.value
                throw error
            }
        }, onCancel: {
            collector.cancel()
            Task { await analyzer.cancelAndFinishNow() }
        })
    }
}
struct VoiceFailure: Error {
    let code: String
    let message: String
    init(_ code: String, _ message: String) { self.code = code; self.message = message }
}
#endif
