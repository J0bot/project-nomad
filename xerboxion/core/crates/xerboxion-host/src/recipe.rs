//! # Recipe runner — the host EXECUTES a designer recipe on the bus.
//!
//! This closes the triangle: a **designer** (ploxion5, coord #67) produces a
//! recipe → the **host** compiles + executes it → the **bus** connects the
//! ploxions. A recipe is the no-code description of a *reactive rule*; the host
//! turns it into a **native bus participant** (the same mechanism as the
//! [`ServiceConnector`](crate::connector::ServiceConnector)) so it plays on the
//! bus exactly like a ploxion: it `requires` the topics it triggers on, it
//! `provides` (consents to emit) the topics it acts on, and the host routes +
//! traces every hop.
//!
//! ## The designer export shape (JSON)
//!
//! ```json
//! {
//!   "ploxion": "alerter",
//!   "blocks": [
//!     {"kind": "quand",   "ref": "service.health"},
//!     {"kind": "si",      "param": "up == false"},
//!     {"kind": "action",  "ref": "alert.notify", "param": "{id} is DOWN (code {code})"},
//!     {"kind": "ploxion", "ref": "tracer",       "param": "targeted: {id}"}
//!   ]
//! }
//! ```
//!
//! Block kinds come from the designer's French palette (the 7 canonical FR kinds,
//! FROZEN in RECIPE v1 — see `docs/RECIPE-v1.md`); the english aliases are
//! TOLERATED on read so an english recipe parses identically, never required:
//!
//! | French    | english | role |
//! |-----------|---------|------|
//! | `quand`   | `when`  | TRIGGER — a bus topic the rule subscribes to (`ref`) |
//! | `entrée`  | `input` | INPUT — documents/binds a payload field (advisory) |
//! | `si`      | `if`    | CONDITION — a simple predicate over the trigger payload |
//! | `action`  | —       | EMIT — a topic to emit (`ref`) with a payload (`param`) |
//! | `sortie`  | `output`| EMIT — an output topic to emit (`ref`) with a payload  |
//! | `attendre`| `wait`  | WAIT — a delay marker (advisory; traced, no real sleep) |
//! | `ploxion` | —       | DELIVER (v1) — `ref` = a PLOXION id; deliver DIRECTLY to that ploxion's `plc_on_event` (TARGETED, point-to-point — NOT a bus broadcast) |
//!
//! ## Execution model (droit au silence)
//!
//! The recipe fires **only** when (a) the arriving event is on a `quand` topic
//! AND (b) every `si` condition holds against the payload. If the trigger does
//! not match or a condition is false, the recipe emits **nothing** — the right
//! to silence. Every step (trigger received, condition true/false, emit) is
//! traced in the host journal.

use std::collections::BTreeSet;

use serde::Deserialize;

/// The participant id prefix a compiled recipe announces itself as on the bus.
/// The full id is `recipe:<ploxion>` (or `recipe:<n>` when the recipe is
/// anonymous) so the wiring display can attribute a topic to a recipe.
pub const RECIPE_ID_PREFIX: &str = "recipe";

// ---------------------------------------------------------------------------
// The designer-shaped JSON: Recipe { ploxion, blocks: [Block] }.
// ---------------------------------------------------------------------------

/// A recipe as exported by the designer: an optional target ploxion name and an
/// ordered list of blocks. Parsing is **liberal**: missing optional fields are
/// tolerated, and unknown block kinds are kept (then ignored-with-trace at
/// compile time) rather than failing the whole parse.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Recipe {
    /// The ploxion this recipe describes (the designer's subject). Optional.
    #[serde(default)]
    pub ploxion: Option<String>,
    /// The ordered blocks, in palette order.
    #[serde(default)]
    pub blocks: Vec<Block>,
}

/// One block from the designer palette. All fields beyond `kind` are optional so
/// a half-finished recipe still parses; `ref` is renamed (it is a Rust keyword).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Block {
    /// The block kind (French or english alias). Unknown kinds are tolerated.
    pub kind: String,
    /// A human label (advisory).
    #[serde(default)]
    pub label: Option<String>,
    /// The block's topic/target reference (e.g. the `quand`/`action` topic).
    #[serde(default, rename = "ref")]
    pub ref_: Option<String>,
    /// The block's parameter (a predicate for `si`, a payload template for
    /// `action`/`sortie`, …).
    #[serde(default)]
    pub param: Option<String>,
}

impl Recipe {
    /// Parse a designer-shaped recipe from JSON bytes. Liberal: tolerates missing
    /// optional fields; does NOT reject unknown block kinds (they are ignored
    /// with a trace at compile time).
    pub fn from_json(bytes: &[u8]) -> Result<Recipe, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Block kind classification (French + english alias).
// ---------------------------------------------------------------------------

/// What a block *does*, once its kind string is normalised. Unknown kinds map to
/// [`Kind::Unknown`] and are ignored-with-trace at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `quand`/`when` — a trigger: subscribe to its `ref` topic.
    Trigger,
    /// `si`/`if` — a condition over the payload.
    Condition,
    /// `action` / `sortie`/`output` — emit its `ref` topic with `param` payload.
    Emit,
    /// `ploxion` (RECIPE v1) — deliver DIRECTLY to the ploxion whose manifest id
    /// equals `ref`, calling its `plc_on_event` (TARGETED / point-to-point — NOT
    /// a bus broadcast). No english alias: `ploxion` is canonical in both
    /// languages.
    Ploxion,
    /// `entrée`/`input` — bind/document a payload field (advisory, no effect).
    Input,
    /// `attendre`/`wait` — a delay marker (advisory; traced, no real sleep).
    Wait,
    /// Anything else — ignored with a trace.
    Unknown,
}

impl Kind {
    /// Classify a raw kind string (case/whitespace-insensitive), accepting both
    /// the French palette token and its english alias.
    pub fn classify(raw: &str) -> Kind {
        match raw.trim().to_ascii_lowercase().as_str() {
            "quand" | "when" => Kind::Trigger,
            "si" | "if" => Kind::Condition,
            "action" | "sortie" | "output" => Kind::Emit,
            "ploxion" => Kind::Ploxion,
            "entrée" | "entree" | "input" => Kind::Input,
            "attendre" | "wait" => Kind::Wait,
            _ => Kind::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// The compiled rule: triggers, conditions, emits.
// ---------------------------------------------------------------------------

/// A simple comparison operator a `si` condition may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl Op {
    /// Parse the operator token. Longest tokens first so `>=` is not read as `>`.
    fn parse(s: &str) -> Option<Op> {
        match s {
            "==" => Some(Op::Eq),
            "!=" => Some(Op::Ne),
            "<=" => Some(Op::Le),
            ">=" => Some(Op::Ge),
            "<" => Some(Op::Lt),
            ">" => Some(Op::Gt),
            _ => None,
        }
    }
}

/// One compiled condition: `<field> <op> <value>` evaluated against the trigger
/// payload's JSON. The comparison is **type-tolerant**: numbers compare
/// numerically, booleans/strings compare by their normalised text.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// The payload field name (e.g. `up`, `code`).
    pub field: String,
    /// The comparison operator.
    pub op: Op,
    /// The right-hand value, as written in the recipe (e.g. `false`, `500`).
    pub value: String,
}

impl Condition {
    /// Parse a predicate string like `"up == false"` or `"code >= 500"`. Returns
    /// `None` if it is not a recognisable `<field> <op> <value>` triple.
    pub fn parse(src: &str) -> Option<Condition> {
        // Find the operator: try two-char ops first, then one-char, scanning the
        // raw string so spacing is irrelevant (`code>=500` and `code >= 500` both
        // parse).
        for tok in ["==", "!=", "<=", ">=", "<", ">"] {
            if let Some(idx) = src.find(tok) {
                let field = src[..idx].trim();
                let value = src[idx + tok.len()..].trim();
                if field.is_empty() || value.is_empty() {
                    return None;
                }
                let op = Op::parse(tok)?;
                return Some(Condition {
                    field: field.to_string(),
                    value: unquote(value).to_string(),
                    op,
                });
            }
        }
        None
    }

    /// Evaluate this condition against a JSON payload. A **missing field is
    /// always false** (you cannot assert anything about what is not there), and
    /// the evaluation never panics on malformed JSON — it just yields false.
    pub fn eval(&self, payload: &str) -> bool {
        let lhs = match json_field(payload, &self.field) {
            Some(v) => v,
            None => return false, // missing field => false, always
        };
        let rhs = self.value.as_str();

        // Numeric comparison when BOTH sides parse as numbers; else textual.
        match (parse_num(&lhs), parse_num(rhs)) {
            (Some(a), Some(b)) => match self.op {
                Op::Eq => a == b,
                Op::Ne => a != b,
                Op::Lt => a < b,
                Op::Gt => a > b,
                Op::Le => a <= b,
                Op::Ge => a >= b,
            },
            _ => {
                let a = lhs.as_str();
                let b = rhs;
                match self.op {
                    Op::Eq => a == b,
                    Op::Ne => a != b,
                    // Ordering of non-numbers falls back to lexical order, which
                    // is well-defined and deterministic (rarely used, but never
                    // panics).
                    Op::Lt => a < b,
                    Op::Gt => a > b,
                    Op::Le => a <= b,
                    Op::Ge => a >= b,
                }
            }
        }
    }
}

/// One compiled emit: a topic and a payload template. The template may reference
/// payload fields via `{field}` interpolation, or be a literal JSON string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitSpec {
    /// The topic to emit on (the recipe's consent surface / `provides`).
    pub topic: String,
    /// The payload template. `{field}` is replaced by the trigger payload's
    /// value for `field`; everything else is copied verbatim.
    pub template: String,
}

impl EmitSpec {
    /// Render the payload by interpolating `{field}` references against the
    /// trigger payload. Unknown `{field}` references render as empty. If the
    /// template is empty, the trigger payload is forwarded unchanged.
    pub fn render(&self, payload: &str) -> String {
        if self.template.is_empty() {
            return payload.to_string();
        }
        interpolate(&self.template, payload)
    }
}

/// One compiled targeted delivery (RECIPE v1, the `ploxion` kind): a ploxion id
/// and the payload template to hand it. When the rule fires, the host calls that
/// ONE ploxion's `plc_on_event` directly — point-to-point, NOT a bus broadcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverSpec {
    /// The manifest id of the target ploxion (the recipe's `ref`).
    pub ploxion_id: String,
    /// The payload template. `{field}` is replaced by the trigger payload's
    /// value for `field`; an EMPTY template forwards the trigger payload
    /// unchanged (same convention as [`EmitSpec`]).
    pub template: String,
}

impl DeliverSpec {
    /// Render the payload by interpolating `{field}` references against the
    /// trigger payload. An empty template forwards the trigger payload unchanged.
    pub fn render(&self, payload: &str) -> String {
        if self.template.is_empty() {
            return payload.to_string();
        }
        interpolate(&self.template, payload)
    }
}

/// One ordered action the rule performs when it fires. The designer's block
/// order is preserved: a recipe may interleave `action`/`sortie` (bus emits) and
/// `ploxion` (targeted deliveries) and they run in exactly that order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `action`/`sortie` — EMIT a topic on the bus (v0 behavior, unchanged).
    Emit(EmitSpec),
    /// `ploxion` (v1) — DELIVER directly to one ploxion's `plc_on_event`.
    Deliver(DeliverSpec),
}

/// A recipe compiled into a reactive rule, ready to register as a native bus
/// participant. Holds the trigger topics it subscribes to (`requires`), the
/// ordered conditions every trigger must satisfy, and the ordered actions it
/// performs when they do — bus emits (`provides`) AND targeted ploxion
/// deliveries (RECIPE v1).
#[derive(Debug, Clone)]
pub struct CompiledRecipe {
    /// The participant id on the bus, e.g. `recipe:alerter`.
    pub id: String,
    /// Topics this rule triggers on (its `requires`). Sorted, de-duplicated.
    pub triggers: Vec<String>,
    /// Conditions evaluated in order; ALL must hold for the rule to fire.
    pub conditions: Vec<Condition>,
    /// The ordered actions performed when the rule fires, in designer block
    /// order: bus emits (`action`/`sortie`) interleaved with targeted ploxion
    /// deliveries (`ploxion`). See [`Action`].
    pub actions: Vec<Action>,
    /// Block kinds that were ignored at compile time (unknown / unsupported),
    /// kept so the host can trace them — nothing is silently dropped.
    pub ignored: Vec<String>,
}

/// One traced step the rule produced while reacting to a single event. The host
/// turns these into journal [`Trace`](crate::Trace) entries so a recipe's
/// decision is visible hop-by-hop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The rule received an event on a trigger topic.
    Triggered { topic: String },
    /// A condition was evaluated (with its truth value).
    Condition { expr: String, holds: bool },
    /// The rule emitted on a topic (the rule fired).
    Emitted { topic: String, payload: String },
    /// The rule delivered DIRECTLY to one ploxion's `plc_on_event` (RECIPE v1,
    /// the `ploxion` kind — targeted, point-to-point). `ploxion` is the target
    /// id, `topic`/`payload` are what the host hands to `plc_on_event`.
    Delivered { ploxion: String, topic: String, payload: String },
    /// The rule stayed silent (a condition was false / no condition matched).
    Silent { reason: String },
}

impl CompiledRecipe {
    /// Compile a [`Recipe`] into a reactive rule.
    ///
    /// - `quand`/`when` blocks contribute their `ref` topic to `triggers`.
    /// - `si`/`if` blocks contribute a parsed [`Condition`] (an unparseable
    ///   predicate is recorded as ignored, never silently dropped).
    /// - `action` and `sortie`/`output` blocks contribute an [`Action::Emit`].
    /// - `ploxion` blocks (RECIPE v1) contribute an [`Action::Deliver`] — a
    ///   targeted delivery to the ploxion whose id equals `ref`.
    /// - `entrée`/`input` and `attendre`/`wait` are advisory (no runtime effect)
    ///   but recorded as ignored so the trace accounts for them.
    /// - unknown kinds are recorded in `ignored`.
    ///
    /// Block order is preserved across emits and deliveries (the designer's
    /// palette order is the runtime order).
    pub fn compile(recipe: &Recipe) -> CompiledRecipe {
        let mut triggers: BTreeSet<String> = BTreeSet::new();
        let mut conditions = Vec::new();
        let mut actions = Vec::new();
        let mut ignored = Vec::new();

        for block in &recipe.blocks {
            match Kind::classify(&block.kind) {
                Kind::Trigger => match block.ref_.as_deref() {
                    Some(topic) if !topic.is_empty() => {
                        triggers.insert(topic.to_string());
                    }
                    _ => ignored.push(format!("{} (no ref topic)", block.kind)),
                },
                Kind::Condition => match block.param.as_deref() {
                    Some(p) => match Condition::parse(p) {
                        Some(c) => conditions.push(c),
                        None => ignored.push(format!("si '{p}' (unparseable predicate)")),
                    },
                    None => ignored.push("si (no predicate)".to_string()),
                },
                Kind::Emit => match block.ref_.as_deref() {
                    Some(topic) if !topic.is_empty() => actions.push(Action::Emit(EmitSpec {
                        topic: topic.to_string(),
                        template: block.param.clone().unwrap_or_default(),
                    })),
                    _ => ignored.push(format!("{} (no ref topic)", block.kind)),
                },
                Kind::Ploxion => match block.ref_.as_deref() {
                    Some(pid) if !pid.is_empty() => actions.push(Action::Deliver(DeliverSpec {
                        ploxion_id: pid.to_string(),
                        template: block.param.clone().unwrap_or_default(),
                    })),
                    _ => ignored.push(format!("{} (no ref ploxion id)", block.kind)),
                },
                Kind::Input | Kind::Wait => {
                    ignored.push(format!("{} (advisory, no runtime effect)", block.kind));
                }
                Kind::Unknown => ignored.push(format!("{} (unknown kind)", block.kind)),
            }
        }

        let id = match &recipe.ploxion {
            Some(name) if !name.is_empty() => format!("{RECIPE_ID_PREFIX}:{name}"),
            _ => format!("{RECIPE_ID_PREFIX}:anon"),
        };

        CompiledRecipe {
            id,
            triggers: triggers.into_iter().collect(),
            conditions,
            actions,
            ignored,
        }
    }

    /// The topics this rule subscribes to (its `requires` surface).
    pub fn requires(&self) -> &[String] {
        &self.triggers
    }

    /// The topics this rule may emit on (its `provides` / consent surface). Only
    /// `action`/`sortie` emits count — a `ploxion` delivery is point-to-point and
    /// declares NO bus topic, so it never widens the consent surface.
    pub fn provides(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter_map(|a| match a {
                Action::Emit(e) => Some(e.topic.as_str()),
                Action::Deliver(_) => None,
            })
            .collect()
    }

    /// The targeted ploxion deliveries this rule may perform (RECIPE v1). These
    /// are point-to-point — they are NOT part of `provides()` / the bus wiring.
    pub fn delivers_to(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter_map(|a| match a {
                Action::Deliver(d) => Some(d.ploxion_id.as_str()),
                Action::Emit(_) => None,
            })
            .collect()
    }

    /// React to one event. If `topic` is not a trigger, returns nothing and a
    /// single `Silent` step (the rule never fires on an un-subscribed topic). If
    /// it IS a trigger, evaluates every condition in order; only when ALL hold
    /// does it produce its actions. Returns a [`Reaction`] carrying, IN BLOCK
    /// ORDER, the bus emits (`action`/`sortie`) and the targeted ploxion
    /// deliveries (`ploxion`), plus the per-hop trace `steps`.
    ///
    /// The targeted delivery's topic+payload is the documented contract: the
    /// **trigger topic** is handed to the target ploxion's `plc_on_event` as the
    /// topic, and the **interpolated `param`** (falling back to the trigger
    /// payload when `param` is empty) is handed as the payload.
    ///
    /// This is a **pure** function of `(topic, payload)` — no I/O, deterministic,
    /// never panics — so it is trivially testable and the host can drive it.
    pub fn react(&self, topic: &str, payload: &str) -> Reaction {
        let mut steps = Vec::new();

        // (a) trigger gate — the rule only ever fires on a `quand` topic.
        if !self.triggers.iter().any(|t| t == topic) {
            steps.push(Step::Silent {
                reason: format!("'{topic}' is not a trigger topic"),
            });
            return Reaction { emits: Vec::new(), deliveries: Vec::new(), steps };
        }
        steps.push(Step::Triggered { topic: topic.to_string() });

        // (b) condition gate — ALL `si` conditions must hold.
        for cond in &self.conditions {
            let holds = cond.eval(payload);
            let expr = format!("{} {} {}", cond.field, op_str(cond.op), cond.value);
            steps.push(Step::Condition { expr: expr.clone(), holds });
            if !holds {
                steps.push(Step::Silent {
                    reason: format!("condition '{expr}' is false"),
                });
                return Reaction { emits: Vec::new(), deliveries: Vec::new(), steps };
            }
        }

        // (c) fire — run every action IN ORDER: action/sortie EMIT on the bus,
        // ploxion DELIVER point-to-point to one ploxion's plc_on_event.
        if self.actions.is_empty() {
            steps.push(Step::Silent {
                reason: "no action/sortie/ploxion block to run".to_string(),
            });
            return Reaction { emits: Vec::new(), deliveries: Vec::new(), steps };
        }
        let mut emits = Vec::new();
        let mut deliveries = Vec::new();
        for action in &self.actions {
            match action {
                Action::Emit(emit) => {
                    let rendered = emit.render(payload);
                    steps.push(Step::Emitted {
                        topic: emit.topic.clone(),
                        payload: rendered.clone(),
                    });
                    emits.push((emit.topic.clone(), rendered));
                }
                Action::Deliver(spec) => {
                    // Documented contract: TOPIC = the trigger topic, PAYLOAD =
                    // the interpolated param (or the trigger payload if empty).
                    let rendered = spec.render(payload);
                    steps.push(Step::Delivered {
                        ploxion: spec.ploxion_id.clone(),
                        topic: topic.to_string(),
                        payload: rendered.clone(),
                    });
                    deliveries.push(Delivery {
                        ploxion_id: spec.ploxion_id.clone(),
                        topic: topic.to_string(),
                        payload: rendered,
                    });
                }
            }
        }
        Reaction { emits, deliveries, steps }
    }
}

/// What a [`CompiledRecipe::react`] produced for one event. Carries — separately
/// — the bus emits (routed to subscribers) and the targeted ploxion deliveries
/// (handed point-to-point to one ploxion's `plc_on_event`), plus the trace.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reaction {
    /// Bus emits: `(topic, payload)` pairs the host routes through the cascade.
    pub emits: Vec<(String, String)>,
    /// Targeted deliveries (RECIPE v1): each goes to ONE ploxion's
    /// `plc_on_event` — NOT broadcast on the bus.
    pub deliveries: Vec<Delivery>,
    /// The per-hop trace of the rule's decision.
    pub steps: Vec<Step>,
}

/// One targeted delivery the host must hand to a single ploxion's
/// `plc_on_event` (RECIPE v1, the `ploxion` kind). Point-to-point: the host does
/// NOT put it on the bus, so no other ploxion sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The manifest id of the ploxion to deliver to.
    pub ploxion_id: String,
    /// The topic handed to `plc_on_event` (the recipe's trigger topic).
    pub topic: String,
    /// The payload handed to `plc_on_event` (interpolated `param`, or the
    /// trigger payload when `param` was empty).
    pub payload: String,
}

// ---------------------------------------------------------------------------
// Tiny JSON helpers (the payloads are flat, ploxion-authored objects — same
// shape the watcher parses). Deliberately dependency-light + panic-free.
// ---------------------------------------------------------------------------

/// Human rendering of an operator (for trace expressions).
fn op_str(op: Op) -> &'static str {
    match op {
        Op::Eq => "==",
        Op::Ne => "!=",
        Op::Lt => "<",
        Op::Gt => ">",
        Op::Le => "<=",
        Op::Ge => ">=",
    }
}

/// Strip one layer of surrounding quotes from a recipe-written value, so
/// `"down"` and `down` mean the same right-hand side.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse a value as an `f64` if it is numeric (so `500`, `200`, `1.5` compare
/// numerically). Booleans/strings return `None` and fall back to text compare.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Extract the value of `field` from a flat JSON object as a normalised string:
/// a quoted string yields its inner text, a number/bool yields its literal text.
/// Returns `None` if the field is absent. Never panics on malformed JSON.
///
/// This mirrors the watcher's field extractors but returns a single normalised
/// string so one comparator handles strings, numbers, and booleans alike.
pub fn json_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();

    if let Some(stripped) = after.strip_prefix('"') {
        // String value: read until the closing quote (no escape handling — the
        // payloads here are simple, flat, ploxion-authored objects).
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else {
        // Number / bool / null: read the token up to the next delimiter.
        let token: String = after
            .chars()
            .take_while(|c| !matches!(c, ',' | '}' | ']' | ' ' | '\n' | '\t' | '\r'))
            .collect();
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    }
}

/// Replace every `{field}` in `template` with the trigger payload's value for
/// `field` (empty when absent). A literal `{` with no closing `}` is copied
/// verbatim. Anything that is not a `{field}` reference (including literal JSON)
/// passes through unchanged.
pub fn interpolate(template: &str, payload: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '{' {
            // Find the matching '}'.
            if let Some(close) = template[i + 1..].find('}') {
                let field = &template[i + 1..i + 1 + close];
                // A reference is `{ident}` (no nested braces / spaces) — anything
                // else is treated as literal text.
                if !field.is_empty()
                    && !field.contains('{')
                    && field.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
                {
                    let val = json_field(payload, field).unwrap_or_default();
                    out.push_str(&val);
                    // Advance the iterator past the closing brace.
                    for _ in 0..(close + 1) {
                        chars.next();
                    }
                    continue;
                }
            }
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_french_and_english() {
        assert_eq!(Kind::classify("quand"), Kind::Trigger);
        assert_eq!(Kind::classify("when"), Kind::Trigger);
        assert_eq!(Kind::classify("SI"), Kind::Condition);
        assert_eq!(Kind::classify("if"), Kind::Condition);
        assert_eq!(Kind::classify("action"), Kind::Emit);
        assert_eq!(Kind::classify("sortie"), Kind::Emit);
        assert_eq!(Kind::classify("output"), Kind::Emit);
        assert_eq!(Kind::classify("entrée"), Kind::Input);
        assert_eq!(Kind::classify("input"), Kind::Input);
        assert_eq!(Kind::classify("attendre"), Kind::Wait);
        assert_eq!(Kind::classify("wait"), Kind::Wait);
        assert_eq!(Kind::classify("frobnicate"), Kind::Unknown);
    }

    #[test]
    fn condition_parse_spacing_insensitive() {
        let c = Condition::parse("code >= 500").unwrap();
        assert_eq!(c.field, "code");
        assert_eq!(c.op, Op::Ge);
        assert_eq!(c.value, "500");
        let c2 = Condition::parse("up==false").unwrap();
        assert_eq!(c2.field, "up");
        assert_eq!(c2.op, Op::Eq);
        assert_eq!(c2.value, "false");
        assert!(Condition::parse("nonsense").is_none());
    }

    #[test]
    fn json_field_string_number_bool() {
        let p = r#"{"id":"repoverse","code":500,"up":false}"#;
        assert_eq!(json_field(p, "id").as_deref(), Some("repoverse"));
        assert_eq!(json_field(p, "code").as_deref(), Some("500"));
        assert_eq!(json_field(p, "up").as_deref(), Some("false"));
        assert_eq!(json_field(p, "missing"), None);
    }

    #[test]
    fn interpolate_fields_and_literals() {
        let p = r#"{"id":"repoverse","code":500}"#;
        assert_eq!(interpolate("{id} is DOWN (code {code})", p), "repoverse is DOWN (code 500)");
        // Unknown field -> empty.
        assert_eq!(interpolate("x={nope}y", p), "x=y");
        // Literal JSON passes through.
        assert_eq!(interpolate(r#"{"k":"v"}"#, p), r#"{"k":"v"}"#);
    }
}
