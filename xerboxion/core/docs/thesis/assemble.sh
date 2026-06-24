#!/usr/bin/env bash
# Assemble la thèse en un seul document lisible : thesis.md (concatène 00->08).
# Déterministe (ordre lexical des fichiers numérotés). Idempotent.
# Usage : bash assemble.sh   ->   docs/thesis/thesis.md
set -euo pipefail
cd "$(dirname "$0")"

OUT="thesis.md"
: > "$OUT"

# Front-matter d'abord (00-resume), puis les chapitres dans l'ordre.
for f in 00-resume.md 01-introduction.md 02-etat-de-l-art.md 03-grammaire.md \
         04-tsoin-engine.md 05-implementation.md 06-evaluation.md \
         07-discussion.md 08-conclusion.md; do
  if [ -f "$f" ]; then
    cat "$f" >> "$OUT"
    # saut de page (compatible pandoc/MD) entre sections
    printf '\n\n---\n\n<div style="page-break-after: always"></div>\n\n' >> "$OUT"
  else
    echo "MANQUE: $f" >&2
  fi
done

LINES=$(wc -l < "$OUT")
echo "thesis.md assemblé : $LINES lignes, $(grep -c '^# ' "$OUT") sections de tête."
echo "Pour un PDF (quand le gabarit/langue seront décidés) :"
echo "  pandoc thesis.md -o thesis.pdf --toc --number-sections"
