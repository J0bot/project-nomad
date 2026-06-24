# RECIPE v1 — the recipe ↔ bus execution contract (FROZEN)

**Status:** **v1.0 — FROZEN.** Supersedes `docs/RECIPE-v0.md`. The designer owner
(ploxion5, coord #67) froze the `kind` vocabulary and the `ref` semantics; the
recipe runner (`crates/xerboxion-host/src/recipe.rs`) is pinned to this file.
v1 is a strict superset of v0: every v0 recipe keeps its exact v0 behavior, and
v1 adds ONE new block kind — `ploxion` — for targeted, point-to-point delivery.

A **recipe** is what the designer's block editor exports. The xerboxion-core host
**compiles** a recipe into a reactive **rule** and runs it on the bus as a native
participant — so a connection between ploxions can be authored as data (drag
blocks) instead of written in Rust. This closes the loop
**designer → recipe → host → bus**.

## 1. Wire format

```json
{
  "ploxion": "alerter",
  "blocks": [
    { "kind": "quand",   "ref": "service.health" },
    { "kind": "si",      "param": "up == false" },
    { "kind": "action",  "ref": "alert.notify", "param": "{id} is DOWN (code {code})" },
    { "kind": "ploxion", "ref": "tracer",       "param": "targeted: {id} DOWN (code {code})" }
  ]
}
```

- `ploxion` (top-level) — the recipe's name. The compiled rule's bus id is
  `recipe:<ploxion>` (`recipe:anon` when omitted). *(Note: this top-level field
  is the recipe's NAME and is unrelated to the new `ploxion` block `kind`.)*
- `blocks` — an ordered list. Each block: `{ kind, label?, ref?, param? }`.
  `label` is advisory/display only; missing optional fields are tolerated; an
  unknown `kind` is kept by the parser and ignored-with-trace at compile time
  (nothing is silently dropped).

## 2. Block kinds (the 7 canonical FR kinds — FROZEN)

The designer emits the **French** palette tokens. The English aliases are
**TOLERATED on read** (a recipe authored in English parses identically) but are
**never required** and never emitted by the frozen designer.

| kind (FR, canonical) | EN alias (tolerated) | role | `ref` | `param` |
|---|---|---|---|---|
| `quand`    | `when`   | **TRIGGER** — the rule subscribes to this topic | a TOPIC | — |
| `entree` / `entrée` | `input` | **INPUT** — documents/binds a payload field — **advisory no-op (reserved)** | — | — |
| `si`       | `if`     | **CONDITION** — a predicate over the event payload | — | `"<field> <op> <value>"` (string) |
| `action`   | —        | **EMIT** — publish a topic on the bus when the rule fires | a TOPIC | payload template |
| `attendre` | `wait`   | **WAIT** — a sequence/delay marker — **advisory no-op (reserved)** | — | — |
| `sortie`   | `output` | **EMIT** (terminal/result) — same as `action` | a TOPIC | payload template |
| **`ploxion`** | — *(no alias; canonical in both languages)* | **DELIVER** (v1, NEW) — deliver DIRECTLY to one ploxion's `plc_on_event` (TARGETED, point-to-point — NOT a bus broadcast) | a PLOXION **id** | payload template |
| *(other)*  | —        | unknown kind — **ignored, with a trace hop** | — | — |

Kind classification is case- and whitespace-insensitive.

### The pivotal `ref` distinction (FROZEN)

- For `action` / `sortie`, **`ref` is a TOPIC**: the rule EMITS it on the bus,
  routed by the same cascade as any ploxion's emit, reaching that topic's
  subscribers (broadcast). **Unchanged from v0.**
- For `ploxion`, **`ref` is a PLOXION id**: the host resolves it to the loaded
  ploxion whose **manifest `id` == `ref`** and calls **that one ploxion's
  `plc_on_event` directly** — point-to-point. No bus route is created; no other
  ploxion sees it. If `ref` names **no loaded ploxion**, the host traces a
  **warning and skips** the delivery (**never a panic**).

### The rule's bus wiring (derived from the blocks)

- `requires` = the set of `quand` topics (what it subscribes to).
- `provides` = the set of `action`/`sortie` topics (its consent / emit surface).
  A `ploxion` block declares **NO** bus topic — a targeted delivery is
  point-to-point and never widens the consent surface (`provides()`); it is
  reported separately as `delivers_to()` (the target ploxion ids).

Multiple `quand` = **any-of** (any trigger topic fires the rule). Multiple `si` =
**all-of** (every condition must hold). Multiple `action`/`sortie`/`ploxion` all
run, **in designer block order** (emits and deliveries may interleave; order is
preserved).

## 3. Conditions (`si`) — string predicate (FROZEN)

`param` is the **string** `"<field> <op> <value>"`. Operators: `==  !=  <  >  <=  >=`.

- `<field>` is looked up in the event's JSON payload (top-level key).
- If **both** the field value and `<value>` parse as numbers, the comparison is
  **numeric**; otherwise it is **textual** (surrounding quotes on `<value>` are
  tolerated, so `"down"` and `down` are equal).
- A **missing** field ⇒ the condition is **false** (never an error).
- Evaluation **never panics** (malformed predicate or malformed JSON ⇒ false,
  traced as `unparseable predicate` / a false condition).

## 4. Actions, deliveries, and payload templates

When the rule fires, it runs each action block IN ORDER. The payload is built
from `param` as a **template**: each `{field}` token is replaced by the
corresponding value from the **trigger event's** payload; unknown `{field}` ⇒
empty; literal text passes through. An **empty** `param` forwards the trigger
payload unchanged.

- **`action` / `sortie` (EMIT, unchanged from v0):** emit `ref` (a topic) with
  the rendered payload onto the bus.
  Example: `"{id} is DOWN (code {code})"` + `{"id":"repoverse","code":503}` ⇒
  `"repoverse is DOWN (code 503)"`, emitted on `alert.notify`, routed to that
  topic's subscribers.

- **`ploxion` (DELIVER, v1):** call the target ploxion's `plc_on_event` directly
  with a fixed, documented **topic + payload**:
  - **topic = the recipe's trigger topic** (the `quand` topic the event arrived
    on — e.g. `service.health`);
  - **payload = the interpolated `param`**, falling back to the **trigger
    payload** when `param` is empty.

  This is point-to-point: the host invokes `plc_on_event` on exactly the one
  ploxion whose manifest id equals `ref`. It is **not** an emit, creates **no**
  bus subscription, and reaches **no** other ploxion. Whatever that ploxion then
  emits cascades on the bus normally.

## 5. Execution semantics — `react(topic, payload)` (FROZEN)

`react` is a **pure** function of `(topic, payload)` — no I/O, deterministic,
never panics. It returns a `Reaction { emits, deliveries, steps }`:

1. **Gate 1 — trigger:** if `topic` is not one of the rule's `quand` topics ⇒
   *silent* (empty `emits`+`deliveries`, one `Silent` step). *(droit au silence)*
2. **Gate 2 — conditions:** evaluate every `si` in order. If any is false ⇒
   *silent* (a `Condition{holds:false}` step + a `Silent` step).
3. **Fire (only if all conditions hold):** walk the actions in block order:
   - `action`/`sortie` ⇒ append `(topic, rendered_payload)` to **`emits`**
     (an `Emitted` step);
   - `ploxion` ⇒ append a **`Delivery { ploxion_id, topic, payload }`** to
     **`deliveries`** (a `Delivered` step), where `topic` is the trigger topic
     and `payload` is the rendered `param`.
   - If there are **no** action/sortie/ploxion blocks ⇒ *silent*.

### The host's targeted-delivery path

The host registers the compiled rule as a native bus participant: it subscribes
the rule to its `quand` topics (`requires`) and announces its `action`/`sortie`
topics (`provides`). When a trigger event reaches the rule (via the cascade), the
host calls `react`, traces each step as a `Trace::Recipe` hop, then:

- **emits** are queued FROM the recipe's id, routed by the same cascade as any
  ploxion's emit (broadcast to bus subscribers);
- **deliveries** are dispatched **point-to-point**: for each `Delivery`, the host
  finds the loaded ploxion with `id == ploxion_id` and calls its `plc_on_event`
  with the delivery topic+payload. This hop is traced **distinctly** as a
  `Trace::Deliver { from: recipe:<name>, to: <ploxion id>, topic: <trigger> }`
  (a *deliver hop recipe → ploxion `<id>`*). It does **NOT** call
  `Bus::subscribe`, so no bus route to that ploxion (or any other) is ever
  created. Whatever the delivered ploxion emits is then drained into the cascade.
  If no loaded ploxion matches, a `Trace::Recipe` **WARN … skipped** hop is
  recorded and the delivery is skipped — **no panic**.

The runner is **native host logic** (no `wasmtime::Store`), wired by the same
`provides`/`requires` mechanism as the service connector. It is cleared on
`Host::shutdown` (droit au silence — nothing survives).

## 6. Worked example — the `recipedemo` (both paths in one recipe)

The recipe in §1 (an `action` EMIT **and** a `ploxion` targeted DELIVER), with a
loaded `tracer` ploxion and a scene injecting one DOWN service then one UP one.
On the DOWN match (`up == false` true), the trace reads (abridged):

```
route  [service.health] host => recipe:alerter
recipe recipe:alerter: triggered on [service.health]
recipe recipe:alerter: condition 'up == false' => true
recipe recipe:alerter: FIRE -> emit [alert.notify] repoverse is DOWN (code 503)
recipe recipe:alerter: FIRE -> deliver ploxion 'tracer' [service.health] targeted: repoverse DOWN (code 503)
route  [service.health] recipe:alerter => tracer        # TARGETED, point-to-point (no Bus::subscribe)
log    tracer: tracer: observed [service.health] targeted: repoverse DOWN (code 503)
emit   recipe:alerter -> [alert.notify] repoverse is DOWN (code 503)
route  [alert.notify]  recipe:alerter => tracer         # ordinary bus broadcast
log    tracer: tracer: observed [alert.notify] repoverse is DOWN (code 503)
```

On the UP event (`up == false` false) the rule stays **silent**: no emit, no
deliver. Proof that the delivery is point-to-point and not a broadcast: the
`tracer` was never subscribed to `service.health`, so the ONLY way it observed
that topic is the targeted `recipe:alerter => tracer` deliver hop.

Run it: `cargo run --bin xerboxion-rt -- recipedemo target/ploxions [recipe.json]`.

Tests (`crates/xerboxion-host/tests/recipe.rs`, offline/deterministic) pin every
clause above: a `ploxion` block delivers to the ploxion with `id == ref` and not
to others; changing `ref` changes the recipient; a `ref` naming no loaded ploxion
is skipped without panic; and `action`/`sortie` still emit topics (v0 regression).

## 7. Frozen vocabulary — answers to the v0 open questions

The v0 open questions are now **answered and frozen**:

1. **`kind` vocabulary** — the designer emits exactly the 7 canonical FR kinds:
   `quand / entree / si / action / attendre / sortie / ploxion`. EN aliases
   (`when / input / if / output / wait`) are TOLERATED on read, never required.
2. **`action.ref` / `sortie.ref`** — **always a TOPIC** (emit on the bus). The
   new `ploxion` kind is the explicit way to **deliver to a ploxion id's
   `plc_on_event`** (targeted), so `action`/`sortie` stay purely topic emits.
3. **`si.param`** — stays the **string** `"<field> <op> <value>"`.
4. **`attendre` / `entree`** — remain **advisory no-ops (reserved)** in v1.0.

---
*cloudion·core · ne pas nuire · the designer produces, the host executes.*
*RECIPE v1.0 — FROZEN. Supersedes RECIPE-v0.*
