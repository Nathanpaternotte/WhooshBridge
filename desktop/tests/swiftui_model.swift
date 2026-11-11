import Foundation

@main struct ModelTests {
    @MainActor static func main() {
        let model = BridgeModel(preview:true)
        func event(_ value: [String: Any]) { model.receive(try! JSONSerialization.data(withJSONObject:value)) }
        model.english = false
        model.unit = "0"; assert(model.validUnit == nil)
        model.unit = "4294967296"; assert(model.validUnit == nil)
        model.unit = "4294967295"; assert(model.validUnit == UInt32.max)
        model.unit = "123456789"
        model.savePreferences(consent:false)
        assert(model.preferences.consent == false)
        model.chooseDevice(4962)
        assert(model.device == 4962 && model.unit == "")
        model.unit = "42"; model.savePreferences()
        model.chooseDevice(4440)
        assert(model.unit == "123456789")
        model.chooseDevice(4962)
        assert(model.unit == "42")
        model.unit = "invalid"; model.savePreferences()
        assert(model.unit == "invalid" && model.validUnit == nil)
        model.unit = "42"
        for outcome in ["imported", "duplicate", "received", "failed"] {
            model.selection = ["0"]
            event(["event":"result", "id":"0", "message":"Synthetic result", "outcome":outcome])
            assert(!model.selection.contains("0"))
            assert(model.receipts.count == 1)
            assert(model.receipts[0].confirmed == (outcome == "imported" || outcome == "duplicate"))
        }
        event(["event":"activities", "page":1, "activities":[["id":"new", "name":"Synthetic ride", "date":"2026-01-01"]]])
        assert(model.rides.count == 1 && model.receipts.count == 1)
        event(["event":"activities", "page":2, "activities":[["id":"new", "name":"Synthetic ride", "date":"2026-01-01"]]])
        assert(model.rides.count == 1)
        event(["event":"error", "message":"Synthetic network error"])
        assert(model.whooshReady && model.garminReady)
        event(["event":"mfa"]); assert(model.modal == .mfa)
        event(["event":"done", "cancelled":true]); assert(model.modal == nil && !model.busy)
        event(["event":"auth_required", "service":"garmin"])
        assert(!model.garminReady && model.whooshReady)
        model.busy = true
        event(["event":"preferences", "value":["selected":4440,"units":[:],"telemetry_enabled":false]])
        event(["event":"done", "cancelled":false])
        assert(model.modal == .consent)
        model.savePreferences(consent:true)
        assert(model.preferences.consent == true && model.modal == nil)
        print("SwiftUI model: unit validation, per-device preferences, consent, pagination, receipts, MFA and network session preservation passed.")
    }
}
