import CoreGraphics
import Foundation
import Vision
import WispIPC

/// Reads the text in a captured region on device (Apple Vision). Never leaves the Mac.
public enum TextRecognizer {
    /// - Parameters:
    ///   - image: the captured pixels of `crop`.
    ///   - crop: where the image came from, in global points (top-left origin).
    public static func recognize(_ image: CGImage, crop: CGRect, maxLines: Int = 300) async -> ScreenText? {
        let started = Date()
        return await Task.detached(priority: .userInitiated) { () -> ScreenText? in
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = true
            request.automaticallyDetectsLanguage = true
            let handler = VNImageRequestHandler(cgImage: image, options: [:])
            do { try handler.perform([request]) } catch { return nil }
            let observations = request.results ?? []
            var lines: [TextLine] = []
            for o in observations {
                guard let best = o.topCandidates(1).first else { continue }
                let text = best.string.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !text.isEmpty else { continue }
                // Vision boxes are normalized with a bottom-left origin.
                let b = o.boundingBox
                let frame = CGRect(
                    x: crop.minX + b.minX * crop.width,
                    y: crop.minY + (1 - b.maxY) * crop.height,
                    width: b.width * crop.width,
                    height: b.height * crop.height
                )
                lines.append(TextLine(confidence: Double(best.confidence), frame: Rect(frame), text: String(text.prefix(500))))
            }
            // Reading order: rows top to bottom (tolerating small baseline differences), then left to right.
            lines.sort { a, b in
                abs(a.frame.y - b.frame.y) < min(a.frame.height, b.frame.height) * 0.5 ? a.frame.x < b.frame.x : a.frame.y < b.frame.y
            }
            return ScreenText(elapsedMs: UInt32(Date().timeIntervalSince(started) * 1000), lines: Array(lines.prefix(maxLines)))
        }.value
    }
}
