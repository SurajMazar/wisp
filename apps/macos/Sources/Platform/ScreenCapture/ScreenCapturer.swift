import AppKit
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers
import WispIPC
import WispUtilities

/// One-shot captures of a screen region via ScreenCaptureKit. Never continuous (spec §37, §86):
/// a capture happens only when the user completes a focus selection.
///
/// Wisp's own windows (buddy, overlay, panel) are excluded from every capture.
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
        let me = excludingSelf ? content.applications.filter { $0.processID == ProcessInfo.processInfo.processIdentifier } : []
        let filter = SCContentFilter(display: scDisplay, excludingApplications: me, exceptingWindows: [])

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
        prune(directory: directory, keep: keep)
        return url
    }

    static func prune(directory: URL, keep: Int) {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.contentModificationDateKey])
            .filter({ $0.pathExtension == "png" }) else { return }
        let sorted = files.sorted {
            let a = (try? $0.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
            let b = (try? $1.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
            return a > b
        }
        for f in sorted.dropFirst(keep) { try? fm.removeItem(at: f) }
    }
}
