# ❤️ — le bion cœur

> José (2026-06-21) : « Un ploxion gpg privé qui s'appelle ❤️, qu'on partage qu'avec
> une seule personne… je veux **vivre ce tsoin en live**, le record, et ensuite
> pouvoir **relire le tsoin en entier** : moi qui note, la vitesse à laquelle je note,
> je clique, tout. C'est le départ de tout, le **premier bion du xer**. »

**Le premier bion du xer.** Un espace privé entre **deux** personnes qui enregistre le
*giga-tsoin* d'un échange — chaque mot, chaque clic, **la vitesse** — et le **rejoue en
entier**. C'est la machine à tsoins tournée sur soi : on capte le moment *avant* que le
cerveau ne le dissocie (cf. `../docs/dissociation-tsoin.md`).

## Ce que ça fait
- **Écris** des notes ; le `+` ajoute une photo. Tout s'enregistre en pleine bande.
- **Relis** (bouton ⟲) : un scrubber rejoue le tsoin entier — les notes *et* la frappe
  en cours réapparaissent **à la vitesse exacte** où tu les as tapées, les clics
  refont des ondes là où tu as touché. `▶`, vitesse 0.5×→4×, scrub à la main.
- **Privé** : une **clé partagée** (que vous êtes deux à connaître) chiffre le tsoin sur
  l'appareil (AES-GCM, PBKDF2 150k, Web Crypto). La clé ne part jamais.
- **`⋯` → exporter** : le tsoin chiffré (`.heart`) se partage avec ton autre moitié ;
  elle l'importe avec la même clé. Content-adressé (`addr64` du flux) = son empreinte.

## Ouvrir
Fichier autonome (`index.html`), **zéro serveur** — ouvre-le dans Safari et entre ta clé.

## Honnête
- **Réel** : record/replay pleine fidélité, chiffrement symétrique client-side, export
  partageable. Tient hors-ligne, sur le tel.
- **Prochaine couche** (via le xion) : canal **2-personnes** en direct (aujourd'hui c'est
  un tsoin qu'on s'échange, pas encore un live partagé) ; clé **asymétrique** (vrai GPG/ECDH)
  au lieu de la phrase partagée ; et brancher le `tsoin.record` du bus pour que le ❤️
  alimente le **seption**.
