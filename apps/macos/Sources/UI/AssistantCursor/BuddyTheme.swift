import AppKit
import WispIPC

/// Resolves cursor settings to concrete colors (mirrors the web accent palette).
public enum BuddyTheme {
    public static func color(for theme: AccentColor, custom: String?) -> NSColor {
        switch theme {
        case .default: NSColor(srgbHex: 0x7C6CF2)
        case .blue: NSColor(srgbHex: 0x3B82F6)
        case .purple: NSColor(srgbHex: 0x9B5DE5)
        case .cyan: NSColor(srgbHex: 0x0FB5C9)
        case .green: NSColor(srgbHex: 0x2FAE66)
        case .orange: NSColor(srgbHex: 0xF08A24)
        case .red: NSColor(srgbHex: 0xE5484D)
        case .pink: NSColor(srgbHex: 0xE5559B)
        case .monochrome: NSColor(srgbHex: 0x8A8A84)
        case .custom: custom.flatMap(NSColor.init(hexString:)) ?? NSColor(srgbHex: 0x7C6CF2)
        }
    }

    /// Wisp's own default has a second, cooler tone for its glow.
    public static func secondary(for theme: AccentColor, primary: NSColor) -> NSColor {
        theme == .default ? NSColor(srgbHex: 0x5EC8F2) : primary
    }

    public static func stateColor(_ state: AssistantState, accent: NSColor) -> NSColor {
        switch state {
        case .waitingForPermission: NSColor(srgbHex: 0xE2A33B)
        case .success: NSColor(srgbHex: 0x3FB86E)
        case .error: NSColor(srgbHex: 0xE5484D)
        default: accent
        }
    }

    public static func label(for state: AssistantState) -> String? {
        switch state {
        case .idle, .following: nil
        case .listening: "Listening…"
        case .selecting: "Point at something"
        case .drawing: "Drawing…"
        case .analyzing: "Looking…"
        case .thinking: "Thinking…"
        case .speaking: "Speaking…"
        case .executing: "Working…"
        case .waitingForPermission: "Waiting for you"
        case .success: "Done"
        case .error: "Something went wrong"
        }
    }
}

extension NSColor {
    convenience init(srgbHex: UInt32) {
        self.init(srgbRed: CGFloat((srgbHex >> 16) & 0xFF) / 255, green: CGFloat((srgbHex >> 8) & 0xFF) / 255,
                  blue: CGFloat(srgbHex & 0xFF) / 255, alpha: 1)
    }

    convenience init?(hexString: String) {
        guard hexString.count == 7, hexString.hasPrefix("#"), let v = UInt32(hexString.dropFirst(), radix: 16) else { return nil }
        self.init(srgbHex: v)
    }
}
