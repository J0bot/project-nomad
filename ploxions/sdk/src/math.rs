//! `math` — les BIONS ARITHMÉTIQUES du xion (i32 + f64), inclus sur le xerboxion.
//!
//! José (2026-06-21) : « c maintenant transformé en bien plus pour les inclure sur
//! le xerboxion ». Les bions math vivaient comme modules WASM côté UI (my_website2 :
//! /bions.json, /bions-f64.json) ; ici ils ENTRENT dans le SDK Rust = leur foyer
//! durable, compilé→WASM avec le reste du cœur (objectif cube 16 mm). Complète
//! [`crate::bions`] (JSON/hash/résidu) avec la couche calcul pur.
//!
//! GÉNÉRÉ par `my_website2/resources/data/bions-to-rust.py` depuis les catalogues
//! (Adressage Génératif : on régénère depuis la source). Sémantique i32 = WASM
//! (wrapping, shr arithmétique). Tests = les tsoins (mêmes sorties que le WASM).
//! Ne pas éditer à la main : régénérer.

// --- bions i32 (85) ---------------------------------------------------
/// a + b  (bion `add`)
pub fn add(a: i32, b: i32) -> i32 { a.wrapping_add(b) }
/// a - b  (bion `sub`)
pub fn sub(a: i32, b: i32) -> i32 { a.wrapping_sub(b) }
/// a * b  (bion `mul`)
pub fn mul(a: i32, b: i32) -> i32 { a.wrapping_mul(b) }
/// a / b (entier signé)  (bion `div`)
pub fn div(a: i32, b: i32) -> i32 { a.wrapping_div(b) }
/// a modulo b  (bion `mod`)
pub fn mod_(a: i32, b: i32) -> i32 { a.wrapping_rem(b) }
/// a²  (bion `square`)
pub fn square(a: i32) -> i32 { a.wrapping_mul(a) }
/// -a  (bion `neg`)
pub fn neg(a: i32) -> i32 { a.wrapping_neg() }
/// valeur absolue  (bion `abs`)
pub fn abs(a: i32) -> i32 { a.wrapping_abs() }
/// min(a,b)  (bion `min`)
pub fn min(a: i32, b: i32) -> i32 { a.min(b) }
/// max(a,b)  (bion `max`)
pub fn max(a: i32, b: i32) -> i32 { a.max(b) }
/// a & b (bits)  (bion `and`)
pub fn and(a: i32, b: i32) -> i32 { a & b }
/// a | b (bits)  (bion `or`)
pub fn or(a: i32, b: i32) -> i32 { a | b }
/// a ^ b (bits)  (bion `xor`)
pub fn xor(a: i32, b: i32) -> i32 { a ^ b }
/// a << b  (bion `shl`)
pub fn shl(a: i32, b: i32) -> i32 { a.wrapping_shl(b as u32) }
/// a >> b (signé)  (bion `shr`)
pub fn shr(a: i32, b: i32) -> i32 { a.wrapping_shr(b as u32) }
/// rotation gauche  (bion `rotl`)
pub fn rotl(a: i32, b: i32) -> i32 { a.rotate_left(b as u32) }
/// nombre de bits à 1  (bion `popcount`)
pub fn popcount(a: i32) -> i32 { a.count_ones() as i32 }
/// zéros en tête  (bion `clz`)
pub fn clz(a: i32) -> i32 { a.leading_zeros() as i32 }
/// zéros en queue  (bion `ctz`)
pub fn ctz(a: i32) -> i32 { a.trailing_zeros() as i32 }
/// 1 si a==0 sinon 0  (bion `iszero`)
pub fn iszero(a: i32) -> i32 { (a == 0) as i32 }
/// 1 si a==b  (bion `eq`)
pub fn eq(a: i32, b: i32) -> i32 { (a == b) as i32 }
/// 1 si a>b  (bion `gt`)
pub fn gt(a: i32, b: i32) -> i32 { (a > b) as i32 }
/// -1, 0 ou 1  (bion `sign`)
pub fn sign(a: i32) -> i32 { a.signum() }
/// n-ième Fibonacci  (bion `fib`)
pub fn fib(a: i32) -> i32 { let (mut x, mut y) = (0i32, 1i32); let mut i = 0i32; while i < a { let t = x.wrapping_add(y); x = y; y = t; i += 1; } x }
/// factorielle n!  (bion `fact`)
pub fn fact(a: i32) -> i32 { let mut r = 1i32; let mut i = 1i32; while i <= a { r = r.wrapping_mul(i); i += 1; } r }
/// PGCD (Euclide)  (bion `gcd`)
pub fn gcd(a: i32, b: i32) -> i32 { let (mut x, mut y) = (a, b); while y != 0 { let t = x.wrapping_rem(y); x = y; y = t; } x }
/// 1+2+...+n  (bion `sumto`)
pub fn sumto(a: i32) -> i32 { let mut s = 0i32; let mut i = 1i32; while i <= a { s = s.wrapping_add(i); i += 1; } s }
/// 1 si n est premier, sinon 0  (bion `isprime`)
pub fn isprime(a: i32) -> i32 { if a < 2 { 0 } else { let mut i = 2i32; let mut p = 1i32; while i.wrapping_mul(i) <= a { if a.wrapping_rem(i) == 0 { p = 0; break; } i += 1; } p } }
/// a puissance b (entier)  (bion `pow`)
pub fn pow(a: i32, b: i32) -> i32 { let mut r = 1i32; let mut i = 0i32; while i < b { r = r.wrapping_mul(a); i += 1; } r }
/// racine carrée entière de n  (bion `isqrt`)
pub fn isqrt(a: i32) -> i32 { if a < 0 { 0 } else { let mut r = 0i32; while (r + 1).wrapping_mul(r + 1) <= a { r += 1; } r } }
/// somme des chiffres (base 10)  (bion `digitsum`)
pub fn digitsum(a: i32) -> i32 { let mut x = a; let mut s = 0i32; loop { s = s.wrapping_add(x.wrapping_rem(10)); x = x.wrapping_div(10); if x == 0 { break; } } s }
/// borne x dans [lo,hi]  (bion `clamp`)
pub fn clamp(a: i32, b: i32, c: i32) -> i32 { a.max(b).min(c) }
/// a + 1  (bion `inc`)
pub fn inc(a: i32) -> i32 { a.wrapping_add(1) }
/// a - 1  (bion `dec`)
pub fn dec(a: i32) -> i32 { a.wrapping_sub(1) }
/// a * 2  (bion `double`)
pub fn double(a: i32) -> i32 { a.wrapping_mul(2) }
/// a / 2  (bion `half`)
pub fn half(a: i32) -> i32 { a.wrapping_div(2) }
/// a * a * a  (bion `cube`)
pub fn cube(a: i32) -> i32 { a.wrapping_mul(a).wrapping_mul(a) }
/// 1 si a==0 sinon 0  (bion `not`)
pub fn not(a: i32) -> i32 { (a == 0) as i32 }
/// 1 si a pair  (bion `even`)
pub fn even(a: i32) -> i32 { ((a & 1) == 0) as i32 }
/// 1 si a impair  (bion `odd`)
pub fn odd(a: i32) -> i32 { a & 1 }
/// popcount(a) & 1  (bion `parity`)
pub fn parity(a: i32) -> i32 { (a.count_ones() & 1) as i32 }
/// a & -a (bit bas)  (bion `lowbit`)
pub fn lowbit(a: i32) -> i32 { a & a.wrapping_neg() }
/// 1 si a < b  (bion `lt`)
pub fn lt(a: i32, b: i32) -> i32 { (a < b) as i32 }
/// 1 si a <= b  (bion `lte`)
pub fn lte(a: i32, b: i32) -> i32 { (a <= b) as i32 }
/// 1 si a >= b  (bion `gte`)
pub fn gte(a: i32, b: i32) -> i32 { (a >= b) as i32 }
/// 1 si a != b  (bion `ne`)
pub fn ne(a: i32, b: i32) -> i32 { (a != b) as i32 }
/// (a + b) / 2  (bion `avg`)
pub fn avg(a: i32, b: i32) -> i32 { a.wrapping_add(b).wrapping_div(2) }
/// abs(a - b)  (bion `diff`)
pub fn diff(a: i32, b: i32) -> i32 { a.wrapping_sub(b).wrapping_abs() }
/// ~(a & b)  (bion `nand`)
pub fn nand(a: i32, b: i32) -> i32 { !(a & b) }
/// ~(a | b)  (bion `nor`)
pub fn nor(a: i32, b: i32) -> i32 { !(a | b) }
/// ~(a ^ b)  (bion `xnor`)
pub fn xnor(a: i32, b: i32) -> i32 { !(a ^ b) }
/// cond ? a : b  (bion `ifel`)
pub fn ifel(a: i32, b: i32, c: i32) -> i32 { if a != 0 { b } else { c } }
/// longueur en bits de n  (bion `bitlen`)
pub fn bitlen(a: i32) -> i32 { 32 - a.leading_zeros() as i32 }
/// nb de chiffres de n  (bion `digits`)
pub fn digits(a: i32) -> i32 { let mut x = a; let mut c = 0i32; loop { x = x.wrapping_div(10); c += 1; if x == 0 { break; } } c }
/// etapes de Collatz vers 1  (bion `collatz`)
pub fn collatz(a: i32) -> i32 { let mut x = a; let mut c = 0i32; while x != 1 { if x & 1 == 0 { x = x.wrapping_div(2); } else { x = x.wrapping_mul(3).wrapping_add(1); } c += 1; } c }
/// ppcm(a,b)  (bion `lcm`)
pub fn lcm(a: i32, b: i32) -> i32 { a.wrapping_div(gcd(a, b)).wrapping_mul(b) }
/// PRNG xorshift32 (coeur deterministe de hasard)  (bion `xorshift`)
pub fn xorshift(a: i32) -> i32 { let mut x = a; x ^= x.wrapping_shl(13); x ^= ((x as u32) >> 17) as i32; x ^= x.wrapping_shl(5); x }
/// Celsius -> Fahrenheit (uncraft convertisseur)  (bion `c2f`)
pub fn c2f(a: i32) -> i32 { a.wrapping_mul(9).wrapping_div(5).wrapping_add(32) }
/// Fahrenheit -> Celsius (uncraft convertisseur)  (bion `f2c`)
pub fn f2c(a: i32) -> i32 { a.wrapping_sub(32).wrapping_mul(5).wrapping_div(9) }
/// n | (1<<i)  (bion `setbit`)
pub fn setbit(a: i32, b: i32) -> i32 { a | 1i32.wrapping_shl(b as u32) }
/// n & ~(1<<i)  (bion `clrbit`)
pub fn clrbit(a: i32, b: i32) -> i32 { a & !1i32.wrapping_shl(b as u32) }
/// n ^ (1<<i)  (bion `togglebit`)
pub fn togglebit(a: i32, b: i32) -> i32 { a ^ 1i32.wrapping_shl(b as u32) }
/// (n>>i) & 1  (bion `getbit`)
pub fn getbit(a: i32, b: i32) -> i32 { a.wrapping_shr(b as u32) & 1 }
/// popcount(a^b)  (bion `hammingdist`)
pub fn hammingdist(a: i32, b: i32) -> i32 { (a ^ b).count_ones() as i32 }
/// rotation droite  (bion `rotr`)
pub fn rotr(a: i32, b: i32) -> i32 { a.rotate_right(b as u32) }
/// décalage droite logique  (bion `shru`)
pub fn shru(a: i32, b: i32) -> i32 { ((a as u32) >> (b as u32)) as i32 }
/// code de Gray : n ^ (n>>>1)  (bion `gray`)
pub fn gray(a: i32) -> i32 { a ^ ((a as u32) >> 1) as i32 }
/// floor(log2 n) = 31-clz  (bion `log2`)
pub fn log2(a: i32) -> i32 { 31 - a.leading_zeros() as i32 }
/// plus petite puissance de 2 >= n  (bion `nextpow2`)
pub fn nextpow2(a: i32) -> i32 { let mut r = 1i32; while r < a { r = r.wrapping_shl(1); } r }
/// nombre de Lucas L(n)  (bion `lucas`)
pub fn lucas(a: i32) -> i32 { let (mut x, mut y) = (2i32, 1i32); let mut i = 0i32; while i < a { let t = x.wrapping_add(y); x = y; y = t; i += 1; } x }
/// (a*b) % m  (bion `mulmod`)
pub fn mulmod(a: i32, b: i32, c: i32) -> i32 { a.wrapping_mul(b).wrapping_rem(c) }
/// (a+b) % m  (bion `addmod`)
pub fn addmod(a: i32, b: i32, c: i32) -> i32 { a.wrapping_add(b).wrapping_rem(c) }
/// (a-b) mod m (positif)  (bion `submod`)
pub fn submod(a: i32, b: i32, c: i32) -> i32 { a.wrapping_sub(b).wrapping_rem(c).wrapping_add(c).wrapping_rem(c) }
/// modulo euclidien (toujours >=0)  (bion `pmod`)
pub fn pmod(a: i32, b: i32) -> i32 { a.wrapping_rem(b).wrapping_add(b).wrapping_rem(b) }
/// arrondi sup. au multiple (a puissance de 2)  (bion `align`)
pub fn align(a: i32, b: i32) -> i32 { a.wrapping_add(b.wrapping_sub(1)) & !b.wrapping_sub(1) }
/// inverse les chiffres décimaux  (bion `reverse10`)
pub fn reverse10(a: i32) -> i32 { let mut x = a; let mut r = 0i32; while x != 0 { r = r.wrapping_mul(10).wrapping_add(x.wrapping_rem(10)); x = x.wrapping_div(10); } r }
/// Gray -> binaire  (bion `ungray`)
pub fn ungray(a: i32) -> i32 { let mut g = a; let mut b = g; g = ((g as u32) >> 1) as i32; while g != 0 { b ^= g; g = ((g as u32) >> 1) as i32; } b }
/// plus grande puissance de 2 <= n  (bion `bitfloor`)
pub fn bitfloor(a: i32) -> i32 { let mut r = 1i32; while r.wrapping_shl(1) <= a { r = r.wrapping_shl(1); } r }
/// base^exp mod m  (bion `powmod`)
pub fn powmod(a: i32, b: i32, c: i32) -> i32 { let (mut base, mut e) = (a.wrapping_rem(c), b); let mut r = 1i32; while e != 0 { if e & 1 == 1 { r = r.wrapping_mul(base).wrapping_rem(c); } base = base.wrapping_mul(base).wrapping_rem(c); e = ((e as u32) >> 1) as i32; } r }
/// inverse l'ordre des octets  (bion `bswap`)
pub fn bswap(a: i32) -> i32 { a.swap_bytes() }
/// division arrondie au sup.  (bion `ceildiv`)
pub fn ceildiv(a: i32, b: i32) -> i32 { a.wrapping_add(b).wrapping_sub(1).wrapping_div(b) }
/// arrondi sup. au multiple de m  (bion `roundup`)
pub fn roundup(a: i32, b: i32) -> i32 { a.wrapping_add(b).wrapping_sub(1).wrapping_div(b).wrapping_mul(b) }
/// racine numérique (1+(n-1)%9)  (bion `digitroot`)
pub fn digitroot(a: i32) -> i32 { a.wrapping_sub(1).wrapping_rem(9).wrapping_add(1) }
/// extension de signe 8 bits  (bion `signext8`)
pub fn signext8(a: i32) -> i32 { a.wrapping_shl(24).wrapping_shr(24) }
/// interpolation entière a+((b-a)*t>>8)  (bion `lerpi`)
pub fn lerpi(a: i32, b: i32, c: i32) -> i32 { a.wrapping_add(b.wrapping_sub(a).wrapping_mul(c).wrapping_shr(8)) }

/// bions à VIRGULE (f64) — pour les ploxions float (convertisseur, etc.).
pub mod f64m {
// --- bions f64 (30) ---------------------------------------------------
/// a + b (f64)  (bion `add`)
pub fn add(a: f64, b: f64) -> f64 { a + b }
/// a - b (f64)  (bion `sub`)
pub fn sub(a: f64, b: f64) -> f64 { a - b }
/// a * b (f64)  (bion `mul`)
pub fn mul(a: f64, b: f64) -> f64 { a * b }
/// a / b (f64)  (bion `div`)
pub fn div(a: f64, b: f64) -> f64 { a / b }
/// v*a + b (coeur du convertisseur)  (bion `affine`)
pub fn affine(a: f64, b: f64, c: f64) -> f64 { a * b + c }
/// a + (b-a)*t  (bion `lerp`)
pub fn lerp(a: f64, b: f64, c: f64) -> f64 { a + (b - a) * c }
/// sqrt(a^2 + b^2)  (bion `hypot`)
pub fn hypot(a: f64, b: f64) -> f64 { (a * a + b * b).sqrt() }
/// v * p / 100  (bion `pct`)
pub fn pct(a: f64, b: f64) -> f64 { a * b / 100.0 }
/// 1 / x  (bion `recip`)
pub fn recip(a: f64) -> f64 { 1.0 / a }
/// racine carree  (bion `sqrt`)
pub fn sqrt(a: f64) -> f64 { a.sqrt() }
/// -x  (bion `neg`)
pub fn neg(a: f64) -> f64 { -a }
/// |x|  (bion `abs`)
pub fn abs(a: f64) -> f64 { a.abs() }
/// Celsius -> Fahrenheit (exact)  (bion `c2f`)
pub fn c2f(a: f64) -> f64 { a * 1.8 + 32.0 }
/// Fahrenheit -> Celsius (exact)  (bion `f2c`)
pub fn f2c(a: f64) -> f64 { (a - 32.0) / 1.8 }
/// plancher  (bion `floor`)
pub fn floor(a: f64) -> f64 { a.floor() }
/// plafond  (bion `ceil`)
pub fn ceil(a: f64) -> f64 { a.ceil() }
/// arrondi (ties-even)  (bion `round`)
pub fn round(a: f64) -> f64 { a.round_ties_even() }
/// troncature  (bion `trunc`)
pub fn trunc(a: f64) -> f64 { a.trunc() }
/// min(a,b) f64  (bion `fmin`)
pub fn fmin(a: f64, b: f64) -> f64 { a.min(b) }
/// max(a,b) f64  (bion `fmax`)
pub fn fmax(a: f64, b: f64) -> f64 { a.max(b) }
/// borne x dans [lo,hi]  (bion `clamp`)
pub fn clamp(a: f64, b: f64, c: f64) -> f64 { a.min(c).max(b) }
/// magnitude de a, signe de b  (bion `copysign`)
pub fn copysign(a: f64, b: f64) -> f64 { a.copysign(b) }
/// a * a  (bion `square`)
pub fn square(a: f64) -> f64 { a * a }
/// a * a * a  (bion `cube`)
pub fn cube(a: f64) -> f64 { a * a * a }
/// (a + b) / 2  (bion `avg`)
pub fn avg(a: f64, b: f64) -> f64 { (a + b) / 2.0 }
/// degrés -> radians  (bion `deg2rad`)
pub fn deg2rad(a: f64) -> f64 { a * (std::f64::consts::PI / 180.0) }
/// radians -> degrés  (bion `rad2deg`)
pub fn rad2deg(a: f64) -> f64 { a * (180.0 / std::f64::consts::PI) }
/// signe (-1/+1) via copysign  (bion `fsign`)
pub fn fsign(a: f64) -> f64 { 1.0_f64.copysign(a) }
/// borne dans [0,1]  (bion `clamp01`)
pub fn clamp01(a: f64) -> f64 { a.max(0.0).min(1.0) }
/// partie fractionnaire  (bion `frac`)
pub fn frac(a: f64) -> f64 { a - a.trunc() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tsoins_i32() {
        assert_eq!(add(7, 5), 12, "add");
        assert_eq!(sub(7, 5), 2, "sub");
        assert_eq!(mul(7, 5), 35, "mul");
        assert_eq!(div(20, 3), 6, "div");
        assert_eq!(mod_(20, 3), 2, "mod");
        assert_eq!(square(9), 81, "square");
        assert_eq!(neg(5), -5, "neg");
        assert_eq!(abs(-8), 8, "abs");
        assert_eq!(min(7, 5), 5, "min");
        assert_eq!(max(7, 5), 7, "max");
        assert_eq!(and(12, 10), 8, "and");
        assert_eq!(or(12, 10), 14, "or");
        assert_eq!(xor(12, 10), 6, "xor");
        assert_eq!(shl(1, 4), 16, "shl");
        assert_eq!(shr(256, 2), 64, "shr");
        assert_eq!(rotl(1, 1), 2, "rotl");
        assert_eq!(popcount(255), 8, "popcount");
        assert_eq!(clz(1), 31, "clz");
        assert_eq!(ctz(8), 3, "ctz");
        assert_eq!(iszero(0), 1, "iszero");
        assert_eq!(eq(5, 5), 1, "eq");
        assert_eq!(gt(7, 5), 1, "gt");
        assert_eq!(sign(-3), -1, "sign");
        assert_eq!(fib(10), 55, "fib");
        assert_eq!(fact(5), 120, "fact");
        assert_eq!(gcd(48, 18), 6, "gcd");
        assert_eq!(sumto(100), 5050, "sumto");
        assert_eq!(isprime(17), 1, "isprime");
        assert_eq!(pow(2, 10), 1024, "pow");
        assert_eq!(isqrt(50), 7, "isqrt");
        assert_eq!(digitsum(1234), 10, "digitsum");
        assert_eq!(clamp(15, 0, 10), 10, "clamp");
        assert_eq!(inc(5), 6, "inc");
        assert_eq!(dec(5), 4, "dec");
        assert_eq!(double(5), 10, "double");
        assert_eq!(half(10), 5, "half");
        assert_eq!(cube(3), 27, "cube");
        assert_eq!(not(0), 1, "not");
        assert_eq!(even(4), 1, "even");
        assert_eq!(odd(5), 1, "odd");
        assert_eq!(parity(7), 1, "parity");
        assert_eq!(lowbit(12), 4, "lowbit");
        assert_eq!(lt(3, 5), 1, "lt");
        assert_eq!(lte(5, 5), 1, "lte");
        assert_eq!(gte(5, 3), 1, "gte");
        assert_eq!(ne(3, 5), 1, "ne");
        assert_eq!(avg(10, 20), 15, "avg");
        assert_eq!(diff(3, 10), 7, "diff");
        assert_eq!(nand(12, 10), -9, "nand");
        assert_eq!(nor(12, 10), -15, "nor");
        assert_eq!(xnor(12, 10), -7, "xnor");
        assert_eq!(ifel(1, 100, 200), 100, "ifel");
        assert_eq!(bitlen(255), 8, "bitlen");
        assert_eq!(digits(4096), 4, "digits");
        assert_eq!(collatz(7), 16, "collatz");
        assert_eq!(lcm(21, 6), 42, "lcm");
        assert_eq!(xorshift(1), 270369, "xorshift");
        assert_eq!(c2f(100), 212, "c2f");
        assert_eq!(f2c(212), 100, "f2c");
        assert_eq!(setbit(0, 3), 8, "setbit");
        assert_eq!(clrbit(15, 0), 14, "clrbit");
        assert_eq!(togglebit(0, 2), 4, "togglebit");
        assert_eq!(getbit(8, 3), 1, "getbit");
        assert_eq!(hammingdist(5, 3), 2, "hammingdist");
        assert_eq!(rotr(2, 1), 1, "rotr");
        assert_eq!(shru(256, 4), 16, "shru");
        assert_eq!(gray(5), 7, "gray");
        assert_eq!(log2(8), 3, "log2");
        assert_eq!(nextpow2(5), 8, "nextpow2");
        assert_eq!(lucas(5), 11, "lucas");
        assert_eq!(mulmod(7, 8, 10), 6, "mulmod");
        assert_eq!(addmod(7, 8, 10), 5, "addmod");
        assert_eq!(submod(3, 5, 7), 5, "submod");
        assert_eq!(pmod(-3, 5), 2, "pmod");
        assert_eq!(align(10, 8), 16, "align");
        assert_eq!(reverse10(123), 321, "reverse10");
        assert_eq!(ungray(7), 5, "ungray");
        assert_eq!(bitfloor(100), 64, "bitfloor");
        assert_eq!(powmod(2, 10, 1000), 24, "powmod");
        assert_eq!(bswap(1), 16777216, "bswap");
        assert_eq!(ceildiv(7, 3), 3, "ceildiv");
        assert_eq!(roundup(10, 4), 12, "roundup");
        assert_eq!(digitroot(18), 9, "digitroot");
        assert_eq!(signext8(255), -1, "signext8");
        assert_eq!(lerpi(0, 256, 128), 128, "lerpi");
    }
    #[test]
    fn tsoins_f64() {
        use super::f64m;
        assert!((f64m::add(2.5, 0.5) - 3.0).abs() < 1e-09, "add");
        assert!((f64m::sub(3.0, 0.5) - 2.5).abs() < 1e-09, "sub");
        assert!((f64m::mul(2.5, 4.0) - 10.0).abs() < 1e-09, "mul");
        assert!((f64m::div(10.0, 4.0) - 2.5).abs() < 1e-09, "div");
        assert!((f64m::affine(100.0, 1.8, 32.0) - 212.0).abs() < 1e-09, "affine");
        assert!((f64m::lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-09, "lerp");
        assert!((f64m::hypot(3.0, 4.0) - 5.0).abs() < 1e-09, "hypot");
        assert!((f64m::pct(200.0, 15.0) - 30.0).abs() < 1e-09, "pct");
        assert!((f64m::recip(4.0) - 0.25).abs() < 1e-09, "recip");
        assert!((f64m::sqrt(2.0) - 1.4142135623730951).abs() < 1e-09, "sqrt");
        assert!((f64m::neg(5.0) - -5.0).abs() < 1e-09, "neg");
        assert!((f64m::abs(-5.0) - 5.0).abs() < 1e-09, "abs");
        assert!((f64m::c2f(37.5) - 99.5).abs() < 1e-09, "c2f");
        assert!((f64m::f2c(212.0) - 100.0).abs() < 1e-09, "f2c");
        assert!((f64m::floor(3.7) - 3.0).abs() < 1e-09, "floor");
        assert!((f64m::ceil(3.2) - 4.0).abs() < 1e-09, "ceil");
        assert!((f64m::round(2.6) - 3.0).abs() < 1e-09, "round");
        assert!((f64m::trunc(3.9) - 3.0).abs() < 1e-09, "trunc");
        assert!((f64m::fmin(3.0, 5.0) - 3.0).abs() < 1e-09, "fmin");
        assert!((f64m::fmax(3.0, 5.0) - 5.0).abs() < 1e-09, "fmax");
        assert!((f64m::clamp(5.0, 0.0, 3.0) - 3.0).abs() < 1e-09, "clamp");
        assert!((f64m::copysign(3.0, -1.0) - -3.0).abs() < 1e-09, "copysign");
        assert!((f64m::square(2.5) - 6.25).abs() < 1e-09, "square");
        assert!((f64m::cube(2.0) - 8.0).abs() < 1e-09, "cube");
        assert!((f64m::avg(3.0, 5.0) - 4.0).abs() < 1e-09, "avg");
        assert!((f64m::deg2rad(180.0) - 3.141592653589793).abs() < 1e-09, "deg2rad");
        assert!((f64m::rad2deg(3.141592653589793) - 180.0).abs() < 1e-09, "rad2deg");
        assert!((f64m::fsign(-5.0) - -1.0).abs() < 1e-09, "fsign");
        assert!((f64m::clamp01(2.5) - 1.0).abs() < 1e-09, "clamp01");
        assert!((f64m::frac(3.75) - 0.75).abs() < 1e-09, "frac");
    }
}
