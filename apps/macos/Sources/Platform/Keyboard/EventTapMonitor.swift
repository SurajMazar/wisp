import AppKit
import CoreGraphics
import WispUtilities

/// Listen-only global event tap for modifier changes, key downs and auxiliary mouse buttons.
///
/// Requires Accessibility or Input Monitoring permission; `start()` returns false without it.
/// Runs on the main run loop. Re-enables itself if macOS disables the tap for timeout.
@MainActor
public final class EventTapMonitor {
    public enum Event {
        case flagsChanged(CGEventFlags)
        case keyDown(keyCode: Int64, isRepeat: Bool)
        case otherMouseDown(button: Int64)
        case otherMouseUp(button: Int64)
    }

    private var tap: CFMachPort?
    private var source: CFRunLoopSource?
    private let handler: (Event) -> Void

    public init(handler: @escaping (Event) -> Void) {
        self.handler = handler
    }

    public var isRunning: Bool { tap.map { CGEvent.tapIsEnabled(tap: $0) } ?? false }

    public static var isPermitted: Bool {
        AXIsProcessTrusted() || CGPreflightListenEventAccess()
    }

    @discardableResult
    public func start() -> Bool {
        guard tap == nil else { return true }
        let mask: CGEventMask =
            (1 << CGEventType.flagsChanged.rawValue) | (1 << CGEventType.keyDown.rawValue)
            | (1 << CGEventType.otherMouseDown.rawValue) | (1 << CGEventType.otherMouseUp.rawValue)
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: mask,
            callback: { _, type, event, refcon in
                guard let refcon else { return Unmanaged.passUnretained(event) }
                let monitor = Unmanaged<EventTapMonitor>.fromOpaque(refcon).takeUnretainedValue()
                MainActor.assumeIsolated { monitor.handle(type: type, event: event) }
                return Unmanaged.passUnretained(event)
            },
            userInfo: refcon
        ) else {
            Log.app.info("event tap unavailable (permission not granted)")
            return false
        }
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        self.tap = tap
        self.source = source
        return true
    }

    public func stop() {
        if let tap { CGEvent.tapEnable(tap: tap, enable: false); CFMachPortInvalidate(tap) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil
        source = nil
    }

    private func handle(type: CGEventType, event: CGEvent) {
        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
        case .flagsChanged:
            handler(.flagsChanged(event.flags))
        case .keyDown:
            handler(.keyDown(
                keyCode: event.getIntegerValueField(.keyboardEventKeycode),
                isRepeat: event.getIntegerValueField(.keyboardEventAutorepeat) != 0
            ))
        case .otherMouseDown:
            handler(.otherMouseDown(button: event.getIntegerValueField(.mouseEventButtonNumber)))
        case .otherMouseUp:
            handler(.otherMouseUp(button: event.getIntegerValueField(.mouseEventButtonNumber)))
        default:
            break
        }
    }
}
