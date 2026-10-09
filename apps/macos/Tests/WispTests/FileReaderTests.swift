import AppKit
import Foundation
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite(.serialized) struct FileReaderTests {
    /// Point the data folder at a temporary directory for the duration of a test.
    @MainActor private func withTempData(_ body: @MainActor (URL) async throws -> Void) async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-files-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("files"), withIntermediateDirectories: true)
        setenv("WISP_DATA_DIR", dir.path, 1)
        defer {
            unsetenv("WISP_DATA_DIR")
            try? FileManager.default.removeItem(at: dir)
        }
        try await body(dir)
    }

    @Test @MainActor func readsPdfPagesAndRtf() async throws {
        try await withTempData { dir in
            // A two-page PDF with real text.
            let pdfURL = dir.appendingPathComponent("files/report.pdf")
            var box = CGRect(x: 0, y: 0, width: 612, height: 792)
            let ctx = try #require(CGContext(pdfURL as CFURL, mediaBox: &box, nil))
            for text in ["Quarterly revenue was 4.2 million.", "Next year we hire two designers."] {
                ctx.beginPDFPage(nil)
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
                (text as NSString).draw(at: CGPoint(x: 72, y: 700), withAttributes: [.font: NSFont.systemFont(ofSize: 14)])
                NSGraphicsContext.restoreGraphicsState()
                ctx.endPDFPage()
            }
            ctx.closePDF()
            let pdf = try await FileReader.extract("files/report.pdf")
            #expect(pdf.paged)
            #expect(pdf.pages.count == 2)
            #expect(pdf.pages[1].contains("two designers"))

            let rtf = NSAttributedString(string: "Meeting notes: ship on Friday.")
            let data = try rtf.data(from: NSRange(location: 0, length: rtf.length), documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf])
            try data.write(to: dir.appendingPathComponent("files/notes.rtf"))
            let doc = try await FileReader.extract("files/notes.rtf")
            #expect(!doc.paged && doc.pages.first?.contains("ship on Friday") == true)
        }
    }

    @Test @MainActor func refusesPathsOutsideTheFilesFolder() async throws {
        try await withTempData { dir in
            try "secret".write(to: dir.appendingPathComponent("outside.txt"), atomically: true, encoding: .utf8)
            for bad in ["outside.txt", "files/../outside.txt", "/etc/hosts", "files/\u{0}x"] {
                await #expect(throws: FileReader.Failure.self) { _ = try await FileReader.extract(bad) }
            }
            #expect(FileReader.safeName("../../evil;rm.pdf") == "evil_rm.pdf")
            #expect(FileReader.safeName(".hidden") == "hidden")
        }
    }
}
