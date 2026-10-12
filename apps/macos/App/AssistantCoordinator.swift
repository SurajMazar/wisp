import AppKit
import WispIPC
import WispPlatform
import WispUI

/// Single owner of the assistant's on-screen presence: the buddy beside the pointer, the panel
/// that expands out of it, and the assistant state everyone else observes.
///
/// * Click the buddy, toggle-invoke, or use the menu bar → panel expands from the buddy.
/// * Hold the shortcut → the buddy becomes attentive ("following") without opening anything.
/// * Esc, Close, or clicking elsewhere → panel collapses back into the buddy.
@MainActor
final class AssistantCoordinator {
    private let router: IPCRouter
    private let invocation: InvocationController
    let buddy = BuddyController()
    /// Status capsule hanging from the notch.
    let island = IslandController()
    let guidance = GuidanceOverlayController()
    private var panel: FloatingAssistantPanel?
    private(set) var state: AssistantState = .idle
    /// The panel was opened by a toggle invocation, so ending that invocation closes it.
    private var expandedByInvocation = false

    init(router: IPCRouter, invocation: InvocationController) {
        self.router = router
        self.invocation = invocation
        guidance.onDismiss = { [weak self] _ in self?.buddy.stopPointing() }
        island.onClick = { [weak self] in self?.expand() }
    }

    var isExpanded: Bool { panel?.isExpanded ?? false }

    var snapshot: AssistantStateChanged { AssistantStateChanged(expanded: isExpanded, state: state) }

    func apply(_ settings: CursorSettings) {
        buddy.apply(settings)
        island.set(accent: BuddyTheme.color(for: settings.theme, custom: settings.customColor))
        island.enabled = settings.showIsland
        guidance.tint = BuddyTheme.color(for: settings.theme, custom: settings.customColor)
    }

    // MARK: Guidance

    /// Draw guidance and send the buddy to point at the target (spec §54).
    func showGuidance(_ g: GuidanceShow) {
        guidance.show(g)
        if let t = g.buddyTarget {
            let p = FocusGeometry.appKitPoint(fromCG: CGPoint(x: t.x, y: t.y), primaryHeight: Displays.primaryHeight)
            buddy.point(at: p, for: Double(max(1500, min(g.durationMs, 60_000))) / 1000)
        }
    }

    func clearGuidance() {
        guidance.clear()
        buddy.stopPointing()
    }

    // MARK: Expansion

    func toggleExpanded() {
        if isExpanded { collapse() } else { expand() }
    }

    func expand() {
        guard !isExpanded else { return }
        let anchor = buddy.anchorFrame
        buddy.setSuppressed(true)
        makePanel().expand(from: anchor)
        publish()
    }

    func collapse() {
        guard let panel, panel.isExpanded else { return }
        panel.collapse(to: buddy.anchorFrame)
        expandedByInvocation = false
        buddy.setSuppressed(false)
        publish()
        // An invocation in toggle mode ends when its panel goes away.
        invocation.dismissed()
    }

    // MARK: Invocation

    func invocationStarted(behavior: InvocationBehavior) {
        clearGuidance()
        switch behavior {
        case .hold, .pushToTalk:
            buddy.summon(true)
            set(state: .following)
        case .toggle:
            expandedByInvocation = !isExpanded
            expand()
        }
    }

    func invocationEnded() {
        buddy.summon(false)
        if state == .following { set(state: .idle) }
        if expandedByInvocation { collapse() }
        expandedByInvocation = false
    }

    // MARK: State

    func set(state newState: AssistantState) {
        guard newState != state else { return }
        state = newState
        buddy.set(state: newState)
        island.set(state: newState)
        publish()
    }

    func preview(_ params: CursorPreviewParams) {
        buddy.preview(states: params.states, step: .milliseconds(Int(params.stepMs)))
    }

    private func publish() {
        router.emit(IPCEvents.assistantStateChanged, snapshot)
    }

    private func makePanel() -> FloatingAssistantPanel {
        if let panel { return panel }
        let p = FloatingAssistantPanel(router: router)
        p.onDismiss = { [weak self] in self?.collapse() }
        p.onResignKey = { [weak self] in
            // Auto-hide when the user clicks elsewhere (deferred so focus moves settle first).
            Task { @MainActor [weak self] in
                guard let self, let panel = self.panel, panel.isExpanded, !panel.isKeyWindow else { return }
                self.collapse()
            }
        }
        panel = p
        return p
    }
}
