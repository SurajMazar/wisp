import AppKit
import SwiftUI
import WispIPC
import WispUtilities

/// The assistant cursor ("buddy"): a small companion that stays beside the pointer on every
/// display, shows what the assistant is doing, and opens the assistant when clicked.
///
/// It never replaces or hides the system cursor (spec §26) and is excluded from screenshots
/// and screen recordings (`sharingType = .none`), so Wisp never sees itself when it looks at the screen.
@MainActor
public final class BuddyController {
    public var onClick: (() -> Void)?

    private let window: NSPanel
    private let view: BuddyView
    private let labelWindow: NSPanel
    private let labelModel = BuddyLabelModel()
    private var motion = BuddyMotion()
    private var timer: Timer?
    private var fastTicks = true
    private var lastMove = Date.distantPast
    private var lastCursor = CGPoint(x: -1, y: -1)
    private var lastScreen: CGDirectDisplayID?
    private var settings: CursorSettings
    private var observers: [NSObjectProtocol] = []
    private var previewTask: Task<Void, Never>?

    public private(set) var state: AssistantState = .idle
    private var summoned = false
    private var suppressed = false
    private var hovering = false

    public init(settings: CursorSettings = CursorSettings(
        animation: .normal, customColor: nil, enabled: true, glow: 0.6, label: "Wisp", opacity: 0.95,
        showLabel: false, size: 1, theme: .default, visibility: .always
    )) {
        self.settings = settings
        view = BuddyView(style: Self.style(for: settings))
        window = Self.makePanel(size: view.frame.size, clickable: true)
        window.contentView = view
        labelWindow = Self.makePanel(size: NSSize(width: 120, height: 24), clickable: false)
        let host = NSHostingView(rootView: BuddyLabel(model: labelModel))
        host.sizingOptions = [.intrinsicContentSize]
        labelWindow.contentView = host
        view.onClick = { [weak self] in self?.onClick?() }

        let ws = NSWorkspace.shared.notificationCenter
        observers.append(ws.addObserver(forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.restyle() }
        })
        observers.append(NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.motion.reset() }
        })
        updateVisibility()
    }

    // MARK: Public API

    public func apply(_ newSettings: CursorSettings) {
        settings = newSettings
        restyle()
        updateLabel()
        updateVisibility()
    }

    public func set(state newState: AssistantState) {
        state = newState
        view.set(state: newState)
        updateLabel()
        updateVisibility()
    }

    /// The user is holding the invocation shortcut.
    public func summon(_ on: Bool) {
        summoned = on
        if on, state == .idle { set(state: .following) }
        if !on, state == .following { set(state: .idle) }
        updateVisibility()
    }

    /// Hide while the assistant panel is expanded from the buddy.
    public func setSuppressed(_ s: Bool) {
        suppressed = s
        updateVisibility()
    }

    /// Screen frame of the buddy (for anchoring the panel), or nil when hidden.
    public var frame: NSRect? { window.isVisible ? window.frame : nil }

    /// Where the buddy is or would be: anchors the panel even when the buddy is hidden.
    public var anchorFrame: NSRect {
        if let f = frame { return f }
        let cursor = NSEvent.mouseLocation
        let size = window.frame.size
        let bounds = (NSScreen.screens.first { NSMouseInRect(cursor, $0.frame, false) } ?? NSScreen.main)?.frame ?? .zero
        let c = BuddyMotion.anchor(cursor: cursor, size: size, bounds: bounds, gap: motion.gap)
        return NSRect(x: c.x - size.width / 2, y: c.y - size.height / 2, width: size.width, height: size.height)
    }

    /// Play states on the real buddy for previewing, then restore.
    public func preview(states: [AssistantState], step: Duration) {
        previewTask?.cancel()
        let restore = state
        let wasSuppressed = suppressed
        suppressed = false
        summoned = true
        previewTask = Task { @MainActor [weak self] in
            for s in states {
                guard let self, !Task.isCancelled else { return }
                self.set(state: s)
                try? await Task.sleep(for: step)
            }
            guard let self, !Task.isCancelled else { return }
            self.summoned = false
            self.suppressed = wasSuppressed
            self.set(state: restore)
        }
    }

    // MARK: Visibility & motion

    private var shouldShow: Bool {
        guard settings.enabled, !suppressed else { return false }
        return settings.visibility == .always || summoned || state != .idle
    }

    private func updateVisibility() {
        if shouldShow {
            if !window.isVisible {
                motion.reset()
                tick()
                window.orderFrontRegardless()
                if labelModel.text != nil { labelWindow.orderFrontRegardless() }
            }
            startTimer(fast: true)
        } else {
            window.orderOut(nil)
            labelWindow.orderOut(nil)
            timer?.invalidate()
            timer = nil
        }
    }

    private func startTimer(fast: Bool) {
        guard timer == nil || fast != fastTicks else { return }
        timer?.invalidate()
        fastTicks = fast
        let t = Timer(timeInterval: fast ? 1.0 / 60 : 1.0 / 10, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    private func tick() {
        let cursor = NSEvent.mouseLocation
        let now = Date()
        let moved = cursor != lastCursor
        if moved { lastMove = now; lastCursor = cursor }
        // Drop to 10 Hz once the pointer has rested and the buddy has settled.
        if !moved, now.timeIntervalSince(lastMove) > 1, fastTicks { startTimer(fast: false); return }
        if moved, !fastTicks { startTimer(fast: true) }

        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(cursor, $0.frame, false) }) ?? NSScreen.main else { return }
        let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
        if displayID != lastScreen {
            // Jump, don't glide, across displays.
            motion.reset()
            lastScreen = displayID
        }
        let size = window.frame.size
        let center = motion.step(
            cursor: cursor, time: now.timeIntervalSinceReferenceDate, size: size, bounds: screen.frame,
            smoothing: smoothing
        )
        let origin = NSPoint(x: (center.x - size.width / 2).rounded(), y: (center.y - size.height / 2).rounded())
        if window.frame.origin != origin { window.setFrameOrigin(origin) }

        let r = BuddyView.baseOrb * CGFloat(settings.size) / 2 + 6
        let isHovering = hypot(cursor.x - center.x, cursor.y - center.y) <= r
        if isHovering != hovering {
            hovering = isHovering
            view.set(hovering: isHovering)
            updateLabel()
        }
        positionLabel(buddyCenter: center, cursor: cursor, screen: screen.frame)
    }

    private var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    private var smoothing: CGFloat {
        if reduceMotion { return 1 }
        return switch settings.animation {
        case .minimal: 0.55
        case .normal: 0.3
        case .expressive: 0.2
        }
    }

    // MARK: Styling & label

    private func restyle() {
        view.apply(style: Self.style(for: settings, reduceMotion: reduceMotion))
        var f = window.frame
        let side = BuddyView.side(for: CGFloat(settings.size))
        f.origin.x += (f.width - side) / 2
        f.origin.y += (f.height - side) / 2
        f.size = NSSize(width: side, height: side)
        window.setFrame(f, display: true)
        motion.reset()
    }

    private func updateLabel() {
        let text: String? = hovering ? "Open \(settings.label)" : (BuddyTheme.label(for: state) ?? (settings.showLabel ? settings.label : nil))
        labelModel.text = text
        labelModel.accent = Color(nsColor: BuddyTheme.stateColor(state, accent: BuddyTheme.color(for: settings.theme, custom: settings.customColor)))
        if text == nil { labelWindow.orderOut(nil) } else if window.isVisible { labelWindow.orderFrontRegardless() }
    }

    private func positionLabel(buddyCenter: CGPoint, cursor: CGPoint, screen: CGRect) {
        guard labelModel.text != nil, let content = labelWindow.contentView else { return }
        let size = content.fittingSize
        let half = window.frame.width / 2
        let rightSide = buddyCenter.x >= cursor.x
        var x = rightSide ? buddyCenter.x + half - 6 : buddyCenter.x - half + 6 - size.width
        if x + size.width > screen.maxX { x = buddyCenter.x - half + 6 - size.width }
        if x < screen.minX { x = buddyCenter.x + half - 6 }
        let frame = NSRect(x: x.rounded(), y: (buddyCenter.y - size.height / 2).rounded(), width: size.width, height: size.height)
        if labelWindow.frame != frame { labelWindow.setFrame(frame, display: true) }
    }

    private static func style(for s: CursorSettings, reduceMotion: Bool = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion) -> BuddyView.Style {
        let accent = BuddyTheme.color(for: s.theme, custom: s.customColor)
        return BuddyView.Style(
            accent: accent, secondary: BuddyTheme.secondary(for: s.theme, primary: accent),
            scale: CGFloat(s.size), opacity: CGFloat(s.opacity), glow: CGFloat(s.glow),
            animation: s.animation, reduceMotion: reduceMotion
        )
    }

    private static func makePanel(size: NSSize, clickable: Bool) -> NSPanel {
        let p = NSPanel(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = false
        // Above the focus overlay (.popUpMenu) so the buddy stays visible while selecting.
        p.level = NSWindow.Level(rawValue: NSWindow.Level.popUpMenu.rawValue + 1)
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        p.ignoresMouseEvents = !clickable
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        // Hidden from screenshots/recordings so Wisp never captures itself. WISP_BUDDY_CAPTURABLE=1
        // makes it visible to capture for visual verification and demos.
        p.sharingType = ProcessInfo.processInfo.environment["WISP_BUDDY_CAPTURABLE"] == "1" ? .readOnly : .none
        p.animationBehavior = .none
        return p
    }
}

@MainActor
final class BuddyLabelModel: ObservableObject {
    @Published var text: String?
    @Published var accent: Color = .purple
}

/// Small pill beside the buddy naming the current state.
struct BuddyLabel: View {
    @ObservedObject var model: BuddyLabelModel

    var body: some View {
        if let text = model.text {
            HStack(spacing: 5) {
                Circle().fill(model.accent).frame(width: 5, height: 5)
                Text(text).font(.system(size: 11, weight: .medium)).foregroundStyle(.primary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.regularMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(.white.opacity(0.08)))
            .fixedSize()
        }
    }
}
