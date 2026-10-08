import AppKit
import WispIPC

/// Opens System Settings at a specific privacy pane.
public enum SystemSettings {
    public static func url(for pane: SettingsPane) -> URL {
        let privacy = "x-apple.systempreferences:com.apple.preference.security?"
        let s = switch pane {
        case .accessibility: privacy + "Privacy_Accessibility"
        case .inputMonitoring: privacy + "Privacy_ListenEvent"
        case .screenRecording: privacy + "Privacy_ScreenCapture"
        case .microphone: privacy + "Privacy_Microphone"
        case .automation: privacy + "Privacy_Automation"
        case .notifications: "x-apple.systempreferences:com.apple.Notifications-Settings.extension"
        case .keyboard: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension"
        }
        return URL(string: s)!
    }

    @MainActor
    public static func open(_ pane: SettingsPane) {
        NSWorkspace.shared.open(url(for: pane))
    }
}
