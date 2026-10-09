import Foundation
import UserNotifications
import WispUtilities

/// macOS notifications for routines (spec §66). Asks for permission the first time it's needed.
public enum Notifier {
    public static func post(title: String, body: String) async {
        // Notifications need a real app bundle (not a bare test binary).
        guard Bundle.main.bundleIdentifier != nil else { return }
        let center = UNUserNotificationCenter.current()
        let settings = await center.notificationSettings()
        if settings.authorizationStatus == .notDetermined {
            _ = try? await center.requestAuthorization(options: [.alert, .sound])
        }
        guard await center.notificationSettings().authorizationStatus == .authorized else { return }
        let content = UNMutableNotificationContent()
        content.title = String(title.prefix(80))
        content.body = String(body.prefix(240))
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        do { try await center.add(request) } catch {
            Log.app.error("notification failed: \(error.localizedDescription, privacy: .public)")
        }
    }
}
