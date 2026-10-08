import Foundation
import Observation
import WispIPC

/// Native-side view of application state, fed by core events. UI state for React lives in
/// Zustand; this only holds what native surfaces need (menu bar, overlays).
@MainActor
@Observable
final class AppState {
    var engine: CoreProcess.State = .stopped
    var settings: AppSettings?
}
