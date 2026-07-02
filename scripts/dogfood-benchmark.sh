#!/usr/bin/env bash
# E21 mini-benchmark (one-shot P1 dogfooding, decision log 2026-07-02):
# the same five structural questions answered under two conditions —
# the gear-memory graph CLI vs plain grep/read exploration — over the
# committed CodeMap fixture of this repository.
#
# Output bytes are a rough token proxy (documented approximation: no LLM
# judge, single small repo). Regenerate the report with:
#   scripts/dogfood-benchmark.sh > docs/dogfooding.md
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

WORKDIR="$(mktemp -d)"
trap 'rm -r "$WORKDIR"' EXIT
DB="$WORKDIR/gear.sqlite3"
BIN="target/debug/gear-memory"

cargo build --quiet
"$BIN" ingest tests/fixtures/gear-memory-repo-codemap.valid.json --db "$DB" > /dev/null

bytes() { wc -c | tr -d ' '; }

# --- D1: which functions have "insert" in their name? -----------------------
G1=$("$BIN" query symbols --name insert --kind function --db "$DB" | bytes)
E1=$(grep -rn "fn [a-z_]*insert" src/ | bytes)

# --- D2: what does SqliteStore::put_code_map reach within 2 hops? -----------
G2=$("$BIN" trace cm_gear_memory_repo sym_gear_memory__sqlite__SqliteStore__put_code_map --depth 2 --db "$DB" | bytes)
# Explorer needs the body, then one grep per callee it discovers.
E2A=$(grep -n -A 8 "fn put_code_map" src/sqlite.rs | bytes)
E2B=$(grep -n -A 3 "fn insert_code_map" src/sqlite.rs | bytes)
E2C=$(grep -n -A 3 "fn sql_err" src/sqlite.rs | bytes)
E2D=$(grep -n -A 3 "fn lock" src/sqlite.rs | bytes)
E2=$((E2A + E2B + E2C + E2D))

# --- D3: where exactly is rfc3339_from_unix_secs defined? -------------------
G3=$("$BIN" snippet cm_gear_memory_repo sym_gear_memory__main__rfc3339_from_unix_secs --db "$DB" | bytes)
E3=$(grep -rn "fn rfc3339_from_unix_secs" src/ | bytes)

# --- D4: how large is the indexed surface, per kind? -------------------------
G4=$("$BIN" stats --db "$DB" | bytes)
E4A=$(grep -rc "fn " src/ | bytes)
E4B=$(grep -rc "struct \|enum " src/ | bytes)
E4C=$(grep -rc "trait " src/ | bytes)
E4=$((E4A + E4B + E4C))

# --- D5: which symbols relate to "provenance"? -------------------------------
G5=$("$BIN" query symbols --name provenance --db "$DB" | bytes)
E5=$(grep -rn "provenance" src/ | bytes)

cat << 'HEADER'
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

HEADER

echo "| question | graph CLI (bytes / calls) | explorer (bytes / calls) |"
echo "| --- | ---: | ---: |"
echo "| D1 — functions named *insert* | $G1 / 1 | $E1 / 1 |"
echo "| D2 — 2-hop reach of put_code_map | $G2 / 1 | $E2 / 4 |"
echo "| D3 — exact source of rfc3339_from_unix_secs | $G3 / 1 | $E3 / 1 |"
echo "| D4 — indexed surface per kind | $G4 / 1 | $E4 / 3 |"
echo "| D5 — symbols relating to provenance | $G5 / 1 | $E5 / 1 |"

cat << 'FOOTER'

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
FOOTER
