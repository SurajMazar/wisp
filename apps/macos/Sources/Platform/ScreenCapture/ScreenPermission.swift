import CoreGraphics
import Foundation
import WispIPC

/// Screen Recording (TCC) status. macOS offers no "denied vs never asked" distinction here, so we
/// remember whether we've asked before.
public enum ScreenPermission {
    private static let askedKey = "WispScreenRecordingRequested"

    public static var status: PermissionState {
        if CGPreflightScreenCaptureAccess() { return .enabled }
        return UserDefaults.standard.bool(forKey: askedKey) ? .needsAttention : .notRequested
    }

    /// Shows the system prompt the first time; afterwards macOS only lists Wisp in System Settings.
    @discardableResult
    public static func request() -> PermissionState {
        UserDefaults.standard.set(true, forKey: askedKey)
        return CGRequestScreenCaptureAccess() ? .enabled : status
    }
}
