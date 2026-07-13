import SwiftUI
import FamiliarMesh

/// The iPad Glass — a peer console. Where the iPhone is a pocket sensor with a compact form, the
/// iPad is a *full peer*: it reads the familiar's worldview over `/mesh/worldview` and presents it
/// the way the macOS Glass does — the three constitutional meters, the live observation feed, the
/// peer roster — alongside what this device itself contributes. A NavigationSplitView because the
/// iPad has the room the phone doesn't.
struct GlassConsole: View {
    @EnvironmentObject var model: AppModel
    @State private var pane: Pane? = .overview

    enum Pane: String, CaseIterable, Identifiable {
        case overview = "Overview"
        case feed = "Live feed"
        case peers = "Peers"
        case device = "This iPad"
        var id: String { rawValue }
        var icon: String {
            switch self {
            case .overview: return "circle.hexagongrid.fill"
            case .feed: return "waveform.path.ecg"
            case .peers: return "point.3.connected.trianglepath.dotted"
            case .device: return "ipad"
            }
        }
    }

    var body: some View {
        NavigationSplitView {
            List(Pane.allCases, selection: $pane) { p in
                Label(p.rawValue, systemImage: p.icon).tag(p)
            }
            .navigationTitle(model.worldview?.group_label ?? model.groupLabel)
            .safeAreaInset(edge: .bottom) { MarbleBadge(withdrawn: model.worldview?.withdrawn ?? false) }
        } detail: {
            ScrollView {
                switch pane ?? .overview {
                case .overview: OverviewPane(view: model.worldview)
                case .feed: FeedPane(recent: model.worldview?.recent ?? [])
                case .peers: PeersPane(peers: model.worldview?.peers ?? [])
                case .device: DevicePane()
                }
            }
            .navigationTitle((pane ?? .overview).rawValue)
        }
        .onAppear { model.startWorldviewPolling() }
        .onDisappear { model.stopWorldviewPolling() }
    }
}

/// The marble, and the one bit that must never read false: withdrawal (Law II — the empty world).
private struct MarbleBadge: View {
    let withdrawn: Bool
    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(LinearGradient(colors: [Color(red: 0.47, green: 0.72, blue: 1), Color(red: 0.07, green: 0.25, blue: 0.59)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing))
                .frame(width: 22, height: 22)
            Text(withdrawn ? "withdrawn — empty world" : "present")
                .font(.caption).foregroundStyle(withdrawn ? .orange : .secondary)
            Spacer()
        }
        .padding(12)
    }
}

// MARK: - Overview: the three constitutional meters + counts

private struct OverviewPane: View {
    let view: Worldview?
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            if let v = view {
                Meter(label: "Presence", value: v.presence, tint: v.withdrawn ? .orange : .blue,
                      note: v.withdrawn ? "the served have withdrawn" : "the served are engaged (Law II)")
                Meter(label: "Service", value: v.service, tint: .green, note: "how much touches the served (Law II)")
                Meter(label: "Capacity", value: v.capacity, tint: .purple, note: "room to act (Law III)")
                HStack(spacing: 24) {
                    Stat(number: v.observation_count, label: "observations")
                    Stat(number: v.peers.count, label: "peers")
                    Stat(number: v.recent.count, label: "recent")
                }
                .padding(.top, 4)
                Text("familiar node \(v.node_id)").font(.caption2).foregroundStyle(.secondary)
            } else {
                ContentUnavailableView("Reading the familiar…", systemImage: "antenna.radiowaves.left.and.right",
                                       description: Text("Polling /mesh/worldview. If this stays empty, check the familiar is reachable and the mesh is open."))
            }
        }
        .padding()
    }
}

private struct Meter: View {
    let label: String
    let value: Double
    let tint: Color
    let note: String
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(label).font(.headline)
                Spacer()
                Text(String(format: "%.2f", value)).font(.system(.body, design: .monospaced)).foregroundStyle(.secondary)
            }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(.quaternary).frame(height: 10)
                    Capsule().fill(tint).frame(width: max(6, geo.size.width * CGFloat(min(max(value, 0), 1))), height: 10)
                }
            }
            .frame(height: 10)
            Text(note).font(.caption2).foregroundStyle(.secondary)
        }
    }
}

private struct Stat: View {
    let number: Int
    let label: String
    var body: some View {
        VStack {
            Text("\(number)").font(.system(.title2, design: .rounded)).bold()
            Text(label).font(.caption2).foregroundStyle(.secondary)
        }
    }
}

// MARK: - Live feed: the recent observation tail

private struct FeedPane: View {
    let recent: [ObsView]
    var body: some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            if recent.isEmpty {
                Text("No observations yet.").foregroundStyle(.secondary).padding()
            }
            ForEach(recent) { o in
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(o.actor).font(.subheadline).bold()
                        Text(o.action).font(.subheadline).foregroundStyle(.secondary)
                        Text(o.object).font(.subheadline)
                        Spacer()
                        Text(GlassTime.ago(o.ts)).font(.caption2).foregroundStyle(.secondary)
                    }
                    HStack(spacing: 6) {
                        if !o.context.isEmpty {
                            Text(o.context).font(.caption2).foregroundStyle(.secondary)
                        }
                        Text(o.source).font(.caption2).foregroundStyle(o.source.hasPrefix("mesh:") ? .blue : .secondary)
                    }
                }
                .padding(.vertical, 8).padding(.horizontal)
                Divider()
            }
        }
    }
}

// MARK: - Peers

private struct PeersPane: View {
    let peers: [PeerView]
    var body: some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            if peers.isEmpty {
                Text("No federated peers yet.").foregroundStyle(.secondary).padding()
            }
            ForEach(peers) { p in
                HStack {
                    Image(systemName: "cpu").foregroundStyle(.blue)
                    VStack(alignment: .leading) {
                        Text(p.label).font(.headline)
                        Text(p.node_id).font(.caption2).foregroundStyle(.secondary)
                    }
                    Spacer()
                    VStack(alignment: .trailing) {
                        Text("\(p.tools_offered) tools · \(p.patterns_offered) patterns").font(.caption2)
                        Text(GlassTime.ago(p.last_seen)).font(.caption2).foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 10).padding(.horizontal)
                Divider()
            }
        }
    }
}

// MARK: - This iPad: what this peer itself contributes (consent + status), reusing the phone form.

private struct DevicePane: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            GroupBox("What this iPad shares (derived only)") {
                VStack(alignment: .leading, spacing: 12) {
                    Toggle("Location — home / away", isOn: $model.locationEnabled)
                        .onChange(of: model.locationEnabled) { _ in model.startSensingIfConsented() }
                    Toggle("Motion — walking / driving / still", isOn: $model.motionEnabled)
                        .onChange(of: model.motionEnabled) { _ in model.startSensingIfConsented() }
                    Toggle("Network — devices & services nearby", isOn: $model.discoveryEnabled)
                        .onChange(of: model.discoveryEnabled) { _ in model.startDiscoveryIfConsented() }
                    Button("Set “home” to my current location") { model.setHomeToCurrentLocation() }
                }
            }
            GroupBox("Voice & presence") {
                VStack(alignment: .leading, spacing: 12) {
                    VoiceControl(voice: model.voice)
                    FaceControl(model: model, face: model.face)
                }
            }
            GroupBox("Activity") {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(model.log.prefix(12), id: \.self) { Text($0).font(.caption) }
                }
            }
            Button("Unenroll this device", role: .destructive) { model.unenroll() }
        }
        .padding()
    }
}

/// Coarse relative time for the console ("just now", "3m", "2h", "5d").
enum GlassTime {
    static func ago(_ ts: Int64) -> String {
        let secs = Int64(Date().timeIntervalSince1970) - ts
        if secs < 5 { return "just now" }
        if secs < 60 { return "\(secs)s" }
        if secs < 3600 { return "\(secs / 60)m" }
        if secs < 86400 { return "\(secs / 3600)h" }
        return "\(secs / 86400)d"
    }
}
