import AppKit
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers
import WispIPC
import WispUtilities

/// One-shot captures of a screen region via ScreenCaptureKit. Never continuous (spec §37, §86):
/// a capture happens only when the user completes a focus selection.
///
/// Wisp's floating windows (buddy, island, drawing overlay, panel, guidance) are excluded from every
/// capture. Its main window is a normal window like any other app's, so a question about it sees it.
public enum ScreenCapturer {
    public enum CaptureError: Error, LocalizedError {
        case permissionDenied
        case displayNotFound(CGDirectDisplayID)
        case emptyRegion
        case encodeFailed

        public var errorDescription: String? {
            switch self {
            case .permissionDenied: "Screen access is disabled."
            case .displayNotFound(let id): "Display \(id) is no longer connected."
            case .emptyRegion: "The selected area is empty."
            case .encodeFailed: "The capture couldn't be saved."
            }
        }
    }

    /// Capture `rect` (global CG points) on `display`, at the display's native pixel density.
    public static func capture(rect: CGRect, display: DisplayInfo, excludingSelf: Bool = true) async throws -> CGImage {
        guard CGPreflightScreenCaptureAccess() else { throw CaptureError.permissionDenied }
        let local = FocusGeometry.local(rect, in: display.frame.cg)
        guard local.width >= 1, local.height >= 1 else { throw CaptureError.emptyRegion }

        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let scDisplay = content.displays.first(where: { $0.displayID == CGDirectDisplayID(display.id) }) else {
            throw CaptureError.displayNotFound(CGDirectDisplayID(display.id))
        }
        let pid = ProcessInfo.processInfo.processIdentifier
        let overlays = excludingSelf
            ? content.windows.filter { $0.owningApplication?.processID == pid && $0.windowLayer != 0 }
            : []
        let filter = SCContentFilter(display: scDisplay, excludingWindows: overlays)

        let config = SCStreamConfiguration()
        let px = FocusGeometry.pixelSize(local, scale: display.scale)
        config.sourceRect = local
        config.width = px.width
        config.height = px.height
        config.scalesToFit = false
        config.showsCursor = false
        config.captureResolution = .best
        config.colorSpaceName = CGColorSpace.sRGB
        return try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
    }

    /// Write `image` as PNG into `directory/<id>.png`, keeping at most `keep` recent captures.
    public static func save(_ image: CGImage, id: String, directory: URL, keep: Int = 12) throws -> URL {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent("\(id).png")
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            throw CaptureError.encodeFailed
        }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else { throw CaptureError.encodeFailed }
        // A compressed copy sized for vision models (spec §98: compress screenshots).
        if let small = downscaled(image, maxSide: 1280) {
            let modelURL = directory.appendingPathComponent("\(id).model.jpg")
            if let d = CGImageDestinationCreateWithURL(modelURL as CFURL, UTType.jpeg.identifier as CFString, 1, nil) {
                CGImageDestinationAddImage(d, small, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
                CGImageDestinationFinalize(d)
            }
        }
        prune(directory: directory, keep: keep)
        return url
    }

    /// Image no larger than `maxSide` on its longest edge (unchanged if already smaller).
    public static func downscaled(_ image: CGImage, maxSide: Int) -> CGImage? {
        let longest = max(image.width, image.height)
        guard longest > maxSide else { return image }
        let scale = Double(maxSide) / Double(longest)
        let w = Int((Double(image.width) * scale).rounded()), h = Int((Double(image.height) * scale).rounded())
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        return ctx.makeImage()
    }

    static func prune(directory: URL, keep: Int) {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.contentModificationDateKey])
            .filter({ $0.pathExtension == "png" }) else { return }
        // Remove the model JPEG alongside each pruned PNG.
        let sorted = files.sorted {
            let a = (try? $0.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
            let b = (try? $1.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
            return a > b
        }
        for f in sorted.dropFirst(keep) {
            try? fm.removeItem(at: f)
            try? fm.removeItem(at: f.deletingPathExtension().appendingPathExtension("model.jpg"))
            try? fm.removeItem(at: f.deletingPathExtension().appendingPathExtension("context.txt"))
        }
    }
}
