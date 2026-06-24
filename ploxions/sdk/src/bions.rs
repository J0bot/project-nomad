//! `bions` — les **blocs de code partagés** par tous les ploxions : la BIONLIB du xion.
//!
//! José (2026-06-20) : « il faut faire le taff de tout diviser et partager le plus
//! le même code entre tous les ploxions, pour toujours reprendre les mêmes blocs
//! comme sur le papier ». Avant, **14 ploxions** ré-implémentaient `json_str`, 6 un
//! `to_hex`, 5 un `esc`… Ici on les sort UNE fois : chaque ploxion `use`-importe le
//! même bion au lieu d'en garder une copie. C'est la grammaire `bion -> ploxion`
//! appliquée au code lui-même — la voie vers le xerboxion.
//!
//! Tous déterministes et sans dépendance hôte. Comportement **identique** aux copies
//! locales qu'ils remplacent (vérifié par diff de sortie sur synthe/carte/kion/science).

// --- bions de parsing JSON ---------------------------------------------------

/// `"key":"..."` → la valeur string **décodée** (gère `\"`, `\n`, `\t`, `\\`).
pub fn json_str(s: &str, key: &str) -> Option<String> {
    json_str_from(s, key, 0).map(|(v, _)| v)
}

/// Comme [`json_str`] mais cherche à partir de `from` et renvoie aussi la position
/// (octet) juste après le guillemet fermant — pour itérer plusieurs champs.
pub fn json_str_from(s: &str, key: &str, from: usize) -> Option<(String, usize)> {
    let pat = format!("\"{key}\"");
    let i = s[from..].find(&pat)? + from + pat.len();
    let c = s[i..].find(':')? + 1;
    let tail = s[i + c..].trim_start().strip_prefix('"')?;
    let base = s.len() - tail.len();
    let mut out = String::new();
    let mut end = base;
    let mut chars = tail.char_indices();
    while let Some((off, ch)) = chars.next() {
        if ch == '\\' {
            if let Some((_, n)) = chars.next() {
                match n {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000C}'),
                    'u' => {
                        // \uXXXX : lit les 4 hex suivants
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some((_, h)) = chars.next() {
                                hex.push(h);
                            }
                        }
                        if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(cp) {
                                out.push(c);
                            }
                        }
                    }
                    other => out.push(other),
                }
            }
        } else if ch == '"' {
            end = base + off + 1;
            break;
        } else {
            out.push(ch);
        }
    }
    Some((out, end))
}

/// `"key":N` entier signé ; renvoie `default` si absent/illisible.
pub fn json_int(s: &str, key: &str, default: i64) -> i64 {
    let pat = format!("\"{key}\"");
    let Some(i) = s.find(&pat) else { return default };
    let i = i + pat.len();
    let Some(c) = s[i..].find(':') else { return default };
    let tail = s[i + c + 1..].trim_start();
    let neg = tail.starts_with('-');
    let t2 = tail.strip_prefix('-').unwrap_or(tail);
    let mut n: i64 = 0;
    let mut seen = false;
    for ch in t2.chars() {
        if ch.is_ascii_digit() {
            n = n * 10 + (ch as i64 - '0' as i64);
            seen = true;
        } else {
            break;
        }
    }
    if seen {
        if neg { -n } else { n }
    } else {
        default
    }
}

/// `"key":N` **entier non signé** (`u64`) — prend les chiffres ascii après le `:`,
/// `None` si absent ou pas de chiffre. Le bloc partagé par les adaptateurs (qui le
/// recopiaient en `json_u64`/`json_u16`). Pour un `u16` : `json_uint(j,k).and_then(|v| u16::try_from(v).ok())`.
pub fn json_uint(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// `"key":N` flottant (gère `-`, `+`, `.`, `e`/`E`).
pub fn json_num(s: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)? + pat.len();
    let rest = &s[i..];
    let c = rest.find(':')? + 1;
    let tail = rest[c..].trim_start();
    let end = tail
        .find(|ch: char| {
            !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' || ch == 'E')
        })
        .unwrap_or(tail.len());
    tail[..end].parse::<f64>().ok()
}

// --- bions de parsing profond (tableaux JSON, sans parseur complet) ----------

/// Le **token numérique** brut de `"key"` (int ou float), p.ex. `{"lat":-12.5}`
/// → `Some("-12.5")`. Tolère `-`/`+`/`.`/`e`/`E`. Le bloc partagé par les
/// adaptateurs (osiris/repoverse) qui le recopiaient.
pub fn json_num_raw(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let tok: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+' || *c == '.' || *c == 'e' || *c == 'E')
        .collect();
    if tok.is_empty() || tok == "-" {
        None
    } else {
        Some(tok)
    }
}

/// Le **corps** du premier tableau JSON nommé `"key"` (le texte entre ses `[` et
/// `]` appariés), p.ex. `{"repos":[ … ]}`. Conscient des chaînes + de la
/// profondeur de crochets (les tableaux imbriqués ne le terminent pas tôt).
pub fn array_body<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let kpos = json.find(&needle)? + needle.len();
    let rest = &json[kpos..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open_rel = after.find('[')?;
    let after_open = &after[open_rel + 1..];
    let bytes = after_open.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' => depth += 1,
            b']' => {
                if depth == 0 {
                    return Some(&after_open[..i]);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Découpe le corps d'un tableau (le texte entre `[` et `]`) en ses sous-chaînes
/// d'objets `{…}` de premier niveau, conscient des accolades + des chaînes.
pub fn split_objects(array_body: &str) -> Vec<&str> {
    let bytes = array_body.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start.take() {
                        out.push(&array_body[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

// --- bions d'échappement -----------------------------------------------------

/// Échappe pour rendu **HTML** (`&` `<` `>`).
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Échappe pour réinjection dans une **string JSON** (`\` `"` `\n` `\t`).
pub fn json_esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t")
}

// --- bions d'encodage / hachage ----------------------------------------------

/// Octets → hex minuscule (2 caractères par octet).
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &x in bytes {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// FNV-1a 64-bit d'une string (adresse déterministe stable).
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// `true` si la chaîne est une URL `http(s)://…`.
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

// --- bions de cycle de vie (décodage des args d'événement) -------------------

/// Décode une paire `(ptr, len)` que l'hôte a passée dans `plc_on_event` en
/// `String` (lossy UTF-8). Le bloc partagé qui évite le `String::from_utf8_lossy(
/// unsafe { read_args(..) }).into_owned()` recopié dans chaque ploxion.
pub fn decode_str(ptr: i32, len: i32) -> String {
    String::from_utf8_lossy(unsafe { crate::read_args(ptr, len) }).into_owned()
}

/// Décode le couple `(topic, payload)` d'un `plc_on_event` d'un coup.
pub fn decode_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) -> (String, String) {
    (decode_str(topic_ptr, topic_len), decode_str(payload_ptr, payload_len))
}

// --- bion de la machine à tsoins : la frame tsoin.record ---------------------

/// Émet la **frame canonique** `tsoin.record {"name":<name>,"bytes":<hex>}` sur
/// le bus — le générateur (name) + le résidu (les octets du réel en hex). C'est
/// le bloc partagé par tous les ploxions qui gravent un tsoin ; le nom est pris
/// **brut**, les octets sont hexés ici. (Voir [`tsoin_record_hex`] si le hex est
/// déjà calculé ou si le nom doit être échappé par l'appelant.)
pub fn tsoin_record(name: &str, bytes: &[u8]) {
    tsoin_record_hex(name, &to_hex(bytes));
}

/// Variante : le **hex est déjà calculé** (et l'appelant a déjà échappé le nom si
/// besoin). Le bloc commun = la **forme** de la frame `{"name":…,"bytes":…}` ;
/// l'encodage/échappement reste le résidu propre à chaque ploxion.
pub fn tsoin_record_hex(name: &str, hex: &str) {
    crate::emit(
        "tsoin.record",
        format!("{{\"name\":\"{name}\",\"bytes\":\"{hex}\"}}").as_bytes(),
    );
}

// ============================================================================
// --- bions de la machine à tsoins : adressage GÉNÉRATIF + dédup -------------
// ============================================================================
//
// José : la colonne de la machine à tsoins, c'est l'**adressage génératif** —
// un tsoin n'est plus repéré par un id séquentiel (« le 3ᵉ qu'on a gravé »)
// mais par son CONTENU : `addr = addr64(générateur, résidu)`.
//
// Deux gravures du même (générateur, résidu) tombent sur la MÊME adresse : on
// peut donc dédupliquer et compter les références sans jamais re-stocker.
//
// Ces bions (`addr64` + `fnv1a64_bytes` + `from_hex` + `DedupTable`) sont sortis
// ICI, une fois, pour que `tsoin-store` — et tout futur ploxion à contenu-adressé
// — reprenne le même bloc au lieu d'en recopier la logique. C'est « faire plus
// de bions ».
//
// ⚠️ `addr64` est un handle de DÉDUP **non-cryptographique** (64 bits, FNV-1a) :
// il sert d'index, PAS de preuve d'intégrité. Une collision FNV est possible
// (anniversaire ~1/2^32) ; tout consommateur qui dédup sur l'égalité d'adresse
// DOIT vérifier l'égalité des octets `(gen, residu)` avant de conclure « dédup »
// (cf. `tsoin-store::handle_put`). L'intégrité bit-exact, elle, reste le rôle du
// moteur `tsoin` (store BLAKE3 / Merkle).

/// **Décode** une string hex (minuscule ou majuscule) en octets. L'inverse de
/// [`to_hex`]. `None` si la longueur est impaire ou si un caractère n'est pas
/// hex. Le bloc partagé que `tsoin`/`tsoin-store` recopiaient en local : le
/// résidu d'un tsoin voyage en hex dans le JSON, ce bion le ramène aux octets.
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

// ============================================================================
// --- bions du DIFF : XOR-delta, popcount (surprise), résidu minimal ---------
// ============================================================================
//
// Le cœur du résidu de la machine à tsoins : entre deux états `a` et `b`, le
// résidu N'EST PAS `b` entier mais ce qui DIFFÈRE de `a` — le delta. On le
// calcule en XOR (réversible : `b == a ^ delta`), on mesure la SURPRISE (les
// bits qui ont changé = la DISTANCE DE HAMMING en bits entre `a` et `b`, une
// borne brute de nouveauté — PAS l'entropie de Shannon), et on COMPRIME le
// delta par run-length des zéros (deux états identiques ⇒ delta tout-zéro ⇒
// résidu ~0, surprise 0). Ces trois bions, sortis ICI une fois, donnent à TOUT
// ploxion le moyen de produire son résidu minimal pour nourrir `tsoin-store`.

/// Le **XOR-delta** général entre deux suites d'octets `a` et `b`.
///
/// - Même longueur ⇒ XOR octet par octet (`out[i] = a[i] ^ b[i]`), réversible :
///   `b == a ^ out` partout.
/// - Longueurs différentes ⇒ XOR sur le préfixe commun (`min(len)`) puis on
///   **ajoute la queue** du plus long tel quel (l'autre côté y est implicitement
///   zéro, donc `x ^ 0 == x`). Le résultat a la longueur du plus long, et reste
///   réversible côté préfixe ; la queue EST la nouveauté brute.
///
/// Deux blocs identiques ⇒ delta entièrement nul (cf. [`residu_minimal`] et
/// [`popcount_bytes`] : résidu compressé minuscule, surprise nulle).
pub fn xor_delta(a: &[u8], b: &[u8]) -> Vec<u8> {
    let n = a.len().min(b.len());
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    for i in 0..n {
        out.push(a[i] ^ b[i]);
    }
    // La queue du plus long (l'autre côté = 0 ⇒ XOR = la queue elle-même).
    if a.len() > b.len() {
        out.extend_from_slice(&a[n..]);
    } else if b.len() > a.len() {
        out.extend_from_slice(&b[n..]);
    }
    out
}

/// La **surprise** = le nombre de BITS à 1 dans la suite d'octets (population
/// count global). Appliqué à un [`xor_delta`], c'est le nombre exact de bits qui
/// DIFFÈRENT entre `a` et `b` : la **distance de Hamming en bits** — `0` si
/// identiques, au plus `8 * len`. C'est une borne brute de « combien de neuf »
/// (poids de Hamming), PAS l'entropie de Shannon (qui dépendrait de la
/// distribution des symboles, pas du nombre de bits flippés).
pub fn popcount_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().map(|&x| x.count_ones() as u64).sum()
}

/// Le **résidu MINIMAL** : la compression run-length des octets d'un delta, qui
/// effondre les longues plages de zéros (le « rien n'a changé ici » d'un
/// [`xor_delta`]). Deux états identiques ⇒ delta tout-zéro ⇒ ce résidu est
/// minuscule (5 octets quelle que soit la taille), ce qui matérialise « pas de
/// surprise = quasi pas de résidu ».
///
/// **Format** (préfixe-libre, déterministe, décodable par [`residu_expand`]) :
/// une suite de tokens —
/// - `0x00  <count: u32 LE>`                      ⇒ une plage de `count` octets ZÉRO ;
/// - `0x01  <count: u32 LE>  <count octets bruts>` ⇒ une plage littérale (non gérée
///   comme zéros) recopiée telle quelle.
///
/// Les plages littérales gardent leurs octets nuls internes éventuels : on ne
/// coupe une plage littérale que sur un zéro qui PROLONGE en une vraie plage de
/// zéros — la longueur d'un cycle de zéros n'est extraite que si elle vaut. Ici
/// on reste simple et exact : on alterne plages de zéros / plages de non-zéros.
pub fn residu_minimal(delta: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < delta.len() {
        if delta[i] == 0 {
            // Mesure la plage de zéros.
            let start = i;
            while i < delta.len() && delta[i] == 0 {
                i += 1;
            }
            let count = (i - start) as u32;
            out.push(0x00);
            out.extend_from_slice(&count.to_le_bytes());
        } else {
            // Mesure la plage de non-zéros, recopiée littéralement.
            let start = i;
            while i < delta.len() && delta[i] != 0 {
                i += 1;
            }
            let lit = &delta[start..i];
            let count = lit.len() as u32;
            out.push(0x01);
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(lit);
        }
    }
    out
}

/// L'inverse de [`residu_minimal`] : ré-étend un résidu minimal en son delta
/// d'origine (les plages de zéros redeviennent des octets nuls). `None` si le
/// flux est tronqué ou contient un tag inconnu — pour qu'un consommateur puisse
/// VÉRIFIER que `residu_expand(residu_minimal(d)) == d` (round-trip exact).
pub fn residu_expand(min: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < min.len() {
        let tag = min[i];
        i += 1;
        if i + 4 > min.len() {
            return None;
        }
        let count = u32::from_le_bytes([min[i], min[i + 1], min[i + 2], min[i + 3]]) as usize;
        i += 4;
        match tag {
            0x00 => out.resize(out.len() + count, 0u8),
            0x01 => {
                if i + count > min.len() {
                    return None;
                }
                out.extend_from_slice(&min[i..i + count]);
                i += count;
            }
            _ => return None,
        }
    }
    Some(out)
}

/// FNV-1a 64-bit sur **octets bruts** (variante de [`fnv1a64`] qui prend une
/// `&str`). Même constante d'amorçage et même premier : pour une string,
/// `fnv1a64(s) == fnv1a64_bytes(s.as_bytes())`. Sorti pour hacher un résidu
/// binaire (les octets du réel) sans passer par une string.
pub fn fnv1a64_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// **ADRESSE GÉNÉRATIVE** d'un tsoin : le handle déterministe content-addressed
/// du couple `(générateur, résidu)`.
///
/// Le générateur est une string (le « nom » / le code qui produit l'état) ;
/// le résidu sont les octets bruts du réel. Même couple `(gen, residu)` ⇒ même
/// `addr64`, partout, à jamais. C'est la clé de voûte de la dédup (cf.
/// [`DedupTable`]) et de la requête par générateur.
///
/// **Construction (anti-symétrie + domain-separation).** On ne fait PAS un XOR
/// naïf `fnv(gen) ^ fnv(residu)` : comme `fnv1a64(s) == fnv1a64_bytes(s.as_bytes())`
/// et que le XOR est commutatif, un tel schéma rendrait l'adresse INVARIANTE par
/// échange `gen ↔ residu` (`addr("a", b"X") == addr("X", b"a")`) — une collision
/// *constructible*. On hache donc une **concaténation canonique len-préfixée** en
/// UNE seule passe FNV-1a : `len(gen) (LE u64) ‖ gen ‖ len(residu) (LE u64) ‖ residu`.
/// Le préfixe de longueur sépare les domaines, casse la symétrie, et garantit
/// qu'aucun autre couple ne peut produire le même flux d'octets (préfixe-libre).
pub fn addr64(gen: &str, residu: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    let gen_b = gen.as_bytes();
    for &x in &(gen_b.len() as u64).to_le_bytes() {
        feed(x);
    }
    for &x in gen_b {
        feed(x);
    }
    for &x in &(residu.len() as u64).to_le_bytes() {
        feed(x);
    }
    for &x in residu {
        feed(x);
    }
    h
}

/// La **table de dédup adressée** de la machine à tsoins : `addr64 -> refcount`.
///
/// On ne stocke jamais deux fois le même tsoin : la première gravure d'une
/// adresse l'insère (refcount = 1, `inserted = true`) ; toute gravure ultérieure
/// de la même adresse **incrémente seulement le compteur** (`inserted = false`).
/// Le refcount mesure combien de fois cette adresse a été (re)présentée.
///
/// ⚠️ La table dédup sur l'ADRESSE seule (refcount minimal, bion réutilisable) ;
/// elle ne connaît pas les octets. C'est à l'appelant de VÉRIFIER, sur
/// `inserted == false`, que le `(gen, residu)` entrant est bien identique à celui
/// déjà détenu (égalité des octets), pour distinguer une vraie dédup d'une
/// collision FNV (cf. [`addr64`]). Le STOCKAGE des octets reste le résidu propre
/// au ploxion qui l'utilise.
#[derive(Default)]
pub struct DedupTable {
    counts: std::collections::HashMap<u64, u64>,
}

impl DedupTable {
    /// Table vide.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Présente l'adresse `addr`. Renvoie `(refcount_après, inserted)` :
    /// - `inserted == true`  ⇒ première fois (refcount devient 1) : l'appelant
    ///   DOIT stocker les octets.
    /// - `inserted == false` ⇒ adresse déjà connue : refcount incrémenté.
    ///   L'appelant DOIT alors vérifier l'égalité des octets avant de conclure
    ///   « dédup » (sinon c'est une collision).
    pub fn touch(&mut self, addr: u64) -> (u64, bool) {
        match self.counts.get_mut(&addr) {
            Some(c) => {
                *c += 1;
                (*c, false)
            }
            None => {
                self.counts.insert(addr, 1);
                (1, true)
            }
        }
    }

    /// Annule la dernière incrémentation de `addr` (compteur −1, retiré si 0).
    /// Pour le cas collision : `touch` a incrémenté, mais l'appelant constate que
    /// les octets diffèrent et refuse la dédup ; il rembobine le compteur.
    pub fn untouch(&mut self, addr: u64) {
        if let Some(c) = self.counts.get_mut(&addr) {
            *c -= 1;
            if *c == 0 {
                self.counts.remove(&addr);
            }
        }
    }

    /// `true` si l'adresse a déjà été gravée au moins une fois.
    pub fn contains(&self, addr: u64) -> bool {
        self.counts.contains_key(&addr)
    }

    /// Le compteur de références de `addr` (`0` si jamais gravée).
    pub fn refcount(&self, addr: u64) -> u64 {
        self.counts.get(&addr).copied().unwrap_or(0)
    }

    /// Nombre d'adresses **distinctes** détenues (tsoins uniques après dédup).
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// `true` si aucune adresse n'a encore été gravée.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Somme de tous les refcounts : le nombre **total de présentations** (avant
    /// dédup), à distinguer de [`len`](Self::len) (adresses distinctes après dédup).
    pub fn total_touches(&self) -> u64 {
        self.counts.values().sum()
    }
}

// ============================================================================
// --- tests : ancrage des bions du DIFF (round-trip + propriétés clés) -------
// ============================================================================

#[cfg(test)]
mod diff_tests {
    use super::{popcount_bytes, residu_expand, residu_minimal, xor_delta};

    /// `residu_expand(residu_minimal(d)) == d` pour tout delta — l'inverse exact.
    /// Ancre `residu_expand` (sinon code public non couvert) ET prouve la
    /// réversibilité du résidu minimal.
    #[test]
    fn residu_round_trip() {
        let cases: &[&[u8]] = &[
            &[],
            &[0, 0, 0, 0],
            &[1, 2, 3, 4],
            &[0, 0, 1, 0, 0, 2, 3, 0],
            &[1, 0, 1, 0, 1], // delta dense : RLE gonfle mais reste réversible.
            &[0xff; 17],
        ];
        for &d in cases {
            let min = residu_minimal(d);
            assert_eq!(residu_expand(&min).as_deref(), Some(d), "round-trip {d:?}");
        }
    }

    /// Flux tronqué ou tag inconnu ⇒ `None` (jamais de panic / OOB).
    #[test]
    fn residu_expand_rejects_garbage() {
        assert_eq!(residu_expand(&[0x00, 0x01]), None); // header u32 tronqué.
        assert_eq!(residu_expand(&[0x01, 0x02, 0, 0, 0, 0x41]), None); // littéral court.
        assert_eq!(residu_expand(&[0x07, 0, 0, 0, 0]), None); // tag inconnu.
    }

    /// XOR-delta réversible sur le préfixe commun ; queue brute du plus long.
    #[test]
    fn xor_delta_reversible_and_tail() {
        let a = [0xa0, 0x0f, 0x12];
        let b = [0xaa, 0x0f, 0x12, 0x99, 0x88];
        let d = xor_delta(&a, &b);
        assert_eq!(d.len(), 5);
        // Préfixe commun : b[i] == a[i] ^ d[i].
        for i in 0..a.len() {
            assert_eq!(b[i], a[i] ^ d[i]);
        }
        // Queue = octets bruts du plus long (a côté = 0).
        assert_eq!(&d[a.len()..], &b[a.len()..]);
        // Identiques ⇒ delta tout-zéro ⇒ surprise nulle.
        assert_eq!(popcount_bytes(&xor_delta(&a, &a)), 0);
    }

    /// `popcount_bytes` = poids de Hamming en bits (distance, pas Shannon).
    #[test]
    fn popcount_is_hamming_weight() {
        assert_eq!(popcount_bytes(&[0xff, 0x0f]), 12);
        assert_eq!(popcount_bytes(&[0, 0, 0]), 0);
    }
}

// ============================================================================
// --- bions du GENERATOR : reconstruction residu -> reel (P3 du triplet) ------
// ============================================================================
//
// Le sens INVERSE de la machine a tsoins. Partout les ploxions font reel ->
// residu (carte replie, kion collapse, spectre decompose, diff XOR-delta,
// xerbion predit-et-JETTE). Il manquait le retour : reconstruire le reel a
// partir du residu + (optionnel) la base = l'etat precedent. C'est le replay,
// la moitie qui rend la compression LOSSLESS *verifiable* (re-addr64 du reel
// reconstruit == l'adresse attendue) -- DANS LES LIMITES du format (cf. infra).
//
// On factorise ici DEUX choses :
//   1. `gen_apply` : le dispatch par `kind` (xor-delta / lcg / raw) ;
//   2. `lcg_next` / `lcg_bytes` : SEULE LA RECURRENCE LCG (Knuth/PCG-adjacent
//      *6364136223846793005 +1442695040888963407, wrapping) est partagee avec
//      `xerbion::seed_weights`. Le TAP de sortie differe (cf. lcg_bytes).
//
// ⚠️ LIMITE DE REVERSIBILITE DE xor-delta (importante, ancree par les tests) :
//    le XOR-delta de la machine (`xor_delta`) ENCODE le prefixe commun + la
//    queue BRUTE du plus long, mais N'ENCODE PAS la longueur du reel cible. Il
//    est donc reversible reel<->residu UNIQUEMENT quand `base.len() <= reel.len()`
//    (le reel ne RETRECIT pas sous la base). Quand `base` est plus longue que le
//    reel, `gen_apply("xor-delta", residu_minimal(xor_delta(base, reel)), base)`
//    rend un reel de longueur `base.len()` (queue mise a zero), PAS le reel court
//    -- l'info de longueur est perdue par le format. C'est une propriete du
//    format `residu_minimal`/`xor_delta` partage avec `diff`/`tsoin-store`, pas un
//    bug local ; on ne change pas ce format (interop) -- on le DOCUMENTE et on le
//    VERROUILLE par un test negatif (cf. `xor_delta_shrink_is_not_reversible`).

/// Le **LCG canonique** du xion : la recurrence de Knuth (les constantes de
/// `PCG`/`splitmix`-adjacent) `state' = state * 6364136223846793005 + 1442695040888963407`
/// (en arithmetique `wrapping`). Deterministe, rejouable : meme graine ⇒ meme
/// suite, a jamais.
///
/// ⚠️ Ce qui est partage avec `xerbion::seed_weights` est UNIQUEMENT cette
/// **recurrence** (`lcg_next`). Le TAP de sortie n'est PAS partage : `xerbion`
/// derive ses poids `f64` via `(state >> 33)`, alors que [`lcg_bytes`] prend
/// l'octet de poids fort `(state >> 56)`. Un refactor `xerbion -> lcg_next`
/// reutiliserait donc `lcg_next` SEUL (et son propre mapping `f64`), pas
/// `lcg_bytes`. Renvoie le **nouvel etat** ; pour un flux d'octets voir [`lcg_bytes`].
pub fn lcg_next(state: u64) -> u64 {
    state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}

/// Le **flux d'octets deterministe** de longueur `n` engendre par le LCG a partir
/// de `seed`. On avance l'etat a chaque pas et on prend l'octet de poids fort
/// (`>> 56`) — le bit haut d'un LCG est de bien meilleure qualite que le bas
/// (dont la periode des bits faibles est courte). Meme `(seed, n)` ⇒ meme suite,
/// partout : c'est un **generateur** au sens de la machine a tsoins (le `gen`
/// d'un tsoin peut etre « lcg »: le reel ENTIER se rederive de la seule graine,
/// residu de 8 octets quelle que soit la longueur du reel).
///
/// Le tap `>> 56` est PROPRE au generator : ce n'est PAS le flux que `xerbion`
/// consomme (lui prend `>> 33` en `f64`). Seule [`lcg_next`] (la recurrence) est
/// commune.
pub fn lcg_bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        s = lcg_next(s);
        out.push((s >> 56) as u8);
    }
    out
}

/// **Le dispatch du generator** : reconstruit le REEL a partir d'un `residu` et,
/// selon le `kind`, d'une `base` (l'etat precedent). L'INVERSE de la reduction
/// reel -> residu de chaque ploxion. `None` si le residu est mal forme pour le
/// `kind` (flux tronque, seed absente…), pour que l'appelant puisse VERIFIER
/// plutot que produire un reel faux.
///
/// `kind` :
/// - `"raw"`       ⇒ le residu EST le reel (aucune compression). `base` ignoree.
/// - `"xor-delta"` ⇒ le residu est un **residu minimal** (RLE des zeros d'un
///   XOR-delta, cf. [`residu_minimal`]). On le re-etend ([`residu_expand`]) en
///   delta brut, puis `reel = base XOR delta` via [`xor_delta`]. C'est le replay
///   du codage predictif `b = a ^ (a ^ b)`.
///
///   ⚠️ **Reversibilite CONDITIONNELLE** : garantie SEULEMENT quand
///   `base.len() <= reel.len()` (le reel ne retrecit pas sous la base). Alors
///   `gen_apply("xor-delta", residu_minimal(xor_delta(base, reel)), base) == Some(reel)`.
///   Quand `base.len() > reel.len()`, `xor_delta(base, reel)` a longueur
///   `base.len()` et perd la longueur du reel court ; le replay rend un reel de
///   longueur `base.len()` (queue a zero), PAS le reel court. Ne PAS s'appuyer
///   sur xor-delta pour un reel plus court que la base (utiliser `raw` / `lcg`,
///   ou changer le format pour transporter `len(reel)`).
/// - `"lcg"`       ⇒ le residu sont **8 octets** = une graine `u64` (little-endian)
///   et le reel se rederive entierement par [`lcg_bytes`]. La longueur a
///   reconstruire = `base.len()` (la base sert ici de gabarit de taille). ⚠️ Une
///   base vide ⇒ reel vide (la taille du flux vient ENTIEREMENT de `base.len()`).
pub fn gen_apply(kind: &str, residu: &[u8], base: &[u8]) -> Option<Vec<u8>> {
    match kind {
        "raw" => Some(residu.to_vec()),
        "xor-delta" => {
            // Residu minimal -> delta brut -> reel = base ^ delta (reuse xor_delta).
            // Reversible ssi base.len() <= reel.len() (cf. doc + test negatif).
            let delta = residu_expand(residu)?;
            Some(xor_delta(base, &delta))
        }
        "lcg" => {
            // Le residu = la graine (u64 LE, 8 octets) ; la base donne la taille.
            if residu.len() != 8 {
                return None;
            }
            let seed = u64::from_le_bytes([
                residu[0], residu[1], residu[2], residu[3], residu[4], residu[5], residu[6],
                residu[7],
            ]);
            Some(lcg_bytes(seed, base.len()))
        }
        _ => None,
    }
}

// ============================================================================
// --- tests : ancrage des bions du GENERATOR (reversibilite reel<->residu) ---
// ============================================================================

#[cfg(test)]
mod generator_tests {
    use super::{gen_apply, lcg_bytes, lcg_next, residu_minimal, xor_delta};

    /// LA propriete cle, DANS SA LIMITE : reconstruire `reel` depuis
    /// (residu xor-delta, base) rend `reel` quand `base.len() <= reel.len()`
    /// (le reel ne retrecit pas). On NE met PAS de cas a-plus-long ici : le
    /// format xor-delta ne l'encode pas (verrouille par
    /// `xor_delta_shrink_is_not_reversible`).
    #[test]
    fn xor_delta_round_trips_when_reel_not_shorter() {
        let cases: &[(&[u8], &[u8])] = &[
            (&[1, 2, 3, 4], &[1, 2, 3, 4]),           // identiques ⇒ delta nul.
            (&[0xa0, 0x0f, 0x12], &[0xaa, 0x0f, 0x12]), // meme longueur.
            (&[1, 2, 3], &[1, 2, 3, 9, 8, 7]),         // reel plus long (croit, OK).
            (&[], &[5, 6, 7]),                          // base vide -> tout est nouveau.
            (&[], &[]),                                 // les deux vides.
        ];
        for &(base, reel) in cases {
            assert!(base.len() <= reel.len(), "precondition du cas");
            let residu = residu_minimal(&xor_delta(base, reel));
            assert_eq!(
                gen_apply("xor-delta", &residu, base).as_deref(),
                Some(reel),
                "replay base={base:?} -> reel={reel:?}"
            );
        }
    }

    /// VERROU de la LIMITE : quand le reel RETRECIT (`base.len() > reel.len()`),
    /// xor-delta n'est PAS reversible — le format perd la longueur du reel court.
    /// On documente le comportement EXACT (queue a zero, longueur = base.len())
    /// pour que personne ne s'y fie a tort. C'est une propriete du format
    /// `xor_delta`/`residu_minimal` partage avec diff/tsoin-store, pas un bug local.
    #[test]
    fn xor_delta_shrink_is_not_reversible() {
        let base: &[u8] = &[1, 2, 3, 9, 8, 7];
        let reel: &[u8] = &[1, 2, 3];
        let residu = residu_minimal(&xor_delta(base, reel));
        let got = gen_apply("xor-delta", &residu, base);
        // PAS Some(reel) : on recupere la queue de base remise a zero, longueur base.len().
        assert_eq!(got.as_deref(), Some(&[1, 2, 3, 0, 0, 0][..]));
        assert_ne!(got.as_deref(), Some(reel));
    }

    /// `raw` rend le residu tel quel ; `base` est ignoree.
    #[test]
    fn raw_is_identity() {
        assert_eq!(
            gen_apply("raw", &[9, 8, 7], &[1, 2, 3, 4]).as_deref(),
            Some(&[9, 8, 7][..])
        );
    }

    /// `lcg` est deterministe et rederive toute la longueur depuis la graine ;
    /// la base ne sert que de gabarit de taille.
    #[test]
    fn lcg_deterministic_and_sized_by_base() {
        let seed: u64 = 0x9E3779B97F4A7C15;
        let residu = seed.to_le_bytes();
        let base = [0u8; 5]; // 5 octets ⇒ on veut 5 octets de flux.
        let got = gen_apply("lcg", &residu, &base).unwrap();
        assert_eq!(got, lcg_bytes(seed, 5));
        assert_eq!(got.len(), 5);
        // Base vide -> reel vide (la taille vient de base.len()).
        assert_eq!(gen_apply("lcg", &residu, &[]).as_deref(), Some(&[][..]));
        // Stabilite du bion brut (recurrence ancree).
        assert_eq!(lcg_next(0), 1442695040888963407);
    }

    /// kind inconnu / residu mal forme ⇒ None (jamais de reel faux silencieux).
    #[test]
    fn rejects_unknown_kind_and_bad_lcg() {
        assert_eq!(gen_apply("zigzag", &[1, 2], &[]), None);
        assert_eq!(gen_apply("lcg", &[1, 2, 3], &[0u8; 4]), None); // graine != 8 octets.
    }
}

// ============================================================================
// --- bions de l'HORLOGE DU XION : fenetre de coherence + tick gate ----------
// ============================================================================
//
// Loi du cahier p.42 (Jose, signal #596) : « le temps n'existe pas, il faut le
// recreer ; le temps = la coherence = la synchronisation ». Le wall-clock n'a
// aucun sens pour le xion — le seul temps qui compte est : a quel point
// l'orchestre est SYNCHRONE. La `carte` mesure deja une coherence EN BATCH
// (`t = c*1000/max_cat` : la part du theme le plus represente dans un nuage de
// points, carte/src/lib.rs:132-141). On sort ICI la version LIVE (flux) de
// cette MEME mesure de concentration : une FENETRE glissante des derniers topics
// observes sur le bus, et un compteur logique qui n'avance QUE quand la part
// dominante est suffisante (l'orchestre est d'accord sur une adresse).
//
// Deux bions, factorises une fois pour `clock-coherence` et tout futur ploxion
// qui veut une horloge conforme (un player, la carte en mode live) :
//   - `CoherenceWindow` : ring-buffer borne des TOPICS recents (les strings
//     brutes, comme carte bucketise sur la string `cat` litterale) +
//     `coherence_milli()` = part du topic dominant (le streaming-twin EXACT de
//     `c*1000/max_cat`).
//   - `GatedTick` : un compteur qui n'avance que si un ratio milli >= seuil.

/// La **FENETRE DE COHERENCE** : un ring-buffer borne des derniers topics vus
/// passer sur le bus. La coherence = a quel point l'orchestre est synchrone,
/// mesuree LIVE (le pendant flux de la mesure batch de `carte`).
///
/// On garde les `cap` derniers TOPICS (les strings brutes, exactement comme
/// `carte` bucketise sur la string `cat` litterale — pas de hash, donc pas de
/// collision a documenter) dans un anneau (le plus vieux est ecrase). La mesure
/// [`CoherenceWindow::coherence_milli`] = la PART du topic DOMINANT dans la
/// fenetre, en milli (`0..=1000`). C'est le streaming-twin exact de la coherence
/// batch de `carte` (`count_du_theme * 1000 / max_count`).
///
/// Lecture de la mesure (CONCENTRATION — meme esprit que `carte`, qui recompense
/// la densite thematique) :
/// - **un seul topic domine** la fenetre ⇒ l'orchestre bat a l'unisson sur une
///   adresse ⇒ coherence HAUTE ;
/// - **aucune adresse ne domine** (tout le monde parle de choses differentes, ou
///   plusieurs factions se partagent egalement la fenetre) ⇒ dis-coordonne ⇒
///   coherence BASSE.
///
/// Formellement, avec `f` = nombre d'observations dans la fenetre (`<= cap`) et
/// `top` = nombre d'occurrences du topic le plus frequent :
///   `coherence_milli = top * 1000 / f`  (sature a `1000`).
/// - fenetre vide (`f = 0`) ⇒ `0` (rien n'est encore synchronise) ;
/// - fenetre a l'unisson (`top = f`, un seul topic) ⇒ `1000`, et ce DES la
///   premiere observation (la mesure est INDEPENDANTE du niveau de remplissage —
///   pas de warm-up wall-clock qui se reinvite : le temps peut passer tout de
///   suite si l'orchestre est d'accord) ;
/// - deux factions egales `A`/`B` (`top = f/2`) ⇒ `500` (orchestre coupe en
///   deux : un seul dominant ferait passer le temps, deux factions egales non) ;
/// - `f` topics TOUS distincts (`top = 1`) ⇒ `1000/f` ≈ `0` (chaos maximal).
/// Deterministe ; meme suite d'observations ⇒ meme coherence.
pub struct CoherenceWindow {
    cap: usize,
    ring: Vec<String>, // les topics bruts, taille <= cap, le plus vieux ecrase quand plein
    head: usize,       // prochain index d'ecriture dans `ring` une fois plein
}

impl CoherenceWindow {
    /// Une fenetre de capacite `cap` (au moins 1). Vide.
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            ring: Vec::with_capacity(cap),
            head: 0,
        }
    }

    /// Observe un topic : pousse sa string brute dans l'anneau (ecrase le plus
    /// vieux si plein). Le bloc qui rend la mesure LIVE — un push par evenement.
    pub fn observe(&mut self, topic: &str) {
        if self.ring.len() < self.cap {
            self.ring.push(topic.to_string());
            if self.ring.len() == self.cap {
                self.head = 0;
            }
        } else {
            self.ring[self.head] = topic.to_string();
            self.head = (self.head + 1) % self.cap;
        }
    }

    /// Nombre d'observations actuellement dans la fenetre (`<= cap`).
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// `true` si aucune observation n'a encore ete poussee.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// La capacite de la fenetre (sa borne).
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Nombre de topics DISTINCTS actuellement dans la fenetre (transparence de
    /// la mesure ; expose pour le payload `clock.now`).
    pub fn distinct(&self) -> usize {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for t in &self.ring {
            seen.insert(t.as_str());
        }
        seen.len()
    }

    /// Nombre d'occurrences du topic le plus frequent (le « chorus dominant »).
    /// Le coeur de la mesure : c'est ce comptage, rapporte a `len()`, qui donne
    /// la part dominante — l'analogue exact du `max_cat` de `carte`.
    pub fn dominant(&self) -> usize {
        let mut counts: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for t in &self.ring {
            *counts.entry(t.as_str()).or_insert(0) += 1;
        }
        counts.values().copied().max().unwrap_or(0)
    }

    /// La **COHERENCE en milli** (`0..=1000`) : la PART du topic dominant dans la
    /// fenetre courante, `dominant * 1000 / len`, `0` sur fenetre vide. C'est le
    /// streaming-twin exact de la coherence batch de `carte` (`c*1000/max_cat`) :
    /// l'orchestre est synchrone quand une adresse domine. INDEPENDANT du niveau
    /// de remplissage — voir la doc du type pour l'intuition.
    pub fn coherence_milli(&self) -> u32 {
        let f = self.ring.len();
        if f == 0 {
            return 0;
        }
        let top = self.dominant();
        let milli = (top as u64) * 1000 / (f as u64);
        milli.min(1000) as u32
    }
}

/// Le **TICK GATE** : un compteur de temps LOGIQUE qui n'avance QUE quand
/// l'orchestre est d'accord. Materialise « le temps n'avance que si la coherence
/// est suffisante » : si la part dominante est sous le seuil, le temps NE PASSE
/// PAS (le compteur reste fige).
///
/// NB sur la semantique : `t` est le NOMBRE de moments ou l'orchestre etait
/// d'accord (une horloge logique MONOTONE), ce n'est PAS le NIVEAU courant de
/// synchronisation. Quand l'orchestre desynchronise, le temps se fige mais ne
/// revient PAS en arriere (le temps physique ne se rembobine pas) : `t` est
/// l'integrale des moments synchrones, distinct du `coherence_milli` LIVE. Le
/// futur player / carte-live consomment les DEUX, sans confondre `t` (compteur)
/// et `coherence_milli` (niveau).
///
/// [`GatedTick::gated_tick`] prend un ratio en milli (typiquement
/// [`CoherenceWindow::coherence_milli`]) et n'incremente `t` que si
/// `ratio_milli >= threshold_milli`. Il renvoie `(t, passed)` : `passed` =
/// `true` ssi le gate s'est ouvert ce coup-ci. Deterministe ; rejouable.
pub struct GatedTick {
    t: u64,
    threshold_milli: u32,
}

impl GatedTick {
    /// Un tick a `t = 0`, avec le seuil de coherence (en milli) sous lequel le
    /// temps ne passe pas. `threshold_milli` est sature a `1000`.
    pub fn new(threshold_milli: u32) -> Self {
        Self {
            t: 0,
            threshold_milli: threshold_milli.min(1000),
        }
    }

    /// Le compteur logique courant (le `t` du xion) — monotone, jamais rembobine.
    pub fn t(&self) -> u64 {
        self.t
    }

    /// Le seuil de coherence (milli) au-dessus duquel le temps passe.
    pub fn threshold_milli(&self) -> u32 {
        self.threshold_milli
    }

    /// `true` ssi `ratio_milli` franchit le seuil (l'orchestre est synchrone).
    pub fn is_sync(&self, ratio_milli: u32) -> bool {
        ratio_milli >= self.threshold_milli
    }

    /// **LE GATE** : si `ratio_milli >= threshold_milli`, le temps passe
    /// (`t += 1`) et on renvoie `(nouveau_t, true)` ; sinon le compteur reste
    /// fige et on renvoie `(t, false)`. C'est la regle « le temps n'avance que si
    /// la coherence est suffisante » — un temps qui EST la synchronisation.
    pub fn gated_tick(&mut self, ratio_milli: u32) -> (u64, bool) {
        if self.is_sync(ratio_milli) {
            self.t += 1;
            (self.t, true)
        } else {
            (self.t, false)
        }
    }
}

// ============================================================================
// --- bions du PLAYER : curseur de sequence + schedule (qui est DU a t) ------
// ============================================================================
//
// La machine a tsoins sait deja GRAVER (diff -> residu), ADRESSER (tsoin-store),
// et REJOUER UN tsoin (generator). Mais un tsoin tout seul est un INSTANTANE
// mort. Le `player` deroule une SEQUENCE de tsoins DANS LE TEMPS : « rejouer une
// journee », pas « rejouer un instant ». Il faut donc deux bions, sortis ICI une
// fois (comme CoherenceWindow/GatedTick pour l'horloge) :
//
//   - `SeqCursor<T>` : un CURSEUR sur une liste TRIEE par temps logique `t`. Il
//     sait `peek` le prochain item du, `advance` (consommer un item), `seek`
//     (repositionner = scrub) et `reset`. C'est le bion d'etat du deroulage.
//   - `schedule` : une FONCTION PURE qui, donne un curseur et un `t` cible (le
//     `clock.now`), renvoie COMBIEN d'items en tete sont DUS (leur `t` <= cible).
//     Separe le « QUI est du » (decision, pure) du « avancer + emettre » (effet).
//
// Determinisme = colonne du player : MEME sequence + MEME suite de `t` de
// clock.now ⇒ MEMES items deroules, dans le MEME ordre. Aucun wall-clock, aucune
// source d'alea : tout vient de la sequence triee et du `t` gate de l'horloge.

/// Un **item de sequence** du player : un temps logique `t` (quand le rejouer) et
/// une **adresse** de tsoin (quoi rejouer — l'`addr64` que `tsoin-store` ressort).
///
/// L'ordre canonique d'un item est `(t, addr)` : on trie d'abord par temps, puis
/// par adresse pour DEPARTAGER les ex-aequo de facon STABLE et deterministe (deux
/// tsoins au meme `t` se deroulent toujours dans le meme ordre, partout, a jamais
/// — sinon le replay ne serait pas reproductible).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqItem {
    /// Le temps logique auquel ce tsoin est du (le `t` de l'horloge gated).
    pub t: u64,
    /// L'adresse content-addressed du tsoin a ressortir (`tsoin.get {addr}`).
    pub addr: u64,
}

impl SeqItem {
    /// Construit un item `(t, addr)`.
    pub fn new(t: u64, addr: u64) -> Self {
        Self { t, addr }
    }
}

/// Le **CURSEUR DE SEQUENCE** du player : un curseur monotone sur une liste
/// d'items TRIEE par `(t, addr)`. Il materialise « ou en est le deroulage ».
///
/// Invariant : `items` est TRIE croissant par `(t, addr)` — le constructeur
/// [`SeqCursor::new`] le garantit en triant lui-meme (et en rendant l'ordre des
/// ex-aequo STABLE via la cle secondaire `addr`). Le curseur `pos` pointe le
/// PROCHAIN item pas encore deroule (`pos == len` ⇒ sequence epuisee).
///
/// Operations :
/// - [`peek`](Self::peek) : le prochain item du (sans le consommer) ;
/// - [`advance`](Self::advance) : consomme le prochain item (le rend + avance) ;
/// - [`seek`](Self::seek) : SCRUB — repositionne le curseur juste APRES tous les
///   items de temps `<= t` (rejouer « depuis t ») ; deterministe ;
/// - [`reset`](Self::reset) : revient au debut (le `loop` du player) ;
/// - [`due_count`](Self::due_count) : combien d'items EN TETE sont dus a `t`
///   (le bion [`schedule`] s'appuie dessus).
///
/// Deterministe et SANS etat cache : meme construction + meme suite d'operations
/// ⇒ meme deroulage. C'est le bion d'etat partage par le player et tout futur
/// ploxion qui deroule une timeline triee.
pub struct SeqCursor {
    items: Vec<SeqItem>,
    pos: usize,
}

impl SeqCursor {
    /// Construit un curseur a partir d'items QUELCONQUES : on les TRIE par
    /// `(t, addr)` (cle secondaire = ordre STABLE des ex-aequo) et on place le
    /// curseur au debut. L'appelant n'a donc pas a pre-trier — l'invariant de tri
    /// est etabli ICI, ce qui rend le deroulage deterministe quelle que soit
    /// l'ordre d'arrivee des `{t,addr}` dans le payload `player.load`.
    pub fn new(mut items: Vec<SeqItem>) -> Self {
        items.sort_by(|a, b| a.t.cmp(&b.t).then(a.addr.cmp(&b.addr)));
        Self { items, pos: 0 }
    }

    /// Une sequence vide (rien a derouler).
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            pos: 0,
        }
    }

    /// Nombre total d'items dans la sequence (independant du curseur).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` si la sequence ne contient aucun item.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// La position courante du curseur (index du PROCHAIN item a derouler ;
    /// `== len()` quand la sequence est epuisee). Expose pour la transparence
    /// (payload `player.tick` / log).
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// `true` si le curseur a depasse le dernier item (plus rien a derouler tant
    /// qu'on n'a pas `reset`/`seek` en arriere).
    pub fn exhausted(&self) -> bool {
        self.pos >= self.items.len()
    }

    /// Le PROCHAIN item du (a `pos`), SANS le consommer. `None` si epuise. Sert au
    /// player a regarder le `t` du prochain tsoin avant de decider s'il est du.
    pub fn peek(&self) -> Option<&SeqItem> {
        self.items.get(self.pos)
    }

    /// CONSOMME le prochain item : le renvoie (clone) et avance le curseur de 1.
    /// `None` si epuise (le curseur ne bouge alors pas). C'est l'unite de
    /// deroulage : un `advance` ⇒ un `player.tick` + un `tsoin.get`.
    pub fn advance(&mut self) -> Option<SeqItem> {
        let it = self.items.get(self.pos).cloned();
        if it.is_some() {
            self.pos += 1;
        }
        it
    }

    /// Combien d'items EN TETE (a partir de `pos`) sont DUS a l'instant `t`,
    /// c.-a-d. ont un temps `item.t <= t`. Comme la sequence est triee par `t`,
    /// c'est un prefixe contigu : on compte tant que le `t` du prochain item ne
    /// depasse pas la cible. NE consomme RIEN (pure lecture). Le coeur du bion
    /// [`schedule`].
    pub fn due_count(&self, t: u64) -> usize {
        let mut n = 0;
        while let Some(it) = self.items.get(self.pos + n) {
            if it.t <= t {
                n += 1;
            } else {
                break;
            }
        }
        n
    }

    /// **SCRUB** : repositionne le curseur juste APRES tous les items de temps
    /// `<= t` — de sorte qu'un deroulage repris a `clock.now == t` ne RE-emette
    /// PAS les tsoins deja dus a `t` (ils sont consideres « passes »). Recherche
    /// dichotomique sur la sequence triee (deterministe, O(log n)). Le `pos`
    /// devient le nombre d'items dont `item.t <= t` depuis le DEBUT (pas depuis la
    /// position courante : un seek peut aller en avant comme en arriere).
    pub fn seek(&mut self, t: u64) {
        // partition_point : 1er index dont le predicat est FAUX, sur une suite ou
        // le predicat (item.t <= t) est croissant-puis-faux ⇒ l'index juste apres
        // le dernier item du a `t`. Exact sur sequence triee par (t, addr).
        self.pos = self.items.partition_point(|it| it.t <= t);
    }

    /// **LOOP / rewind** : ramene le curseur au tout debut de la sequence (rien
    /// n'est efface — on REJOUE depuis le premier item). Le `loop` du player en
    /// fin de sequence rappelle ce bion.
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// Le `t` du prochain item du (le temps du tsoin qui suit), ou `None` si la
    /// sequence est epuisee. Raccourci de `peek().map(|it| it.t)`, pour que le
    /// player sache « a quel t le prochain tick aura lieu ».
    pub fn next_t(&self) -> Option<u64> {
        self.peek().map(|it| it.t)
    }
}

/// Le bion **schedule** : la DECISION PURE « quels items sont dus a `t` ». Donne
/// un curseur et un temps cible `t` (typiquement le `t` de `clock.now`), renvoie
/// la liste (clonee, dans l'ordre de deroulage) des items EN TETE dont
/// `item.t <= t`, ET AVANCE le curseur d'autant. Separe le « QUI est du » de
/// l'effet de bord (emettre `player.tick` / `tsoin.get`) que fait l'appelant.
///
/// Determinisme : la sequence etant triee par `(t, addr)`, pour un curseur donne
/// et un `t` donne, l'ensemble ET l'ordre des items renvoyes sont uniques — meme
/// sequence + meme suite de `t` ⇒ meme deroulage. C'est ce qui rend le player
/// REJOUABLE. (`t` non-decroissant entre appels = usage normal, l'horloge etant
/// monotone ; un `t` qui recule ne renverra rien tant que le curseur est deja
/// au-dela — un vrai retour-arriere passe par [`SeqCursor::seek`].)
pub fn schedule(cursor: &mut SeqCursor, t: u64) -> Vec<SeqItem> {
    let n = cursor.due_count(t);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        match cursor.advance() {
            Some(it) => out.push(it),
            None => break,
        }
    }
    out
}

// ============================================================================
// --- tests : ancrage des bions du PLAYER (curseur + schedule) ---------------
// ============================================================================

#[cfg(test)]
mod player_tests {
    use super::{schedule, SeqCursor, SeqItem};

    /// Le constructeur TRIE par (t, addr) : l'appelant peut balancer les items
    /// dans le desordre, le deroulage reste deterministe et trie.
    #[test]
    fn cursor_sorts_on_build() {
        let c = SeqCursor::new(vec![
            SeqItem::new(5, 0xbb),
            SeqItem::new(1, 0xff),
            SeqItem::new(5, 0xaa), // meme t que le 0xbb : departage par addr
            SeqItem::new(1, 0x11),
        ]);
        let order: Vec<(u64, u64)> = (0..c.len())
            .map(|i| {
                let it = c.peek_at(i);
                (it.t, it.addr)
            })
            .collect();
        assert_eq!(order, vec![(1, 0x11), (1, 0xff), (5, 0xaa), (5, 0xbb)]);
    }

    /// peek ne consomme pas ; advance consomme et avance ; epuise ⇒ None.
    #[test]
    fn peek_and_advance() {
        let mut c = SeqCursor::new(vec![SeqItem::new(1, 0xa), SeqItem::new(2, 0xb)]);
        assert_eq!(c.peek(), Some(&SeqItem::new(1, 0xa)));
        assert_eq!(c.pos(), 0, "peek ne bouge pas");
        assert_eq!(c.advance(), Some(SeqItem::new(1, 0xa)));
        assert_eq!(c.pos(), 1);
        assert_eq!(c.advance(), Some(SeqItem::new(2, 0xb)));
        assert!(c.exhausted());
        assert_eq!(c.advance(), None, "epuise ⇒ None, curseur fige");
        assert_eq!(c.pos(), 2);
    }

    /// schedule deroule TOUS les items dus a `t` (prefixe contigu), dans l'ordre,
    /// et avance le curseur ; un t plus grand deroule la suite.
    #[test]
    fn schedule_releases_due_prefix() {
        let mut c = SeqCursor::new(vec![
            SeqItem::new(0, 0xa),
            SeqItem::new(0, 0xb),
            SeqItem::new(3, 0xc),
            SeqItem::new(7, 0xd),
        ]);
        // t=0 ⇒ les deux items a t=0 (ordre addr stable).
        let due = schedule(&mut c, 0);
        assert_eq!(due, vec![SeqItem::new(0, 0xa), SeqItem::new(0, 0xb)]);
        // t=2 ⇒ rien de nouveau (le prochain est a t=3).
        assert_eq!(schedule(&mut c, 2), vec![]);
        // t=3 ⇒ l'item a t=3.
        assert_eq!(schedule(&mut c, 3), vec![SeqItem::new(3, 0xc)]);
        // t=100 ⇒ le reste.
        assert_eq!(schedule(&mut c, 100), vec![SeqItem::new(7, 0xd)]);
        assert!(c.exhausted());
    }

    /// LE replay deterministe : meme sequence + meme suite de `t` ⇒ meme
    /// deroulage exact (la propriete-cle du player).
    #[test]
    fn replay_is_deterministic() {
        let build = || {
            SeqCursor::new(vec![
                SeqItem::new(2, 0x20),
                SeqItem::new(0, 0x00),
                SeqItem::new(2, 0x21),
                SeqItem::new(5, 0x50),
            ])
        };
        let ts = [0u64, 1, 2, 2, 4, 5];
        let run = |mut c: SeqCursor| -> Vec<SeqItem> {
            let mut out = Vec::new();
            for &t in &ts {
                out.extend(schedule(&mut c, t));
            }
            out
        };
        assert_eq!(run(build()), run(build()), "meme entree ⇒ meme deroulage");
    }

    /// seek = scrub : repositionne juste APRES les items de t <= cible (en avant
    /// comme en arriere), sans rien re-derouler de deja passe.
    #[test]
    fn seek_scrubs_both_ways() {
        let mut c = SeqCursor::new(vec![
            SeqItem::new(0, 0xa),
            SeqItem::new(2, 0xb),
            SeqItem::new(2, 0xc),
            SeqItem::new(9, 0xd),
        ]);
        c.seek(2); // apres les items de t<=2 ⇒ pos pointe l'item a t=9.
        assert_eq!(c.peek(), Some(&SeqItem::new(9, 0xd)));
        c.seek(0); // retour-arriere : apres l'item a t=0 ⇒ pointe le 1er a t=2.
        assert_eq!(c.peek(), Some(&SeqItem::new(2, 0xb)));
        c.seek(100); // au-dela de tout ⇒ epuise.
        assert!(c.exhausted());
        c.seek(0);
        assert_eq!(c.pos(), 1);
    }

    /// reset = loop : ramene au debut, on REJOUE toute la sequence.
    #[test]
    fn reset_loops() {
        let mut c = SeqCursor::new(vec![SeqItem::new(1, 0xa), SeqItem::new(2, 0xb)]);
        assert_eq!(schedule(&mut c, 10).len(), 2);
        assert!(c.exhausted());
        c.reset();
        assert_eq!(c.pos(), 0);
        assert_eq!(schedule(&mut c, 10).len(), 2, "loop rejoue tout");
    }

    /// next_t expose le t du prochain item du (pour que le player sache quand le
    /// prochain tick aura lieu) ; None une fois epuise.
    #[test]
    fn next_t_reports_upcoming() {
        let mut c = SeqCursor::new(vec![SeqItem::new(3, 0xa), SeqItem::new(8, 0xb)]);
        assert_eq!(c.next_t(), Some(3));
        c.advance();
        assert_eq!(c.next_t(), Some(8));
        c.advance();
        assert_eq!(c.next_t(), None);
    }
}

// helper de test interne (acces indexe a un item pour ancrer le tri) ----------
impl SeqCursor {
    #[cfg(test)]
    fn peek_at(&self, i: usize) -> &SeqItem {
        &self.items[i]
    }
}

// ============================================================================
// --- tests : ancrage des bions de l'HORLOGE (coherence live + gate) ---------
// ============================================================================

#[cfg(test)]
mod clock_tests {
    use super::{CoherenceWindow, GatedTick};

    /// Fenetre vide ⇒ coherence nulle ; fenetre a l'unisson ⇒ 1000 ; fenetre de
    /// topics tous distincts ⇒ part dominante = 1/f (bas). Le coeur de la mesure.
    #[test]
    fn coherence_extremes() {
        let mut w = CoherenceWindow::new(8);
        assert_eq!(w.coherence_milli(), 0, "vide => 0");

        // pleine d'un SEUL topic ⇒ dominant=8=len ⇒ 1000.
        for _ in 0..8 {
            w.observe("tick");
        }
        assert_eq!(w.len(), 8);
        assert_eq!(w.distinct(), 1);
        assert_eq!(w.coherence_milli(), 1000, "unisson => 1000");

        // pleine de 8 topics tous distincts ⇒ dominant=1, f=8 ⇒ 1*1000/8 = 125.
        let mut c = CoherenceWindow::new(8);
        for i in 0..8 {
            c.observe(&format!("topic-{i}"));
        }
        assert_eq!(c.distinct(), 8);
        assert_eq!(c.coherence_milli(), 125, "chaos => bas (1 dominant sur 8)");
    }

    /// La mesure est INDEPENDANTE du niveau de remplissage : N observations
    /// identiques donnent la MEME (haute) coherence, que N soit petit ou = cap.
    /// (Regression du defaut « fill-level deguise en coherence » : pas de warm-up
    /// wall-clock qui se reinvite dans la mesure.)
    #[test]
    fn coherence_is_independent_of_fill_level() {
        let mut a = CoherenceWindow::new(32);
        for _ in 0..4 {
            a.observe("u");
        }
        let mut b = CoherenceWindow::new(32);
        for _ in 0..32 {
            b.observe("u");
        }
        assert_eq!(a.coherence_milli(), 1000);
        assert_eq!(b.coherence_milli(), 1000);
        assert_eq!(
            a.coherence_milli(),
            b.coherence_milli(),
            "le niveau de remplissage ne doit PAS compter"
        );
    }

    /// Le gate peut s'ouvrir DES la premiere observation sur un flux a l'unisson :
    /// le temps n'attend pas que le buffer soit plein (la loi t=coherence, pas le
    /// wall-clock).
    #[test]
    fn gate_opens_during_warmup_on_unison() {
        let mut w = CoherenceWindow::new(32);
        let mut g = GatedTick::new(500);
        w.observe("u");
        let (t, sync) = g.gated_tick(w.coherence_milli());
        assert!(sync, "unisson => SYNC des l'obs #1");
        assert_eq!(t, 1, "le temps passe tout de suite sous unisson");
    }

    /// La part DOMINANTE distingue « un chorus + du bruit » (synchrone) de « deux
    /// factions egales » (non synchrone) — ce qu'un simple comptage de distincts
    /// ne saurait faire. Un seul dominant fait passer le temps ; un orchestre
    /// coupe en deux, non.
    #[test]
    fn dominant_distinguishes_chorus_from_split() {
        // factions egales A/B ⇒ dominant=16, f=32 ⇒ 500 (a la limite du seuil).
        let mut split = CoherenceWindow::new(32);
        for i in 0..32 {
            split.observe(if i % 2 == 0 { "A" } else { "B" });
        }
        assert_eq!(split.distinct(), 2);
        assert_eq!(split.coherence_milli(), 500, "split egal => 50%");

        // un chorus dominant + du bruit ⇒ dominant=28, f=32 ⇒ 875 (synchrone).
        let mut chorus = CoherenceWindow::new(32);
        for i in 0..32 {
            chorus.observe(if i < 28 { "A" } else { "noise" });
        }
        assert_eq!(chorus.coherence_milli(), 875, "28/32 dominant => haut");
    }

    /// Le ring-buffer est BORNE : il ne garde que les `cap` derniers et le plus
    /// vieux est ecrase. Apres avoir noye une rafale distincte sous des "sync"
    /// repetes, la coherence remonte (l'ancien chaos est sorti de la fenetre).
    #[test]
    fn window_is_bounded_and_slides() {
        let mut w = CoherenceWindow::new(4);
        w.observe("a");
        w.observe("b");
        w.observe("c");
        w.observe("d"); // 4 distincts, plein
        assert_eq!(w.len(), 4);
        assert_eq!(w.distinct(), 4);
        // 4 "sync" ecrasent toute la fenetre ⇒ un seul dominant ⇒ 1000.
        for _ in 0..4 {
            w.observe("sync");
        }
        assert_eq!(w.len(), 4, "borne a cap");
        assert_eq!(w.distinct(), 1);
        assert_eq!(w.coherence_milli(), 1000);
    }

    /// Stress : 100 observations dans une fenetre cap=3 restent bornees (pas de
    /// fuite memoire, le ring n'enfle jamais au-dela de cap).
    #[test]
    fn ring_stress_stays_bounded() {
        let mut w = CoherenceWindow::new(3);
        for i in 0..100 {
            w.observe(&format!("t{i}"));
        }
        assert!(w.len() <= 3, "borne a cap=3");
        assert!(w.distinct() <= 3);
    }

    /// Le gate : le temps n'avance QUE quand le ratio franchit le seuil. Sous le
    /// seuil, `t` reste fige (le temps ne passe pas) ; au-dessus il incremente, et
    /// ne rembobine jamais quand le ratio retombe (monotone).
    #[test]
    fn gate_only_advances_when_coherent() {
        let mut g = GatedTick::new(500);
        assert_eq!(g.t(), 0);
        // sous le seuil ⇒ fige.
        assert_eq!(g.gated_tick(0), (0, false));
        assert_eq!(g.gated_tick(499), (0, false));
        assert_eq!(g.t(), 0, "le temps n'a pas passe");
        // au seuil exact ⇒ passe.
        assert_eq!(g.gated_tick(500), (1, true));
        assert_eq!(g.gated_tick(1000), (2, true));
        assert_eq!(g.t(), 2);
        // retombe sous le seuil ⇒ se fige a 2 (pas de retour en arriere).
        assert_eq!(g.gated_tick(10), (2, false));
        assert_eq!(g.t(), 2);
    }

    /// Seuil sature a 1000 ; seuil 0 ⇒ le temps passe toujours (jamais bloque).
    #[test]
    fn threshold_saturates_and_zero_never_blocks() {
        let g = GatedTick::new(9999);
        assert_eq!(g.threshold_milli(), 1000);
        let mut g0 = GatedTick::new(0);
        assert_eq!(g0.gated_tick(0), (1, true));
        assert_eq!(g0.gated_tick(0), (2, true));
    }
}
