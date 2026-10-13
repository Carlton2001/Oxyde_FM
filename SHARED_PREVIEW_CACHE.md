# Cache disque partagé des aperçus — 14 octobre 2026

Les infobulles et la grille partagent un PNG de 1 024 px maximum pour les images et la première page des PDF. Le premier consommateur génère et enregistre ; les suivants peuvent lire sans redécoder le fichier source ni refaire le rendu PDF, y compris après redémarrage d’Oxyde. Un verrou de génération commun évite deux rendus simultanés de la même entrée. Les petits fichiers image ne sont pas agrandis.

Le PNG conserve l’alpha et les couleurs. La conversion RGBA/BGRA respecte GPUI ; les flags de première frame/animation sont conservés dans un trailer après le PNG. Les visionneuses PNG ordinaires peuvent ouvrir l’image. La grille utilise un bitmap mémoire de 256 px ; les aperçus gardent le rendu commun de 1 024 px. Les infobulles animées gardent leur animation et le cache mémoire existant ; leur première frame disque sert à la grille.

## Stockage et invalidation

- Répertoire existant : `dirs::cache_dir()/oxyde/thumbnails` (`%LOCALAPPDATA%/oxyde/thumbnails` sous Windows).
- Clé BLAKE3 versionnée : chemin absolu, type image/PDF, taille, date de modification et taille de rendu.
- Lecture et écriture via `Platform`, publication atomique, temporaire nettoyé par l’implémentation native.
- Quota Windows partagé avec les miniatures : 256 Mio, 4 096 entrées, maximum 4 Mio par entrée. Une image dépassant le budget reste utilisable en mémoire.
- Encodage et décodage bornés ; cache illisible, corrompu ou périmé ignoré. Les changements pendant le chargement empêchent sa réutilisation/publication sous une ancienne signature.
- La commande Effacer le cache inclut ces fichiers, car ils sont dans le répertoire déjà nettoyé. L’invalidation des miniatures retire maintenant aussi les pixels d’images du LRU mémoire, avec une révision contre les insertions tardives.
- Les anciennes miniatures JPEG restent distinctes et ne sont pas agrandies pour fabriquer les nouveaux aperçus.

Le cache ne remplace ni l’isolation des décodeurs de sources, ni leurs budgets. Une écriture impossible est une optimisation manquée, pas une erreur d’affichage. Les hits disque ont le même comportement de transition que les hits mémoire : pas d’attente artificielle de 150 ms après résolution.

## Validation

Les tests de persistance utilisent exclusivement des fixtures temporaires : réouverture indépendante, PNG lisible et couleurs/alpha, flags animés, signature changée, données invalides/trop grandes, échec d’écriture et verrou partagé. Les tests Windows PDF vérifient le rendu natif à 1 024 px, les ratios extrêmes, les limites de buffers et la libération du document. Le test d’invalidation mémoire vérifie qu’une génération antérieure ne se réinsère pas après invalidation.

Vérifications réalisées : `cargo check --offline` sans avertissement ; `cargo test --offline --bin oxyde ui:: -- --test-threads=1` avec 219 tests réussis et un ignoré ; `cargo test --offline --lib windows_pdf_thumbnail -- --test-threads=1` avec six tests réussis ; exécutable reconstruit et diff vérifié.

La validation visuelle dans une session Oxyde et la compilation Linux complète restent à réaliser. Le quota décrit est celui de l’implémentation Windows existante. La première visite entraîne une génération et une écriture PNG ; le gain concerne surtout la réutilisation et les redémarrages. Le cache disque n’archive pas toutes les frames des animations.

Pour essayer : survoler une image et un PDF, activer les miniatures de grille et retrouver leurs rendus ; redémarrer Oxyde puis recommencer. Modifier un fichier doit produire un nouveau rendu. Effacer le cache doit supprimer les fichiers partagés ; les aperçus actifs peuvent être régénérés ensuite. Les logs debug signalent les hits disque.
