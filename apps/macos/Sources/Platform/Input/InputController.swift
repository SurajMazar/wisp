import AppKit
import ApplicationServices
import Carbon.HIToolbox
import WispIPC

/// Performs approved computer-control actions (spec §55) and observes whether they worked (§62).
///
/// Called only by the core after validation, risk analysis and the user's approval. Refuses on its
/// own when it can't act safely: without Accessibility permission, while secure input is on, when
/// typing would land in a password field, or when the target is one of Wisp's own windows.
@MainActor
public enum InputController {
    public enum Failure: Error, LocalizedError {
        case notTrusted, refused(String), failed(String)
        public var errorDescription: String? {
            switch self {
            case .notTrusted: "Wisp needs Accessibility permission to click and type for you."
            case .refused(let m), .failed(let m): m
            }
        }
    }

    // MARK: Entry point

    /// Frontmost app when each approval was shown (bounded).
    private static var approvalFronts: [String: pid_t] = [:]

    public static func noteApprovalShown(id: String) {
        if approvalFronts.count > 64 { approvalFronts.removeAll() }
        approvalFronts[id] = NSWorkspace.shared.frontmostApplication?.processIdentifier
    }

    public static func perform(_ call: ToolCall, approvalId: String? = nil, expectedPid: Int32? = nil) async throws -> ToolExecution {
        // The approved action must land where the user approved it.
        if let id = approvalId, let front = approvalFronts.removeValue(forKey: id),
           NSWorkspace.shared.frontmostApplication?.processIdentifier != front, isKeyboard(call) {
            throw Failure.refused("The app in front changed since you approved this, so Wisp didn't do it.")
        }
        if let expected = expectedPid, let pt = pointOf(call), let e = element(at: pt), pid(of: e) != expected {
            throw Failure.refused("Something else is under that spot now, so Wisp didn't click it.")
        }
        return try await performChecked(call)
    }

    private static func isKeyboard(_ call: ToolCall) -> Bool {
        switch call {
        case .typeText, .keypress, .hotkey, .paste, .copy: true
        default: false
        }
    }

    private static func pointOf(_ call: ToolCall) -> CGPoint? {
        switch call {
        case .click(let p), .doubleClick(let p), .rightClick(let p), .moveMouse(let p): CGPoint(x: p.x, y: p.y)
        default: nil
        }
    }

    private static func performChecked(_ call: ToolCall) async throws -> ToolExecution {
        switch call {
        case .readClipboard:
            let text = NSPasteboard.general.string(forType: .string)
            return ToolExecution(output: text.map { String($0.prefix(10_000)) } ?? "", verification: .init(detail: text == nil ? "The clipboard has no text" : "Read the clipboard", status: text == nil ? .unknown : .verified))
        case .writeClipboard(let t):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(t.text, forType: .string)
            let ok = NSPasteboard.general.string(forType: .string) == t.text
            return ToolExecution(verification: .init(detail: ok ? "The clipboard has the text" : "The clipboard didn't take the text", status: ok ? .verified : .failed))
        case .openUrl(let u):
            guard let url = URL(string: u.url), ["http", "https"].contains(url.scheme?.lowercased() ?? "") else { throw Failure.refused("Only web addresses can be opened.") }
            let ok = NSWorkspace.shared.open(url)
            return ToolExecution(verification: .init(detail: ok ? "Opened in your browser" : "macOS couldn't open it", status: ok ? .verified : .failed))
        case .openApplication(let a): return try await openApp(a.app)
        case .switchApplication(let a): return try await switchApp(a.app)
        case .closeApplication(let a): return try await quitApp(a.app)
        default: break
        }

        guard AXIsProcessTrusted() else { throw Failure.notTrusted }
        switch call {
        case .click(let p): return try await click(p, count: 1, button: .left)
        case .doubleClick(let p): return try await click(p, count: 2, button: .left)
        case .rightClick(let p): return try await click(p, count: 1, button: .right)
        case .moveMouse(let p):
            let pt = CGPoint(x: p.x, y: p.y)
            try ensureOnScreen(pt)
            move(to: pt)
            let now = cgCursor()
            let ok = hypot(now.x - pt.x, now.y - pt.y) < 2
            return ToolExecution(verification: .init(detail: ok ? "The pointer is there" : "The pointer didn't move", status: ok ? .verified : .failed))
        case .drag(let d): return try await drag(from: d.from.cg, to: d.to.cg)
        case .scroll(let s): return try await scroll(s)
        case .typeText(let t): return try await type(t.text)
        case .keypress(let k):
            guard let code = namedKeys[k.key] else { throw Failure.refused("Unknown key \(k.key).") }
            let before = observe(at: nil)
            try refuseIfOwnFocus()
            press(code, flags: [])
            return await verifyChange(from: before, at: nil, action: "pressed \(k.key)")
        case .hotkey(let h):
            guard let code = keyCode(for: h.key) else { throw Failure.refused("Unknown key \(h.key).") }
            let before = observe(at: nil)
            try refuseIfOwnFocus()
            press(code, flags: flags(h.modifiers))
            return await verifyChange(from: before, at: nil, action: "pressed the shortcut")
        case .copy:
            let count = NSPasteboard.general.changeCount
            try refuseIfOwnFocus()
            press(kVK_ANSI_C, flags: .maskCommand)
            try? await Task.sleep(for: .milliseconds(250))
            let ok = NSPasteboard.general.changeCount != count
            return ToolExecution(verification: .init(detail: ok ? "Copied" : "Nothing was copied (is anything selected?)", status: ok ? .verified : .unknown))
        case .paste:
            let before = observe(at: nil)
            try refuseIfOwnFocus()
            press(kVK_ANSI_V, flags: .maskCommand)
            return await verifyChange(from: before, at: nil, action: "pasted")
        default:
            throw Failure.refused("That action is handled by Wisp's engine.")
        }
    }

    // MARK: Pointer

    private static let ownPID = ProcessInfo.processInfo.processIdentifier

    private static func cgCursor() -> CGPoint {
        CGEvent(source: nil)?.location ?? .zero
    }

    private static func ensureOnScreen(_ p: CGPoint) throws {
        let onScreen = NSScreen.screens.contains {
            FocusGeometry.cgRect(fromAppKit: $0.frame, primaryHeight: Displays.primaryHeight).contains(p)
        }
        if !onScreen { throw Failure.refused("That spot isn't on any display.") }
    }

    private static func element(at p: CGPoint) -> AXUIElement? {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.3)
        var hit: AXUIElement?
        guard AXUIElementCopyElementAtPosition(system, Float(p.x), Float(p.y), &hit) == .success else { return nil }
        return hit
    }

    private static func pid(of e: AXUIElement) -> pid_t {
        var p: pid_t = 0
        AXUIElementGetPid(e, &p)
        return p
    }

    private static func string(_ e: AXUIElement, _ attr: String) -> String? {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(e, attr as CFString, &v) == .success else { return nil }
        if let s = v as? String { return s }
        if let n = v as? NSNumber { return n.stringValue }
        return nil
    }

    private static func parent(_ e: AXUIElement) -> AXUIElement? {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(e, kAXParentAttribute as CFString, &v) == .success, let v else { return nil }
        return (v as! AXUIElement)  // swiftlint:disable:this force_cast — CF type checked by the API
    }

    private static func actions(_ e: AXUIElement) -> [String] {
        var names: CFArray?
        guard AXUIElementCopyActionNames(e, &names) == .success else { return [] }
        return (names as? [String]) ?? []
    }

    private static func matches(_ e: AXUIElement, _ label: String) -> Bool {
        let want = label.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)
        return [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute, "AXIdentifier"].contains { attr in
            guard let s = string(e, attr)?.lowercased(), !s.isEmpty else { return false }
            return s == want || s.contains(want)
        }
    }

    private static func click(_ p: PointerArgs, count: Int, button: CGMouseButton) async throws -> ToolExecution {
        let pt = CGPoint(x: p.x, y: p.y)
        try ensureOnScreen(pt)
        let target = element(at: pt)
        if let target, pid(of: target) == ownPID { throw Failure.refused("That spot is covered by Wisp's own window.") }
        let before = observe(at: pt)

        // Prefer pressing the labeled control directly: exact, and works even if it moved slightly.
        if count == 1, button == .left, let label = p.label, var e = target {
            for _ in 0..<4 {
                if matches(e, label), actions(e).contains(kAXPressAction as String) {
                    if AXUIElementPerformAction(e, kAXPressAction as CFString) == .success {
                        return await verifyChange(from: before, at: pt, action: "pressed “\(label)”")
                    }
                    break
                }
                guard let up = parent(e) else { break }
                e = up
            }
        }

        move(to: pt)
        let (down, up): (CGEventType, CGEventType) = button == .right ? (.rightMouseDown, .rightMouseUp) : (.leftMouseDown, .leftMouseUp)
        for n in 1...count {
            for type in [down, up] {
                guard let ev = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: pt, mouseButton: button) else { continue }
                ev.setIntegerValueField(.mouseEventClickState, value: Int64(n))
                ev.post(tap: .cghidEventTap)
            }
            try? await Task.sleep(for: .milliseconds(40))
        }
        return await verifyChange(from: before, at: pt, action: button == .right ? "right-clicked" : count == 2 ? "double-clicked" : "clicked")
    }

    private static func move(to p: CGPoint) {
        CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
        CGWarpMouseCursorPosition(p)
    }

    private static func drag(from a: CGPoint, to b: CGPoint) async throws -> ToolExecution {
        try ensureOnScreen(a)
        try ensureOnScreen(b)
        if let e = element(at: a), pid(of: e) == ownPID { throw Failure.refused("That spot is covered by Wisp's own window.") }
        let before = observe(at: a)
        move(to: a)
        CGEvent(mouseEventSource: nil, mouseType: .leftMouseDown, mouseCursorPosition: a, mouseButton: .left)?.post(tap: .cghidEventTap)
        for i in 1...16 {
            let t = CGFloat(i) / 16
            let p = CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
            CGEvent(mouseEventSource: nil, mouseType: .leftMouseDragged, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
            try? await Task.sleep(for: .milliseconds(12))
        }
        CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp, mouseCursorPosition: b, mouseButton: .left)?.post(tap: .cghidEventTap)
        return await verifyChange(from: before, at: b, action: "dragged")
    }

    private static func scroll(_ s: ScrollArgs) async throws -> ToolExecution {
        let pt = CGPoint(x: s.x, y: s.y)
        try ensureOnScreen(pt)
        move(to: pt)
        let before = observe(at: pt)
        // CoreGraphics: positive wheel values scroll content up.
        let dy = Int32(clamping: max(-50, min(50, -s.dy))), dx = Int32(clamping: max(-50, min(50, -s.dx)))
        CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0)?.post(tap: .cghidEventTap)
        return await verifyChange(from: before, at: pt, action: "scrolled")
    }

    // MARK: Keyboard

    private static let namedKeys: [String: Int] = [
        "return": kVK_Return, "tab": kVK_Tab, "escape": kVK_Escape, "space": kVK_Space, "delete": kVK_Delete,
        "forwardDelete": kVK_ForwardDelete, "up": kVK_UpArrow, "down": kVK_DownArrow, "left": kVK_LeftArrow,
        "right": kVK_RightArrow, "home": kVK_Home, "end": kVK_End, "pageUp": kVK_PageUp, "pageDown": kVK_PageDown,
        "f1": kVK_F1, "f2": kVK_F2, "f3": kVK_F3, "f4": kVK_F4, "f5": kVK_F5, "f6": kVK_F6,
        "f7": kVK_F7, "f8": kVK_F8, "f9": kVK_F9, "f10": kVK_F10, "f11": kVK_F11, "f12": kVK_F12,
    ]

    static func keyCode(for key: String) -> Int? {
        if let k = namedKeys[key] { return k }
        guard key.count == 1 else { return nil }
        return KeyNames.keyCode(forLabel: key.uppercased())
    }

    private static func flags(_ m: [KeyModifier]) -> CGEventFlags {
        var f: CGEventFlags = []
        if m.contains(.command) { f.insert(.maskCommand) }
        if m.contains(.option) { f.insert(.maskAlternate) }
        if m.contains(.control) { f.insert(.maskControl) }
        if m.contains(.shift) { f.insert(.maskShift) }
        return f
    }

    private static func press(_ code: Int, flags: CGEventFlags) {
        for down in [true, false] {
            guard let ev = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(code), keyDown: down) else { continue }
            ev.flags = flags
            ev.post(tap: .cghidEventTap)
        }
    }

    private static func focusedElement() -> AXUIElement? {
        let system = AXUIElementCreateSystemWide()
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &v) == .success, let v else { return nil }
        return (v as! AXUIElement)  // swiftlint:disable:this force_cast
    }

    private static func refuseIfOwnFocus() throws {
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == ownPID {
            throw Failure.refused("Wisp's own window is in front. Click the app you want first.")
        }
    }

    private static func type(_ text: String) async throws -> ToolExecution {
        if IsSecureEventInputEnabled() { throw Failure.refused("A password field has secure input on, so Wisp won't type there.") }
        try refuseIfOwnFocus()
        let focused = focusedElement()
        if let f = focused {
            if string(f, kAXSubroleAttribute) == kAXSecureTextFieldSubrole as String {
                throw Failure.refused("That's a password field. Wisp never types passwords.")
            }
            if pid(of: f) == ownPID { throw Failure.refused("The cursor is in Wisp's own window.") }
        }
        let before = focused.flatMap { string($0, kAXValueAttribute) }
        for line in text.split(separator: "\n", omittingEmptySubsequences: false).enumerated() {
            if line.offset > 0 { press(kVK_Return, flags: []) }
            let utf16 = Array(String(line.element).utf16)
            var i = 0
            while i < utf16.count {
                // Re-check before every chunk: focus can move into a password field mid-typing.
                if IsSecureEventInputEnabled() { throw Failure.refused("A password field took focus, so Wisp stopped typing.") }
                if let f = focusedElement(), string(f, kAXSubroleAttribute) == kAXSecureTextFieldSubrole as String {
                    throw Failure.refused("A password field took focus, so Wisp stopped typing.")
                }
                let chunk = Array(utf16[i..<min(i + 16, utf16.count)])
                for down in [true, false] {
                    guard let ev = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down) else { continue }
                    chunk.withUnsafeBufferPointer { ev.keyboardSetUnicodeString(stringLength: chunk.count, unicodeString: $0.baseAddress) }
                    ev.post(tap: .cghidEventTap)
                }
                i += 16
                try? await Task.sleep(for: .milliseconds(8))
            }
        }
        try? await Task.sleep(for: .milliseconds(300))
        let after = focusedElement().flatMap { string($0, kAXValueAttribute) }
        let lastLine = text.split(separator: "\n").last.map(String.init) ?? text
        if let after, after.contains(lastLine) {
            return ToolExecution(verification: .init(detail: "The text is in the field", status: .verified))
        }
        if after != nil, after != before {
            return ToolExecution(verification: .init(detail: "The field changed", status: .verified))
        }
        return ToolExecution(verification: .init(detail: "Couldn't confirm the text landed (the app doesn't expose its field)", status: .unknown))
    }

    // MARK: Applications

    private static func appURL(_ name: String) -> URL? {
        if name.contains("."), !name.contains(" "), let u = NSWorkspace.shared.urlForApplication(withBundleIdentifier: name) { return u }
        let dirs = ["/Applications", "/System/Applications", "/System/Applications/Utilities", "/Applications/Utilities",
                    NSHomeDirectory() + "/Applications"]
        let want = name.lowercased().replacingOccurrences(of: ".app", with: "")
        for d in dirs {
            guard let items = try? FileManager.default.contentsOfDirectory(atPath: d) else { continue }
            if let hit = items.first(where: { $0.lowercased() == want + ".app" }) {
                return URL(fileURLWithPath: d).appendingPathComponent(hit)
            }
        }
        return nil
    }

    private static func running(_ name: String) -> NSRunningApplication? {
        let want = name.lowercased()
        return NSWorkspace.shared.runningApplications.first {
            $0.bundleIdentifier?.lowercased() == want || $0.localizedName?.lowercased() == want
        }
    }

    private static func waitFront(_ check: @escaping () -> Bool) async -> Bool {
        for _ in 0..<30 {
            if check() { return true }
            try? await Task.sleep(for: .milliseconds(100))
        }
        return false
    }

    private static func openApp(_ name: String) async throws -> ToolExecution {
        guard let url = appURL(name) else { throw Failure.failed("Couldn't find an app called \(name).") }
        let bundle = Bundle(url: url)?.bundleIdentifier
        let config = NSWorkspace.OpenConfiguration()
        config.activates = true
        _ = try await NSWorkspace.shared.openApplication(at: url, configuration: config)
        let ok = await waitFront { NSWorkspace.shared.frontmostApplication?.bundleIdentifier == bundle }
        return ToolExecution(verification: .init(detail: ok ? "\(name) is open and in front" : "\(name) opened but isn't in front", status: ok ? .verified : .unknown))
    }

    private static func switchApp(_ name: String) async throws -> ToolExecution {
        guard let app = running(name) else { throw Failure.failed("\(name) isn't running.") }
        app.activate()
        let ok = await waitFront { NSWorkspace.shared.frontmostApplication?.processIdentifier == app.processIdentifier }
        return ToolExecution(verification: .init(detail: ok ? "\(name) is in front" : "\(name) didn't come to the front", status: ok ? .verified : .failed))
    }

    private static func quitApp(_ name: String) async throws -> ToolExecution {
        guard let app = running(name) else { throw Failure.failed("\(name) isn't running.") }
        if app.processIdentifier == ownPID { throw Failure.refused("Wisp won't quit itself.") }
        _ = app.terminate()
        let ok = await waitFront { app.isTerminated }
        return ToolExecution(verification: .init(
            detail: ok ? "\(name) quit" : "\(name) was asked to quit — it may be waiting for you to save",
            status: ok ? .verified : .unknown
        ))
    }

    // MARK: Observation (verification)

    struct Observed: Equatable {
        var frontPID: pid_t?
        var window: String?
        var focused: String?
        var atPoint: String?
    }

    static func describe(_ e: AXUIElement?) -> String? {
        guard let e else { return nil }
        let parts = [kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute, kAXEnabledAttribute, kAXSelectedAttribute, "AXExpanded"]
            .map { string(e, $0) ?? "" }
        return parts.joined(separator: "|")
    }

    static func observe(at p: CGPoint?) -> Observed {
        let front = NSWorkspace.shared.frontmostApplication
        var window: String?
        if let pid = front?.processIdentifier {
            let app = AXUIElementCreateApplication(pid)
            AXUIElementSetMessagingTimeout(app, 0.2)
            var w: CFTypeRef?
            if AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute as CFString, &w) == .success, let w {
                window = string(w as! AXUIElement, kAXTitleAttribute)  // swiftlint:disable:this force_cast
            }
        }
        return Observed(frontPID: front?.processIdentifier, window: window, focused: describe(focusedElement()), atPoint: p.flatMap { describe(element(at: $0)) })
    }

    private static func verifyChange(from before: Observed, at p: CGPoint?, action: String) async -> ToolExecution {
        try? await Task.sleep(for: .milliseconds(450))
        let after = observe(at: p)
        let what: String? =
            if after.frontPID != before.frontPID { "a different app came to the front" }
            else if after.window != before.window { "the window is now “\(after.window ?? "untitled")”" }
            else if after.focused != before.focused { "the focus moved" }
            else if after.atPoint != before.atPoint { "the control changed" }
            else { nil }
        if let what {
            return ToolExecution(verification: .init(detail: "Wisp \(action); \(what)", status: .verified))
        }
        return ToolExecution(verification: .init(detail: "Wisp \(action), but nothing visibly changed", status: .unknown))
    }
}
