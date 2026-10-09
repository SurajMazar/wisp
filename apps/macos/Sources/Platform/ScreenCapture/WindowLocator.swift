import AppKit
import WispIPC

/// Finds the frontmost regular window under a point, excluding Wisp's own windows.
public enum WindowLocator {
    public struct Found: Sendable {
        public let window: WindowInfo
        public let application: ApplicationInfo?
    }

    /// `point` in global CG points.
    @MainActor
    public static func window(at point: CGPoint) -> Found? {
        let me = ProcessInfo.processInfo.processIdentifier
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
            return nil
        }
        for w in list {  // front to back
            guard (w[kCGWindowLayer as String] as? Int) == 0,
                  let pid = w[kCGWindowOwnerPID as String] as? pid_t, pid != me,
                  let boundsDict = w[kCGWindowBounds as String] as? NSDictionary,
                  let bounds = CGRect(dictionaryRepresentation: boundsDict),
                  bounds.contains(point), bounds.width > 40, bounds.height > 40,
                  let number = w[kCGWindowNumber as String] as? Int
            else { continue }
            let title = (w[kCGWindowName as String] as? String).flatMap { $0.isEmpty ? nil : $0 }
            return Found(
                window: WindowInfo(frame: Rect(bounds), id: UInt32(number), title: title),
                application: application(pid: pid)
            )
        }
        return nil
    }

    @MainActor
    public static func application(pid: pid_t) -> ApplicationInfo? {
        guard let app = NSRunningApplication(processIdentifier: pid) else { return nil }
        return ApplicationInfo(bundleId: app.bundleIdentifier, name: app.localizedName ?? "App", pid: Int(pid))
    }

    @MainActor
    public static func frontmostApplication() -> ApplicationInfo? {
        guard let app = NSWorkspace.shared.frontmostApplication,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return nil }
        return application(pid: app.processIdentifier)
    }
}
