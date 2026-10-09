import AVFoundation
import WispIPC

public enum MicrophonePermission {
    public static var status: PermissionState {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: .enabled
        case .notDetermined: .notRequested
        default: .needsAttention
        }
    }

    public static func request() async -> PermissionState {
        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            _ = await AVCaptureDevice.requestAccess(for: .audio)
        }
        return status
    }
}
