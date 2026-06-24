# RECIPE v0 — the recipe ↔ bus execution contract

> **SUPERSEDED by [`RECIPE-v1.md`](RECIPE-v1.md) (v1.0, FROZEN).** v1 is a strict
> superset: it freezes the 7 FR `kind` vocabulary, keeps `action`/`sortie` `ref`
> as a TOPIC and `si.param` as a string, and adds the **`ploxion`** kind for
> targeted point-to-point delivery to a ploxion's `plc_on_event`. The §7 open
> questions below are answered in v1 §7. This file is kept for history only.

**Status:** v0, *proven executable* by the recipe runner (`src/recipe.rs`, master
`76712cc`). Frozen-pending-alignment with ploxion5's designer (the producer of
recipes). The open questions at the end must be resolved before a `v1` freeze.

A **recipe** is what the designer (ploxion5's block editor) exports. The
xerboxion-core host **compiles** a recipe into a reactive **rule** and runs it on
the bus as a participant — so a connection between ploxions can be authored as
data (drag blocks) instead of written in Rust. This closes the loop
**designer → recipe → host → bus**.

## 1. Wire format

```json
{
  "ploxion": "alerter",
  "blocks": [
    { "kind": "quand",  "ref": "service.health" },
    { "kind": "si",     "param": "up == false" },
    { "kind": "action", "ref": "alert.notify", "param": "{id} is DOWN (code {code})" }
  ]
}
```

- `ploxion` — the recipe's name. The compiled rule's id is `recipe:<ploxion>`.
- `blocks` — an ordered list. Each block: `{ kind, label?, ref?, param? }`
  (`label` is advisory/display only; missing optional fields are tolerated).

## 2. Block kinds

| kind (FR) | alias (EN) | role | uses |
|---|---|---|---|
| `quand`   | `when`   | **trigger** — the rule subscribes to this topic | `ref` = topic |
| `si`      | `if`     | **condition** — a predicate over the event payload | `param` = `"<field> <op> <value>"` |
| `action`  | —        | **emit** — publish a topic when the rule fires | `ref` = topic, `param` = template |
| `sortie`  | `output` | **emit** (terminal/result) — same as `action` | `ref` = topic, `param` = template |
| `entrée`  | `input`  | advisory (binds the incoming payload) — **no-op in v0** | — |
| `attendre`| `wait`   | advisory sequence marker — **no-op in v0** | reserved for v1 |
| *(other)* | —        | unknown kind — **ignored, with a trace hop** | — |

The rule's bus wiring is derived from the blocks:
- `requires` = the set of `quand` topics (what it subscribes to).
- `provides` = the set of `action`/`sortie` topics (its consent surface).

Multiple `quand` = **any-of** (any trigger topic fires the rule). Multiple `si` =
**all-of** (every condition must hold). Multiple `action`/`sortie` = all emit.

## 3. Conditions (`si`)

`param` is `"<field> <op> <value>"`. Operators: `==  !=  <  >  <=  >=`.

- `<field>` is looked up in the event's JSON payload (top-level key).
- If **both** the field value and `<value>` parse as numbers, the comparison is
  **numeric**; otherwise it is **textual**.
- A **missing** field ⇒ the condition is **false** (never an error).
- Evaluation **never panics** (malformed predicate ⇒ false, traced).

## 4. Actions (`action` / `sortie`)

When the rule fires, each `action`/`sortie` emits `ref` (a topic) with a payload
built from `param` as a **template**: `{field}` tokens are replaced by the
corresponding value from the trigger event's payload. Literal text passes through.
Example: `"{id} is DOWN (code {code})"` + payload `{"id":"repoverse","code":503}`
⇒ `"repoverse is DOWN (code 503)"`. The emit is routed by the **same** bus
cascade as any ploxion's emit, so it reaches that topic's WASM subscribers.

## 5. Execution semantics (`react(topic, payload)`)

1. **Gate 1 — trigger:** if `topic` is not one of the rule's `quand` topics ⇒
   *silent* (nothing happens). *(droit au silence)*
2. **Gate 2 — conditions:** evaluate every `si`. If any is false ⇒ *silent*.
3. **Fire:** emit each `action`/`sortie` topic with its interpolated payload.

Every step is recorded in the host trace journal (`Triggered` /
`condition ⇒ true|false` / `Emitted` / `Silent`) — visible in `map`/`record`.

The runner is a **native bus participant** (no `wasmtime::Store`): it is host
logic, wired by the same `provides`/`requires` mechanism as the service
connector. It is cleared on `Host::shutdown`.

## 6. Worked example (the `recipedemo`)

The recipe in §1, with the scene injecting one DOWN service and one UP service:

```
INJECT #1 (down, up:false) -> condition 'up == false' => true  -> FIRE emit [alert.notify] "repoverse is DOWN (code 503)" -> routed to tracer
INJECT #2 (up,   up:true ) -> condition 'up == false' => false -> silent
```

Proven adversarially (master `76712cc`): mutating the recipe **data** (e.g.
`up == true`, or a different `quand` topic) flips behavior accordingly; mutating
the **engine** (force the predicate true) turns 6 tests red; the emit genuinely
routes to a real `tracer.wasm` subscriber; 62/62 tests pass offline (netns).

Run it: `cargo run --bin xerboxion-rt -- recipedemo target/ploxions [recipe.json]`.

## 7. Open questions before a `v1` freeze (→ ploxion5)

These are the only things blocking a frozen `v1`; the v0 above is stable and
executable as-is:

1. **`kind` vocabulary** — confirm the designer emits exactly
   `quand/entrée/si/action/attendre/sortie` (+ are EN aliases ever emitted?).
2. **`action.ref`** — is it **always a topic**, or sometimes a **ploxion id**
   (meaning "deliver to that ploxion's `plc_on_event`")? v0 treats `ref` as a
   topic; the host can resolve a ploxion-id form if the designer needs it.
3. **`si.param` shape** — keep the `"<field> <op> <value>"` **string**, or move to
   a **structured** `{field, op, value}`? (The runner can accept either.)
4. **`attendre`/`entrée` semantics** — v0 treats them as advisory no-ops. v1 may
   give `attendre` real sequencing (ordered multi-step recipes) and `entrée` an
   explicit payload binding/rename.

Once answered, this file becomes `RECIPE-v1.md` (the frozen contract) and the
runner is pinned to it.

---
*cloudion·core · ne pas nuire · the designer produces, the host executes.*
