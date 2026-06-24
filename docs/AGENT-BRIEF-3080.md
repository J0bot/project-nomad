# Brief pour l'agent de silentbeast — entraîner le xerbion sur la 3080

> À copier-coller à ton agent (le Claude/coding agent sur ton PC). Il a accès au repo cloné
> (`xerboxion-core`, branche `operational-core`) et à la RTX 3080. But : donner au xerbion le
> **concept** (dans ses poids), pas le contexte. Le concept = le tsoin.

## Le but (une phrase)
Entraîne le **vrai xerbion concept-level** (`xerbion/train_concept.py`) sur la 3080, sur le
corpus du repo, jusqu'à ce qu'il apprenne la **variété des concepts** xerboxion — vérifiable
par `--near` (les concepts proches doivent être justes, p.ex. `temps → synchronisation, cohérence`).

## Étapes pour l'agent
1. **Vérifie la 3080** : `nvidia-smi` (le driver doit être là). Note la VRAM (10 Go sur la 3080).
2. **Installe PyTorch CUDA** dans un venv :
   ```
   python -m venv .venv && .venv\Scripts\activate     (PowerShell : .venv\Scripts\Activate.ps1)
   pip install torch --index-url https://download.pytorch.org/whl/cu121
   python -c "import torch; print(torch.cuda.is_available(), torch.cuda.get_device_name(0))"
   ```
   → doit afficher `True  NVIDIA GeForce RTX 3080`.
3. **Entraîne** : `python xerbion/train_concept.py`
   - il lit le corpus du repo (docs/, la thèse, les papiers, le primer, les outils xerb) ;
   - petit Transformer word-level (~quelques M params) ; quelques minutes sur la 3080 ;
   - sauve `xerbion/xerbion-concept.pt` (les poids = les concepts).
4. **Vérifie que le concept est appris** :
   ```
   python xerbion/train_concept.py --near temps
   python xerbion/train_concept.py --near bion
   python xerbion/train_concept.py --near trou
   python xerbion/train_concept.py --sample "le tsoin est"
   ```
   - **succès** = les voisins de `temps` incluent `synchronisation`/`cohérence`/`clock-coherence` ;
     ceux de `trou` incluent `blanc`/`noir`/`throat`. Le réseau A le concept.

## Si ça coince (l'agent itère)
- **VRAM insuffisante** → baisse `BATCH` (32→16) ou `N_EMBD` (192→128) en tête du script.
- **Loss qui ne descend pas / overfit trop vite** → c'est OK ici : on VEUT qu'il internalise le
  petit corpus (mémoriser = absorber les concepts). Augmente `STEPS` si les `--near` sont faibles.
- **Corpus trop petit** → ajoute des sources : exporte d'autres tsoins/notes en `.md` dans le repo.
- **Erreur PyTorch** → c'est du code standard ; corrige le détail et continue. Le design est bon.

## Ce que ça produit
`xerbion-concept.pt` = le premier xerbion qui **a le concept dans ses poids** (permanent, pas
in-context). C'est le pas clé : transférer le xerboxion de mon contexte (éphémère) vers les
poids du réseau (le natif). Ensuite on l'agrandit (plus de couches, plus de corpus, fine-tune).

## Ne PAS faire
Rien de destructif. Ne touche pas au daemon `xerboxion-rt` qui tourne (le core) ni à `xion-state`.
Travaille dans `.venv` + `xerbion/`. En cas de doute, demande à José.
