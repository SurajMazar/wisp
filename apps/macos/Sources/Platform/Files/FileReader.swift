import AppKit
import PDFKit
import UniformTypeIdentifiers
import WispIPC
import WispUtilities

/// Files for local search (spec §68, §103): the user picks files (the host copies them into
/// Wisp's data folder — Wisp never reads the originals in place), and the host reads text from
/// rich formats. Nothing here executes file content.
@MainActor
public enum FileReader {
    public enum Failure: Error, LocalizedError {
        case invalidPath, unreadable(String)
        public var errorDescription: String? {
            switch self {
            case .invalidPath: "That file isn't in Wisp's files folder."
            case .unreadable(let m): m
            }
        }
    }

    static var filesRoot: URL { AppPaths.dataDirectory.appendingPathComponent("files", isDirectory: true) }

    /// `relative` must be `files/…` and stay inside the files folder (no `..`, no symlink escape).
    static func resolve(_ relative: String) throws -> URL {
        guard relative.hasPrefix("files/"), !relative.contains(".."), !relative.contains("\0") else { throw Failure.invalidPath }
        let url = AppPaths.dataDirectory.appendingPathComponent(relative).standardizedFileURL.resolvingSymlinksInPath()
        let root = filesRoot.standardizedFileURL.resolvingSymlinksInPath().path + "/"
        guard url.path.hasPrefix(root) else { throw Failure.invalidPath }
        return url
    }

    static func safeName(_ name: String) -> String {
        let base = (name as NSString).lastPathComponent
        let cleaned = String(base.map { $0.isLetter || $0.isNumber || " ._-()".contains($0) ? $0 : "_" })
            .trimmingCharacters(in: .whitespaces)
            .drop(while: { $0 == "." })
        let s = String(cleaned.prefix(120))
        return s.isEmpty ? "file" : s
    }

    /// Show the open panel; copy the chosen files into the files folder.
    public static func pick() -> StagedFiles {
        let panel = NSOpenPanel()
        panel.title = "Add files to Wisp"
        panel.prompt = "Add"
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [.plainText, .text, .sourceCode, .json, .commaSeparatedText, .pdf, .rtf, .html, .image, .data]
        NSApp.activate()
        guard panel.runModal() == .OK else { return StagedFiles(files: []) }
        try? FileManager.default.createDirectory(at: filesRoot, withIntermediateDirectories: true)
        var out: [StagedFile] = []
        for src in panel.urls.prefix(50) {
            guard let values = try? src.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey]),
                  values.isRegularFile == true, (values.fileSize ?? 0) <= 50 * 1024 * 1024 else { continue }
            let name = safeName(src.lastPathComponent)
            let relative = "files/\(UUID().uuidString.lowercased())-\(name)"
            let dst = AppPaths.dataDirectory.appendingPathComponent(relative)
            do {
                try FileManager.default.copyItem(at: src, to: dst)
                out.append(StagedFile(name: name, path: relative, sizeBytes: UInt64(values.fileSize ?? 0)))
            } catch {
                Log.app.error("copy failed: \(error.localizedDescription, privacy: .public)")
            }
        }
        return StagedFiles(files: out)
    }

    /// Text from a PDF (per page), a document (Word, RTF, HTML) or an image (recognized on device).
    public static func extract(_ relative: String) async throws -> ExtractedText {
        let url = try resolve(relative)
        let ext = url.pathExtension.lowercased()
        if ext == "pdf" {
            guard let doc = PDFDocument(url: url) else { throw Failure.unreadable("This PDF couldn't be opened.") }
            if doc.isLocked { throw Failure.unreadable("This PDF is password-protected.") }
            let pages = (0..<min(doc.pageCount, 2000)).map { doc.page(at: $0)?.string ?? "" }
            if pages.allSatisfy({ $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) {
                throw Failure.unreadable("This PDF has no selectable text (it may be a scan).")
            }
            return ExtractedText(paged: true, pages: pages)
        }
        if ["png", "jpg", "jpeg", "heic", "tiff", "tif", "gif", "bmp", "webp"].contains(ext) {
            guard let image = NSImage(contentsOf: url)?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
                throw Failure.unreadable("This image couldn't be opened.")
            }
            let crop = CGRect(x: 0, y: 0, width: image.width, height: image.height)
            let text = await TextRecognizer.recognize(image, crop: crop, maxLines: 2000)
            let joined = (text?.lines ?? []).map(\.text).joined(separator: "\n")
            if joined.isEmpty { throw Failure.unreadable("No text was found in this image.") }
            return ExtractedText(paged: false, pages: [joined])
        }
        // Word, RTF, HTML, ODT: macOS's own document readers (no scripts, no network).
        let options: [NSAttributedString.DocumentReadingOptionKey: Any] = [:]
        guard let attributed = try? NSAttributedString(url: url, options: options, documentAttributes: nil) else {
            throw Failure.unreadable("This document couldn't be read.")
        }
        return ExtractedText(paged: false, pages: [attributed.string])
    }

    public static func open(_ relative: String) throws {
        NSWorkspace.shared.open(try resolve(relative))
    }
}
