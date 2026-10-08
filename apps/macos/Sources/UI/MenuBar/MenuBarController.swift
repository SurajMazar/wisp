import AppKit
import WispIPC

/// Menu bar status item. Reflects engine state honestly; actions are injected by the app.
@MainActor
public final class MenuBarController: NSObject {
    public struct Actions {
        public var summon: () -> Void
        public var openMain: () -> Void
        public var openSettings: () -> Void
        public var quit: () -> Void
        public init(summon: @escaping () -> Void, openMain: @escaping () -> Void, openSettings: @escaping () -> Void, quit: @escaping () -> Void) {
            self.summon = summon
            self.openMain = openMain
            self.openSettings = openSettings
            self.quit = quit
        }
    }

    private var item: NSStatusItem?
    private let actions: Actions
    private let statusLine = NSMenuItem(title: "Engine: starting…", action: nil, keyEquivalent: "")
    private let summonItem = NSMenuItem(title: "Summon Wisp", action: nil, keyEquivalent: "")

    /// Show the current shortcut next to "Summon Wisp" (display only; the shortcut itself is global).
    public func setShortcutHint(_ text: String) {
        summonItem.title = "Summon Wisp    \(text)"
    }

    public init(actions: Actions) {
        self.actions = actions
        super.init()
    }

    public var isVisible: Bool { item != nil }

    public func setVisible(_ visible: Bool) {
        if visible, item == nil { install() } else if !visible, let item {
            NSStatusBar.system.removeStatusItem(item)
            self.item = nil
        }
    }

    public func update(engine: CoreProcess.State) {
        statusLine.title = switch engine {
        case .running: "Engine: running"
        case .starting: "Engine: starting…"
        case .restarting(let attempt, _): "Engine: restarting (attempt \(attempt))…"
        case .stopped: "Engine: stopped"
        case .failed(let reason): "Engine: \(reason)"
        }
        item?.button?.appearsDisabled = { if case .running = engine { false } else { true } }()
    }

    private func install() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = Self.icon()
        item.button?.setAccessibilityLabel("Wisp")
        let menu = NSMenu()
        summonItem.action = #selector(summon)
        summonItem.target = self
        menu.addItem(summonItem)
        menu.addItem(withTitle: "Open Wisp", action: #selector(openMain), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Settings…", action: #selector(openSettings), keyEquivalent: ",").target = self
        menu.addItem(.separator())
        statusLine.isEnabled = false
        menu.addItem(statusLine)
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Wisp", action: #selector(quit), keyEquivalent: "q").target = self
        item.menu = menu
        self.item = item
    }

    @objc private func summon() { actions.summon() }
    @objc private func openMain() { actions.openMain() }
    @objc private func openSettings() { actions.openSettings() }
    @objc private func quit() { actions.quit() }

    /// Template image: a small core with a halo arc — Wisp's mark, drawn so it adapts to the menu bar.
    static func icon() -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            NSColor.black.setFill()
            NSBezierPath(ovalIn: NSRect(x: 5.5, y: 5.5, width: 7, height: 7)).fill()
            let halo = NSBezierPath()
            halo.appendArc(withCenter: NSPoint(x: rect.midX, y: rect.midY), radius: 7, startAngle: 200, endAngle: 70)
            halo.lineWidth = 1.4
            halo.lineCapStyle = .round
            NSColor.black.setStroke()
            halo.stroke()
            return true
        }
        image.isTemplate = true
        return image
    }
}
