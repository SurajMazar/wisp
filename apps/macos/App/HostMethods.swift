import AppKit
import WispIPC
import WispPlatform
import WispUI

/// Swift implementations of `host.*` IPC methods.
@MainActor
enum HostMethods {
    static func register(on router: IPCRouter, deps: DependencyContainer, assistant: AssistantCoordinator, focus: FocusCoordinator, voice: VoiceCoordinator) {
        router.register(IPCMethods.hostInvocationStatus) { _ in
            deps.invocation.refresh()
            return deps.invocation.status
        }

        router.register(IPCMethods.hostShortcutCheck) { params in
            ShortcutCheckResult(conflicts: ShortcutConflicts.check(params.shortcut))
        }

        router.register(IPCMethods.hostShortcutRecord) { params in
            guard (1_000...30_000).contains(params.timeoutMs) else {
                throw IpcError.invalidRequest("timeoutMs must be 1000–30000")
            }
            deps.invocation.suspend()
            defer { deps.invocation.resume() }
            switch await deps.recorder.record(timeout: .milliseconds(Int(params.timeoutMs))) {
            case .recorded(let shortcut):
                return ShortcutRecordResult(conflicts: ShortcutConflicts.check(shortcut), shortcut: shortcut)
            case .cancelled:
                throw IpcError(code: .cancelled, details: nil, message: "Recording cancelled.")
            case .timedOut:
                throw IpcError(code: .timeout, details: nil, message: "No shortcut was pressed.")
            }
        }

        router.register(IPCMethods.hostSystemOpenSettingsPane) { params in
            SystemSettings.open(params.pane)
            return Empty()
        }

        router.register(IPCMethods.hostAssistantSetVisible) { params in
            if params.visible { assistant.expand() } else { assistant.collapse() }
            return Empty()
        }

        router.register(IPCMethods.hostAssistantState) { _ in
            assistant.snapshot
        }

        router.register(IPCMethods.hostFocusStart) { params in
            // Let the panel's click finish and the panel step aside before the overlay appears.
            assistant.collapse()
            try? await Task.sleep(for: .milliseconds(180))
            focus.start(kind: params.kind)
            return Empty()
        }

        router.register(IPCMethods.hostFocusCancel) { _ in
            focus.cancel()
            return Empty()
        }

        router.register(IPCMethods.hostFocusClear) { _ in
            focus.clear()
            return Empty()
        }

        router.register(IPCMethods.hostScreenPermission) { _ in
            ScreenPermissionStatus(screenRecording: ScreenPermission.status)
        }

        router.register(IPCMethods.hostScreenRequestPermission) { _ in
            ScreenPermissionStatus(screenRecording: ScreenPermission.request())
        }

        router.register(IPCMethods.hostAxStatus) { _ in
            AccessibilityStatus(state: AccessibilityReader.status)
        }

        router.register(IPCMethods.hostAxRequestPermission) { _ in
            let state = AccessibilityReader.requestPermission()
            if state != .enabled { SystemSettings.open(.accessibility) }
            deps.invocation.refresh()
            return AccessibilityStatus(state: state)
        }

        router.register(IPCMethods.hostAxSnapshot) { params in
            guard (1...600).contains(params.maxNodes) else { throw IpcError.invalidRequest("maxNodes must be 1–600") }
            guard AccessibilityReader.isTrusted else {
                throw IpcError(code: .permissionDenied, details: nil, message: "Wisp needs Accessibility permission to read app controls.")
            }
            let point = params.point?.cg ?? FocusGeometry.cgPoint(fromAppKit: NSEvent.mouseLocation, primaryHeight: Displays.primaryHeight)
            return await AccessibilityReader.snapshot(
                point: point, region: params.region?.cg,
                limits: .init(maxNodes: Int(params.maxNodes)),
                excludingPID: ProcessInfo.processInfo.processIdentifier
            )
        }

        router.register(IPCMethods.hostMicPermission) { _ in
            MicrophoneStatus(state: MicrophonePermission.status)
        }

        router.register(IPCMethods.hostMicRequestPermission) { _ in
            let state = await MicrophonePermission.request()
            if state != .enabled { SystemSettings.open(.microphone) }
            return MicrophoneStatus(state: state)
        }

        router.register(IPCMethods.hostAppleStatus) { _ in
            deps.apple.status
        }

        router.register(IPCMethods.hostAppleGenerate) { params in
            let text = try await deps.apple.generate(params) { delta in
                router.sendToCore(IPCEvents.appleDelta, AppleDelta(delta: delta, requestId: params.requestId))
            }
            return AppleGenerateResult(text: text)
        }

        router.register(IPCMethods.hostAppleCancel) { params in
            deps.apple.cancel(params.requestId)
            return Empty()
        }

        router.register(IPCMethods.hostKeychainSet) { params in
            try Keychain.set(params.secret, account: params.account)
            return Empty()
        }

        router.register(IPCMethods.hostKeychainDelete) { params in
            try Keychain.delete(account: params.account)
            return Empty()
        }

        router.register(IPCMethods.hostKeychainGet) { params in
            KeychainSecret(secret: try Keychain.get(account: params.account))
        }

        router.register(IPCMethods.hostOllamaStart) { _ in
            try OllamaLauncher.start()
            return Empty()
        }

        router.register(IPCMethods.hostVoiceStart) { params in
            try voice.start(params.purpose)
            return Empty()
        }

        router.register(IPCMethods.hostVoiceStop) { _ in
            try await voice.stop()
        }

        router.register(IPCMethods.hostVoiceStopSpeaking) { _ in
            voice.interrupt()
            return Empty()
        }

        router.register(IPCMethods.hostCursorPreview) { params in
            guard (300...5_000).contains(params.stepMs), (1...12).contains(params.states.count) else {
                throw IpcError.invalidRequest("stepMs must be 300–5000 and 1–12 states")
            }
            assistant.preview(params)
            return Empty()
        }
    }
}
