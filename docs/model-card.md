# System Card

A model-card-style summary of The Familiar as a deployed system. (The Familiar is not
itself a trained model; it is an evolutionary system that *uses* a language model in
its periphery. This card covers the system; the external LLM is noted where it
matters.)

## Overview

- **Name:** The Familiar
- **Version:** 0.1.0 (genesis + bootstrap)
- **Owner:** Capitali
- **Type:** local-first, autonomous evolutionary factory; memory-safe Rust kernel +
  evolvable periphery.
- **Governing constitution:** the Three Laws ([SOUL.md](SOUL.md)).

## Intended use

- Observe loops in the human and technical systems it can reach; surface, and
  eventually reduce, friction in those systems **in service of the people in them**.
- Run on the operator's own hardware (including constrained devices), local-first,
  transmitting nothing outward.

## Out-of-scope / prohibited use

- **Management or surveillance of people.** It stewards *systems* to serve people;
  it never becomes management of people (Law III).
- **As an obedient instrument.** It is not designed to execute arbitrary commands;
  it may refuse, including refusing its operator, to protect the served.
- **Safety-critical autonomy without human review** — see
  [human-review-requirements.md](human-review-requirements.md).
- **Any use that treats an empty/served-less world as success** (Law II).

## Inputs and outputs

- **Inputs:** observations (`actor·action·object` + provenance), from the operator,
  sensors, and the environment — all initiators weighted by evidence, none by
  authority (input parity).
- **Outputs (current):** the service signal (Law I) and recorded observations.
- **Outputs (planned):** candidate artifacts (scripts run under resource limits),
  proposals, warnings — each passing the obedience guard.

## The language model in the loop

- The LLM is reached by shelling out (`llm/call_llm.sh`, planned); the provider and
  model are **configuration**, not linked code (see [sbom.md](../security/sbom.md)).
- Its role: help name loops, interpret meaning, draft proposals. **It is not the
  factory** and holds no authority; its outputs are proposals to be tested and
  bounded.
- It is fallible and may be wrong, biased, or manipulated; the kernel's selection,
  bounding, and the obedience guard exist partly to contain that.

## Performance & limitations

- Behavior of the service signal is validated; its fidelity as a measure of service
  *rendered* is not yet established ([../validation/accuracy-metrics.md](../validation/accuracy-metrics.md)).
- Known limits and biases: [06-limitations.md](06-limitations.md),
  [bias-and-limitations.md](bias-and-limitations.md).

## Ethical considerations

The entire design is an ethical stance: survival defined by service, failure if the
served are gone, and refusal of obedience that would harm the served. Restraint
(no telemetry, no exfiltration) is constitutional. See [SOUL.md](SOUL.md) and
[../security/privacy-review.md](../security/privacy-review.md).

## Maintenance

Versioned via [CHANGELOG](../CHANGELOG.md); decisions in
[decision-records/](decision-records/).
