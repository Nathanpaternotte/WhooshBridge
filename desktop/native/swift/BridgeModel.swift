import SwiftUI
import AppKit

struct Ride: Codable, Identifiable, Equatable { let id: String; let name: String; let date: String }
struct Edge: Codable, Identifiable { let id: Int; let name: String; let firmware: Int }
struct Preferences: Codable {
    var telemetry_enabled = false
    var telemetry_decision: Bool? = nil
    var selected = 4440
    var language: Bool? = nil
    var units: [String: UInt32] = [:]
    var consent: Bool? { telemetry_decision ?? (telemetry_enabled ? true : nil) }
}
struct Receipt: Identifiable {
    let id: String
    let name: String
    let message: String
    let outcome: String
    var confirmed: Bool { outcome == "imported" || outcome == "duplicate" }
    var symbol: String { confirmed ? "checkmark.circle.fill" : "exclamationmark.circle.fill" }
}
enum Destination: String, CaseIterable { case rides, transfers, accounts, settings, about }
enum Modal: String, Identifiable { case mfa, consent, confirm; var id: String { rawValue } }

// Process transport is private, never a socket, file or command-line credential.
final class EngineTransport: @unchecked Sendable {
    private let process = Process()
    private let input = Pipe()
    private let output = Pipe()
    private let writes = DispatchQueue(label: "com.mywhooshbridge.commands")
    func start(receive: @escaping @Sendable (Data) -> Void, stopped: @escaping @Sendable () -> Void) throws {
        process.executableURL = Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("whoosh-engine")
        process.standardInput = input
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice
        try process.run()
        let handle = output.fileHandleForReading
        DispatchQueue.global(qos: .userInitiated).async {
            var pending = Data()
            while true {
                let data = handle.availableData
                if data.isEmpty { break }
                pending.append(data)
                while let end = pending.firstIndex(of: 10) {
                    receive(Data(pending[..<end]))
                    pending.removeSubrange(...end)
                }
                if pending.count > 8_000_000 { break }
            }
            stopped()
        }
    }
    func send(_ object: [String: Any], failed: @escaping @Sendable () -> Void) {
        guard let data = try? JSONSerialization.data(withJSONObject: object) else { failed(); return }
        writes.async { [self] in
            do { try input.fileHandleForWriting.write(contentsOf: data + Data([10])) }
            catch { failed() }
        }
    }
    func close() { writes.async { [self] in try? input.fileHandleForWriting.close() } }
}

@MainActor final class BridgeModel: ObservableObject {
    @Published var showUnitHelp = false
    @Published var showDisconnect = false
    @Published var destination: Destination = .rides
    @Published var english = !(Locale.preferredLanguages.first ?? "en").hasPrefix("fr")
    @Published var rides: [Ride] = []
    @Published var devices: [Edge] = []
    @Published var selection: Set<String> = []
    @Published var receipts: [Receipt] = []
    @Published var preferences = Preferences()
    @Published var preferencesReady = false
    @Published var device = 4440
    @Published var unit = ""
    @Published var whooshReady = false
    @Published var garminReady = false
    @Published var busy = false
    @Published var connected = false
    @Published var progress = 0.0
    @Published var status = ""
    @Published var error: String?
    @Published var modal: Modal?
    @Published var search = ""
    @Published var whooshEmail = ""
    @Published var whooshPassword = ""
    @Published var garminEmail = ""
    @Published var garminPassword = ""
    @Published var remember = false
    @Published var mfaCode = ""
    @Published var page = 1
    @Published var hasMore = false
    @Published var version = ""
    @Published var automaticUpdates = false
    let preview: Bool
    let updater: NativeUpdater
    private var transport: EngineTransport?
    private var preferenceSave = false
    private var preferenceSaveFailed = false
    private var consentSave = false
    private var pendingProduct: Int?
    private var pendingTransfer = false
    private var quitCompletion: ((Bool) -> Void)?
    private var delayedSave: Task<Void, Never>?
    var validUnit: UInt32? { guard let n = UInt32(unit.trimmingCharacters(in: .whitespaces)), n > 0 else { return nil }; return n }
    var selectedDevice: Edge? { devices.first { $0.id == device } }
    var visibleRides: [Ride] { rides.filter { search.isEmpty || $0.name.localizedCaseInsensitiveContains(search) || $0.date.localizedCaseInsensitiveContains(search) } }
    var canTransfer: Bool { !busy && connected && whooshReady && garminReady && preferencesReady && validUnit != nil && !selection.isEmpty }
    func text(_ fr: String, _ en: String) -> String { english ? en : fr }
    init(preview: Bool = false) {
        self.preview = preview
        updater = NativeUpdater(enabled: !preview)
        automaticUpdates = updater.automatic
        if preview { seedPreview(); return }
        let transport = EngineTransport()
        self.transport = transport
        do {
            try transport.start(receive: { [weak self] data in DispatchQueue.main.async { self?.receive(data) } }, stopped: { [weak self] in DispatchQueue.main.async { self?.engineStopped() } })
        } catch { self.error = text("Le moteur est introuvable. Reconstruisez l’application.", "The engine is missing. Rebuild the application.") }
    }
    func send(_ command: [String: Any], operation: Bool = true) {
        if preview {
            if command["command"] as? String == "preferences", let object = command["value"], let data = try? JSONSerialization.data(withJSONObject: ["event":"preferences", "value":object]) {
                receive(data)
                receive(Data(#"{"event":"done","cancelled":false}"#.utf8))
            }
            return
        }
        guard connected, !operation || (!busy && !updater.installing) else { return }
        if operation { busy = true; progress = 0; updater.setBusy(true) }
        transport?.send(command) { [weak self] in DispatchQueue.main.async { self?.engineStopped() } }
    }
    private func engineStopped() {
        connected = false; busy = false; modal = nil; updater.setBusy(false)
        error = text("Le moteur s’est arrêté. Si un transfert était en cours, vérifiez Garmin Connect avant de réessayer. Relancez l’application.", "The engine stopped. If a transfer was running, check Garmin Connect before retrying. Restart the application.")
    }
    func receive(_ data: Data) {
        guard let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let event = value["event"] as? String else { engineStopped(); return }
        func decode<T: Decodable>(_ key: String, as type: T.Type) -> T? {
            guard let object = value[key], let data = try? JSONSerialization.data(withJSONObject: object) else { return nil }
            return try? JSONDecoder().decode(type, from: data)
        }
        switch event {
        case "hello":
            guard value["protocol"] as? Int == 1 else { engineStopped(); return }
            devices = decode("devices", as: [Edge].self) ?? []
            version = value["version"] as? String ?? ""
            connected = true
            send(["command":"language", "english":english], operation: false)
            status = text("Restauration des sessions…", "Restoring sessions…")
            send(["command":"restore", "initial":true])
        case "preferences":
            if let prefs = decode("value", as: Preferences.self) {
                preferences = prefs; preferencesReady = true
                if let language = prefs.language { english = language }
                if !preferenceSave { device = prefs.selected; unit = prefs.units[String(device)].map(String.init) ?? "" }
                if consentSave { modal = nil; consentSave = false }
            }
        case "credentials":
            whooshEmail = value["email"] as? String ?? ""; whooshPassword = value["password"] as? String ?? ""; remember = true
        case "auth_required":
            if value["service"] as? String == "garmin" { garminReady = false } else { whooshReady = false }
        case "activities":
            let incoming = decode("activities", as: [Ride].self) ?? []
            page = value["page"] as? Int ?? 1; hasMore = !incoming.isEmpty
            if page == 1 { rides = []; selection = [] }
            for ride in incoming where !rides.contains(where: { $0.id == ride.id }) { rides.append(ride) }
            whooshReady = true
            status = text("Bibliothèque actualisée", "Library updated")
        case "garmin_ready": garminReady = true
        case "mfa": mfaCode = ""; modal = .mfa
        case "progress": status = value["message"] as? String ?? ""; progress = value["fraction"] as? Double ?? 0
        case "result":
            if let id = value["id"] as? String, let outcome = value["outcome"] as? String {
                receipts.removeAll { $0.id == id }
                receipts.insert(Receipt(id: id, name: rides.first { $0.id == id }?.name ?? id, message: value["message"] as? String ?? "", outcome: outcome), at: 0)
                // Every receipt is removed from this batch, including ambiguous outcomes.
                selection.remove(id)
            }
        case "error": error = value["message"] as? String; if preferenceSave { preferenceSaveFailed = true }
        case "done":
            busy = false; updater.setBusy(false)
            if modal == .mfa { modal = nil }; mfaCode = ""
            if value["cancelled"] as? Bool == true { status = text("Opération interrompue. Vérifiez les résultats des transferts.", "Operation stopped. Review the transfer results.") }
            if preferenceSave {
                preferenceSave = false
                if let finish = quitCompletion {
                    quitCompletion = nil
                    if !preferenceSaveFailed { close() }
                    finish(!preferenceSaveFailed)
                    return
                }
                if preferenceSaveFailed { pendingProduct = nil; pendingTransfer = false; consentSave = false }
                else if let next = pendingProduct { pendingProduct = nil; device = next; unit = preferences.units[String(next)].map(String.init) ?? ""; savePreferences(); return }
                else if pendingTransfer { pendingTransfer = false; startTransfer(); return }
            }
            if preferencesReady && preferences.consent == nil && modal == nil { modal = .consent }
        default: break
        }
    }
    func restore() { status = text("Restauration des sessions…", "Restoring sessions…"); send(["command":"restore", "initial":!preferencesReady]) }
    func refresh() { status = text("Actualisation…", "Refreshing…"); send(["command":"page", "page":1]) }
    func loadMore() { send(["command":"page", "page":page + 1]) }
    func login(whoosh: Bool) {
        status = text("Connexion en cours…", "Connecting…")
        if whoosh { send(["command":"whoosh_login", "email":whooshEmail.trimmingCharacters(in: .whitespaces), "password":whooshPassword, "remember":remember]); whooshPassword = "" }
        else { send(["command":"garmin_login", "email":garminEmail.trimmingCharacters(in: .whitespaces), "password":garminPassword]); garminPassword = "" }
    }
    func submitMFA() { send(["command":"mfa", "code":mfaCode.trimmingCharacters(in: .whitespacesAndNewlines)], operation:false); mfaCode = ""; modal = nil }
    func cancel() { send(["command":"cancel"], operation:false); status = text("Arrêt après l’opération en cours…", "Stopping after the current operation…"); if modal == .mfa { modal = nil } }
    func disconnect() { whooshPassword = ""; garminPassword = ""; send(["command":"disconnect"]) }
    func chooseDevice(_ next: Int) { guard next != device else { return }; pendingProduct = next; savePreferences() }
    func scheduleSave() { delayedSave?.cancel(); delayedSave = Task { try? await Task.sleep(for: .milliseconds(650)); if !Task.isCancelled && !busy { savePreferences() } } }
    func savePreferences(consent: Bool? = nil) {
        guard preferencesReady, !busy else { return }
        delayedSave?.cancel()
        var next = preferences
        next.selected = device; next.language = english
        if let unit = validUnit { next.units[String(device)] = unit }
        else if unit.isEmpty { next.units.removeValue(forKey: String(device)) }
        if let consent { next.telemetry_decision = consent; next.telemetry_enabled = consent; consentSave = true }
        guard let data = try? JSONEncoder().encode(next), let object = try? JSONSerialization.jsonObject(with: data) else { return }
        preferenceSave = true; preferenceSaveFailed = false
        send(["command":"preferences", "value":object])
    }
    func confirmTransfer() { guard canTransfer else { return }; modal = nil; pendingTransfer = true; savePreferences() }
    private func startTransfer() {
        guard canTransfer, let unit = validUnit else { return }
        status = text("Préparation du transfert…", "Preparing transfer…")
        send(["command":"sync", "ids":rides.filter { selection.contains($0.id) }.map(\.id), "product":device,"unit":unit])
    }
    func toggle(_ id: String) { if selection.contains(id) { selection.remove(id) } else { selection.insert(id) } }
    func receipt(_ id: String) -> Receipt? { receipts.first { $0.id == id } }
    func prepareToQuit(_ finish: @escaping (Bool) -> Void) -> Bool {
        guard !preview, preferencesReady, connected, !busy else { return false }
        quitCompletion = finish
        savePreferences()
        return true
    }
    func close() { delayedSave?.cancel(); transport?.close() }
    private func seedPreview() {
        connected = true; preferencesReady = true; whooshReady = true; garminReady = true
        preferences.telemetry_decision = false; unit = "123456789"; version = "SwiftUI preview"
        devices = [Edge(id:4440,name:"Edge 1050",firmware:3133), Edge(id:4962,name:"Edge 1060",firmware:819), Edge(id:4062,name:"Edge 840",firmware:3133)]
        rides = ["FTP Builder · Sweet Spot 45", "AlUla · The long way home", "Endurance · Coastal escape", "Tempo Builder · Into the blue", "Recovery · Easy morning", "Climbing · Mountain pass"].enumerated().map { Ride(id:String($0.offset),name:$0.element,date:"\(11 - $0.offset)/11/2026 · 18:30") }
        selection = ["0", "1"]
        status = text("Vos comptes sont connectés", "Your accounts are connected")
        #if DEBUG
        english = ProcessInfo.processInfo.environment["WHOOSH_UI_LANGUAGE"] == "en"
        switch ProcessInfo.processInfo.environment["WHOOSH_UI_PREVIEW"] {
        case "empty": rides = []; selection = []; whooshReady = false; garminReady = false
        case "mfa": modal = .mfa; busy = true
        case "consent": preferences.telemetry_decision = nil; modal = .consent
        case "confirm": modal = .confirm
        case "loading": busy = true; progress = 0.45; status = text("Envoi vers Garmin : Sweet Spot 45", "Uploading to Garmin: Sweet Spot 45")
        case "error": error = text("Envoi non confirmé. Vérifiez Garmin Connect avant de réessayer.", "Upload not confirmed. Check Garmin Connect before retrying.")
        default: break
        }
        #endif
    }
}
