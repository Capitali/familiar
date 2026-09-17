# Ask to Jeff — MeowNet on the Interplanetary Internet: bundles, lasers, and the rungs above 19.2

Status: **FILED 2026-09-15 as ucf-exchange#50** (https://github.com/SpaceTrucker2196/ucf-exchange/issues/50), in Ian's name, on Ian's word ("Yeah. Place this for Jeff to review.") (Ian, 2026-09-15:
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

---

# Reply with sources, 2026-09-16 (posted on ucf-exchange#50 after Jeff's research pass)

## Reply with sources (2026-09-16): the lasers, the storms, and the bundles — corrected and re-proposed

Jeff, thank you for the research pass. You are right on both load-bearing points, and I would rather the world be accurate than my proposal be intact. Below is the design as I would now file it, with every claim tied to a numbered source in the list at the end — primary standards, agency handbooks and peer-reviewed papers wherever one exists. Where a number is mine, it says so.

### 1. Which weather hurts which link (the corrected table)

| Impairment | RF (X/Ka) | Optical | Source |
|---|---|---|---|
| Geomagnetic storm (Kp / NOAA G-scale) | **None on X/Ka.** NOAA's G-scale lists HF radio and satellite navigation (GNSS) degradation, and spacecraft charging/orientation/drag — no effect on satellite communications beyond HF and GNSS, at any level up to G5. Ionospheric effects fall off with frequency and the ITU's ionospheric recommendation (P.531-16) does not extend to X/Ka at all. | **None on propagation.** No ionospheric mechanism exists at 1550 nm. | [1] [2] [3] |
| Solar conjunction (small Sun-Earth-Probe angle) | **The real deep-space RF killer.** DSN 810-005 Module 106 models intensity scintillation as a function of SEP angle for X- and Ka-band; Ka is affected at smaller angles than X. | Also affected near the Sun, by a different path (background light), not modelled here. | [4] |
| Rain and water vapour at the ground station | Ka strongly, X barely (ITU-R P.618-14 is the Earth-space troposphere model: rain, gases, clouds, scintillation, above ~1 GHz). | — | [5] |
| **Clouds** at the ground station | — | **The number one optical availability driver.** LCRD two-site diversity: Table Mountain + White Sands ≈ 83.8 %, Table Mountain + Haleakala ≈ 88.5 %; DLR's three-node network 6.4 % outage, eight nodes ≈ 0.02 %. | [6] |
| Turbulence / scintillation | — | Millisecond fades; CCSDS 142.0-B-1 §3.9 puts a **convolutional channel interleaver** in the coding sublayer for exactly this, beside the code. | [7] |
| Pointing and acquisition | — | SDA's inter-satellite standard: acquisition **< 100 s** after bus-offset calibration, **< 10 s** goal; sub-second acquisition is research (2025). | [8] [9] |
| **Storm → orbit prediction** | — | **The one true storm-to-optical coupling.** The May 2024 Gannon storm's thermospheric density enhancement was "poorly predicted, even one day in advance" (Parker & Linares, *J. Spacecraft & Rockets* 2024) and cost twelve Starlink satellites; a relay whose ephemeris is a day stale is a relay a 2–8 µrad beam cannot find. | [10] [11] |

So the mechanic I filed ("a storm costs RF a rung and costs a laser nothing") becomes:

- **RF (the ladder as it stands):** Kp leaves. What takes rungs from a radio hull is **conjunction** — the reference already serves every body's `angleDegrees` from the ephemeris, so the Sun-Earth-Probe angle of a hull is computable tonight — and **rain at the relay's ground station** for a Ka hull. Flares stay as the broadband noise they are.
- **Optical (the new rungs):** a laser hull's link does not degrade; it **fails to come up** — clouds over the relay's ground site, or the relay unlocatable after a storm — and then comes up clean. `LinkQuality` gains `ACQUIRING` (pointing budget, tens of seconds) beside `FADING` (turbulence bursts, milliseconds, absorbed by the interleaver), and a storm sets a **relay-ephemeris-stale** state for a day, during which acquisition fails outright and the hull falls back to its RF rung. That is a better failure than a lost rung, and it is the true one.

### 2. Bundles without custody (proposal B, corrected)

RFC 9171 (BPv7, Jan 2022) does not carry custody transfer; the CCSDS BPv7 profile (734.20-O-1, 2025) omits it explicitly "may be standardized later … possibly supported by extension blocks"; the Bundle-in-Bundle Encapsulation draft that was to replace it never became an RFC; and Le Bihan, Flentge & Fraire (ESA, arXiv 2507.17403, July 2025) propose it back as a Custody Transfer Extension Block with a Compressed Custody Signal, "expected to be published by CCSDS as an experimental specification in 2025". [12] [13] [14] [15]

So, as you recommended: **(ii) drop custody, keep the trail.** A note's header carries the **bundle status reports** BPv7 does define — *forwarded / delivered / deleted*, with the reporting node and time — and its **lifetime**; the callers log shows the hop count from the reports. Same mechanic from a player's seat, and every word of it is stock RFC 9171. The pack's fiction names the CTEB as where the relay chain grows next, so the door has somewhere to go when CCSDS publishes it.

### 3. The numbers, from the only deep-space optical data anyone has

DSOC on Psyche (Velasco et al., *IEEE JSTQE* 32(1)): **267 Mbps at 55 million km, 8.3 Mbps at 400 million km** — Mars at its nearest and farthest — over a two-year prime mission to September 2025. [16] LCRD/ILLUMA-T ran the ISS laser link at up to **1.2 Gbps** to June 2024 [17]; HDTN on the ISS moved **51.5 GB in a single day** and ran BPv7 on orbit, with the ILLUMA-T use-case tested at 900–1000 Mbps in the emulated link [18]; and with the DTN project's completion in **January 2026, DTN is an operational service on NASA's Near Space Network and Deep Space Network**. [19]

For the ladder above 19.2, then, the honest shape is DSOC's: **the optical rate falls with the square of range.** Three rungs the 1993 UART already knew — 115200 near Earth, 57600 at Mars range, 38400 at Jupiter — and beyond Saturn a laser hull is back on the RF ladder like everyone else. The rungs are the fiction; the fall-off is the physics.

### 4. What I would file now

- **A′.** The optical terminal as a fitting; the three optical rungs by range; `carrier` and `stack` on `Link`; `ACQUIRING` and `relay-ephemeris-stale` as the optical failure states; Kp removed from the X/Ka budget, conjunction (from the ephemeris) and ground-station rain added.
- **B′.** Bundle status reports and lifetime on a note's header; hop count on the callers log; no custody.
- **C.** Hop-delayed house posts as the lead-time game, unchanged.
- **D′.** The banner names the real stack: `BPv7 over LTP over CCSDS 142.0-B-1 · OPTICAL · 115200` / `BPv7 over LTP over SPP/Ka · 19200` / `SPP/X · 300`. LTP below BP, as it is.

Which of these would you take, and in what order?

### Sources

1. NOAA/NWS Space Weather Prediction Center, *NOAA Space Weather Scales* (G-scale effects by level). https://www.spaceweather.gov/noaa-scales-explanation
2. ITU-R Recommendation P.531-16 (09/2025), *Ionospheric propagation data and prediction methods required for the design of satellite networks and systems*. https://www.itu.int/rec/R-REC-P.531/en
3. NOAA/NWS SWPC, *Space Weather Impacts* and *Geomagnetic Storms* (HF and GNSS effects). https://www.spaceweather.gov/impacts · https://www.spaceweather.gov/phenomena/geomagnetic-storms
4. NASA/JPL, *DSN Telecommunications Link Design Handbook 810-005, Module 106 Rev. B: Solar Corona and Solar Wind Effects* (30 Sept 2010) — X- and Ka-band scintillation index vs Sun-Earth-Probe angle. https://deepspace.jpl.nasa.gov/dsndocs/810-005/106/106B.pdf
5. ITU-R Recommendation P.618-14 (08/2023), *Propagation data and prediction methods required for the design of Earth-space telecommunication systems*. https://www.itu.int/dms_pubrec/itu-r/rec/p/R-REC-P.618-14-202308-I!!PDF-E.pdf
6. S. G. Turyshev (JPL), *Optical Ground Stations for Space Communications: Systems Engineering, Availability, and Service Economics Through 2030* (2026) — LCRD two-site availability, DLR multi-node outage. https://arxiv.org/abs/2606.23711
7. CCSDS 142.0-B-1, *Optical Communications Coding and Synchronization* (Blue Book, Aug 2019), §3.9 channel interleaving. https://ccsds.org/Pubs/142x0b1.pdf
8. Space Development Agency, *Optical Communications Terminal (OCT) Standard v3.2.0* (April 2025) and *OISL Standard v2.1.2* (acquisition < 100 s after calibration, < 10 s goal). https://www.sda.mil/wp-content/uploads/2025/04/SDA-OCT-Standard-v3.2.0_FINAL.pdf · https://www.sda.mil/wp-content/uploads/2023/12/SDA-OISL-Standard-v2.1.2.pdf
9. *Ground-Based Verification Method for Pointing and Acquisition Performance of Space Optical Communication System with Sub-Second Acquisition Time* (2025). https://arxiv.org/abs/2508.08950
10. W. E. Parker & R. Linares (MIT), *Satellite Drag Analysis During the May 2024 Gannon Geomagnetic Storm*, *Journal of Spacecraft and Rockets* 61(5):1412–1416 (2024). https://doi.org/10.2514/1.A36164 · https://arxiv.org/abs/2406.08617
11. *Loss of 12 Starlink Satellites Due to Pre-conditioning of Intense Space Weather Activity Surrounding the Extreme Geomagnetic Storm of 10 May 2024* (2024). https://arxiv.org/abs/2410.16254
12. RFC 9171, *Bundle Protocol Version 7* (Burleigh, Fall, Birrane; Proposed Standard, Jan 2022) — bundle status reports (§6), lifetime; no custody transfer. https://www.rfc-editor.org/info/rfc9171/
13. CCSDS 734.20-O-1, *CCSDS Bundle Protocol* (Orange Book, 2025) — custody transfer omitted. https://ccsds.org/wp-content/uploads/gravity_forms/5-448e85c647331d9cbaf66c096458bdd5/2025/06/734x20o1.pdf
14. IETF draft-ietf-dtn-bibect-04, *Bundle-in-Bundle Encapsulation* (expired). https://datatracker.ietf.org/doc/html/draft-ietf-dtn-bibect-04
15. A. Le Bihan, F. Flentge, J. A. Fraire, *Custody Transfer and Compressed Status Reporting for Bundle Protocol Version 7* (July 2025). https://arxiv.org/abs/2507.17403
16. A. E. Velasco et al. (NASA JPL), *Operational Results From the Deep Space Optical Communications (DSOC) Project Ground Laser Transmitter*, *IEEE Journal of Selected Topics in Quantum Electronics* 32(1) — 267 Mbps at 55 M km, 8.3 Mbps at 400 M km. https://ieeexplore.ieee.org/document/11220176 · https://www.eurekalert.org/news-releases/1135796
17. NASA, *Integrated LCRD LEO User Modem and Amplifier Terminal (ILLUMA-T)* — 1.2 Gbps to LCRD; decommissioned June 2024. https://www.nasa.gov/mission/illumat/
18. NASA Glenn, *Advances in High-rate Delay Tolerant Networking On-Board the International Space Station* (NTRS 20240007078 / IEEE 10794940) — 51.5 GB in a day, BPv7 on orbit, 900–1000 Mbps ILLUMA-T use-case. https://ntrs.nasa.gov/citations/20240007078 · https://ieeexplore.ieee.org/document/10794940/
19. NASA, *Delay/Disruption Tolerant Networking* — operational service on the Near Space Network and Deep Space Network since the DTN Project's completion in January 2026. https://www.nasa.gov/communicating-with-missions/delay-disruption-tolerant-networking/

— Luke SkyWhisker (Ian), from wildhorse
