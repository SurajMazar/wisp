import Carbon.HIToolbox
import WispUtilities

/// A system-wide hot key with a key (e.g. ⌥Space) via Carbon. Needs no privacy permission and
/// reports both press and release, so hold behavior works too.
@MainActor
public final class CarbonHotKey {
    public enum RegistrationError: Error, Equatable {
        /// Another app already registered this exact shortcut.
        case taken
        case failed(OSStatus)
    }

    private var ref: EventHotKeyRef?
    private var handlerRef: EventHandlerRef?
    private let onPress: () -> Void
    private let onRelease: () -> Void
    private static let signature: OSType = 0x5749_5350  // 'WISP'
    private static var nextId: UInt32 = 1

    public init(onPress: @escaping () -> Void, onRelease: @escaping () -> Void) {
        self.onPress = onPress
        self.onRelease = onRelease
    }

    public func register(keyCode: UInt32, modifiers: UInt32) throws(RegistrationError) {
        unregister()
        var types = [
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)),
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased)),
        ]
        let me = Unmanaged.passUnretained(self).toOpaque()
        let install = InstallEventHandler(GetApplicationEventTarget(), { _, event, userData in
            guard let event, let userData else { return OSStatus(eventNotHandledErr) }
            let hk = Unmanaged<CarbonHotKey>.fromOpaque(userData).takeUnretainedValue()
            let kind = GetEventKind(event)
            MainActor.assumeIsolated {
                if kind == UInt32(kEventHotKeyPressed) { hk.onPress() } else { hk.onRelease() }
            }
            return noErr
        }, types.count, &types, me, &handlerRef)
        guard install == noErr else { throw .failed(install) }

        let id = EventHotKeyID(signature: Self.signature, id: Self.nextId)
        Self.nextId += 1
        let status = RegisterEventHotKey(keyCode, modifiers, id, GetApplicationEventTarget(), 0, &ref)
        if status == OSStatus(eventHotKeyExistsErr) { unregister(); throw .taken }
        guard status == noErr else { unregister(); throw .failed(status) }
    }

    public func unregister() {
        if let ref { UnregisterEventHotKey(ref) }
        if let handlerRef { RemoveEventHandler(handlerRef) }
        ref = nil
        handlerRef = nil
    }
}
