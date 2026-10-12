import AppKit
import ApplicationServices
import Carbon.HIToolbox
import WispIPC

/// Types dictated text where the user's cursor is (Step 5): paste it into the focused field, then
/// put the clipboard back. Pasting works in every kind of app (native, browsers, Electron); typing
/// key by key would be slow and break on non-ASCII text. Never types into a password field.
@MainActor
public enum TextInserter {
    public enum Failure: Error, LocalizedError, Equatable {
        case notTrusted
        case secureField
        public var errorDescription: String? {
            switch self {
            case .notTrusted: "Wisp needs Accessibility permission to type for you."
            case .secureField: "Wisp doesn't type into password fields."
            }
        }
    }

    /// How long the target app gets to read the pasted text before the clipboard is restored.
    static let restoreDelay: Duration = .milliseconds(350)

    public static func insert(_ text: String) async throws {
        guard AXIsProcessTrusted() else { throw Failure.notTrusted }
        if focusedFieldIsSecure() { throw Failure.secureField }
        let board = NSPasteboard.general
        let saved = ClipboardSnapshot(board)
        board.clearContents()
        board.setString(text, forType: .string)
        let ours = board.changeCount
        postCommandV()
        try? await Task.sleep(for: restoreDelay)
        saved.restore(to: board, ifUnchangedSince: ours)
    }

    /// The focused element is a secure text field (or reports itself as protected content).
    static func focusedFieldIsSecure() -> Bool {
        let system = AXUIElementCreateSystemWide()
        var focused: CFTypeRef?
        guard AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
              let element = focused, CFGetTypeID(element) == AXUIElementGetTypeID() else { return false }
        let el = element as! AXUIElement  // swiftlint:disable:this force_cast — type checked above
        func string(_ attr: String) -> String? {
            var v: CFTypeRef?
            return AXUIElementCopyAttributeValue(el, attr as CFString, &v) == .success ? v as? String : nil
        }
        return string(kAXRoleAttribute) == "AXSecureTextField" || string(kAXSubroleAttribute) == "AXSecureTextField"
    }

    private static func postCommandV() {
        let source = CGEventSource(stateID: .combinedSessionState)
        let v = CGKeyCode(kVK_ANSI_V)
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: source, virtualKey: v, keyDown: down)
            e?.flags = .maskCommand
            e?.post(tap: .cgSessionEventTap)
        }
    }
}

/// Everything on a pasteboard (every item, every type), so it can be put back exactly.
struct ClipboardSnapshot {
    private let items: [[NSPasteboard.PasteboardType: Data]]

    init(_ board: NSPasteboard) {
        items = (board.pasteboardItems ?? []).map { item in
            Dictionary(uniqueKeysWithValues: item.types.compactMap { t in item.data(forType: t).map { (t, $0) } })
        }
    }

    /// Put the saved contents back — unless something else changed the clipboard meanwhile (the
    /// user copied something new), which then wins.
    func restore(to board: NSPasteboard, ifUnchangedSince changeCount: Int) {
        guard board.changeCount == changeCount else { return }
        board.clearContents()
        let restored = items.map { saved -> NSPasteboardItem in
            let item = NSPasteboardItem()
            for (type, data) in saved { item.setData(data, forType: type) }
            return item
        }
        if !restored.isEmpty { board.writeObjects(restored) }
    }
}
