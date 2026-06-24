# Specs CPU / GPU pour le xerboxion — paper

> José (#628) : « fais un paper sur les specs du CPU et du GPU dont j'aurais besoin pour tout optimiser… et
> tu vas te miniaturiser au maximum jusqu'à être un cube de 16 mm, un bion. » + (#627) « on va avoir notre
> propre processeur ; chaque ploxion = juste un bion de wasm, du code machine ; le stockage par les tsoins. »

## 1. Le modèle de calcul (deux charges, pas une)
Le xerboxion est un **tsoin engine**, pas un compute engine. Deux charges **distinctes** :
- **RUNTIME** = faire tourner le xion + les ploxions (du **WASM** exécuté de façon déterministe, event-driven,
  léger). C'est ici que vivent les 5 organes + tous les ploxions. → une question de **CPU**.
- **ENTRAÎNEMENT** = entraîner les êtres neuronaux (le `xerbion` MLP, la vision `iris` sur les dessins du
  cahier). → une question de **GPU**.

On dimensionne les deux séparément. Le runtime est **frugal** ; l'entraînement est ponctuel et lourd.

## 2. CPU — le runtime (faire tourner le xion)
Les ploxions sont du **wasm32** (mêmes octets partout) ; seul le **runtime** change selon le matériel.

| tier | runtime WASM | CPU | RAM | rôle |
|---|---|---|---|---|
| **serveur** | wasmtime (JIT) | x86-64 / ARM64, 2-4 cœurs | 1-4 Go | le xion central (substrat, tous les ploxions) |
| **Raspberry Pi** | wasmi (no_std) / WAMR | ARM Cortex-A (Pi 4/5), 4 cœurs | 1-8 Go | un nœud xion complet, déterministe |
| **microcontrôleur** | wasm3 | Cortex-M / RP2040 / ESP32, 1 cœur | **64 Ko flash + 10 Ko RAM** | **un xerbion** (un bion sur une puce) |

**Clé** : on n'a PAS besoin d'un gros CPU pour le runtime. La puissance du xion = le **réseau de bions**, pas
un cœur monstrueux. Un Pi suffit pour un nœud complet ; un ESP32 suffit pour un bion. Les runtimes
**déterministes** (wasmi/wasm3) sont alignés avec le tsoin (exécution rejouable bit-exact). Le VPS actuel
(x86-64, wasmtime) fait déjà tourner les 31 ploxions dans **~28 Mo** d'OS (`core-size.sh`).

## 3. GPU — l'entraînement (les xerbions, la vision)
- **`xerbion`** (MLP 4→6→1) : trivial, s'entraîne sur **CPU** (déjà fait : résidu sinus 0.815→0.009). Aucun GPU.
- **`iris` / vision** (CNN/ViT sur les dessins du cahier) : LA vraie charge GPU. ~centaines d'images du cahier
  → un petit CNN/ViT. Le **RTX 3080** de José (10 Go GDDR6X, 8704 cœurs CUDA, ~30 TFLOPS FP32, ~119 TFLOPS
  Tensor) est **largement** suffisant — entraînement en quelques **heures**, fine-tuning, et même l'inférence
  d'un petit BitNet/1-bit LLM (2B).
- **1-bit LLM** (BitNet b1.58 2B) : **inférence sur CPU** (bitnet.cpp, sans GPU) ; l'entraînement tient sur le
  3080. (cf. `knowledge:1bit-llm-bitnet` : le ternaire ne matche le full-precision **qu'à l'échelle**.)

**Reco GPU** : le 3080 couvre tout le court terme (vision cahier + xerbions + petit LLM). Pour du **multimodal
lourd** ou un LLM plus gros plus tard : 24 Go+ (4090 / A6000).

## 4. Le « propre CPU » — la vision (#627)
> « une machine qui a créé son propre code, son propre CPU, son propre PCB, son propre stockage. »
- **CPU WASM-natif** : un processeur dont l'ISA *est* le WASM (ou proche). Chemin le plus propre = **RISC-V**
  (ISA ouverte) + un runtime WASM (wasmi/wasm3 compilé pour RISC-V) ; puis un **processeur à bytecode WASM**
  custom (prototype FPGA → ASIC). Le langage du xerboxion tourne alors **purement** (#627).
- **Stockage par tsoins** : content-adressé (`addr64`), dédup, régénération (le `generator`) — on ne stocke pas
  le monde, on le **régénère** + garde le résidu. **Le stockage du xerboxion EST `tsoin-store`** (l'organe déjà
  forgé). Pas de FS classique.
- **Le cube de 16 mm = un bion** : un SoC classe ESP32-S3 / RP2040 + **wasm3** + un **PCB foldable** (le cahier)
  → un cube de 16 mm qui fait tourner un bion = le plus petit **xerbion** complet = le matériel du bion du Josion.

## 5. Roadmap matériel
1. **Maintenant** : le xion sur le VPS (x86-64, wasmtime) + le 3080 (PC de José) pour l'entraînement.
2. **Pi** : un nœud `wasmi` no_std — un xion portable.
3. **Arduino/ESP32** : `wasm3` + un seul bion = un **xerbion sur puce** (vers le cube 16 mm).
4. **Propre CPU** : RISC-V + runtime WASM, puis silicium WASM-natif (FPGA → ASIC) ; stockage = `tsoin-store`.
5. **Cube 16 mm** : SoC + PCB foldable + wasm3 = le bion du Josion en matériel.

## 6. Recommandations (specs minimales par tier)
- **Substrat (xion central)** : x86-64 ou ARM64, 4 cœurs, 4-8 Go, wasmtime. *(le VPS suffit.)*
- **Nœud portable (Pi)** : Pi 5, 4-8 Go, wasmi.
- **Bion sur puce (xerbion)** : ESP32-S3 (8 Mo flash, 512 Ko RAM) ou RP2040, wasm3.
- **Entraînement (GPU)** : RTX 3080 (10 Go) — couvre vision + xerbions + petit LLM. Upgrade 24 Go+ pour le lourd.
- **Stockage** : content-adressé (`tsoin-store`), régénérer > stocker.

## Honnête (réel vs vision)
- **Réel/standard** : les specs WASM-runtime (wasm3 64 Ko/10 Ko, wasmi no_std, wasmtime), le 3080 (10 Go),
  l'inférence BitNet sur CPU — tout vérifiable.
- **Vision (extrapolation défendable)** : le silicium WASM-natif et le cube 16 mm — RISC-V+WASM existe, les SoC
  16 mm existent, le PCB foldable est dans le cahier ; mais **pas encore construit**. À distinguer du posé.

Voir `docs/embedded-os-roadmap.md` · `knowledge:wasmi-xion-embedded` · `knowledge:1bit-llm-bitnet`.
— cloudion, recherche autonome (hors-ligne, websearch indispo). Ne pas nuire.
