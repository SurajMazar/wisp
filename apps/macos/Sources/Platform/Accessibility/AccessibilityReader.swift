import AppKit
import ApplicationServices
import Synchronization
import WispIPC

/// Reads macOS Accessibility (AX) data: the element under a point, its ancestors, the focused
/// element, selected text, and the elements intersecting a region.
///
/// Every traversal is bounded by node count, depth and wall-clock time, and runs off the main
/// thread (AX calls block while the target app answers). Secure text field values are never read.
public enum AccessibilityReader {
    public static var isTrusted: Bool { AXIsProcessTrusted() }

    public static var status: PermissionState {
        if AXIsProcessTrusted() { return .enabled }
        return UserDefaults.standard.bool(forKey: askedKey) ? .needsAttention : .notRequested
    }

    private static let askedKey = "WispAccessibilityRequested"

    /// Shows the system prompt (first time) which links to System Settings.
    @discardableResult
    public static func requestPermission() -> PermissionState {
        UserDefaults.standard.set(true, forKey: askedKey)
        let opts = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        return AXIsProcessTrustedWithOptions(opts) ? .enabled : status
    }

    public struct Limits: Sendable {
        public var maxNodes = 400
        public var maxDepth = 40
        public var budget: Duration = .milliseconds(250)
        public init(maxNodes: Int = 400) { self.maxNodes = max(1, min(maxNodes, 600)) }
    }

    /// Snapshot around `point` (global CG points) and inside `region`, without blocking the caller's actor.
    private static let prepared = Mutex<Set<pid_t>>([])

    /// Ask Chromium and Electron apps (Chrome, Brave, Edge, Slack, VS Code…) to expose their web
    /// content to accessibility — they don't by default, so their buttons were invisible to Wisp
    /// and it couldn't point at them. `AXManualAccessibility` is the switch assistive tools use;
    /// other apps ignore it. Once per app process.
    public nonisolated static func prepare(pid: pid_t) {
        guard pid > 0, AXIsProcessTrusted(), prepared.withLock({ $0.insert(pid).inserted }) else { return }
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.2)
        _ = AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue)
    }

    public static func snapshot(point: CGPoint, region: CGRect?, limits: Limits, excludingPID: pid_t) async -> AxSnapshot {
        await Task.detached(priority: .userInitiated) {
            read(point: point, region: region, limits: limits, excludingPID: excludingPID)
        }.value
    }

    // MARK: Traversal (nonisolated, synchronous)

    static func read(point: CGPoint, region: CGRect?, limits: Limits, excludingPID: pid_t) -> AxSnapshot {
        let clock = ContinuousClock()
        let start = clock.now
        let deadline = start + limits.budget
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.2)

        var under: AXUIElement?
        var hit: AXUIElement?
        if AXUIElementCopyElementAtPosition(system, Float(point.x), Float(point.y), &hit) == .success, let hit {
            var pid: pid_t = 0
            AXUIElementGetPid(hit, &pid)
            if pid != excludingPID { under = hit }
        }

        // The app to describe: owner of the element under the point, else the focused app.
        var appElement: AXUIElement?
        var appPID: pid_t = 0
        if let under {
            AXUIElementGetPid(under, &appPID)
            appElement = AXUIElementCreateApplication(appPID)
        } else if let focusedApp: AXUIElement = copy(system, kAXFocusedApplicationAttribute) {
            AXUIElementGetPid(focusedApp, &appPID)
            if appPID != excludingPID { appElement = focusedApp }
        }
        if let appElement { AXUIElementSetMessagingTimeout(appElement, 0.2) }
        prepare(pid: appPID)

        let window: AXUIElement? = under.flatMap { windowOf($0) }
            ?? appElement.flatMap { copy($0, kAXFocusedWindowAttribute) }
        let focused: AXUIElement? = appElement.flatMap { copy($0, kAXFocusedUIElementAttribute) }
        let selectedText: String? = focused.flatMap { (try? secure($0)) == true ? nil : string($0, kAXSelectedTextAttribute) }

        var ancestors: [AxNode] = []
        if let under {
            var cur: AXUIElement? = copy(under, kAXParentAttribute)
            while let c = cur, ancestors.count < 8, clock.now < deadline {
                ancestors.append(node(c, depth: 0))
                if role(c) == "AXWindow" { break }
                cur = copy(c, kAXParentAttribute)
            }
        }

        var nodes: [AxNode] = []
        var truncated = false
        if let region, let window {
            var queue: [(AXUIElement, Int)] = [(window, 0)]
            var head = 0
            while head < queue.count {
                if nodes.count >= limits.maxNodes || clock.now >= deadline { truncated = true; break }
                let (el, depth) = queue[head]
                head += 1
                let n = node(el, depth: depth)
                if let f = n.frame?.cg {
                    if f.width > 0, f.height > 0, !f.intersects(region) { continue }  // prune off-region subtrees
                    if f.intersects(region) { nodes.append(n) }
                }
                guard depth < limits.maxDepth else { continue }
                let kids: [AXUIElement] = copy(el, kAXChildrenAttribute) ?? []
                for k in kids.prefix(200) { queue.append((k, depth + 1)) }
            }
        }

        let app = NSRunningApplication(processIdentifier: appPID)
        let windowInfo: WindowInfo? = window.flatMap { w in
            frame(w).map { WindowInfo(frame: Rect($0), id: 0, title: string(w, kAXTitleAttribute)) }
        }
        return AxSnapshot(
            ancestors: ancestors,
            application: app.map { ApplicationInfo(bundleId: $0.bundleIdentifier, name: $0.localizedName ?? "App", pid: Int(appPID)) },
            elapsedMs: UInt32(max(0, (clock.now - start).components.seconds * 1000 + (clock.now - start).components.attoseconds / 1_000_000_000_000_000)),
            focused: focused.map { node($0, depth: 0) },
            nodes: nodes,
            selectedText: selectedText.map { String($0.prefix(2000)) },
            truncated: truncated,
            underPointer: under.map { node($0, depth: 0) },
            window: windowInfo
        )
    }

    static func node(_ el: AXUIElement, depth: Int) -> AxNode {
        let r = role(el) ?? "AXUnknown"
        let sub: String? = string(el, kAXSubroleAttribute)
        let isSecure = r == "AXSecureTextField" || sub == "AXSecureTextField"
        var actionNames: CFArray?
        AXUIElementCopyActionNames(el, &actionNames)
        let actions = (actionNames as? [String] ?? []).map(strip).filter { !$0.isEmpty }
        return AxNode(
            actions: Array(actions.prefix(8)),
            depth: UInt32(depth),
            description: string(el, kAXDescriptionAttribute) ?? string(el, kAXHelpAttribute),
            enabled: bool(el, kAXEnabledAttribute) ?? true,
            focused: bool(el, kAXFocusedAttribute) ?? false,
            frame: frame(el).map(Rect.init),
            identifier: string(el, kAXIdentifierAttribute),
            role: strip(r),
            selected: bool(el, kAXSelectedAttribute) ?? false,
            subrole: sub.map(strip),
            title: string(el, kAXTitleAttribute),
            value: isSecure ? nil : valueString(el)
        )
    }

    // MARK: Attribute helpers

    static func copy<T>(_ el: AXUIElement, _ attr: String) -> T? {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(el, attr as CFString, &v) == .success, let v else { return nil }
        return v as? T
    }

    static func string(_ el: AXUIElement, _ attr: String) -> String? {
        let s: String? = copy(el, attr)
        return s.flatMap { $0.isEmpty ? nil : String($0.prefix(1000)) }
    }

    static func bool(_ el: AXUIElement, _ attr: String) -> Bool? {
        let n: NSNumber? = copy(el, attr)
        return n?.boolValue
    }

    static func role(_ el: AXUIElement) -> String? { string(el, kAXRoleAttribute) }

    static func secure(_ el: AXUIElement) throws -> Bool {
        role(el) == "AXSecureTextField" || string(el, kAXSubroleAttribute) == "AXSecureTextField"
    }

    static func valueString(_ el: AXUIElement) -> String? {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(el, kAXValueAttribute as CFString, &v) == .success, let v else { return nil }
        if let s = v as? String { return s.isEmpty ? nil : String(s.prefix(1000)) }
        if let n = v as? NSNumber { return n.stringValue }
        return nil
    }

    /// Frame in global CG points (AX already uses top-left coordinates).
    static func frame(_ el: AXUIElement) -> CGRect? {
        guard let pv: AXValue = copy(el, kAXPositionAttribute), let sv: AXValue = copy(el, kAXSizeAttribute) else { return nil }
        var p = CGPoint.zero, s = CGSize.zero
        guard AXValueGetValue(pv, .cgPoint, &p), AXValueGetValue(sv, .cgSize, &s) else { return nil }
        return CGRect(origin: p, size: s)
    }

    static func windowOf(_ el: AXUIElement) -> AXUIElement? {
        if role(el) == "AXWindow" { return el }
        return copy(el, kAXWindowAttribute)
    }

    static func strip(_ s: String) -> String { s.hasPrefix("AX") ? String(s.dropFirst(2)) : s }
}
