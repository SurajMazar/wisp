import AppKit
import Testing
@testable import WispPlatform

@MainActor
@Suite struct TextInserterTests {
    /// A private pasteboard, so the test never touches the user's clipboard.
    private func board() -> NSPasteboard { NSPasteboard(name: .init("wisp-test-\(UUID().uuidString)")) }

    @Test func restoresEveryItemAndTypeAfterPasting() throws {
        let b = board()
        defer { b.releaseGlobally() }
        let rich = NSPasteboardItem()
        rich.setString("Hello", forType: .string)
        rich.setData(Data("<b>Hello</b>".utf8), forType: .html)
        let second = NSPasteboardItem()
        second.setString("second item", forType: .string)
        b.clearContents()
        b.writeObjects([rich, second])

        let saved = ClipboardSnapshot(b)
        b.clearContents()
        b.setString("dictated text", forType: .string)
        saved.restore(to: b, ifUnchangedSince: b.changeCount)

        let items = try #require(b.pasteboardItems)
        #expect(items.count == 2)
        #expect(items[0].string(forType: .string) == "Hello")
        #expect(items[0].data(forType: .html) == Data("<b>Hello</b>".utf8))
        #expect(items[1].string(forType: .string) == "second item")
    }

    @Test func keepsSomethingTheUserCopiedMeanwhile() {
        let b = board()
        defer { b.releaseGlobally() }
        b.clearContents()
        b.setString("old", forType: .string)
        let saved = ClipboardSnapshot(b)
        b.clearContents()
        b.setString("dictated", forType: .string)
        let ours = b.changeCount
        b.clearContents()
        b.setString("copied by the user", forType: .string)  // changed after our paste
        saved.restore(to: b, ifUnchangedSince: ours)
        #expect(b.string(forType: .string) == "copied by the user")
    }

    @Test func restoresAnEmptyClipboardAsEmpty() {
        let b = board()
        defer { b.releaseGlobally() }
        b.clearContents()
        let saved = ClipboardSnapshot(b)
        b.setString("dictated", forType: .string)
        saved.restore(to: b, ifUnchangedSince: b.changeCount)
        #expect(b.string(forType: .string) == nil)
    }
}
