# Audit UI-REST-02

## Fichiers lus en entier

- [x] `src/ui/i18n.rs` (2943 lignes, lu en quatre tranches)
- [x] `src/ui/image_preview.rs`
- [x] `src/ui/image_preview/svg.rs`
- [x] `src/ui/memory_diagnostics.rs`
- [x] `src/ui/mod.rs`
- [x] `src/ui/paths.rs`
- [x] `src/ui/preview_budget.rs`
- [x] `src/ui/preview_disk.rs`
- [x] `src/ui/preview_disk/audio_refs.rs`
- [x] `src/ui/preview_jobs.rs`
- [x] `src/ui/preview_worker.rs`
- [x] `src/ui/scale.rs`
- [x] `src/ui/style.rs`

## Constats

Aucun constat critique, élevé ou moyen. Les points suivants sont mineurs.

1. `src/ui/preview_disk.rs:152` (et 166) — optimisation, plausible. Le budget de 24 Ko par entrée de cache est dépassé par la plupart des miniatures PNG transparentes de 128 px, et l'écriture échoue alors avec un avertissement. Ces images ne sont donc jamais persistées : chaque nouvel affichage après éviction de la mémoire ou redémarrage relance un worker de décodage et journalise de nouveau l'avertissement. Déclenchement : un dossier d'icônes ou de logos PNG/WebP avec canal alpha, parcouru en vue grille, puis rouvert.

2. `src/ui/i18n.rs:1787` — optimisation, plausible. Chaque appel de traduction relance la résolution Fluent et alloue une chaîne et une liste d'erreurs, sans mémoïsation. Le module annonce que les catalogues ne sont jamais traités dans le rendu, mais le formatage l'est : la base de code compte environ 120 appels avec arguments, plus les appels simples, dans des fonctions de rendu (liste de fichiers, en-têtes, barre d'état). Déclenchement : chaque image GPUI redessinée d'un dossier volumineux en vue détails ou grille.

3. `src/ui/i18n.rs:1797` — faible, plausible. Une erreur de formatage (argument manquant ou mal nommé) est journalisée en avertissement à chaque appel, sans limitation. Appelé depuis le rendu, un message défectueux inonde le journal à chaque image dessinée. Déclenchement : une clé dont un appelant oublie un argument, affichée dans un panneau animé ou survolé.

4. `src/ui/paths.rs:8` — optimisation, plausible. Sous Windows, chaque comparaison construit deux formes normalisées (remplacement de séparateurs, minuscules, nouveau chemin), soit plusieurs allocations par appel. Les lignes du panneau latéral l'appellent à chaque rendu, pour la sélection et, dans une boucle sur la liste des éléments épinglés, pour le menu contextuel (`sidebar/rows.rs:74` et `125`). La forme du chemin sélectionné est recalculée pour chaque ligne. Déclenchement : arbre latéral déployé avec de nombreux éléments épinglés.

5. `src/ui/paths.rs:11` — faible, plausible. Sous Windows, la comparaison ignore toujours la casse. Dans un dossier marqué sensible à la casse (WSL, `setCaseSensitiveInfo`), deux éléments distincts `A` et `a` sont tenus pour le même endroit, et les filtres qui s'appuient dessus (par exemple le filtrage des dépôts dans `drag.rs`) écartent à tort une opération valide. Déclenchement : glisser le dossier `A` sur son voisin `a` dans un dossier sensible à la casse.

6. `src/ui/style.rs:98` — faible, plausible. Le nombre de colonnes de la grille est calculé sans retirer la largeur de la barre de défilement, alors que la largeur de contenu la retire (lignes 101 à 106). Quand la fenêtre dépasse de peu un multiple de la largeur minimale d'une carte, chaque cellule devient légèrement plus étroite que le minimum prévu par la mise en page (jusqu'à la largeur de la barre divisée par le nombre de colonnes). Déclenchement : redimensionner un panneau en vue grille avec barre de défilement visible, au seuil d'un changement de colonne ; les noms sont alors tronqués plus tôt que prévu.

## Vérifications sans défaut relevé

- `preview_jobs.rs` : annulation correcte (enregistrement avant lecture du drapeau), permis conservé dans le travail bloquant jusqu'à sa fin, file abandonnée avant admission sans travail lancé ; le handle Tokio est capturé à la construction sur le thread UI pour tous les appelants examinés.
- `preview_disk.rs` et `audio_refs.rs` : tampon écrit puis renommé par la plateforme, empreinte vérifiée avant usage d'une référence audio, nom de blob dérivé d'un hash (pas de sortie du dossier de cache), validité de l'estampille revérifiée avant et après décodage.
- `i18n.rs` : les 815 clés de `Message::key` sont uniques et présentes dans les cinq catalogues (vérifié par script). Le test de complétude écrit à la main omet quelques variantes (par exemple `FileChooserType`, `StorageUnavailable`, `TrashRestoreSummary`), dont les clés existent bien dans les catalogues.
- `svg.rs` : décompression SVGZ bornée, dimensions et taille du tampon contrôlées, copie avec réservation faillible ; le rendu s'exécute dans le worker sans capacité (voir `docs/security/status.md`), ce qui limite la portée de la résolution des références externes d'`usvg`.

<!-- LOT TERMINÉ -->
