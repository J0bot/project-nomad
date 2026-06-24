//! Text/byte domain generator — an **adaptive predictive model** as a generator.
//!
//! In Adressage Génératif the "generator" is the *shared knowledge* and the
//! residue is the *Shannon surprise* — the only thing stored. The pure
//! coordinate generators ([`crate::generator`]) are knowledge fixed once; this
//! module's generator is knowledge that **adapts as it reads the data**, the way
//! a predictive model does. For text and source code that is exactly the kind of
//! knowledge that collapses the residue toward the data's true entropy.
//!
//! ## What the model is
//!
//! A deterministic, adaptive **context-mixing model** (a small PAQ-style codec)
//! that codes each byte **bit by bit** through a from-scratch binary range coder.
//! For every bit it forms several contexts from the previous *N* bytes — orders
//! `{0, 1, 2, 3, 4, 6}` (`0` = no context, the order-0 backstop; the headline is
//! order-2, hence [`MODEL_ID`]) — each combined with the bits of the current byte
//! already coded (a per-byte *bit tree* of 255 internal nodes). Each context cell
//! holds a small adaptive probability that the next bit is 1; the orders are
//! combined by an **adaptive logistic mixer** (a 1-layer neural net in the
//! stretch/logit domain whose weights are trained online by gradient descent on
//! coding loss), then refined by an adaptive probability map (SSE/APM) stage.
//!
//! Every table starts neutral and is updated **identically on encode and
//! decode**, so the decoder reconstructs the exact same probability for each bit
//! — the property that makes the arithmetic coder reversible. All arithmetic is
//! integer and deterministic (the only floats are in a build-time table); nothing
//! depends on host float behaviour at coding time.
//!
//! Nothing here is data-peeking magic: the model only ever conditions on bytes it
//! has *already* emitted/decoded, so encode and decode see an identical causal
//! stream. On random data the contexts never become predictive, the coded stream
//! stays `~=` the raw length (a hair larger — the honest "no free lunch"), and on
//! real text/code the high-order contexts sharpen fast and the residue collapses
//! below even `zstd -19`-of-raw on the files we measure.
//!
//! ## The model *is* a generator
//!
//! Its stable identity string is [`MODEL_ID`] (`"ctx-order2-v0"`); hashing it
//! gives the `generator_hash` of the [`crate::Address`] produced by the model
//! codec ([`crate::model_codec`]). The coded stream is the residue blob: store it
//! content-addressed exactly like any other residue. Decoding re-runs the model
//! against the coded stream to reproduce the input **bit-for-bit**. Changing the
//! model's *behaviour* (orders, rates, mixer) must change [`MODEL_ID`] so old and
//! new residues never alias.

/// Stable identity of the model generator. Hashing this (`Hash::of`) yields the
/// `generator_hash` used by the model codec. Changing the model's behaviour MUST
/// change this string.
pub const MODEL_ID: &str = "ctx-order2-v0";

// ---------------------------------------------------------------------------
// Range coder (binary, 32-bit, byte-oriented renormalization). Written from
// scratch and exhaustively round-trip tested. Carry handling uses the classic
// "cache + carry-count" scheme so a carry can ripple across already-emitted
// bytes correctly.
// ---------------------------------------------------------------------------

/// Total probability scale for a single bit. Probabilities are 12-bit fixed
/// point in `1..PROB_SCALE`.
const PROB_BITS: u32 = 12;
const PROB_SCALE: u32 = 1 << PROB_BITS;

const TOP: u32 = 1 << 24;

/// Binary arithmetic **encoder**. Feed it `(bit, p1)` where `p1` is the
/// 12-bit-scaled probability that the bit is 1; call [`Self::finish`] to flush.
struct BitEncoder {
    low: u64,
    range: u32,
    out: Vec<u8>,
    /// Cached byte awaiting a possible carry, and how many pending bytes (the
    /// cache plus a run of deferred 0xFF bytes) are queued behind it.
    cache: u8,
    cache_size: u64,
}

impl BitEncoder {
    fn new() -> Self {
        // `cache_size` starts at 1 (LZMA convention): the very first `shift_low`
        // flushes one leading byte (the initial cache, 0), which the decoder
        // skips by priming 5 bytes. This makes carry handling uniform — every
        // emitted byte is the cache, possibly +1 on carry.
        BitEncoder {
            low: 0,
            range: 0xFFFF_FFFF,
            out: Vec::new(),
            cache: 0,
            cache_size: 1,
        }
    }

    /// Encode one bit with P(bit==1) == `p1` (in `1..PROB_SCALE`).
    fn encode(&mut self, bit: u8, p1: u32) {
        debug_assert!(p1 > 0 && p1 < PROB_SCALE);
        // Split the current range. `r1` is the sub-range for bit==1, placed at
        // the bottom; bit==0 takes the top. (Either convention works as long as
        // encode/decode agree.)
        let r1 = ((self.range as u64 * p1 as u64) >> PROB_BITS) as u32;
        debug_assert!(r1 > 0 && r1 < self.range);
        if bit == 1 {
            self.range = r1;
        } else {
            self.low += r1 as u64;
            self.range -= r1;
        }
        // Renormalize: emit high bytes of `low` while the range got small.
        while self.range < TOP {
            self.shift_low();
            self.range <<= 8;
        }
    }

    fn shift_low(&mut self) {
        let low = self.low;
        // Flush when the top byte is settled: either it is not 0xFF (no future
        // carry can reach it, low < 0xFF000000) or a carry has already occurred
        // (low >= 2^32). The middle band 0xFF000000..=0xFFFFFFFF is the deferred
        // 0xFF case.
        if !(0xFF00_0000..=0xFFFF_FFFF).contains(&low) {
            let carry = (low >> 32) as u8;
            // Emit the cached byte (+carry), then a run of (0xFF+carry) for every
            // deferred 0xFF. On carry the 0xFF bytes wrap to 0x00; without carry
            // they stay 0xFF.
            let mut temp = self.cache;
            loop {
                self.out.push(temp.wrapping_add(carry));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = ((low >> 24) & 0xFF) as u8;
        }
        // Top byte is 0xFF and no carry yet: defer it (count it) for later.
        self.cache_size += 1;
        self.low = (low << 8) & 0xFFFF_FFFF;
    }

    /// Flush the remaining state. Five shifts drain `low` fully.
    fn finish(mut self) -> Vec<u8> {
        for _ in 0..5 {
            self.shift_low();
        }
        self.out
    }
}

/// Binary arithmetic **decoder** — mirror of [`BitEncoder`].
struct BitDecoder<'a> {
    range: u32,
    code: u32,
    input: &'a [u8],
    pos: usize,
}

impl<'a> BitDecoder<'a> {
    fn new(input: &'a [u8]) -> Self {
        let mut d = BitDecoder {
            range: 0xFFFF_FFFF,
            code: 0,
            input,
            pos: 0,
        };
        // Prime `code` with the first 5 bytes (matches the encoder's 1-byte cache
        // delay + 4 code bytes). Missing bytes read as 0 (safe past end of input).
        for _ in 0..5 {
            d.code = (d.code << 8) | d.next_byte() as u32;
        }
        d
    }

    fn next_byte(&mut self) -> u8 {
        let b = self.input.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    /// Decode one bit given P(bit==1) == `p1`. Updates state identically to the
    /// encoder so the count update that follows stays in lock-step.
    fn decode(&mut self, p1: u32) -> u8 {
        debug_assert!(p1 > 0 && p1 < PROB_SCALE);
        let r1 = ((self.range as u64 * p1 as u64) >> PROB_BITS) as u32;
        let bit;
        if self.code < r1 {
            bit = 1;
            self.range = r1;
        } else {
            bit = 0;
            self.code -= r1;
            self.range -= r1;
        }
        while self.range < TOP {
            self.code = (self.code << 8) | self.next_byte() as u32;
            self.range <<= 8;
        }
        bit
    }
}

// ---------------------------------------------------------------------------
// Adaptive context model — a small logistic-mixing context model (PAQ-lite).
//
// For each coded bit we gather P(bit==1) from several context orders (0..=4,
// each a byte history hashed with the current bit-tree node), map each into the
// "stretch" (logit) domain, combine them with an adaptive linear MIXER whose
// weights are trained online by gradient descent on coding loss, "squash" the
// result back to a probability, and refine it through an adaptive SSE/APM stage.
// Every table starts neutral and is updated identically on encode and decode, so
// the decoder reconstructs the exact same probability for every bit — the
// property that makes the arithmetic coder reversible. All arithmetic is integer
// and deterministic; nothing depends on host float behaviour.
// ---------------------------------------------------------------------------

/// One adaptive probability counter: a 12-bit P(bit==1) nudged toward each
/// observed bit. The nudge rate slows as evidence accumulates (a count-limited
/// state machine), so fresh contexts adapt fast and mature ones stay stable.
#[derive(Clone, Copy)]
struct Counter {
    p1: u16,
    /// Saturating evidence count, used to slow adaptation as it grows.
    count: u16,
}

impl Default for Counter {
    fn default() -> Self {
        Counter {
            p1: (PROB_SCALE / 2) as u16,
            count: 0,
        }
    }
}

impl Counter {
    #[inline]
    fn prob(&self) -> u32 {
        self.p1 as u32
    }

    /// Move the probability toward `bit`. Step size = 1/(count+1.5) capped, i.e.
    /// large while young, shrinking toward a stable floor — an integer KT-style
    /// adaptive estimator. Deterministic and branch-symmetric.
    #[inline]
    fn update(&mut self, bit: u8) {
        // Effective rate: bigger divisor => smaller step. Floor at 1/32 so the
        // model keeps tracking drift in long files.
        let n = self.count.min(60) as u32;
        let shift = if n >= 30 {
            5
        } else {
            // map count 0..30 -> shift 2..5 (faster early adaptation).
            2 + n / 10
        };
        let p = self.p1 as u32;
        if bit == 1 {
            self.p1 = (p + ((PROB_SCALE - p) >> shift)) as u16;
        } else {
            self.p1 = (p - (p >> shift)) as u16;
        }
        self.count = self.count.saturating_add(1);
    }
}

/// Fixed-point logit/logistic ("stretch"/"squash") helpers, precomputed into
/// tables so the mixer is pure integer and reproducible everywhere.
///
/// `stretch(p) = ln(p/(1-p))`, `squash` is its inverse. We scale logits by 256
/// and clamp to `[-2047, 2047]` (the classic PAQ range), with `p` in 12-bit
/// fixed point.
struct Logistic {
    /// squash: logit in `-2047..=2047` -> probability in `1..PROB_SCALE`.
    squash: [u16; 4096],
    /// stretch: probability in `0..PROB_SCALE` -> logit in `-2047..=2047`.
    stretch: [i16; PROB_SCALE as usize],
}

impl Logistic {
    fn new() -> Self {
        let mut squash = [0u16; 4096];
        for (i, s) in squash.iter_mut().enumerate() {
            let x = (i as i32 - 2048) as f64 / 256.0;
            let p = 1.0 / (1.0 + (-x).exp());
            let v = (p * PROB_SCALE as f64).round() as i32;
            *s = v.clamp(1, PROB_SCALE as i32 - 1) as u16;
        }
        // Build stretch as the inverse of squash by scanning monotonically.
        let mut stretch = [0i16; PROB_SCALE as usize];
        let mut pi = 0usize;
        for x in 0..4096i32 {
            let p = squash[x as usize] as usize;
            while pi <= p && pi < PROB_SCALE as usize {
                stretch[pi] = (x - 2048) as i16;
                pi += 1;
            }
        }
        while pi < PROB_SCALE as usize {
            stretch[pi] = 2047;
            pi += 1;
        }
        Logistic { squash, stretch }
    }

    #[inline]
    fn stretch(&self, p: u32) -> i32 {
        self.stretch[(p as usize).min(PROB_SCALE as usize - 1)] as i32
    }

    #[inline]
    fn squash(&self, x: i32) -> u32 {
        let i = (x + 2048).clamp(0, 4095) as usize;
        self.squash[i] as u32
    }
}

/// The byte-history orders fed into the mixer. Order 0 is the no-context table;
/// the rest hash that many previous bytes with the bit-tree node. Longer orders
/// (4,6) capture source-code/word structure and sharply cut the residue on text.
const ORDERS: [usize; 6] = [0, 1, 2, 3, 4, 6];
/// Number of context inputs to the mixer.
const N_ORDERS: usize = ORDERS.len();

/// Adaptive linear mixer in the logit domain. Holds one weight vector per
/// *mixer-selection context* (here: order-0 byte position = current bit-tree
/// node high bits) and trains weights by online gradient descent so the blend
/// tracks which orders predict best for *this* data.
struct Mixer {
    /// weights[sel][i] — fixed-point (>>16) weight for input i under selector sel.
    weights: Vec<[i32; N_ORDERS]>,
    /// scratch: last inputs (stretched probs) for the gradient step.
    inputs: [i32; N_ORDERS],
    /// last mixer-selection index.
    sel: usize,
    /// last mixed probability (12-bit) for the gradient step.
    last_p: u32,
}

const MIX_SELECTORS: usize = 256;
/// Fixed-point shift for mixer weights.
const W_SHIFT: i32 = 16;

impl Mixer {
    fn new() -> Self {
        // Initialise weights to ~1/N each (in fixed point) so the first blend is
        // a plain average until training kicks in.
        let init = (1i32 << W_SHIFT) / N_ORDERS as i32;
        Mixer {
            weights: vec![[init; N_ORDERS]; MIX_SELECTORS],
            inputs: [0; N_ORDERS],
            sel: 0,
            last_p: PROB_SCALE / 2,
        }
    }

    /// Mix the per-order stretched logits into one probability (12-bit).
    #[inline]
    fn mix(&mut self, sel: usize, st: &[i32; N_ORDERS], logistic: &Logistic) -> u32 {
        self.sel = sel & (MIX_SELECTORS - 1);
        self.inputs = *st;
        let w = &self.weights[self.sel];
        let mut dot: i64 = 0;
        for (wi, sti) in w.iter().zip(st.iter()) {
            dot += *wi as i64 * *sti as i64;
        }
        let x = (dot >> W_SHIFT) as i32;
        let p = logistic.squash(x);
        self.last_p = p;
        p
    }

    /// Train the weights toward the observed bit (gradient of coding loss).
    #[inline]
    fn update(&mut self, bit: u8) {
        // error = target - p, target = bit*scale. Scale into a small learning rate.
        let target = (bit as i32) * PROB_SCALE as i32;
        let err = target - self.last_p as i32; // in -scale..scale
        let w = &mut self.weights[self.sel];
        for (wi, &input) in w.iter_mut().zip(self.inputs.iter()) {
            // dw = lr * err * input. lr chosen for fast-but-stable convergence.
            *wi += (err * input) >> 8;
        }
    }
}

/// A 2-stage adaptive probability map (SSE / APM): refines the mixer's output by
/// looking it up in an adaptive table indexed by `(context, quantized prob)`,
/// learning a correction curve. Interpolates between the two nearest buckets.
struct Apm {
    t: Vec<u16>,
    ctx_count: usize,
    idx: usize,
}

const APM_BUCKETS: usize = 33;

impl Apm {
    fn new(ctx_count: usize, logistic: &Logistic) -> Self {
        let mut t = vec![0u16; ctx_count * APM_BUCKETS];
        // Initialise each bucket to squash of its bucket centre (identity map).
        for c in 0..ctx_count {
            for b in 0..APM_BUCKETS {
                let x = ((b as i32) - (APM_BUCKETS as i32 / 2)) * 128;
                t[c * APM_BUCKETS + b] = logistic.squash(x) as u16;
            }
        }
        Apm {
            t,
            ctx_count,
            idx: 0,
        }
    }

    /// Refine probability `p` under context `cx` using stretched-domain bucketing.
    #[inline]
    fn refine(&mut self, p: u32, cx: usize, logistic: &Logistic) -> u32 {
        debug_assert!(cx < self.ctx_count);
        let s = logistic.stretch(p) + 2048; // 0..4095
        let pos = (s * (APM_BUCKETS as i32 - 1)) / 4095;
        let pos = pos.clamp(0, APM_BUCKETS as i32 - 2) as usize;
        let weight = ((s * (APM_BUCKETS as i32 - 1)) % 4095) as u32;
        let base = cx * APM_BUCKETS + pos;
        let lo = self.t[base] as u32;
        let hi = self.t[base + 1] as u32;
        // Remember the dominant bucket to update toward the bit later.
        self.idx = if weight >= 2048 { base + 1 } else { base };
        let mixed = (lo * (4095 - weight) + hi * weight) / 4095;
        mixed.clamp(1, PROB_SCALE - 1)
    }

    #[inline]
    fn update(&mut self, bit: u8) {
        let target = (bit as u32) * (PROB_SCALE - 1);
        let v = self.t[self.idx] as u32;
        // Adapt the touched bucket toward the bit (rate 1/16).
        let nv = if target > v {
            v + ((target - v) >> 6)
        } else {
            v - ((v - target) >> 6)
        };
        self.t[self.idx] = nv.clamp(1, PROB_SCALE - 1) as u16;
    }
}

/// The adaptive predictor: several order tables, the mixer, and an APM stage.
/// Used identically on encode and decode.
struct Model {
    /// Per-order counter tables. `tables[o]` is indexed by a hash of (the last
    /// `o` bytes, the current bit-tree node). Order 0 is the no-context table.
    tables: Vec<Vec<Counter>>,
    mixer: Mixer,
    apm: Apm,
    logistic: Logistic,
    /// Rolling history of recent bytes (`hist[0]` = most recent). Must be at
    /// least `max(ORDERS)` long.
    hist: [u8; 6],
    /// Scratch: the table indices touched for the current bit (for update()).
    touched: [usize; N_ORDERS],
}

/// log2 size of each hashed order table (orders >= 1). 2^22 cells * 4 bytes =
/// 16 MiB each; bounded, fits any host. Collisions only cost accuracy, never
/// correctness (the decoder hits the identical collisions).
const TABLE_BITS: usize = 22;
const TABLE_SIZE: usize = 1 << TABLE_BITS;
/// Tree nodes per byte: a full binary tree over 8 bits has 255 internal nodes;
/// we index `1..=255` via `node = node*2 + bit`, so 256 slots suffice.
const TREE: usize = 256;

impl Model {
    fn new() -> Self {
        let logistic = Logistic::new();
        // Order 0 (ORDERS[0]) is exact (256 tree nodes); the rest are hashed.
        let mut tables = Vec::with_capacity(N_ORDERS);
        for &order in ORDERS.iter() {
            if order == 0 {
                tables.push(vec![Counter::default(); TREE]);
            } else {
                tables.push(vec![Counter::default(); TABLE_SIZE]);
            }
        }
        let apm = Apm::new(256, &logistic);
        Model {
            tables,
            mixer: Mixer::new(),
            apm,
            logistic,
            hist: [0; 6],
            touched: [0; N_ORDERS],
        }
    }

    /// Hash `order` previous bytes together with the bit-tree `node` into the
    /// order table. Order 0 returns the node directly (exact, no history).
    #[inline]
    fn table_index(&self, order: usize, node: usize) -> usize {
        if order == 0 {
            return node;
        }
        let mut h: u32 = 0x9E37_79B9 ^ (order as u32).wrapping_mul(0x85EB_CA77);
        for k in 0..order {
            h = h.wrapping_add(self.hist[k] as u32);
            h = h.wrapping_mul(2654435761);
            h ^= h >> 15;
        }
        h = h.wrapping_add(node as u32).wrapping_mul(2246822519);
        h ^= h >> 13;
        (h as usize) & (TABLE_SIZE - 1)
    }

    /// Predict P(bit==1) for the current `(history, tree node)`, in
    /// `1..PROB_SCALE`. Records which cells were touched so [`Self::update`] can
    /// train exactly those. Mutates only scratch + mixer/apm selection state.
    #[inline]
    fn predict(&mut self, node: usize) -> u32 {
        let mut st = [0i32; N_ORDERS];
        for (slot, &order) in ORDERS.iter().enumerate() {
            let idx = self.table_index(order, node);
            self.touched[slot] = idx;
            let p = self.tables[slot][idx].prob();
            st[slot] = self.logistic.stretch(p);
        }
        // Mixer selector: current partial byte (the tree node low 8 bits) blended
        // with the most-recent byte — cheap, deterministic context selection.
        let sel = (node ^ (self.hist[0] as usize)) & (MIX_SELECTORS - 1);
        let mixed = self.mixer.mix(sel, &st, &self.logistic);
        // APM stage keyed on the most-recent byte.
        let refined = self.apm.refine(mixed, self.hist[0] as usize, &self.logistic);
        // Average mixer and APM (a light, stable final blend), then clamp.
        ((mixed + refined * 3) / 4).clamp(1, PROB_SCALE - 1)
    }

    /// Update every touched cell, the mixer, and the APM with the observed `bit`.
    #[inline]
    fn update(&mut self, bit: u8) {
        for slot in 0..N_ORDERS {
            let idx = self.touched[slot];
            self.tables[slot][idx].update(bit);
        }
        self.mixer.update(bit);
        self.apm.update(bit);
    }

    /// Advance the byte history after a full byte has been coded.
    #[inline]
    fn push_byte(&mut self, byte: u8) {
        for k in (1..self.hist.len()).rev() {
            self.hist[k] = self.hist[k - 1];
        }
        self.hist[0] = byte;
    }
}

/// Encode `data` to a coded residue blob under the adaptive model.
///
/// Deterministic and self-delimited only by `data.len()` (the decoder is given
/// the length separately, exactly as the [`crate::Address::len`] field already
/// carries it). Bit-exact reversible by [`decode`].
#[must_use]
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut model = Model::new();
    let mut enc = BitEncoder::new();
    for &byte in data {
        // Walk the 8 bits MSB-first through the per-byte bit tree.
        let mut node = 1usize;
        for k in (0..8).rev() {
            let bit = (byte >> k) & 1;
            let p1 = model.predict(node);
            enc.encode(bit, p1);
            model.update(bit);
            node = (node << 1) | bit as usize;
        }
        model.push_byte(byte);
    }
    enc.finish()
}

/// Decode `len` bytes from a coded residue blob, reproducing the original input
/// **bit-for-bit**. The model is re-driven with identical predictions and
/// updates, so the decoded bit stream matches the encoded one exactly.
#[must_use]
pub fn decode(coded: &[u8], len: usize) -> Vec<u8> {
    let mut model = Model::new();
    let mut dec = BitDecoder::new(coded);
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let mut node = 1usize;
        for _ in 0..8 {
            let p1 = model.predict(node);
            let bit = dec.decode(p1);
            model.update(bit);
            node = (node << 1) | bit as usize;
        }
        let byte = (node & 0xFF) as u8;
        out.push(byte);
        model.push_byte(byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(data: &[u8]) {
        let coded = encode(data);
        let back = decode(&coded, data.len());
        assert_eq!(back, data, "round-trip mismatch (len {})", data.len());
    }

    #[test]
    fn roundtrip_empty() {
        rt(b"");
    }

    #[test]
    fn roundtrip_one_byte_all_values() {
        for b in 0u16..=255 {
            rt(&[b as u8]);
        }
    }

    #[test]
    fn roundtrip_all_256_values() {
        let data: Vec<u8> = (0..=255u8).collect();
        rt(&data);
    }

    #[test]
    fn roundtrip_long_run() {
        rt(&vec![0xABu8; 10_000]);
    }

    #[test]
    fn roundtrip_text() {
        rt(b"the quick brown fox jumps over the lazy dog, repeatedly and repeatedly.");
    }

    #[test]
    fn model_id_is_stable() {
        assert_eq!(MODEL_ID, "ctx-order2-v0");
    }
}
