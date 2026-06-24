//! Integration tests for the **recipe runner**: parsing the designer's export
//! shape, the `si` predicate truth table, trigger routing, and the end-to-end
//! property that a compiled recipe registered on the host fires its `action`
//! emit ONLY when the trigger matches AND the condition holds — and stays
//! SILENT otherwise (*droit au silence*).
//!
//! Everything here is DETERMINISTIC and OFFLINE: no network, no wasm needed for
//! the rule logic itself. One end-to-end test loads the real `tracer` wasm to
//! prove the rule's emit is routed to a WASM ploxion across the bus; if the
//! tracer wasm is missing it SKIPs with a clear message (never silently passes).

use std::path::PathBuf;

use xerboxion_host::recipe::{CompiledRecipe, Condition, Kind, Op, Recipe};
use xerboxion_host::{Host, Trace};

// ---------------------------------------------------------------------------
// Parse: the designer's exported JSON shape.
// ---------------------------------------------------------------------------

/// The exact shape ploxion5's designer exports (coord #67): a `ploxion` name and
/// an ordered `blocks` array of French-kinded blocks, `ref` for topics, `param`
/// for predicates/templates.
const DESIGNER_JSON: &[u8] = br#"{
  "ploxion": "alerter",
  "blocks": [
    {"kind": "quand",  "label": "on health",  "ref": "service.health"},
    {"kind": "si",     "label": "is down",     "param": "up == false"},
    {"kind": "action", "label": "notify",      "ref": "alert.notify", "param": "{id} is DOWN (code {code})"}
  ]
}"#;

#[test]
fn parses_designer_shaped_json() {
    let r = Recipe::from_json(DESIGNER_JSON).expect("designer JSON must parse");
    assert_eq!(r.ploxion.as_deref(), Some("alerter"));
    assert_eq!(r.blocks.len(), 3);
    assert_eq!(r.blocks[0].kind, "quand");
    assert_eq!(r.blocks[0].ref_.as_deref(), Some("service.health"));
    assert_eq!(r.blocks[1].kind, "si");
    assert_eq!(r.blocks[1].param.as_deref(), Some("up == false"));
    assert_eq!(r.blocks[2].kind, "action");
    assert_eq!(r.blocks[2].ref_.as_deref(), Some("alert.notify"));
    assert_eq!(r.blocks[2].param.as_deref(), Some("{id} is DOWN (code {code})"));
    // `ref` (a Rust keyword) round-trips via the serde rename.
    assert_eq!(r.blocks[0].label.as_deref(), Some("on health"));
}

#[test]
fn parse_is_liberal_missing_optionals_and_unknown_kinds() {
    // No `ploxion`, blocks with missing optional fields, plus an unknown kind.
    let json = br#"{
      "blocks": [
        {"kind": "quand", "ref": "t.in"},
        {"kind": "frobnicate"},
        {"kind": "action", "ref": "t.out"}
      ]
    }"#;
    let r = Recipe::from_json(json).expect("must tolerate missing optionals");
    assert!(r.ploxion.is_none());
    assert_eq!(r.blocks.len(), 3);

    // The unknown kind is kept by the parser but ignored-with-trace at compile.
    let c = CompiledRecipe::compile(&r);
    assert_eq!(c.triggers, vec!["t.in".to_string()]);
    assert_eq!(c.provides(), vec!["t.out"]);
    assert!(c.ignored.iter().any(|s| s.contains("frobnicate")));
}

#[test]
fn classify_covers_french_and_english_aliases() {
    for (raw, want) in [
        ("quand", Kind::Trigger),
        ("when", Kind::Trigger),
        ("si", Kind::Condition),
        ("if", Kind::Condition),
        ("action", Kind::Emit),
        ("sortie", Kind::Emit),
        ("output", Kind::Emit),
        ("entrée", Kind::Input),
        ("input", Kind::Input),
        ("attendre", Kind::Wait),
        ("wait", Kind::Wait),
        ("xyz", Kind::Unknown),
    ] {
        assert_eq!(Kind::classify(raw), want, "kind {raw:?}");
    }
}

// ---------------------------------------------------------------------------
// Predicate eval — the `si` truth table.
// ---------------------------------------------------------------------------

#[test]
fn predicate_eval_truth_table() {
    let payload = r#"{"id":"svc","code":500,"up":false,"name":"alpha"}"#;

    // == on a bool
    assert!(Condition::parse("up == false").unwrap().eval(payload));
    assert!(!Condition::parse("up == true").unwrap().eval(payload));
    // != on a bool
    assert!(Condition::parse("up != true").unwrap().eval(payload));
    assert!(!Condition::parse("up != false").unwrap().eval(payload));

    // numeric == / != / < / > / <= / >= (type-tolerant: numbers compare numerically)
    assert!(Condition::parse("code == 500").unwrap().eval(payload));
    assert!(Condition::parse("code != 200").unwrap().eval(payload));
    assert!(Condition::parse("code > 499").unwrap().eval(payload));
    assert!(Condition::parse("code < 501").unwrap().eval(payload));
    assert!(Condition::parse("code >= 500").unwrap().eval(payload));
    assert!(Condition::parse("code <= 500").unwrap().eval(payload));
    assert!(!Condition::parse("code > 500").unwrap().eval(payload));
    assert!(!Condition::parse("code < 500").unwrap().eval(payload));

    // string == / != (the value's surrounding quotes are tolerated)
    assert!(Condition::parse("id == svc").unwrap().eval(payload));
    assert!(Condition::parse(r#"id == "svc""#).unwrap().eval(payload));
    assert!(Condition::parse("name != beta").unwrap().eval(payload));
    assert!(!Condition::parse("name == beta").unwrap().eval(payload));
}

#[test]
fn missing_field_is_always_false() {
    let payload = r#"{"code":200}"#;
    // Every operator against an absent field is false — you cannot assert about
    // what is not there.
    for expr in ["up == false", "up != false", "missing > 0", "missing < 9", "ghost == x"] {
        assert!(
            !Condition::parse(expr).unwrap().eval(payload),
            "missing-field predicate {expr:?} must be false"
        );
    }
}

#[test]
fn predicate_is_type_tolerant_and_panic_free() {
    // Comparing a numeric field to a non-numeric value falls back to text and
    // never panics; malformed JSON yields false rather than panicking.
    let payload = r#"{"code":500}"#;
    assert!(!Condition::parse("code == down").unwrap().eval(payload)); // 500 != "down"
    assert!(Condition::parse("code != down").unwrap().eval(payload));

    let garbage = "this is not json at all";
    assert!(!Condition::parse("code == 500").unwrap().eval(garbage));
}

#[test]
fn op_parse_longest_token_first() {
    // ">=" must not be read as ">".
    assert_eq!(Condition::parse("code >= 5").unwrap().op, Op::Ge);
    assert_eq!(Condition::parse("code <= 5").unwrap().op, Op::Le);
    assert_eq!(Condition::parse("code != 5").unwrap().op, Op::Ne);
    assert_eq!(Condition::parse("code == 5").unwrap().op, Op::Eq);
    assert_eq!(Condition::parse("code > 5").unwrap().op, Op::Gt);
    assert_eq!(Condition::parse("code < 5").unwrap().op, Op::Lt);
}

// ---------------------------------------------------------------------------
// Trigger routing — only the `quand` topic triggers; the rule reacts purely.
// ---------------------------------------------------------------------------

fn alerter() -> CompiledRecipe {
    CompiledRecipe::compile(&Recipe::from_json(DESIGNER_JSON).unwrap())
}

#[test]
fn only_the_quand_topic_triggers() {
    let rule = alerter();
    assert_eq!(rule.requires(), &["service.health".to_string()]);

    // A non-trigger topic: zero emits, and a Silent step explaining why.
    let r = rule.react("some.other.topic", r#"{"up":false}"#);
    assert!(r.emits.is_empty(), "a non-trigger topic must produce no emits");
    assert!(r.steps.iter().any(|s| matches!(s,
        xerboxion_host::recipe::Step::Silent { reason } if reason.contains("not a trigger"))));
}

#[test]
fn matching_event_emits_interpolated_payload() {
    let rule = alerter();
    let payload = r#"{"id":"repoverse","code":503,"up":false}"#;
    let r = rule.react("service.health", payload);

    assert_eq!(r.emits.len(), 1, "a matching DOWN event must fire exactly one emit");
    assert_eq!(r.emits[0].0, "alert.notify");
    assert_eq!(r.emits[0].1, "repoverse is DOWN (code 503)");

    // The decision steps are all present: triggered, condition true, emitted.
    assert!(r.steps.iter().any(|s| matches!(s, xerboxion_host::recipe::Step::Triggered { .. })));
    assert!(r.steps.iter().any(|s| matches!(s,
        xerboxion_host::recipe::Step::Condition { holds: true, .. })));
    assert!(r.steps.iter().any(|s| matches!(s, xerboxion_host::recipe::Step::Emitted { .. })));
}

#[test]
fn non_matching_event_emits_nothing() {
    let rule = alerter();
    // up == true -> condition false -> droit au silence.
    let r = rule.react("service.health", r#"{"id":"x","code":200,"up":true}"#);
    assert!(r.emits.is_empty(), "an UP event must produce ZERO emits");
    assert!(r.steps.iter().any(|s| matches!(s,
        xerboxion_host::recipe::Step::Condition { holds: false, .. })));
    assert!(r.steps.iter().any(|s| matches!(s, xerboxion_host::recipe::Step::Silent { .. })));
    // And it must NEVER have an Emitted step.
    assert!(!r.steps.iter().any(|s| matches!(s, xerboxion_host::recipe::Step::Emitted { .. })));
}

#[test]
fn all_conditions_must_hold_to_fire() {
    // Two conditions ANDed: code >= 500 AND up == false.
    let json = br#"{
      "ploxion": "strict",
      "blocks": [
        {"kind": "quand",  "ref": "service.health"},
        {"kind": "si",     "param": "code >= 500"},
        {"kind": "si",     "param": "up == false"},
        {"kind": "action", "ref": "alert.notify", "param": "down {id}"}
      ]
    }"#;
    let rule = CompiledRecipe::compile(&Recipe::from_json(json).unwrap());
    assert_eq!(rule.conditions.len(), 2);

    // Both hold -> fire.
    let e = rule.react("service.health", r#"{"id":"a","code":503,"up":false}"#).emits;
    assert_eq!(e.len(), 1);
    // First condition fails (code 200) -> silent, even though up==false.
    let e = rule.react("service.health", r#"{"id":"a","code":200,"up":false}"#).emits;
    assert!(e.is_empty(), "a single false condition must keep the rule silent");
    // Second condition fails (up true) -> silent, even though code 503.
    let e = rule.react("service.health", r#"{"id":"a","code":503,"up":true}"#).emits;
    assert!(e.is_empty());
}

// ---------------------------------------------------------------------------
// End-to-end on the Host: register the rule, inject events, check the bus.
// ---------------------------------------------------------------------------

#[test]
fn host_registers_recipe_wiring() {
    let mut host = Host::new();
    let compiled = host.register_recipe(&Recipe::from_json(DESIGNER_JSON).unwrap());

    // requires -> subscribed to the trigger topic.
    assert!(host
        .bus()
        .subscribers("service.health")
        .iter()
        .any(|s| s == &compiled.id));
    // provides -> registered as the native provider of its emit topic.
    assert!(host
        .native_participants()
        .any(|(id, topics)| id == &compiled.id && topics.iter().any(|t| t == "alert.notify")));
}

#[test]
fn host_executes_recipe_match_fires_nonmatch_silent() {
    let mut host = Host::new();
    let compiled = host.register_recipe(&Recipe::from_json(DESIGNER_JSON).unwrap());

    // MATCH: a DOWN service -> the rule must emit alert.notify with interpolation.
    host.inject(
        "test",
        "service.health",
        br#"{"id":"repoverse","code":503,"up":false}"#,
    )
    .unwrap();

    let fired = host.trace().iter().any(|t| matches!(t, Trace::Emit { from, topic, payload }
        if from == &compiled.id && topic == "alert.notify"
            && payload == "repoverse is DOWN (code 503)"));
    assert!(fired, "matching DOWN event must fire the interpolated alert.notify emit");

    let emits_after_match = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, .. } if from == &compiled.id))
        .count();
    assert_eq!(emits_after_match, 1, "exactly one emit after the match");

    // NON-MATCH: an UP service -> ZERO additional emits from the rule.
    host.inject(
        "test",
        "service.health",
        br#"{"id":"ideas-map","code":200,"up":true}"#,
    )
    .unwrap();

    let emits_total = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, .. } if from == &compiled.id))
        .count();
    assert_eq!(
        emits_total, 1,
        "the UP (non-matching) event must add NO emits — droit au silence"
    );

    // The non-match left a traced 'condition false' decision (visible silence).
    assert!(host.trace().iter().any(|t| matches!(t, Trace::Recipe { who, step }
        if who == &compiled.id && step.contains("false"))));
}

#[test]
fn non_trigger_topic_never_fires_the_rule() {
    let mut host = Host::new();
    let compiled = host.register_recipe(&Recipe::from_json(DESIGNER_JSON).unwrap());

    // Inject on a topic the rule does NOT subscribe to. The rule isn't even a
    // subscriber, so it is never dispatched — and certainly never emits.
    host.inject("test", "unrelated.topic", br#"{"up":false}"#).unwrap();
    let emits = host
        .trace()
        .iter()
        .filter(|t| matches!(t, Trace::Emit { from, .. } if from == &compiled.id))
        .count();
    assert_eq!(emits, 0, "an event on a non-trigger topic must never fire the rule");
}

// ---------------------------------------------------------------------------
// RECIPE v1 — the `ploxion` kind: a TARGETED, point-to-point delivery to the
// ploxion whose manifest id == ref, calling its plc_on_event. NOT a bus
// broadcast. These tests load real wasm ploxions (the deliver path runs inside a
// Store), so they SKIP with a clear message if the wasm is not built.
// ---------------------------------------------------------------------------

/// A recipe with ONE `ploxion` block (no `action`) targeting `target` by id on
/// trigger topic `trigger`, with no `si` (so it fires on any matching trigger).
/// The delivered payload is the interpolated `param` (the delivery TOPIC is the
/// trigger topic — the v1 contract).
fn deliver_recipe_on(trigger: &str, target: &str) -> Recipe {
    let json = format!(
        r#"{{
          "ploxion": "router",
          "blocks": [
            {{"kind": "quand",   "ref": "{trigger}"}},
            {{"kind": "ploxion", "ref": "{target}", "param": "targeted: {{id}}"}}
          ]
        }}"#
    );
    Recipe::from_json(json.as_bytes()).unwrap()
}

/// The common case: deliver on `service.health` (the tracer logs any topic).
fn deliver_recipe(target: &str) -> Recipe {
    deliver_recipe_on("service.health", target)
}

#[test]
fn classify_recognises_the_ploxion_kind() {
    // The new v1 canonical kind (case-insensitive); no EN alias needed.
    assert_eq!(Kind::classify("ploxion"), Kind::Ploxion);
    assert_eq!(Kind::classify("PLOXION"), Kind::Ploxion);
    assert_eq!(Kind::classify(" Ploxion "), Kind::Ploxion);
}

#[test]
fn compile_ploxion_block_is_a_delivery_not_a_provide() {
    // A `ploxion` block contributes a targeted delivery, NOT a bus `provides`
    // topic (point-to-point declares no consent surface). An `action` alongside
    // it still provides its topic, and block order is preserved.
    let json = br#"{
      "ploxion": "mix",
      "blocks": [
        {"kind": "quand",   "ref": "t.in"},
        {"kind": "action",  "ref": "t.out"},
        {"kind": "ploxion", "ref": "tracer"}
      ]
    }"#;
    let c = CompiledRecipe::compile(&Recipe::from_json(json).unwrap());
    assert_eq!(c.provides(), vec!["t.out"], "ploxion ref must NOT widen the bus provides surface");
    assert_eq!(c.delivers_to(), vec!["tracer"], "the ploxion ref is a targeted delivery id");
}

#[test]
fn react_delivers_topic_is_trigger_payload_is_interpolated_param() {
    // The documented v1 delivery contract: topic = the trigger topic; payload =
    // the interpolated `param` (falling back to the trigger payload when empty).
    let rule = CompiledRecipe::compile(&deliver_recipe("tracer"));
    let r = rule.react("service.health", r#"{"id":"repoverse","up":false}"#);
    assert!(r.emits.is_empty(), "a ploxion-only recipe emits nothing on the bus");
    assert_eq!(r.deliveries.len(), 1);
    assert_eq!(r.deliveries[0].ploxion_id, "tracer");
    assert_eq!(r.deliveries[0].topic, "service.health", "delivery topic is the trigger topic");
    assert_eq!(r.deliveries[0].payload, "targeted: repoverse", "delivery payload is interpolated param");
}

fn skip_if_no_wasm(p: &str) -> Option<Vec<u8>> {
    match std::fs::read(ploxion_dir().join(p)) {
        Ok(b) => Some(b),
        Err(_) => {
            eprintln!("SKIP: {p} not built — run scripts/build-ploxions.sh first");
            None
        }
    }
}

#[test]
fn ploxion_block_delivers_to_the_named_ploxion_and_not_others() {
    // Load TWO ploxions (tracer + pong). The recipe targets the tracer by id; the
    // delivery must reach the tracer's plc_on_event (it logs) and NOT pong's.
    let (Some(tracer), Some(pong)) = (skip_if_no_wasm("tracer.wasm"), skip_if_no_wasm("pong.wasm"))
    else {
        return;
    };

    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.load_bytes(&pong, "pong").unwrap();
    host.init_all().unwrap();

    let compiled = host.register_recipe(&deliver_recipe("tracer"));
    let trace_start = host.trace().len();
    host.inject("test", "service.health", br#"{"id":"repoverse","up":false}"#).unwrap();

    // (a) a DISTINCT deliver hop recipe -> tracer on the TRIGGER topic.
    let delivered_to_tracer = host.trace()[trace_start..].iter().any(|t| matches!(t,
        Trace::Deliver { from, to, topic }
        if from == &compiled.id && to == "tracer" && topic == "service.health"));
    assert!(delivered_to_tracer, "the ploxion block must deliver to the tracer (point-to-point)");

    // The tracer actually RAN its plc_on_event (it logged the interpolated payload).
    let tracer_reacted = host.trace()[trace_start..].iter().any(|t| matches!(t,
        Trace::Log { from, line }
        if from == "tracer" && line.contains("targeted: repoverse")));
    assert!(tracer_reacted, "the targeted ploxion's plc_on_event must have run");

    // (b) NOT delivered to pong, and pong never reacted.
    let delivered_to_pong = host.trace()[trace_start..].iter().any(|t| matches!(t,
        Trace::Deliver { to, .. } if to == "pong"));
    assert!(!delivered_to_pong, "the delivery must be point-to-point, never reaching pong");
    let pong_reacted = host.trace()[trace_start..].iter().any(|t| matches!(t,
        Trace::Log { from, .. } if from == "pong"));
    assert!(!pong_reacted, "pong must not have reacted to a delivery aimed at the tracer");

    // And NO bus subscription to the trigger was created for the tracer — the
    // delivery is point-to-point, not a broadcast route.
    assert!(
        !host.bus().subscribers("service.health").iter().any(|s| s == "tracer"),
        "a targeted delivery must NOT subscribe the ploxion to the trigger topic"
    );
    host.shutdown().unwrap();
}

#[test]
fn changing_the_ref_changes_the_recipient() {
    // Same recipe shape, different `ref` => a different ploxion reacts. Proves the
    // recipient is resolved from `ref`, not hard-wired.
    let (Some(tracer), Some(pong)) = (skip_if_no_wasm("tracer.wasm"), skip_if_no_wasm("pong.wasm"))
    else {
        return;
    };

    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.load_bytes(&pong, "pong").unwrap();
    host.init_all().unwrap();

    // Trigger on "ping" (the topic pong's plc_on_event acts on); ref = pong, so
    // the delivery topic == "ping" and pong actually reacts (it logs "got ...").
    // The tracer is NOT targeted and is NOT a bus subscriber of "ping" => it must
    // not react to the targeted delivery.
    let compiled = host.register_recipe(&deliver_recipe_on("ping", "pong"));
    let start = host.trace().len();
    host.inject("test", "ping", br#"{"id":"x","up":false}"#).unwrap();

    // pong got the targeted delivery (deliver hop recipe -> pong) and reacted.
    let delivered_to_pong = host.trace()[start..].iter().any(|t| matches!(t,
        Trace::Deliver { from, to, .. } if from == &compiled.id && to == "pong"));
    assert!(delivered_to_pong, "changing ref to 'pong' must deliver to pong");
    let pong_reacted = host.trace()[start..].iter().any(|t| matches!(t,
        Trace::Log { from, line } if from == "pong" && line.contains("got")));
    assert!(pong_reacted, "pong's plc_on_event must have run from the targeted delivery");
    // The tracer was NOT the target and not bus-subscribed to "ping": no targeted
    // deliver to it from the recipe.
    let delivered_to_tracer = host.trace()[start..].iter().any(|t| matches!(t,
        Trace::Deliver { from, to, .. } if from == &compiled.id && to == "tracer"));
    assert!(!delivered_to_tracer, "the tracer must NOT be the recipient when ref names pong");
    host.shutdown().unwrap();
}

#[test]
fn unknown_ploxion_ref_is_skipped_with_a_warning_no_panic() {
    let Some(tracer) = skip_if_no_wasm("tracer.wasm") else { return };

    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();

    let compiled = host.register_recipe(&deliver_recipe("ghost-not-loaded"));
    let start = host.trace().len();
    // Must NOT panic.
    host.inject("test", "service.health", br#"{"id":"x","up":false}"#).unwrap();

    // A WARN recipe hop names the missing ploxion; nothing was delivered.
    let warned = host.trace()[start..].iter().any(|t| matches!(t, Trace::Recipe { who, step }
        if who == &compiled.id && step.contains("WARN") && step.contains("ghost-not-loaded")));
    assert!(warned, "a ref naming no loaded ploxion must be traced as a skipped warning");
    // No targeted deliver hop FROM the recipe occurred (the host->recipe route
    // is a separate Deliver and not from the recipe id).
    let any_deliver_from_recipe = host.trace()[start..].iter().any(|t| matches!(t,
        Trace::Deliver { from, .. } if from == &compiled.id));
    assert!(!any_deliver_from_recipe, "no delivery hop must occur for a missing ploxion");
    host.shutdown().unwrap();
}

#[test]
fn action_and_sortie_still_emit_topics_v0_regression() {
    // v0 regression: action/sortie still EMIT their ref TOPIC (not a delivery).
    let json = br#"{
      "ploxion": "emitter",
      "blocks": [
        {"kind": "quand",  "ref": "service.health"},
        {"kind": "action", "ref": "alert.notify", "param": "{id} down"},
        {"kind": "sortie", "ref": "audit.log",    "param": "{id}"}
      ]
    }"#;
    let rule = CompiledRecipe::compile(&Recipe::from_json(json).unwrap());
    assert_eq!(rule.delivers_to(), Vec::<&str>::new(), "no ploxion block => no deliveries");
    let r = rule.react("service.health", r#"{"id":"repoverse","up":false}"#);
    assert!(r.deliveries.is_empty(), "action/sortie must never produce a targeted delivery");
    assert_eq!(r.emits.len(), 2, "action + sortie both emit on the bus");
    assert_eq!(r.emits[0], ("alert.notify".to_string(), "repoverse down".to_string()));
    assert_eq!(r.emits[1], ("audit.log".to_string(), "repoverse".to_string()));
}

#[test]
fn action_then_ploxion_both_run_in_order_on_the_host() {
    // One recipe, BOTH paths: an action EMIT (bus) and a ploxion DELIVER
    // (targeted), in block order. Proves v0 emit and v1 deliver coexist. The
    // target is the tracer (it logs ANY topic, so it observably reacts to the
    // targeted delivery of the trigger topic).
    let Some(tracer) = skip_if_no_wasm("tracer.wasm") else { return };

    let json = br#"{
      "ploxion": "both",
      "blocks": [
        {"kind": "quand",   "ref": "service.health"},
        {"kind": "action",  "ref": "alert.notify", "param": "{id} down"},
        {"kind": "ploxion", "ref": "tracer",       "param": "targeted: {id}"}
      ]
    }"#;
    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();

    let compiled = host.register_recipe(&Recipe::from_json(json).unwrap());
    let start = host.trace().len();
    host.inject("test", "service.health", br#"{"id":"repoverse","up":false}"#).unwrap();

    // The bus EMIT happened (v0 path) — alert.notify, FROM the recipe.
    let emitted = host.trace()[start..].iter().any(|t| matches!(t, Trace::Emit { from, topic, .. }
        if from == &compiled.id && topic == "alert.notify"));
    assert!(emitted, "the action block must still EMIT alert.notify on the bus");
    // The TARGETED deliver happened (v1 path), on the TRIGGER topic, FROM recipe.
    let delivered = host.trace()[start..].iter().any(|t| matches!(t, Trace::Deliver { from, to, topic }
        if from == &compiled.id && to == "tracer" && topic == "service.health"));
    assert!(delivered, "the ploxion block must deliver to the tracer point-to-point");
    // The tracer's plc_on_event ran with the targeted (interpolated) payload.
    let reacted = host.trace()[start..].iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "tracer" && line.contains("targeted: repoverse")));
    assert!(reacted, "the tracer's plc_on_event must have run from the targeted delivery");
    host.shutdown().unwrap();
}

// --- one wasm-backed test: the rule's emit is routed to a real WASM ploxion. -

fn ploxion_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target/ploxions"))
        .unwrap()
}

#[test]
fn recipe_emit_routes_to_a_wasm_subscriber() {
    // Load the real tracer wasm and subscribe it to the rule's output topic; the
    // rule's emit must be DELIVERED into the tracer (proof it lands on the bus
    // and reaches a sandboxed ploxion, not just a host counter).
    let tracer = match std::fs::read(ploxion_dir().join("tracer.wasm")) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("SKIP: tracer.wasm not built — run scripts/build-ploxions.sh first");
            return;
        }
    };

    let mut host = Host::new();
    host.load_bytes(&tracer, "tracer").unwrap();
    host.init_all().unwrap();

    let compiled = host.register_recipe(&Recipe::from_json(DESIGNER_JSON).unwrap());
    host.subscribe("tracer", "alert.notify");

    host.inject(
        "test",
        "service.health",
        br#"{"id":"repoverse","code":503,"up":false}"#,
    )
    .unwrap();

    let delivered = host.trace().iter().any(|t| matches!(t, Trace::Deliver { to, topic, .. }
        if to == "tracer" && topic == "alert.notify"));
    assert!(delivered, "the rule's alert.notify emit must be routed to the WASM tracer");

    // And the tracer logged it (it ran in its own Store and saw the payload).
    let observed = host.trace().iter().any(|t| matches!(t, Trace::Log { from, line }
        if from == "tracer" && line.contains("alert.notify") && line.contains("repoverse")));
    assert!(observed, "the WASM tracer should have observed the routed alert");

    let _ = compiled; // (id used implicitly above)
    host.shutdown().unwrap();
}
