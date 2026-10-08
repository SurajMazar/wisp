import AppKit
import Carbon.HIToolbox
import WispIPC

/// Detects shortcut conflicts with macOS, common app shortcuts and Wisp's own bindings (spec §16).
///
/// macOS system shortcuts come from two sources: the user's overrides in the
/// `com.apple.symbolichotkeys` domain (which can disable or remap defaults) and the built-in
/// defaults below for entries the user never touched.
public enum ShortcutConflicts {
    struct Known {
        let name: String
        let modifiers: Set<Modifier>
        let keyCode: Int
    }

    /// Default system shortcuts keyed by their symbolic hot key id.
    static let symbolicDefaults: [Int: Known] = [
        64: Known(name: "Spotlight", modifiers: [.command], keyCode: kVK_Space),
        65: Known(name: "Finder search window", modifiers: [.command, .option], keyCode: kVK_Space),
        60: Known(name: "Select previous input source", modifiers: [.control], keyCode: kVK_Space),
        61: Known(name: "Select next input source", modifiers: [.control, .option], keyCode: kVK_Space),
        28: Known(name: "Screenshot", modifiers: [.command, .shift], keyCode: kVK_ANSI_3),
        29: Known(name: "Screenshot to clipboard", modifiers: [.control, .command, .shift], keyCode: kVK_ANSI_3),
        30: Known(name: "Screenshot of selection", modifiers: [.command, .shift], keyCode: kVK_ANSI_4),
        31: Known(name: "Selection to clipboard", modifiers: [.control, .command, .shift], keyCode: kVK_ANSI_4),
        184: Known(name: "Screenshot and recording options", modifiers: [.command, .shift], keyCode: kVK_ANSI_5),
        32: Known(name: "Mission Control", modifiers: [.control], keyCode: kVK_UpArrow),
        33: Known(name: "Application windows", modifiers: [.control], keyCode: kVK_DownArrow),
        79: Known(name: "Move left a space", modifiers: [.control], keyCode: kVK_LeftArrow),
        81: Known(name: "Move right a space", modifiers: [.control], keyCode: kVK_RightArrow),
    ]

    /// System shortcuts that aren't configurable symbolic hot keys.
    static let fixedSystem: [Known] = [
        Known(name: "App switcher", modifiers: [.command], keyCode: kVK_Tab),
        Known(name: "Force Quit", modifiers: [.command, .option], keyCode: kVK_Escape),
        Known(name: "Lock Screen", modifiers: [.control, .command], keyCode: kVK_ANSI_Q),
        Known(name: "Emoji & Symbols", modifiers: [.control, .command], keyCode: kVK_Space),
    ]

    /// Shortcuts most apps use; binding them globally would shadow them everywhere.
    static let commonApp: [Known] = [
        ("Copy", kVK_ANSI_C), ("Paste", kVK_ANSI_V), ("Cut", kVK_ANSI_X), ("Undo", kVK_ANSI_Z),
        ("Select All", kVK_ANSI_A), ("Save", kVK_ANSI_S), ("Find", kVK_ANSI_F), ("New", kVK_ANSI_N),
        ("New Tab", kVK_ANSI_T), ("Open", kVK_ANSI_O), ("Print", kVK_ANSI_P), ("Close Window", kVK_ANSI_W),
        ("Quit", kVK_ANSI_Q), ("Hide", kVK_ANSI_H), ("Minimize", kVK_ANSI_M), ("Settings", kVK_ANSI_Comma),
        ("Command palette in many apps", kVK_ANSI_K), ("Address bar", kVK_ANSI_L), ("Reload", kVK_ANSI_R),
    ].map { Known(name: $0.0, modifiers: [.command], keyCode: $0.1) } + [
        Known(name: "Command palette (VS Code and others)", modifiers: [.command, .shift], keyCode: kVK_ANSI_P),
        Known(name: "Developer tools in browsers", modifiers: [.command, .option], keyCode: kVK_ANSI_I),
    ]

    public static func check(_ shortcut: Shortcut, systemOverrides: [String: Any]? = currentSymbolicHotKeys()) -> [ShortcutConflict] {
        let mods = Set(shortcut.modifiers)
        guard let code = shortcut.keyCode else {
            // Modifier-only chords can't collide with key shortcuts; Wisp ignores the chord when
            // another key is pressed during it (D-006). Only Wisp-internal collisions are possible.
            return []
        }
        var out: [ShortcutConflict] = []
        for k in systemShortcuts(overrides: systemOverrides) where k.modifiers == mods && k.keyCode == code {
            out.append(ShortcutConflict(severity: .blocking, with: "macOS: \(k.name)"))
        }
        for k in fixedSystem where k.modifiers == mods && k.keyCode == code {
            out.append(ShortcutConflict(severity: .blocking, with: "macOS: \(k.name)"))
        }
        for k in commonApp where k.modifiers == mods && k.keyCode == code {
            out.append(ShortcutConflict(severity: .warning, with: "\(k.name) (most apps)"))
        }
        if mods == [.command] && code == kVK_ANSI_K {
            out.append(ShortcutConflict(severity: .warning, with: "Wisp's command palette"))
        }
        return out
    }

    /// Effective system shortcuts: defaults, minus disabled ones, plus user remaps.
    static func systemShortcuts(overrides: [String: Any]?) -> [Known] {
        var effective = symbolicDefaults
        for (key, raw) in overrides ?? [:] {
            guard let id = Int(key), let entry = raw as? [String: Any] else { continue }
            let enabled = (entry["enabled"] as? Bool) ?? ((entry["enabled"] as? Int) == 1)
            guard enabled else { effective.removeValue(forKey: id); continue }
            guard let value = entry["value"] as? [String: Any],
                  let params = value["parameters"] as? [Int], params.count == 3,
                  params[1] != 65535 else { continue }
            let name = effective[id]?.name ?? "a system keyboard shortcut"
            let mods = KeyNames.modifiers(from: NSEvent.ModifierFlags(rawValue: UInt(params[2])))
            effective[id] = Known(name: name, modifiers: mods, keyCode: params[1])
        }
        return Array(effective.values)
    }

    public static func currentSymbolicHotKeys() -> [String: Any]? {
        UserDefaults.standard.persistentDomain(forName: "com.apple.symbolichotkeys")?["AppleSymbolicHotKeys"] as? [String: Any]
    }
}
