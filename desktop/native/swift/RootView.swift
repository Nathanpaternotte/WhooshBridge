import SwiftUI

private let accent = Color(red:0.18, green:0.38, blue:0.96)
private let ink = Color(red:0.07, green:0.12, blue:0.24)

struct RootView: View {
    @ObservedObject var model: BridgeModel
    var body: some View {
        NavigationSplitView {
            sidebar
                .navigationSplitViewColumnWidth(min: 190, ideal: 210, max: 250)
        } detail: {
            VStack(spacing: 0) {
                if let error = model.error { errorBanner(error) }
                Group {
                    switch model.destination {
                    case .rides: library
                    case .transfers: transfers
                    case .accounts: accounts
                    case .settings: settings
                    case .about: about
                    }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
                footer
            }
            .background(Color(nsColor: .windowBackgroundColor))
            .navigationTitle(title(model.destination))
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    if model.destination == .rides {
                        Button { model.refresh() } label: { Label(model.text("Actualiser", "Refresh"), systemImage:"arrow.clockwise") }
                            .disabled(model.busy || !model.whooshReady).help(model.text("Actualiser les sorties · ⌘R", "Refresh rides · ⌘R"))
                    }
                }
            }
        }
        .tint(accent)
        .sheet(item: $model.modal) { modal in modalView(modal).interactiveDismissDisabled() }
        .confirmationDialog(model.text("Déconnecter les deux comptes sur ce Mac ?", "Disconnect both accounts on this Mac?"), isPresented: $model.showDisconnect) {
            Button(model.text("Déconnecter", "Disconnect"), role: .destructive) { model.disconnect() }
        } message: { Text(model.text("Les sessions et les identifiants enregistrés seront supprimés. Vos réglages Garmin seront conservés.", "Saved sessions and credentials will be removed. Your Garmin settings will be kept.")) }
    }
    private func title(_ value: Destination) -> String {
        switch value {
        case .rides: return model.text("Mes sorties", "My rides")
        case .transfers: return model.text("Transferts", "Transfers")
        case .accounts: return model.text("Comptes", "Accounts")
        case .settings: return model.text("Réglages", "Settings")
        case .about: return model.text("À propos", "About")
        }
    }
    private func symbol(_ value: Destination) -> String {
        switch value { case .rides: return "bicycle"; case .transfers: return "arrow.up.right.circle"; case .accounts: return "person.crop.circle"; case .settings: return "slider.horizontal.3"; case .about: return "info.circle" }
    }
    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 11) {
                Image(nsImage: NSImage(named: NSImage.applicationIconName) ?? NSImage()).resizable().frame(width: 42,height: 42)
                VStack(alignment: .leading, spacing: 2) { Text("Whoosh").font(.system(size: 19, weight: .bold, design: .rounded)); Text("BRIDGE").font(.system(size: 10,weight:.semibold)).tracking(3).foregroundStyle(.secondary) }
            }.padding(.horizontal,18).padding(.top,26).padding(.bottom,30)
            Text(model.text("VOTRE ESPACE", "YOUR SPACE")).font(.system(size:10,weight:.semibold)).tracking(1.5).foregroundStyle(.secondary).padding(.horizontal,23).padding(.bottom,10)
            List(selection: Binding<Destination?>(get: { model.destination }, set: { if let value = $0 { model.destination = value } })) {
                ForEach([Destination.rides, .transfers, .accounts], id: \.self) { item in
                    Label(title(item), systemImage:symbol(item)).padding(.vertical,6).tag(item)
                }
                Section {
                    ForEach([Destination.settings, .about], id: \.self) { item in Label(title(item), systemImage:symbol(item)).padding(.vertical,6).tag(item) }
                }
            }.listStyle(.sidebar).scrollDisabled(true)
            Spacer(minLength: 10)
            VStack(alignment: .leading, spacing: 14) {
                Text(model.text("CONNEXIONS", "CONNECTIONS")).font(.system(size:10,weight:.semibold)).tracking(1.4).foregroundStyle(.secondary)
                connection("MyWhoosh", ready:model.whooshReady)
                connection("Garmin Connect", ready:model.garminReady)
            }.padding(16).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius:14)).padding(14)
            Text(model.preview ? model.text("APERÇU · DONNÉES FICTIVES", "PREVIEW · SYNTHETIC DATA") : "MYWHOOSH → GARMIN")
                .font(.system(size:9,weight:.medium)).tracking(1).foregroundStyle(.tertiary).padding(.horizontal,22).padding(.bottom,20)
        }
    }
    private func connection(_ name: String, ready: Bool) -> some View {
        Button { model.destination = .accounts } label: {
            HStack { Circle().fill(ready ? Color.green : Color.orange).frame(width:6,height:6); Text(name).font(.system(size:12,weight:.medium)); Spacer(); Image(systemName:ready ? "checkmark" : "arrow.up.right").font(.system(size:10,weight:.semibold)).foregroundStyle(.secondary) }
        }.buttonStyle(.plain).accessibilityLabel(name + ": " + model.text(ready ? "Connecté" : "À connecter", ready ? "Connected" : "Connect account"))
    }
    private var library: some View {
        HStack(alignment: .top, spacing: 0) {
            VStack(alignment: .leading, spacing: 22) {
                hero
                HStack {
                    VStack(alignment: .leading,spacing:5) {
                        Text(model.text("Votre prochaine arrivée.", "Your next finish line.")).font(.system(size:24,weight:.bold,design:.rounded))
                        Text(model.text("Choisissez vos sorties. Retrouvez-les sur Garmin.", "Choose your rides. Bring them to Garmin.")).font(.system(size:12)).foregroundStyle(.secondary)
                    }
                    Spacer(minLength:0)
                }
                HStack {
                    Image(systemName:"magnifyingglass").foregroundStyle(.secondary)
                    TextField(model.text("Rechercher une sortie", "Search rides"), text:$model.search).textFieldStyle(.plain)
                    if !model.search.isEmpty { Button { model.search = "" } label: { Image(systemName:"xmark.circle.fill") }.buttonStyle(.plain).foregroundStyle(.secondary).accessibilityLabel(model.text("Effacer la recherche", "Clear search")) }
                }.padding(11).background(Color(nsColor:.controlBackgroundColor),in:RoundedRectangle(cornerRadius:10))
                if model.rides.isEmpty { emptyLibrary }
                else {
                    HStack {
                        Text(model.text("SORTIES RÉCENTES", "RECENT RIDES")).font(.system(size:10,weight:.semibold)).tracking(1).foregroundStyle(.secondary)
                        Spacer()
                        Button(model.text("Tout sélectionner", "Select all")) { model.selection.formUnion(model.visibleRides.filter { model.receipt($0.id) == nil }.map(\.id)) }.buttonStyle(.link).font(.system(size:11)).disabled(model.busy)
                        Text("\(model.rides.count)").monospacedDigit().font(.system(size:11,weight:.semibold)).foregroundStyle(.secondary)
                    }
                    ScrollView {
                        LazyVStack(spacing:8) {
                            ForEach(model.visibleRides) { ride in rideRow(ride) }
                            if model.visibleRides.isEmpty { ContentUnavailableView.search(text:model.search).frame(minHeight:150) }
                            if model.hasMore { Button(model.text("Charger plus de sorties", "Load more rides")) { model.loadMore() }.disabled(model.busy).padding(12) }
                        }.padding(.bottom,6)
                    }.scrollIndicators(.hidden)
                }
            }.padding(28).frame(maxWidth:.infinity)
            Rectangle().fill(.separator).frame(width:1)
            GeometryReader { size in
                ScrollView { destinationPanel.frame(minHeight:max(0, size.size.height - 44)).padding(22) }.scrollIndicators(.hidden)
            }.frame(width:285).background(.background.opacity(0.45))
        }
    }
    private var hero: some View {
        ZStack(alignment:.leading) {
            RoundedRectangle(cornerRadius:20).fill(LinearGradient(colors:[ink,Color(red:0.11,green:0.23,blue:0.56),accent.opacity(0.95)],startPoint:.leading,endPoint:.bottomTrailing))
            GeometryReader { geometry in
                Path { p in
                    p.move(to:CGPoint(x:geometry.size.width * 0.48,y:170))
                    p.addCurve(to:CGPoint(x:geometry.size.width+35,y:20),control1:CGPoint(x:geometry.size.width*0.70,y:-80),control2:CGPoint(x:geometry.size.width*0.75,y:240))
                }.stroke(.white.opacity(0.13),style:StrokeStyle(lineWidth:38,lineCap:.round))
                Path { p in
                    p.move(to:CGPoint(x:geometry.size.width * 0.48,y:170))
                    p.addCurve(to:CGPoint(x:geometry.size.width+35,y:20),control1:CGPoint(x:geometry.size.width*0.70,y:-80),control2:CGPoint(x:geometry.size.width*0.75,y:240))
                }.stroke(.white.opacity(0.5),style:StrokeStyle(lineWidth:1,dash:[4,7]))
            }.clipped().accessibilityHidden(true)
            VStack(alignment:.leading,spacing:14) {
                HStack(spacing:7) { Image(systemName:"point.topleft.down.to.point.bottomright.curvepath"); Text("RIDE. BRIDGE. REPEAT.").tracking(2) }.font(.system(size:9,weight:.bold)).foregroundStyle(.white.opacity(0.7))
                Text(model.text("L’effort reste.\nL’aventure continue.", "Keep the effort.\nCarry it forward.")).font(.system(size:27,weight:.bold,design:.rounded)).foregroundStyle(.white).lineSpacing(1)
                HStack(spacing:9) { Text("MyWhoosh"); Image(systemName:"arrow.right"); Text("Garmin Connect") }.font(.system(size:10,weight:.medium)).foregroundStyle(.white.opacity(0.75))
            }.padding(24)
        }.frame(height:180)
    }
    private var emptyLibrary: some View {
        VStack(spacing:16) {
            Image(systemName:"bicycle.circle").font(.system(size:50,weight:.ultraLight)).foregroundStyle(accent)
            Text(model.text(model.whooshReady ? "La route est encore libre" : "Tout commence par une connexion", model.whooshReady ? "An open road ahead" : "It starts with a connection")).font(.title3.bold())
            Text(model.text(model.whooshReady ? "Vos sorties MyWhoosh apparaîtront ici." : "Reliez MyWhoosh et Garmin pour retrouver vos sorties.", model.whooshReady ? "Your MyWhoosh rides will appear here." : "Connect MyWhoosh and Garmin to find your rides.")).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Button(model.text(model.whooshReady ? "Actualiser" : "Connecter mes comptes", model.whooshReady ? "Refresh" : "Connect my accounts")) { if model.whooshReady { model.refresh() } else { model.destination = .accounts } }.buttonStyle(.borderedProminent).disabled(model.busy)
        }.frame(maxWidth:.infinity,maxHeight:.infinity).padding(20)
    }
    private func rideRow(_ ride: Ride) -> some View {
        let selected = model.selection.contains(ride.id)
        let receipt = model.receipt(ride.id)
        return Button { model.toggle(ride.id) } label: {
            HStack(spacing:13) {
                Image(systemName:selected ? "checkmark.circle.fill" : "circle").font(.system(size:19,weight:.light)).foregroundStyle(selected ? accent : Color.secondary.opacity(0.4))
                Image(systemName:"bicycle").font(.system(size:19,weight:.medium)).foregroundStyle(selected ? accent : .secondary).frame(width:42,height:42).background(selected ? accent.opacity(0.08) : Color.secondary.opacity(0.06),in:RoundedRectangle(cornerRadius:11))
                VStack(alignment:.leading,spacing:6) {
                    Text(ride.name).font(.system(size:13,weight:.semibold)).lineLimit(2).multilineTextAlignment(.leading)
                    Text(ride.date).font(.system(size:11)).foregroundStyle(.secondary)
                }
                Spacer(minLength:3)
                if let receipt { Image(systemName:receipt.symbol).foregroundStyle(receipt.confirmed ? .green : .orange).help(receipt.message) }
            }.padding(13).frame(maxWidth:.infinity,alignment:.leading)
                .background(selected ? accent.opacity(0.06) : Color(nsColor:.controlBackgroundColor),in:RoundedRectangle(cornerRadius:13))
                .overlay(RoundedRectangle(cornerRadius:13).strokeBorder(selected ? accent.opacity(0.45) : Color.primary.opacity(0.055),lineWidth:1))
                .contentShape(RoundedRectangle(cornerRadius:13))
        }.buttonStyle(.plain).disabled(model.busy || receipt?.outcome == "imported" || receipt?.outcome == "duplicate" || receipt?.outcome == "received")
            .accessibilityLabel(ride.name + ", " + ride.date).accessibilityValue(selected ? model.text("Sélectionnée", "Selected") : model.text("Non sélectionnée", "Not selected"))
    }
    private var destinationPanel: some View {
        VStack(alignment:.leading,spacing:20) {
            HStack { Text(model.text("DESTINATION", "DESTINATION")).font(.system(size:10,weight:.semibold)).tracking(1.5).foregroundStyle(.secondary); Spacer(); Image(systemName:"arrow.down.right").foregroundStyle(accent) }
            VStack(spacing:14) {
                DeviceArtwork().frame(width:92,height:126).padding(.top,8).accessibilityHidden(true)
                Text(model.selectedDevice?.name ?? "Garmin Edge").font(.system(size:22,weight:.bold,design:.rounded))
                HStack(spacing:5) { Circle().fill(model.garminReady ? .green : .orange).frame(width:5,height:5); Text(model.text(model.garminReady ? "Garmin Connect est prêt" : "Garmin Connect à connecter",model.garminReady ? "Garmin Connect is ready" : "Connect Garmin Connect")).font(.system(size:10)).foregroundStyle(.secondary) }
            }.frame(maxWidth:.infinity).padding(.bottom,4)
            VStack(alignment:.leading,spacing:8) {
                Text(model.text("Votre compteur", "Your cycling computer")).font(.system(size:12,weight:.semibold))
                Picker(model.text("Modèle Edge", "Edge model"), selection:Binding(get:{model.device},set:{model.chooseDevice($0)})) {
                    ForEach(model.devices) { edge in Text(edge.name).tag(edge.id) }
                }.labelsHidden().frame(maxWidth:.infinity).disabled(model.busy || !model.preferencesReady)
            }
            VStack(alignment:.leading,spacing:8) {
                HStack { Text("Unit ID").font(.system(size:12,weight:.semibold)); Spacer(); Button { model.showUnitHelp.toggle() } label: { Image(systemName:"questionmark.circle") }.buttonStyle(.plain).foregroundStyle(.secondary).accessibilityLabel(model.text("Où trouver mon Unit ID ?", "Where is my Unit ID?"))
                        .popover(isPresented:$model.showUnitHelp) { unitHelp.padding(22).frame(width:330) }
                }
                TextField(model.text("Identifiant numérique", "Numeric device ID"),text:$model.unit).textFieldStyle(.roundedBorder).onChange(of:model.unit) { _,_ in model.scheduleSave() }.disabled(model.busy || !model.preferencesReady)
                if !model.unit.isEmpty && model.validUnit == nil { Text(model.text("Un nombre entre 1 et 4294967295 est requis.", "Enter a number from 1 to 4294967295.")).font(.caption).foregroundStyle(.red) }
                Text(model.text("Mémorisé sur ce Mac pour chaque appareil.", "Saved on this Mac for each device.")).font(.system(size:10)).foregroundStyle(.tertiary)
            }
            Divider()
            VStack(alignment:.leading,spacing:9) {
                Label(model.text("Vos données, intactes", "Your data, preserved"),systemImage:"shield.lefthalf.filled").font(.system(size:12,weight:.semibold))
                Text(model.text("Mesures et capteurs conservés. Aucune métrique physiologique inventée.", "Measurements and sensors preserved. No fabricated physiological metrics.")).font(.system(size:11)).foregroundStyle(.secondary).lineSpacing(3).fixedSize(horizontal:false,vertical:true)
            }
            Spacer(minLength:12)
            VStack(alignment:.leading,spacing:12) {
                HStack(alignment:.firstTextBaseline) { Text("\(model.selection.count)").font(.system(size:32,weight:.semibold,design:.rounded)).monospacedDigit(); Text(model.text("sélectionnée(s)", "selected")).font(.system(size:11)).foregroundStyle(.secondary); Spacer() }
                Button { model.modal = .confirm } label: { HStack { Text(model.text("Transférer vers Garmin", "Transfer to Garmin")); Spacer(); Image(systemName:"arrow.up.right") }.font(.system(size:12,weight:.semibold)).padding(.vertical,7).frame(maxWidth:.infinity) }.buttonStyle(.borderedProminent).controlSize(.large).disabled(!model.canTransfer)
                Text(model.text("Vérifiez votre modèle et votre Unit ID avant l’envoi.", "Check your model and Unit ID before sending.")).font(.system(size:10)).foregroundStyle(.tertiary).fixedSize(horizontal:false,vertical:true)
            }
        }
    }
    private var unitHelp: some View {
        VStack(alignment:.leading,spacing:13) {
            Text(model.text("Où trouver mon Unit ID ?", "Where is my Unit ID?")).font(.headline)
            Text(model.text("Dans Garmin Connect", "In Garmin Connect")).bold()
            Text(model.text("Plus → Appareils Garmin. Faites un appui long sur le nom de votre appareil. Recopiez Device ID sur iPhone ou Unit ID sur Android.", "More → Garmin Devices. Long-press the device name. Copy Device ID on iPhone or Unit ID on Android."))
            Divider()
            Text(model.text("Sur un Edge 840", "On an Edge 840")).bold()
            Text(model.text("Menu → Système → À propos → Informations de copyright.", "Menu → System → About → Copyright information."))
            Text(model.text("Utilisez l’identifiant numérique, pas le numéro de série imprimé.", "Use the numeric identifier, not the printed serial number.")).foregroundStyle(.secondary)
        }.font(.system(size:12))
    }
    private var footer: some View {
        VStack(spacing:0) {
            if model.busy { ProgressView(value:model.progress).progressViewStyle(.linear).tint(accent) }
            Divider()
            HStack(spacing:9) {
                if model.busy { ProgressView().controlSize(.mini) }
                else { Image(systemName:model.connected ? "checkmark.circle" : "exclamationmark.circle").foregroundStyle(model.connected ? .green : .orange) }
                Text(model.status.isEmpty ? model.text("Prêt pour la prochaine sortie", "Ready for your next ride") : model.status).font(.system(size:11)).foregroundStyle(.secondary).lineLimit(1).help(model.status)
                Spacer()
                if model.busy { Button(model.text("Arrêter", "Stop")) { model.cancel() }.controlSize(.small) }
                else { Text("Whoosh Bridge").font(.system(size:10,weight:.medium)).foregroundStyle(.tertiary) }
            }.padding(.horizontal,20).padding(.vertical,11)
        }.background(.bar)
    }
    private func errorBanner(_ message: String) -> some View {
        HStack(alignment:.top,spacing:10) {
            Image(systemName:"exclamationmark.triangle.fill").foregroundStyle(.orange)
            Text(message).font(.system(size:12)).textSelection(.enabled).frame(maxWidth:.infinity,alignment:.leading)
            Button { model.error = nil } label: { Image(systemName:"xmark") }.buttonStyle(.plain).accessibilityLabel(model.text("Fermer le message", "Dismiss message"))
        }.padding(14).background(Color.orange.opacity(0.1))
    }
    private func pageHeading(_ heading: String, _ subtitle: String, _ icon: String) -> some View {
        VStack(alignment:.leading,spacing:12) {
            Image(systemName:icon).font(.system(size:28,weight:.light)).foregroundStyle(accent).padding(.bottom,4)
            Text(heading).font(.system(size:30,weight:.bold,design:.rounded))
            Text(subtitle).foregroundStyle(.secondary).font(.system(size:13))
        }.frame(maxWidth:.infinity,alignment:.leading).padding(.bottom,14)
    }
    private var accounts: some View {
        ScrollView {
            VStack(alignment:.leading,spacing:24) {
                pageHeading(model.text("Deux comptes. Une seule route.", "Two accounts. One road."),model.text("Vos sessions restent sur ce Mac, protégées par le trousseau.", "Your sessions stay on this Mac, protected by Keychain."),"link")
                HStack(alignment:.top,spacing:20) { accountCard(whoosh:true); accountCard(whoosh:false) }
                HStack { Button(model.text("Réessayer les sessions enregistrées", "Retry saved sessions")) { model.restore() }; Spacer(); Button(model.text("Déconnecter les comptes…", "Disconnect accounts…"),role:.destructive) { model.showDisconnect = true } }.disabled(model.busy || !model.connected)
                Label(model.text("La connexion MyWhoosh s’ouvre sur le site officiel. Validez les éventuels CAPTCHA dans cette fenêtre.", "MyWhoosh sign-in opens the official website. Complete any CAPTCHA in that window."),systemImage:"lock.shield").font(.system(size:12)).foregroundStyle(.secondary)
            }.padding(36).frame(maxWidth:950)
        }
    }
    private func accountCard(whoosh: Bool) -> some View {
        let ready = whoosh ? model.whooshReady : model.garminReady
        return VStack(alignment:.leading,spacing:20) {
            HStack { Image(systemName:whoosh ? "bicycle" : "location.north.circle").font(.system(size:26,weight:.light)).foregroundStyle(accent); Spacer(); Label(model.text(ready ? "Connecté" : "À connecter",ready ? "Connected" : "Not connected"),systemImage:ready ? "checkmark.circle.fill" : "circle.dashed").font(.caption).foregroundStyle(ready ? .green : .secondary) }
            Text(whoosh ? "MyWhoosh" : "Garmin Connect").font(.system(size:22,weight:.bold,design:.rounded))
            Text(model.text(whoosh ? "Le point de départ de vos sorties." : "Votre historique continue ici.",whoosh ? "Where your rides begin." : "Your training story continues here.")).font(.system(size:12)).foregroundStyle(.secondary)
            if ready {
                Label(model.text("Session disponible sur ce Mac", "Session available on this Mac"),systemImage:"checkmark.shield").foregroundStyle(.green).padding(.vertical,24)
            } else {
                TextField(model.text("Adresse e-mail", "Email address"),text:whoosh ? $model.whooshEmail : $model.garminEmail).textContentType(.username)
                SecureField(model.text("Mot de passe", "Password"),text:whoosh ? $model.whooshPassword : $model.garminPassword).textContentType(.password).onSubmit { if !model.busy { model.login(whoosh:whoosh) } }
                if whoosh { Toggle(model.text("Mémoriser dans le trousseau", "Remember in Keychain"),isOn:$model.remember).font(.caption) }
                else { Text(model.text("Le mot de passe Garmin n’est pas enregistré.", "Your Garmin password is never saved.")).font(.caption).foregroundStyle(.secondary) }
                Button(model.text("Se connecter", "Sign in")) { model.login(whoosh:whoosh) }.buttonStyle(.borderedProminent).disabled((whoosh ? model.whooshEmail : model.garminEmail).isEmpty || (whoosh ? model.whooshPassword : model.garminPassword).isEmpty)
            }
            Spacer(minLength:0)
        }.textFieldStyle(.roundedBorder).controlSize(.large).padding(24).frame(maxWidth:.infinity,minHeight:315,alignment:.topLeading).background(.background,in:RoundedRectangle(cornerRadius:18)).disabled(model.busy || !model.connected)
    }
    private var transfers: some View {
        ScrollView {
            VStack(alignment:.leading,spacing:20) {
                pageHeading(model.text("Chaque sortie, à bon port.", "Every ride, accounted for."),model.text("Résultats de cette session. Une réception ne confirme pas un import.", "Results from this session. Receipt does not confirm an import."),"arrow.up.right.circle")
                if model.receipts.isEmpty { ContentUnavailableView(model.text("Aucun transfert pour le moment", "No transfers yet"),systemImage:"tray",description:Text(model.text("Les résultats apparaîtront ici après votre premier envoi.", "Results will appear here after your first transfer."))) }
                ForEach(model.receipts) { receipt in
                    HStack(alignment:.top,spacing:15) {
                        Image(systemName:receipt.symbol).font(.title2).foregroundStyle(receipt.confirmed ? .green : .orange)
                        VStack(alignment:.leading,spacing:7) { Text(receipt.name).font(.headline); Text(receipt.message).font(.system(size:12)).foregroundStyle(.secondary).textSelection(.enabled) }
                        Spacer()
                        Text(receipt.outcome == "imported" ? model.text("IMPORTÉ", "IMPORTED") : receipt.outcome == "duplicate" ? model.text("DOUBLON", "DUPLICATE") : model.text("À VÉRIFIER", "CHECK STATUS")).font(.system(size:9,weight:.bold)).tracking(0.8).foregroundStyle(receipt.confirmed ? .green : .orange)
                    }.padding(20).background(.background,in:RoundedRectangle(cornerRadius:14))
                }
                Link(model.text("Ouvrir Garmin Connect", "Open Garmin Connect"),destination:URL(string:"https://connect.garmin.com/modern/activities")!).font(.system(size:12))
            }.padding(36)
        }
    }
    private var settings: some View {
        Form {
            Section(model.text("Apparence et langue", "Appearance and language")) {
                Picker(model.text("Langue de l’interface", "Interface language"),selection:$model.english) { Text("Français").tag(false); Text("English").tag(true) }.onChange(of:model.english) { _,_ in if model.preferencesReady && !model.busy { model.savePreferences() } }.disabled(model.busy || !model.preferencesReady)
                Text(model.text("L’apparence claire ou sombre suit les réglages de macOS.", "Light and dark appearance follow your macOS settings.")).foregroundStyle(.secondary)
            }
            Section(model.text("Confidentialité", "Privacy")) {
                Toggle(model.text("Partager les statistiques d’utilisation", "Share usage statistics"),isOn:Binding(get:{model.preferences.consent == true},set:{model.savePreferences(consent:$0)})).disabled(model.busy || !model.preferencesReady)
                Text(consentDescription).font(.caption).foregroundStyle(.secondary)
            }
            Section(model.text("Mises à jour", "Updates")) {
                Toggle(model.text("Rechercher automatiquement les mises à jour", "Automatically check for updates"),isOn:$model.automaticUpdates).onChange(of:model.automaticUpdates) { _,value in model.updater.setAutomatic(value) }.disabled(!model.updater.available)
                Button(model.text("Rechercher maintenant…", "Check now…")) { model.updater.check() }.disabled(model.busy || !model.updater.available)
            }
            Section(model.text("Appareil Garmin", "Garmin device")) {
                Text(model.text("Le modèle et l’Unit ID se règlent dans le panneau Destination de Mes sorties.", "Set the model and Unit ID in the Destination panel under My rides."))
                Button(model.text("Configurer mon Garmin", "Configure my Garmin")) { model.destination = .rides }
            }
        }.formStyle(.grouped).padding(12)
    }
    private var about: some View {
        ScrollView {
            VStack(alignment:.leading,spacing:24) {
                pageHeading("Whoosh Bridge",model.text("Vos sorties MyWhoosh, dans Garmin Connect.", "Your MyWhoosh rides, in Garmin Connect."),"bicycle.circle")
                Text("Nathan Paternotte").font(.headline)
                Text(model.version).font(.system(.body,design:.monospaced)).foregroundStyle(.secondary)
                Text(model.text("Une interface native SwiftUI. Un moteur Rust pour préparer vos fichiers FIT et un composant Python pour Garmin.", "A native SwiftUI interface. A Rust engine to prepare your FIT files and a Python component for Garmin."))
                Text(model.text("Whoosh Bridge n’est affilié ni à MyWhoosh ni à Garmin. Les mesures et les identités des capteurs sont préservées. Le calcul de Training Effect, de la charge et du VO₂ max par Garmin n’est pas garanti.", "Whoosh Bridge is not affiliated with MyWhoosh or Garmin. Measurements and sensor identities are preserved. Garmin processing of Training Effect, training load and VO₂ max is not guaranteed.")).foregroundStyle(.secondary)
                Divider()
                Text(model.text("Licence et contributions", "License and acknowledgements")).font(.title3.bold())
                Text("GNU General Public License v3").font(.headline)
                Link("marcelorodrigo/mywhoosh-to-garmin", destination:URL(string:"https://github.com/marcelorodrigo/mywhoosh-to-garmin")!)
                Text(model.text("Projet d’origine sous GPL v3. Merci aux auteurs de garminconnect, garth, Sparkle et aux autres contributeurs open source.", "Original project licensed under GPL v3. Thanks to the authors of garminconnect, garth, Sparkle and the other open-source contributors.")).foregroundStyle(.secondary)
                DisclosureGroup(model.text("Consulter les licences et les attributions", "View licenses and acknowledgements")) {
                    Text(notices).font(.system(size:11)).foregroundStyle(.secondary).textSelection(.enabled).padding(.top,12)
                }
            }.padding(36).frame(maxWidth:850,alignment:.leading)
        }
    }
    private var notices: String {
        guard let url = Bundle.main.url(forResource:"THIRD_PARTY_NOTICES",withExtension:"md"),let value = try? String(contentsOf:url,encoding:.utf8) else { return "GPL v3 · garminconnect · garth · Rust · Python · Sparkle" }
        return value
    }
    private var consentDescription: String {
        model.text("Avec votre accord, un identifiant aléatoire persistant permet de compter les installations actives via PostHog. Aucun identifiant de compte, mot de passe, fichier FIT ou détail de sortie n’est envoyé. Vous pouvez changer ce choix à tout moment.", "With your permission, a persistent random identifier helps count active installations through PostHog. No account identifier, password, FIT file or ride details are sent. You can change this choice at any time.")
    }
    @ViewBuilder private func modalView(_ modal: Modal) -> some View {
        VStack(alignment:.leading,spacing:20) {
            Image(systemName:modal == .mfa ? "lock.shield" : modal == .consent ? "hand.raised" : "arrow.up.right.circle").font(.system(size:35,weight:.light)).foregroundStyle(accent)
            if modal == .mfa {
                Text(model.text("Une dernière vérification", "One last verification")).font(.title2.bold())
                Text(model.text("Entrez le code de vérification envoyé par Garmin. Il expire après quelques minutes.", "Enter the verification code sent by Garmin. It expires after a few minutes.")).foregroundStyle(.secondary)
                TextField(model.text("Code de vérification", "Verification code"),text:$model.mfaCode).textFieldStyle(.roundedBorder).controlSize(.large).onSubmit { if !model.mfaCode.isEmpty { model.submitMFA() } }
                HStack { Button(model.text("Annuler", "Cancel")) { model.cancel() }; Spacer(); Button(model.text("Vérifier", "Verify")) { model.submitMFA() }.buttonStyle(.borderedProminent).disabled(model.mfaCode.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty).keyboardShortcut(.defaultAction) }
            } else if modal == .consent {
                Text(model.text("Aider Whoosh Bridge à avancer", "Help Whoosh Bridge improve")).font(.title2.bold())
                Text(consentDescription).foregroundStyle(.secondary)
                if let error = model.error { Text(error).font(.caption).foregroundStyle(.red) }
                HStack { Button(model.text("Refuser", "Decline")) { model.savePreferences(consent:false) }; Spacer(); Button(model.text("Accepter", "Allow")) { model.savePreferences(consent:true) }.buttonStyle(.borderedProminent) }.disabled(model.busy)
            } else {
                Text(model.text("Prêt pour Garmin ?", "Ready for Garmin?")).font(.title2.bold())
                Text(model.text("\(model.selection.count) sortie(s) seront préparées pour votre \(model.selectedDevice?.name ?? "Garmin") avec l’Unit ID \(model.unit).", "\(model.selection.count) ride(s) will be prepared for your \(model.selectedDevice?.name ?? "Garmin") with Unit ID \(model.unit)."))
                Text(model.text("En cas de réception non confirmée, vérifiez Garmin Connect avant tout nouvel envoi. Aucun transfert ambigu n’est relancé automatiquement.", "If receipt is unconfirmed, check Garmin Connect before sending again. Ambiguous transfers are never retried automatically.")).font(.caption).foregroundStyle(.secondary)
                HStack { Button(model.text("Annuler", "Cancel")) { model.modal = nil }; Spacer(); Button(model.text("Transférer", "Transfer")) { model.confirmTransfer() }.buttonStyle(.borderedProminent).disabled(!model.canTransfer).keyboardShortcut(.defaultAction) }
            }
        }.padding(32).frame(width:440)
    }
}

private struct DeviceArtwork: View {
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius:20).fill(LinearGradient(colors:[Color(white:0.28),Color(white:0.08)],startPoint:.topLeading,endPoint:.bottomTrailing))
                .shadow(color:.black.opacity(0.16),radius:10,x:0,y:9)
            RoundedRectangle(cornerRadius:17).strokeBorder(.white.opacity(0.22),lineWidth:1).padding(2)
            VStack(spacing:5) {
                Capsule().fill(.white.opacity(0.35)).frame(width:18,height:2)
                ZStack {
                    RoundedRectangle(cornerRadius:7).fill(Color(red:0.86,green:0.92,blue:0.87))
                    Path { p in p.move(to:CGPoint(x:8,y:62)); p.addLine(to:CGPoint(x:23,y:48)); p.addLine(to:CGPoint(x:17,y:31)); p.addLine(to:CGPoint(x:42,y:21)); p.addLine(to:CGPoint(x:55,y:8)) }.stroke(accent,style:StrokeStyle(lineWidth:3,lineCap:.round,lineJoin:.round))
                    Image(systemName:"location.north.fill").font(.system(size:12)).foregroundStyle(accent).offset(x:0,y:3)
                }.frame(height:80)
                Color.clear.frame(height:7)
            }.padding(11)
        }
    }
}
