// JoinFailure — when every door fails, the badge says ONE sentence (T-247).
//
// The complaint, live on Ian's iPhone (familiar#10, build 100, on 5G+): the badge under
// the `!` printed `pins 2+1 · 192.168.108.10→t:The request timed out. …` for fifteen
// candidates — every LAN, tailnet and lighthouse address — where a sentence belongs.
// The per-door lines are diagnostics and keep their place on the Device screen
// (`attemptLog`); the badge is for the person holding the phone, and it says how many
// doors were tried, what most of them said, and — when the lighthouse is among them —
// what the one door that should answer from anywhere said.
//
// Pure: strings in, one string out. The attempt lines are exactly what the race writes
// (`host→cause`, with `cause` one of `t:<message>`, `h<status>:<body>`, `enc`, `dec`,
// or a bare error code) — parsed here, never re-shaped there.

import Foundation

public enum JoinFailure {
    /// What one door said, classed for the sentence.
    public enum Cause: Equatable, Sendable {
        case timedOut
        case connectionLost
        case couldNotConnect
        case offline
        case refused(status: Int)
        case unreadable
        case other(String)

        /// The words the sentence uses for this cause, plural-neutral ("timed out").
        public var words: String {
            switch self {
            case .timedOut: return "timed out"
            case .connectionLost: return "dropped the connection"
            case .couldNotConnect: return "could not be reached"
            case .offline: return "found the network offline"
            case .refused(let s): return "refused (HTTP \(s))"
            case .unreadable: return "answered with something unreadable"
            case .other(let c): return "failed (\(c))"
            }
        }
    }

    public struct Attempt: Equatable, Sendable {
        public var host: String
        public var cause: Cause
        public init(host: String, cause: Cause) { self.host = host; self.cause = cause }
    }

    /// `host→cause`, as the race writes it. A line without the arrow is a host with an
    /// unknown cause; an empty line is nothing.
    public static func parse(_ line: String) -> Attempt? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return nil }
        guard let arrow = trimmed.range(of: "→") else { return Attempt(host: trimmed, cause: .other("?")) }
        let host = String(trimmed[..<arrow.lowerBound])
        let code = String(trimmed[arrow.upperBound...])
        return Attempt(host: host, cause: classify(code))
    }

    /// The race truncates transport messages to 30 characters, so every match here is a
    /// prefix-safe substring of the system's own wording ("The network connection was lost."
    /// arrives as "The network connection was los").
    static func classify(_ code: String) -> Cause {
        if code.hasPrefix("t:") {
            let m = code.dropFirst(2).lowercased()
            if m.contains("timed out") { return .timedOut }
            if m.contains("connection was lost") || m.contains("connection was los") { return .connectionLost }
            if m.contains("could not connect") || m.contains("connection refused") { return .couldNotConnect }
            if m.contains("offline") || m.contains("not connected to the internet") { return .offline }
            if m.contains("hostname") || m.contains("could not be found") { return .couldNotConnect }
            return .other(String(m))
        }
        if code.hasPrefix("h"), let colon = code.firstIndex(of: ":"),
           let status = Int(code[code.index(after: code.startIndex)..<colon]) {
            return .refused(status: status)
        }
        if code == "enc" || code == "dec" { return .unreadable }
        return .other(code)
    }

    /// The one line. `lighthouse` is the address the doctrine says must always be worth a
    /// knock; when it was among the doors tried, what it said is named separately, because
    /// "every LAN door timed out" and "the lighthouse dropped the connection" are different
    /// facts about where the phone is.
    public static func sentence(attempts lines: [String], lighthouse: String? = nil) -> String {
        let attempts = lines.compactMap(parse)
        guard !attempts.isEmpty else { return "no door to try" }
        let lh = lighthouse.flatMap { l in attempts.first { $0.host == l || $0.host.hasPrefix(l + ":") } }
        let others = attempts.filter { $0 != lh }
        let n = attempts.count
        let doors = "\(n) door\(n == 1 ? "" : "s") tried"
        var out: String
        if others.isEmpty, let lh {
            // The lighthouse alone was tried.
            out = "the lighthouse \(lh.cause.words) (\(doors))"
        } else {
            // The dominant cause among the other doors, counted.
            var counts: [(Cause, Int)] = []
            for a in others {
                if let i = counts.firstIndex(where: { $0.0 == a.cause }) { counts[i].1 += 1 } else { counts.append((a.cause, 1)) }
            }
            counts.sort { $0.1 > $1.1 }
            let (top, k) = counts[0]
            if k == others.count {
                out = (others.count == 1 ? "the door " : "every door ") + top.words
            } else {
                let rest = counts.dropFirst().map { "\($0.1) \($0.0.words)" }.joined(separator: ", ")
                out = "\(k) of \(others.count) doors \(top.words); \(rest)"
            }
            if let lh { out += ", and the lighthouse \(lh.cause.words)" }
            out += " (\(doors))"
        }
        // Every path to the mesh timing out or unreachable, the lighthouse included, is what
        // a phone off Wi-Fi with no data path looks like — say so as a question, not a verdict.
        let allAway = attempts.allSatisfy { [.timedOut, .couldNotConnect, .offline, .connectionLost].contains($0.cause) }
        if allAway { out += " — off Wi-Fi, or no data path from here?" }
        return out
    }
}
