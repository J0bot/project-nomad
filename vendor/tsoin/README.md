# tsoin — Adressage Génératif engine v0

Store **addresses, not content**. A *tsoin* is the **residue**: the exact
bit-level difference between what a pure deterministic generator predicts and the
real data — the Shannon surprise, the only thing worth storing.

```
address = (generator_hash, coordinates, residue_hash, len)
read    = run generator(coordinates) then apply residue   (bit-exact)
write   = note the address
fork    = free (branches share all past + all unvisited space)
```

Two limit cases fall out for free:

- generator = `Identity` (predicts zeros) ⇒ residue == data == **classic storage**;
- a perfect procedural generator ⇒ residue ≈ empty == **pure generation**.

Between those limits sits an **adaptive generator**: a deterministic predictive
**model** whose knowledge is *built from the data's own past*. For text/code the
model is the right kind of knowledge — the residue (its entropy-coded prediction
errors) collapses toward the data's true entropy. On random data the model learns
nothing and the residue stays `~=` raw: **no free lunch, by construction**.

## Layers (synthesis §4)

- `store` — content-addressed blob store (BLAKE3). *Git généralisé / Merkle.*
- `generator` — pure deterministic predictors (the "software CFC"). WASM is the
  long-term ideal; v0 uses Rust generators behind the `Generator` trait.
- `address` — the triplet, the NEW primitive, proposed FROZEN in
  [`docs/SPEC-v0.md`](docs/SPEC-v0.md).
- `codec` — `encode` / `decode` (bit-exact) and `encode_best` (cheapest generator).
- `model` — the **text/byte domain generator**: a deterministic, adaptive
  context-mixing model (orders {0,1,2,3,4,6} blended by an online logistic mixer +
  SSE/APM stage) over a from-scratch binary **range coder**. `model::encode(x)` →
  entropy-coded residue; `model::decode(blob, len)` → `x` bit-for-bit. The model
  *is* the generator (id `ctx-order2-v0`).
- `model_codec` — the model wired in as a codec path: `encode(x, store) -> Address`
  whose `generator_hash` is the model id, `residue_hash` points at the coded blob;
  `decode` transparently routes a model address through the model and any other
  address through `codec::decode`. `encode_best` includes it as a candidate.
- `branch` — journal + free forks.
- `timeline` — the **temporal tsoin recorder**: a branchable Merkle timeline of
  states where each frame stores only the **delta from the previous frame** (the
  surprise between moments). *Le B0XION = le git des états.* Format proposed
  FROZEN in [`docs/TIMELINE-v0.md`](docs/TIMELINE-v0.md).
- `measure` — honest residue measurements.

## Run

```sh
cargo test                      # round-trip, fork-is-free, corruption, timeline replay/fork/xerboxion, CLI
cargo clippy --all-targets -- -D warnings
cargo run --release --bin tsoin-demo            # prints the measurement table
cargo run --release --bin tsoin-timeline-demo   # temporal recorder: store-vs-naive, fork, xerboxion
```

## `tsoin` CLI — git-of-states from the shell

The `tsoin` binary makes the temporal recorder **usable**: record any file or
directory, replay any old state **bit-exact**, fork timelines for free, and watch
the store collapse — all persisted under `.tsoin/` so it works across separate
command invocations. See [`docs/CLI-v0.md`](docs/CLI-v0.md) for the on-disk
format and `scripts/demo_cli.sh` for the end-to-end flow.

```sh
cargo build --bin tsoin
T=tsoin                                  # = target/debug/tsoin
$T init                                  # create .tsoin/ in CWD
$T record notes.txt -m "first"           # snapshot a file (or a directory)
vim notes.txt && $T record notes.txt -m "edit"
$T log                                    # states newest-first, git-log style
$T replay <state-id> --out old.txt        # reconstruct an old state bit-exact
$T fork experiment <state-id>             # free fork (shares all prior blobs)
$T checkout experiment                    # switch branch
$T record notes.txt -m "diverge"
$T stats                                  # store on disk vs naive, ratio, counts
$T xerboxions                             # identical-state coincidences
bash scripts/demo_cli.sh                  # the whole flow, asserting bit-exactness
```

## What the measurements show (honest — no faked compression)

From `cargo run --release --bin tsoin-demo` (`resid_cost` = the residue's real
stored cost: `zstd(residue)` for a pure XOR generator, the coded length for the
already-entropy-coded model; `zstd(raw)` is plain `zstd -19` of the raw bytes):

| corpus          | raw_len | best_generator | residue | resid_cost | zstd(raw) | resid/raw |
|-----------------|---------|----------------|---------|------------|-----------|-----------|
| random          |    8192 | constant:0x00  |    8192 |       8201 |      8201 |  100.11%  |
| gradient_img    |    4096 | gradient2d     |    4096 |         17 |       178 |    0.42%  |
| smooth_ramp     |    8192 | linear         |    8192 |         17 |       275 |    0.21%  |
| src_lib.rs      |    2457 | ctx-order2-v0  |    1142 |       1142 |      1189 |   46.48%  |
| src_model.rs    |   23799 | ctx-order2-v0  |    7547 |       7547 |      7941 |   31.71%  |
| src_timeline.rs |   23916 | ctx-order2-v0  |    6943 |       6943 |      7089 |   29.03%  |
| Cargo.lock      |   11790 | ctx-order2-v0  |    3063 |       3063 |      3080 |   25.98%  |
| braindump.md    |   13404 | ctx-order2-v0  |    5475 |       5475 |      6048 |   40.85%  |

- **random**: residue stays `~=` raw (a hair larger), nothing shrinks it →
  **no free lunch** (the model is honest — it cannot compress incompressible data).
- **gradient_img / smooth_ramp**: data matches a pure generator → residue collapses
  to ~0 (the procedural limit); `encode_best` picks the cheap pure generator.
- **real text/code** (`src_*.rs`, `Cargo.lock`, a real `braindump.md`): the
  **model is the cheapest generator**. Its entropy-coded residue is well below the
  raw bytes *and* beats `zstd -19` on every one of these files — the model **is**
  the knowledge, and the residue is the only thing stored. Every row round-trips
  **bit-for-bit**.

## Status

This is **P0** (triplet spec) + **P1** (round-trip codec) + **P2** (timeline) of
the build path, now extended with a **text/byte domain generator** (the adaptive
model + range coder, the first non-trivial domain generator). Future work: WASM
generators, Hilbert-4D coords, full Merkle-DAG journal with merge, a learned/LLM
generator stronger than the order-mixed model, model-coded timeline deltas, zoom
navigation, and the separate **P3** CRIU machine-snapshot palier.

Licensed under MIT OR Apache-2.0.
