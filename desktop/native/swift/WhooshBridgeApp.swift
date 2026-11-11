import SwiftUI
import AppKit

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    weak var model: BridgeModel?
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if model?.busy == true { showBusy(); return .terminateCancel }
        if model?.prepareToQuit({ sender.reply(toApplicationShouldTerminate: $0) }) == true { return .terminateLater }
        model?.close(); return .terminateNow
    }
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if model?.busy == true { showBusy(); return false }
        return true
    }
    private func showBusy() {
        let alert = NSAlert()
        alert.messageText = model?.text("Une opération est en cours", "An operation is running") ?? ""
        alert.informativeText = model?.text("Attendez sa fin ou utilisez Arrêter. Un envoi déjà commencé doit se terminer avant de quitter.", "Wait for it to finish or use Stop. An upload already in progress must finish before quitting.") ?? ""
        alert.addButton(withTitle: "OK"); alert.runModal()
    }
}
struct WindowHook: NSViewRepresentable {
    let delegate: AppDelegate
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            view.window?.delegate = delegate
            view.window?.title = "Whoosh Bridge"
            view.window?.toolbarStyle = .unified
        }
    }
}
@main struct WhooshBridgeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model: BridgeModel
    init() {
        #if DEBUG
        let preview = ProcessInfo.processInfo.environment["WHOOSH_UI_PREVIEW"] != nil
        #else
        let preview = false
        #endif
        _model = StateObject(wrappedValue: BridgeModel(preview: preview))
    }
    var body: some Scene {
        Window("Whoosh Bridge", id: "main") {
            RootView(model: model)
                .environment(\.locale, Locale(identifier: model.english ? "en" : "fr"))
                .background(WindowHook(delegate: delegate))
                .onAppear { delegate.model = model }
                .frame(minWidth: 960, minHeight: 640)
        }
        .defaultSize(width: 1220, height: 800)
        .windowResizability(.contentMinSize)
        .commands {
            CommandGroup(replacing: .appInfo) { Button(model.text("À propos de Whoosh Bridge", "About Whoosh Bridge")) { model.destination = .about } }
            CommandGroup(after: .appInfo) {
                Button(model.text("Rechercher des mises à jour…", "Check for Updates…")) { model.updater.check() }
                    .disabled(model.busy || !model.updater.available)
            }
            CommandGroup(replacing: .appSettings) {
                Button(model.text("Réglages…", "Settings…")) { model.destination = .settings }.keyboardShortcut(",")
            }
            CommandGroup(after: .newItem) {
                Button(model.text("Actualiser les sorties", "Refresh rides")) { model.refresh() }.keyboardShortcut("r").disabled(model.busy || !model.whooshReady)
                Button(model.text("Transférer la sélection…", "Transfer selection…")) { model.modal = .confirm }.keyboardShortcut(.return, modifiers: .command).disabled(!model.canTransfer)
            }
        }
    }
}
