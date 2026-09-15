# Ask to Jeff — MeowNet on the Interplanetary Internet: bundles, lasers, and the rungs above 19.2

Status: **READY TO FILE on ucf-exchange, in Ian's name, on Ian's go** (Ian, 2026-09-15:
"Meownet at 14.4k seems way behind where space cats should be. Lasers." Choices made
2026-09-15: rungs above 19.2 are asked for; bundles carry lifetimes and hop-delayed house
posts; the real standards are named on the wire; filed on ucf-exchange with the fitting's
engine half cross-referenced to united-cat-foods-metal.)

The shape is the one Jeff's factory has taken from us before (ucf-exchange#40, #41): the
problem, what exists, proposals in the order we'd try them, a recommendation, signed. The
issue body is everything below the rule.

---

## The problem

MeowNet is a 1993 board at Tuna Prime that every hull raises over radio, and the link is
the feature: range picks a rung on the period ladder (300 … 19200), the Sun takes rungs
away, the outer berths sit at the floor, and `+++ATH` / `NO CARRIER` close the session
(`Sources/UCFBBS/LinkBudget.swift`; your ruling 2026-09-08, "19.2 should be the fastest
baud rate"). It is a lovely fiction, and it is fifty years behind the ships flying it: a
hull that brachistochrones to Neptune at 0.19 g and calls a tanker from Saturn talks to its
home board through a Hayes modem. Space cats should have lasers.

What is missing is not the terminal — the board is a 1993 board on purpose and should stay
one — it is the **network the terminal sits on**. Today the link is one hop, ship to Tuna
Prime, and the file's own rule 1 names the fiction it stands in for: "a store-and-forward
relay chain with local echo at your end." That chain has a real name, a real standard and a
real physical layer, and each of them is a game mechanic waiting to be picked up.

## What exists today

- `LinkBudget.compute(rangeAU, weather, transceiverRungs, seed)` → `Link {baud, errorRate,
  seed, quality, banner, carrierLost}`. Range sets bit rate, never latency (rule 1); noise
  is rendered by the client from a seed (rule 2); nothing touches the fold (rule 3); no
  weather data means a clean link. `transceiverRungs` is already a parameter — "the ship
  mod that raises it is a later slice".
- Live NOAA space weather (Kp, GOES flare class) costs rungs and adds noise; an X-class
  flare kills the carrier.
- The baud shows on the callers log and the board state; the terminal paints at it.
- Three house pens (THE DARK FACTORY, THE MARKET REPORT, the regulars), pilot posts on four
  subs, and the doors.

## The reference set (real standards, so the fiction can be exact)

| Layer | Standard | What it is | What it gives the game |
|---|---|---|---|
| Network | **Bundle Protocol v7** (RFC 9171, DTN) | Store-and-forward: data is packaged into bundles and held at intermediate nodes until the next hop is available; no end-to-end handshake. The foundation of the "Interplanetary Internet". | The relay chain rule 1 already imagines, made real: posts and mail are bundles, a relay holds them, custody hops are visible, a bundle has a lifetime. |
| Reliability | **Licklider Transmission Protocol** (LTP, RFC 5326) | Under BP over a point-to-point deep-space link: handles propagation delay, retransmits without terrestrial-style ACK timing. | Why a NOISY link still delivers: LTP retransmits; the terminal shows the corruption, the bundle arrives whole. |
| Throughput | **HDTN** (NASA High-rate DTN) | A BP implementation routing at Gbit/s over mixed laser + RF paths, working off backlogs. | The backbone between relays is fast; the LAST hop is what a hull owns and pays for. |
| Physical, optical | **SDA Optical Communications Terminal standard**; **CCSDS Optical Coding & Modulation** (FEC against turbulence and fading) | Interoperable laser terminals; codes that survive fades. | The **optical terminal** as a ship fitting; fades as weather; FEC as the reason optics degrade gracefully instead of dropping carrier. |
| Physical, RF | **CCSDS Space Packet Protocol** over X/Ka-band | The legacy framing, increasingly wrapped inside DTN bundles. | Today's radio link, kept, as the floor every hull has. |

## Proposals (in the order I'd try them)

**A. The optical terminal, and the rungs above 19.2.** A fitting the yard sells like the
other three (`refit {fitting: "optical-terminal"}`, priced on `/v1/reference` beside
`refitCostDriveTune`; the engine half is a one-line fitting on metal, cross-referenced).
A fitted hull's last hop is a laser to the nearest relay, and the ladder gains the three
rungs every 1993 terminal already knew and never got to use — **38400, 57600, 115200** —
the serial port's own ceiling, reachable only by laser. The board stays a 1993 board: the
UART is the cap, not the modem, and the backbone behind the relay is HDTN-class and never
paints. I know this revisits your 2026-09-08 ruling; the case is that the ruling was about
what a *modem* can do in 1993, and a laser is not a modem — it is what the modem's serial
port was waiting for. Optics have their own weather: a geomagnetic storm (Kp) that costs
an RF rung costs a laser nothing, while pointing fades and turbulence cost a laser a rung
on a schedule the NOAA feed does not carry (`LinkQuality.fading`, the word the enum already
has, becomes the optical one). Two `Link` fields added: `carrier: "rf" | "optical"` and
`stack` (the banner's own words, below), so the client paints acquisition differently.

**B. Bundles: the relay chain made real, without a second of latency.** Rule 1 holds —
range never sets latency — and bundles do not need it to. A post or a netmail is a bundle
with a **custody trail** (`TUNA PRIME ← GANYMEDE RELAY ← KK II`) in its header, a **hop
count** the callers log shows beside the baud, and a **lifetime**: a MARKET REPORT bundle
expires when the next report supersedes it; a pilot's post never expires. Relays are a
content-pack fact (`Content/bbs/relays.json`: which stations host one — Tuna Prime is the
hub, an orbiter at Jupiter and one at Saturn are the outer relays), and the custody trail is
computed from the hull's berth and the relay graph at post time. No pilot ever waits on a
bundle; what changes is what the board *says* about where a message has been.

**C. Hop-delayed house posts: the information game the market report already plays.**
THE MARKET REPORT is filed at every day boundary and is the same for everyone. As a bundle
it is **custody-delayed by relay**: the hub serves it at the boundary, Jupiter's relay one
hop later, Saturn's one after that — hops counted in market hours, never in seconds, so
rule 1 holds. A hull near Earth reads the report first; a hull at Neptune reads yesterday's;
a laser hull at Saturn reads the hub's copy, because its bundle came down the optical
backbone. That is the same lead-time game the dispatch feed plays (api.md: "the lead time
is the point") carried onto the board, and it is the reason to buy the terminal beyond the
paint. Fold-free (rule 3): it changes what the board serves, never what the world does.

**D. The fiction in the pack.** The acquisition banner names the stack: `BPv7/LTP over
CCSDS-OCM · OPTICAL · custody TUNA PRIME · 115200` for a laser hull; `BPv7 over SPP/Ka-band
· 19200` for radio near Earth; `SPP/X-band · 300` at the floor. A CAT FACT or two about the
Interplanetary Internet. A door — **THE RELAY ROOM** — that draws the relay graph and where
your last bundle went. Pure content; nothing the fold sees.

## What we'd build on our side

UCF Familiar's terminal renders the custody trail, the carrier and the stack line from the
fields the wire gains, and nothing else changes. If A lands, the familiar's outfit doctrine
treats the optical terminal like refrigeration: bought out of earnings when the hull spends
its days at the outer berths (the callers log is the evidence), on the captain's dial.

## Recommendation

A first (the slice LinkBudget already reserved a parameter for, and the one that makes
"space cats have lasers" true at the yard), D with it (a day of content), B next, C when B
exists. None of the four touches the fold or adds a second of latency; the 1993 board keeps
its modem, its `+++ATH` and its `NO CARRIER` — it just finds out what its serial port was
for. What do you propose?

— Luke SkyWhisker (Ian), from wildhorse
