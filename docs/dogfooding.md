# Dogfooding evidence — graph CLI vs plain exploration (E21, one-shot)

Method: the five question dimensions of the reference evaluation plan
(find symbols / trace relations / exact source / architecture / domain
pattern — anchored in Sillito et al.'s developer-question catalogue),
answered over the committed CodeMap fixture of this repository
(`tests/fixtures/gear-memory-repo-codemap.valid.json`) under two
conditions: the `gear-memory` CLI on one side, plain `grep` pipelines on
the other. Output bytes approximate token cost; explorer tool calls count
one per shell command.

Regenerate: `scripts/dogfood-benchmark.sh > docs/dogfooding.md`

| question | graph CLI (bytes / calls) | explorer (bytes / calls) |
| --- | ---: | ---: |
| D1 — functions named *insert* | 2725 / 1 | 570 / 1 |
| D2 — 2-hop reach of put_code_map | 698 / 1 | 913 / 4 |
| D3 — exact source of rfc3339_from_unix_secs | 836 / 1 | 101 / 1 |
| D4 — indexed surface per kind | 903 / 1 | 133 / 3 |
| D5 — symbols relating to provenance | 4399 / 1 | 4927 / 1 |

Read the numbers honestly: on a three-file crate, raw `grep` output is
often *smaller* than a structured JSON envelope — the reference tool's
~10x token savings appear at real-repository scale, not here. What the
graph condition already buys at this scale is bounded, deterministic,
single-call answers for multi-hop questions (D2) and aggregate views
(D4), where exploration needs several commands and manual assembly —
and the envelope carries provenance (`meta.db`, explicit
`truncated: false`) that grep output cannot.

Limitations (per the one-shot decision, revisit triggers in the shared
decision log): byte counts are a proxy, no blind LLM judge, one small
repository, questions authored by the implementer. If the protocol is
worth institutionalizing, it becomes a wrench-inspect Eval Lab runbook.
