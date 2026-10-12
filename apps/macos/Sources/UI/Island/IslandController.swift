import AppKit
import SwiftUI
import WispIPC

/// A Dynamic-Island-style status capsule that hangs from the notch (or the top edge of a display
/// without one). It shows what Wisp is doing — listening, drawing, thinking, speaking (with a live
/// caption) — and folds back into the notch when idle. Clicking it opens the panel.
///
/// The window is transparent outside the capsule, so clicks there reach the menu bar as usual;
/// it never becomes key and is excluded from screen capture.
@MainActor
public final class IslandController {
    public var onClick: (() -> Void)?
    let model = IslandModel()
    private let panel: NSPanel
    private static let canvas = NSSize(width: 640, height: 180)

    public init() {
        panel = NSPanel(contentRect: NSRect(origin: .zero, size: Self.canvas),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.sharingType = .none
        panel.animationBehavior = .none
        let host = NSHostingView(rootView: IslandView(model: model, onClick: { [weak self] in self?.onClick?() }))
        host.frame = NSRect(origin: .zero, size: Self.canvas)
        panel.contentView = host
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.place() }
        }
    }

    public var enabled = true {
        didSet { if !enabled { panel.orderOut(nil) } }
    }

    public func set(state: AssistantState) {
        let wasIdle = !model.isActive
        if [.listening, .drawing].contains(state) { model.caption = "" }
        model.state = state
        guard enabled else { return }
        if model.isActive, wasIdle { place() }
        if model.isActive { panel.orderFrontRegardless() }
    }

    public func set(level: Double) { model.level = level }

    public func set(accent: NSColor) { model.accent = Color(nsColor: accent) }

    /// Answer text as it streams (shown as a caption while thinking/speaking).
    public func appendCaption(_ text: String) {
        model.caption += text
        if model.caption.count > 600 { model.caption = String(model.caption.suffix(400)) }
    }

    public func resetCaption() { model.caption = "" }

    /// Hang from the top of the display the pointer is on, measuring its notch.
    private func place() {
        let mouse = NSEvent.mouseLocation
        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main else { return }
        let f = screen.frame
        if screen.safeAreaInsets.top > 0,
           let left = screen.auxiliaryTopLeftArea, let right = screen.auxiliaryTopRightArea {
            model.notch = CGSize(width: f.width - left.width - right.width, height: screen.safeAreaInsets.top)
        } else {
            model.notch = CGSize(width: 0, height: max(24, f.maxY - screen.visibleFrame.maxY))
        }
        panel.setFrame(NSRect(x: f.midX - Self.canvas.width / 2, y: f.maxY - Self.canvas.height,
                              width: Self.canvas.width, height: Self.canvas.height), display: true)
    }
}

@MainActor @Observable
final class IslandModel {
    var state: AssistantState = .idle
    var level: Double = 0
    var caption = ""
    var accent = Color(red: 0.55, green: 0.49, blue: 0.96)
    /// Notch size in points (width 0: no notch, height = menu bar).
    var notch = CGSize(width: 0, height: 32)

    var isActive: Bool { ![.idle, .following].contains(state) }

    var label: String {
        switch state {
        case .listening: "Listening"
        case .dictating: "Dictating"
        case .selecting: "Draw to point"
        case .drawing: "Release to ask"
        case .analyzing: "Looking"
        case .thinking: "Thinking"
        case .speaking: "Speaking"
        case .executing: "Working"
        case .waitingForPermission: "Needs permission"
        case .success: "Got it"
        case .error: "Something went wrong"
        case .idle, .following: ""
        }
    }
}

struct IslandView: View {
    let model: IslandModel
    let onClick: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var showCaption: Bool {
        [.thinking, .speaking].contains(model.state) && !model.caption.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var size: CGSize {
        let n = model.notch
        guard model.isActive else { return CGSize(width: max(n.width, 120), height: n.height) }
        let width = max(n.width + 150, showCaption ? 420 : 300)
        return CGSize(width: width, height: n.height + (showCaption ? 58 : 34))
    }

    var body: some View {
        VStack(spacing: 0) {
            ZStack(alignment: .top) {
                IslandShape(bottomRadius: model.isActive ? 22 : 10)
                    .fill(.black)
                    .shadow(color: .black.opacity(model.isActive ? 0.35 : 0), radius: 12, y: 4)
                if model.isActive { content.transition(.opacity.combined(with: .scale(scale: 0.92, anchor: .top))) }
            }
            .frame(width: size.width, height: size.height)
            .opacity(model.isActive || model.notch.width > 0 ? 1 : 0)
            .contentShape(IslandShape(bottomRadius: 22))
            .onTapGesture(perform: onClick)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Wisp: \(model.label)")
            .accessibilityAddTraits(.isButton)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .animation(reduceMotion ? .easeOut(duration: 0.12) : .spring(response: 0.42, dampingFraction: 0.78), value: model.state)
        .animation(.easeOut(duration: 0.2), value: showCaption)
    }

    @ViewBuilder private var content: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Beside the notch: who's talking (left) and what's happening (right).
            HStack {
                Circle()
                    .fill(model.state == .error ? Color.red : model.accent)
                    .frame(width: 9, height: 9)
                    .shadow(color: model.accent.opacity(0.9), radius: 5)
                    .modifier(Pulse(active: !reduceMotion && model.state != .error))
                Spacer()
                trailing
            }
            .frame(height: max(model.notch.height - 8, 16))
            .padding(.horizontal, 16)
            .padding(.top, 4)

            if showCaption {
                Text(model.caption)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(.white.opacity(0.92))
                    .lineLimit(2)
                    .truncationMode(.head)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 18)
            } else {
                Text(model.label)
                    .font(.system(size: 12.5, weight: .semibold))
                    .foregroundStyle(model.state == .error ? Color(red: 1, green: 0.45, blue: 0.45) : .white.opacity(0.9))
                    .modifier(Shimmer(active: model.state == .thinking && !reduceMotion))
                    .frame(maxWidth: .infinity)
            }
        }
    }

    @ViewBuilder private var trailing: some View {
        switch model.state {
        case .listening, .dictating:
            Waveform(level: model.level, color: model.accent, animate: !reduceMotion)
        case .thinking, .analyzing, .executing:
            ProgressView().controlSize(.mini).tint(.white)
        case .speaking:
            Waveform(level: 0.45, color: .white.opacity(0.85), animate: !reduceMotion)
        case .drawing, .selecting:
            Image(systemName: "scribble.variable").font(.system(size: 12, weight: .semibold)).foregroundStyle(model.accent)
        case .success:
            Image(systemName: "checkmark").font(.system(size: 12, weight: .bold)).foregroundStyle(.green)
        case .error, .waitingForPermission:
            Image(systemName: "exclamationmark.circle.fill").font(.system(size: 13)).foregroundStyle(.red)
        default:
            EmptyView()
        }
    }
}

/// Flush with the top edge, concave "ears" into the menu bar, rounded bottom corners.
struct IslandShape: Shape {
    var bottomRadius: CGFloat
    var animatableData: CGFloat {
        get { bottomRadius }
        set { bottomRadius = newValue }
    }

    func path(in r: CGRect) -> Path {
        let ear: CGFloat = 8
        let br = min(bottomRadius, r.height / 2, (r.width - 2 * ear) / 2)
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.minY))
        p.addQuadCurve(to: CGPoint(x: r.minX + ear, y: r.minY + ear), control: CGPoint(x: r.minX + ear, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + ear, y: r.maxY - br))
        p.addQuadCurve(to: CGPoint(x: r.minX + ear + br, y: r.maxY), control: CGPoint(x: r.minX + ear, y: r.maxY))
        p.addLine(to: CGPoint(x: r.maxX - ear - br, y: r.maxY))
        p.addQuadCurve(to: CGPoint(x: r.maxX - ear, y: r.maxY - br), control: CGPoint(x: r.maxX - ear, y: r.maxY))
        p.addLine(to: CGPoint(x: r.maxX - ear, y: r.minY + ear))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: r.minY), control: CGPoint(x: r.maxX - ear, y: r.minY))
        p.closeSubpath()
        return p
    }
}

/// Five bars that follow the microphone level.
struct Waveform: View {
    var level: Double
    var color: Color
    var animate: Bool

    var body: some View {
        TimelineView(.animation(paused: !animate)) { t in
            let time = t.date.timeIntervalSinceReferenceDate
            HStack(spacing: 2.5) {
                ForEach(0..<5, id: \.self) { i in
                    let wobble = animate ? (sin(time * 9 + Double(i) * 1.3) + 1) / 2 : 0.5
                    let h = 3 + CGFloat(min(1, level * 1.4) * (0.45 + 0.55 * wobble)) * 13
                    Capsule().fill(color).frame(width: 3, height: h)
                }
            }
            .frame(height: 16)
        }
    }
}

private struct Pulse: ViewModifier {
    var active: Bool
    @State private var on = false

    func body(content: Content) -> some View {
        content
            .scaleEffect(active && on ? 1.25 : 1)
            .opacity(active && on ? 0.7 : 1)
            .onAppear { withAnimation(.easeInOut(duration: 0.8).repeatForever()) { on = true } }
    }
}

private struct Shimmer: ViewModifier {
    var active: Bool

    func body(content: Content) -> some View {
        if active {
            TimelineView(.animation) { t in
                let x = (t.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1.4)) / 1.4
                content.overlay(
                    LinearGradient(colors: [.clear, .white.opacity(0.9), .clear], startPoint: .leading, endPoint: .trailing)
                        .frame(width: 40)
                        .offset(x: CGFloat(x) * 160 - 80)
                        .blendMode(.sourceAtop)
                )
                .compositingGroup()
            }
        } else {
            content
        }
    }
}
