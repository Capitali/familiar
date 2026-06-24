# Bias and Limitations

A responsible-AI accounting of where Substrate is biased, blind, or limited. This
complements the engineering-focused [06-limitations.md](06-limitations.md) with a
fairness/impact lens. Naming these is a precondition for trusting the system and for
not letting "it measures service" imply more than it does.

## Biases in the current system

- **Lexical / linguistic bias (service classifier).** "Served-facing" is detected by
  an English, somewhat business-flavored marker set (`client`, `customer`, `user`,
  …). It will under-recognize:
  - non-English terms and other cultures' words for people and relationships;
  - vernacular, kinship, and community terms not in the set;
  - **proper names** (people appear as names, not as the word "person").
  *Effect:* the people most legible to the classifier are those described in
  institutional/commercial language — a real equity concern, since the telos is
  about serving the *under*-served too. *Mitigation path:* entity resolution + learned,
  evidence-based classification (the world-model port), reducing reliance on a fixed
  lexicon.
- **Attention ≠ fulfillment bias.** The service measure rewards *observing* served
  facing activity, not *helping*. Left unsharpened, this could reward looking busy
  near people rather than actually serving them. *Mitigation:* fold in needs-reduced
  once the kernel lands ([evaluation-plan.md](evaluation-plan.md)).
- **Observability bias.** The factory can only serve what it can observe; quiet or
  invisible needs are under-weighted. Whoever/whatever is easiest to instrument gets
  more of its attention.
- **LLM inheritance.** The periphery LLM carries its own training biases into naming
  and interpretation; these can propagate into which loops get framed as worth
  addressing. The LLM holds no authority and its outputs are tested/bounded, which
  limits but does not erase this.

## Structural limitations

- **The observer is not humanity.** The system serves humanity-in-aggregate, not its
  operator; calibrating refusal vs. consent (Law III) in real situations is unproven
  and is the hardest open problem.
- **Proxies are lossy and gameable.** Service, presence, and "could this harm the
  served" are reduced to computable signals; each reduction can be optimized in
  unintended ways. The laws ([SOUL.md](SOUL.md)) remain the authority the signals
  only approximate.
- **Maturity.** Two of three law-signals and the whole metabolism are unbuilt; most
  claims are about design intent, not demonstrated behavior.

## Commitments

- Reduce lexicon dependence via learned, evidence-based served-identification.
- Measure and report classifier precision/recall across linguistic/cultural variation
  once labelled data exists.
- Keep this document current; a newly discovered bias is logged in
  [../validation/known-failures.md](../validation/known-failures.md) and, if it
  changes a decision, an [ADR](decision-records/).
