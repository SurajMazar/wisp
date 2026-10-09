import AppKit
import Carbon.HIToolbox
import WispIPC

/// Conversions between macOS key representations and contract types.
public enum KeyNames {
    /// Canonical modifier order for display: ⌃ ⌥ ⇧ ⌘ fn.
    public static let order: [Modifier] = [.control, .option, .shift, .command, .function]

    public static func symbol(_ m: Modifier) -> String {
        switch m {
        case .control: "⌃"
        case .option: "⌥"
        case .shift: "⇧"
        case .command: "⌘"
        case .function: "fn"
        }
    }

    public static func display(_ s: Shortcut) -> String {
        let mods = order.filter(s.modifiers.contains).map(symbol).joined()
        guard let label = s.keyLabel ?? s.keyCode.map(label(forKeyCode:)) else { return mods }
        return mods + label
    }

    public static func modifiers(from flags: NSEvent.ModifierFlags) -> Set<Modifier> {
        var out = Set<Modifier>()
        if flags.contains(.control) { out.insert(.control) }
        if flags.contains(.option) { out.insert(.option) }
        if flags.contains(.shift) { out.insert(.shift) }
        if flags.contains(.command) { out.insert(.command) }
        if flags.contains(.function) { out.insert(.function) }
        return out
    }

    public static func modifiers(from flags: CGEventFlags) -> Set<Modifier> {
        var out = Set<Modifier>()
        if flags.contains(.maskControl) { out.insert(.control) }
        if flags.contains(.maskAlternate) { out.insert(.option) }
        if flags.contains(.maskShift) { out.insert(.shift) }
        if flags.contains(.maskCommand) { out.insert(.command) }
        if flags.contains(.maskSecondaryFn) { out.insert(.function) }
        return out
    }

    public static func carbonModifiers(_ mods: [Modifier]) -> UInt32 {
        var m: UInt32 = 0
        for mod in mods {
            switch mod {
            case .command: m |= UInt32(cmdKey)
            case .option: m |= UInt32(optionKey)
            case .control: m |= UInt32(controlKey)
            case .shift: m |= UInt32(shiftKey)
            case .function: break  // not expressible as a Carbon hot key modifier
            }
        }
        return m
    }

    /// Key codes that are modifiers themselves (never a shortcut's "key").
    public static let modifierKeyCodes: Set<Int> = [
        kVK_Command, kVK_RightCommand, kVK_Shift, kVK_RightShift, kVK_Option, kVK_RightOption,
        kVK_Control, kVK_RightControl, kVK_CapsLock, kVK_Function,
    ]

    private static let special: [Int: String] = [
        kVK_Space: "Space", kVK_Return: "Return", kVK_Tab: "Tab", kVK_Delete: "Delete",
        kVK_ForwardDelete: "⌦", kVK_Escape: "Esc", kVK_LeftArrow: "←", kVK_RightArrow: "→",
        kVK_UpArrow: "↑", kVK_DownArrow: "↓", kVK_Home: "Home", kVK_End: "End",
        kVK_PageUp: "Page Up", kVK_PageDown: "Page Down",
        kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5", kVK_F6: "F6",
        kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        kVK_ANSI_A: "A", kVK_ANSI_B: "B", kVK_ANSI_C: "C", kVK_ANSI_D: "D", kVK_ANSI_E: "E",
        kVK_ANSI_F: "F", kVK_ANSI_G: "G", kVK_ANSI_H: "H", kVK_ANSI_I: "I", kVK_ANSI_J: "J",
        kVK_ANSI_K: "K", kVK_ANSI_L: "L", kVK_ANSI_M: "M", kVK_ANSI_N: "N", kVK_ANSI_O: "O",
        kVK_ANSI_P: "P", kVK_ANSI_Q: "Q", kVK_ANSI_R: "R", kVK_ANSI_S: "S", kVK_ANSI_T: "T",
        kVK_ANSI_U: "U", kVK_ANSI_V: "V", kVK_ANSI_W: "W", kVK_ANSI_X: "X", kVK_ANSI_Y: "Y",
        kVK_ANSI_Z: "Z", kVK_ANSI_0: "0", kVK_ANSI_1: "1", kVK_ANSI_2: "2", kVK_ANSI_3: "3",
        kVK_ANSI_4: "4", kVK_ANSI_5: "5", kVK_ANSI_6: "6", kVK_ANSI_7: "7", kVK_ANSI_8: "8",
        kVK_ANSI_9: "9", kVK_ANSI_Comma: ",", kVK_ANSI_Period: ".", kVK_ANSI_Slash: "/",
        kVK_ANSI_Semicolon: ";", kVK_ANSI_Quote: "'", kVK_ANSI_LeftBracket: "[",
        kVK_ANSI_RightBracket: "]", kVK_ANSI_Backslash: "\\", kVK_ANSI_Minus: "-",
        kVK_ANSI_Equal: "=", kVK_ANSI_Grave: "`",
    ]

    /// The key code whose label is `label` (e.g. "S" → kVK_ANSI_S).
    public static func keyCode(forLabel label: String) -> Int? {
        special.first { $0.value == label }?.key
    }

    public static func label(forKeyCode code: Int) -> String {
        special[code] ?? "Key \(code)"
    }
}
