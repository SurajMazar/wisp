import Foundation
import Testing
@testable import WispUI

@Suite @MainActor struct SchemeHandlerTests {
    @Test func confinesPathsToWebRoot() throws {
        let fm = FileManager.default
        let root = fm.temporaryDirectory.appendingPathComponent("wisp-web-\(UUID().uuidString)")
        let outside = fm.temporaryDirectory.appendingPathComponent("wisp-out-\(UUID().uuidString)")
        try fm.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        try fm.createDirectory(at: outside, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root); try? fm.removeItem(at: outside) }
        try Data("ok".utf8).write(to: root.appendingPathComponent("index.html"))
        try Data("x".utf8).write(to: root.appendingPathComponent("assets/app.js"))
        try Data("secret".utf8).write(to: outside.appendingPathComponent("secret.txt"))
        try fm.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: outside)

        let h = WispSchemeHandler(root: root)
        #expect(h.resolve("/index.html") != nil)
        #expect(h.resolve("/assets/app.js") != nil)
        #expect(h.resolve("/../\(outside.lastPathComponent)/secret.txt") == nil)
        #expect(h.resolve("/assets/%2e%2e/%2e%2e/etc/passwd") == nil)
        #expect(h.resolve("/link/secret.txt") == nil)
        #expect(h.resolve("/assets") == nil, "directories are not served")
    }
}
