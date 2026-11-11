import Foundation
import Darwin

@MainActor final class NativeUpdater {
    typealias Query = @convention(c) () -> Bool
    typealias Action = @convention(c) () -> Void
    typealias Setting = @convention(c) (Bool) -> Void
    private var handle: UnsafeMutableRawPointer?
    init(enabled: Bool) {
        guard enabled, let frameworks = Bundle.main.privateFrameworksURL else { return }
        handle = dlopen(frameworks.appendingPathComponent("libWhooshUpdater.dylib").path, RTLD_NOW | RTLD_LOCAL)
        if !query("wb_updater_start") { handle = nil }
    }
    var available: Bool { handle != nil }
    private func query(_ name: String) -> Bool {
        guard let handle, let symbol = dlsym(handle, name) else { return false }
        return unsafeBitCast(symbol, to: Query.self)()
    }
    private func set(_ name: String, _ value: Bool) {
        guard let handle, let symbol = dlsym(handle, name) else { return }
        unsafeBitCast(symbol, to: Setting.self)(value)
    }
    var installing: Bool { query("wb_updater_installing") }
    var automatic: Bool { query("wb_updater_automatic") }
    var canCheck: Bool { query("wb_updater_can_check") }
    func setBusy(_ busy: Bool) { set("wb_updater_set_busy", busy) }
    func setAutomatic(_ enabled: Bool) { set("wb_updater_set_automatic", enabled) }
    func check() {
        guard canCheck, let handle, let symbol = dlsym(handle, "wb_updater_check") else { return }
        unsafeBitCast(symbol, to: Action.self)()
    }
}
