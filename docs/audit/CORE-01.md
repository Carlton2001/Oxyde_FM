# CORE-01 : audit en lecture seule

## Fichiers lus en entier

- [x] `build.rs`
- [x] `src/app_paths.rs`
- [x] `src/bin/copy_benchmark.rs`
- [x] `src/bin/oxyde-folder-launcher.rs`
- [x] `src/bin/oxyde-image-worker.rs`
- [x] `src/commands/audio_handle_worker.rs`
- [x] `src/commands/duplicates.rs`
- [x] `src/commands/icon_session_worker.rs`
- [x] `src/commands/image_handle_worker.rs`
- [x] `src/commands/image_handle_worker/session.rs`
- [x] `src/commands/io.rs`
- [x] `src/commands/mod.rs`
- [x] `src/commands/office_handle_worker.rs`
- [x] `src/commands/ops.rs`
- [x] `src/commands/ops/conflicts.rs`
- [x] `src/commands/ops/conflicts/summaries.rs`
- [x] `src/commands/ops/linux_trash.rs`
- [x] `src/commands/ops/trash_listing.rs`
- [x] `src/commands/ops/trash_restore.rs`
- [x] `src/commands/search.rs`
- [x] `src/commands/search/content.rs`
- [x] `src/commands/search/content/normalize.rs`

Vérifications ciblées hors liste (Grep ou lecture partielle) : appelants de `ops` et `io` (tous passent par `spawn_blocking`, pas de blocage du thread UI), consommateur de la recherche (`ui/app/search.rs`), plafonds de réponses côté parent des workers, `platform/hardware.rs`, `linux_volumes/disks.rs`, `systems/item_rename.rs`, `utils/path_text.rs`, `search/criteria.rs`.

Aucun constat sur `build.rs`, `copy_benchmark.rs`, `oxyde-folder-launcher.rs`, `oxyde-image-worker.rs`, `commands/mod.rs`, `audio_handle_worker.rs`, `office_handle_worker.rs`, `icon_session_worker.rs` et `summaries.rs` hors celui du code de sortie des workers. Les tailles de messages des workers sont bornées côté lecture par le parent. La session d'icônes borne ses entrées et ses réponses.

## Constats

### Élevé

1. `src/commands/search.rs:460` — élevé, confirmé. La recherche de contenu ouvre chaque entrée non dossier avant tout contrôle de type, et l'ouverture n'est pas annulable. Scénario : un tube nommé (FIFO) dans l'arborescence (`/tmp`, `/run`, caches) fait bloquer `File::open` indéfiniment. Le fil de recherche reste figé, aucun `Finished` n'arrive et l'annulation n'a aucun effet. Même effet sur un partage réseau qui ne répond plus.

### Moyen

2. `src/commands/search/content.rs:155` (propagé par `content.rs:60-67` et `search.rs:544`) — moyen, confirmé. La première ligne qui n'est pas de l'UTF-8 valide interrompt la lecture du fichier. L'échec n'est consigné qu'en debug, sans événement `ContentSkipped`, et le fichier est déclaré sans correspondance. Scénario : un fichier texte Windows-1252 ou Latin-1 contenant un « é » en ligne 3 n'est plus cherché après la ligne 2. Les correspondances plus loin sont perdues sans avertissement.
3. `src/commands/search.rs:517-530` — moyen, plausible. Aucun contrôle des fichiers cloud non hydratés (Windows, attribut de rappel sur accès aux données). Scénario : une recherche de contenu dans un dossier OneDrive en Fichiers à la demande déclenche le téléchargement de chaque fichier candidat.
4. `src/commands/io.rs:135-138` — moyen, plausible. Une seule erreur de lecture d'une entrée (ou de ses métadonnées) fait échouer tout le listage du dossier. `visible_contents` et `walk_contents` ignorent ces erreurs, donc le comportement est incohérent. Scénario (Linux, où `DirEntry::metadata` fait un `lstat`) : un fichier temporaire disparaît entre la lecture du dossier et son `lstat`, dans `/tmp` ou un cache de navigateur. L'onglet affiche une erreur au lieu du contenu.
5. `src/commands/ops/conflicts.rs:91-99` — moyen, plausible. La récursion n'a pas de borne de profondeur, et toute erreur de `read_dir` ou d'entrée interrompt la vérification entière. Sous Windows, `copied_as_link` renvoie toujours faux, donc les jonctions et liens de dossier sont suivis. Scénario : copier un dossier contenant une jonction qui boucle vers lui-même vers une destination qui contient déjà le même dossier avec la même jonction. Les deux côtés existent à chaque niveau, la récursion ne s'arrête qu'à la fin de la pile du fil `spawn_blocking`, et l'application plante. Autre scénario : un sous-dossier illisible dans un dossier en conflit bloque tout le transfert avec une erreur.
6. `src/commands/ops.rs:321` avec `src/commands/ops/linux_trash.rs:81` — moyen, plausible. Sous Linux, l'annulation d'une suppression retrouve l'élément en corbeille par son chemin d'origine, si un seul élément correspond, sans vérifier l'identité ni la date de suppression. Scénario : l'utilisateur supprime `a/doc.txt` (historique), vide la corbeille depuis un autre outil, puis un autre programme met un nouveau `a/doc.txt` à la corbeille. Annuler dans Oxyde restaure ce fichier étranger, qui est le seul candidat. Avec deux candidats, l'annulation échoue sans risque (ambigu).

### Faible

7. `src/commands/search.rs:467` — faible, confirmé. Un fichier est considéré binaire s'il contient un octet nul dans les 1024 premiers. Les fichiers texte UTF-16 (`.reg`, exports CSV et PowerShell Windows) sont donc ignorés sans événement ni message.
8. `src/commands/duplicates.rs:94` — faible, plausible. Le hash partiel (début et fin) s'appuie sur une seule lecture, qui peut rendre moins d'octets que demandé. Scénario : sur un partage réseau ou un système de fichiers FUSE, deux fichiers identiques obtiennent des hash partiels différents et le doublon est manqué.
9. `src/commands/duplicates.rs:341` (aussi 396 et 446) — faible, confirmé. Les erreurs de lecture lors du hash sont avalées. Un fichier illisible ou verrouillé disparaît du résultat sans trace, et l'interface ne peut pas signaler que la recherche est incomplète.
10. `src/commands/duplicates.rs:150-156` — faible, plausible. Le préfixe `\\?\` est retiré même pour un chemin canonique `\\?\Volume{GUID}\`, ce qui donne un chemin relatif. Scénario : un volume monté dans un dossier sans lettre de lecteur est refusé avec « pas un dossier ».
11. `src/commands/ops.rs:131-135` et `src/commands/ops/linux_trash.rs:112` — faible, confirmé. Le vidage sélectif de la corbeille ignore les sélections introuvables ou ambiguës (simple avertissement dans le journal) et renvoie un succès. Scénario : le contenu de la corbeille change entre l'affichage et l'action, et l'interface annonce une suppression qui n'a pas eu lieu.
12. `src/commands/ops/conflicts.rs:54` — faible, plausible. Sous Windows, un conflit est testé avec `exists()`, qui renvoie faux pour un lien cassé ou une erreur d'accès. Scénario : un lien symbolique cassé occupe déjà le nom cible. Aucun conflit n'est signalé, et l'exécuteur doit rattraper la collision.
13. `src/commands/ops/trash_listing.rs:53` — faible, plausible. Le repli de listage Windows lit les métadonnées en suivant les liens (la version Linux n'en suit pas). Scénario : un lien de dossier mis à la corbeille est présenté comme dossier avec la taille et le type de sa cible, qui peut être extérieure à la corbeille.
14. `src/commands/ops/trash_listing.rs:50` et `src/commands/ops/linux_trash.rs:30` — faible, plausible. Le produit date × 1000 n'est pas protégé contre le débordement, alors que `io.rs:680` le protège avec `checked_mul`. Scénario : une date de suppression absurde (fichier `.trashinfo` ou base corrompu) fait paniquer le listage en dev, ou affiche une date faussée en release.
15. `src/commands/ops.rs:332-335` et `515-517` — faible, plausible. L'annulation et la reprise d'un renommage reconstruisent les chemins avec `PathBuf::from` sans décoder l'échappement des noms non UTF-8 (`path_text::decode`), alors que le renommage lui-même le décode (`io.rs:387-390`). Scénario (Linux) : renommer `bad\xFF.txt` en `bon.txt` (cas que le code cite comme usage prévu), puis annuler. L'annulation échoue avec une erreur de chemin.
16. `src/commands/image_handle_worker.rs:76-79` (même motif dans les workers audio, office, session d'icônes et session d'images) — faible, confirmé. Un worker en échec écrit sur stderr puis finit par `TerminateProcess` avec le code 0. Le parent ne peut pas distinguer un échec d'une réponse vide ou tronquée par le code de sortie, et doit tout deviner à partir de stdout.
17. `src/commands/ops.rs:482` — faible, confirmé. Le rejeu d'une copie (redo) se fait avec un drapeau d'annulation qui n'est jamais levé. Une grosse copie en cours bloque les autres actions d'historique (`history_operation_in_progress`) jusqu'à sa fin, sans moyen de l'arrêter.

### Optimisation

18. `src/commands/duplicates.rs:335`, `390`, `445` — optimisation, confirmé. Un verrou par disque physique est tenu pendant tout le hash, y compris la phase de hash complet. La détection SSD n'est utilisée que pour le parcours. Sur un SSD NVMe, tout le hash tourne donc sur un seul cœur par volume alors que le blake3 pourrait occuper tous les cœurs.
19. `src/commands/duplicates.rs:330`, `385`, `440` et `src/commands/search.rs:520` — optimisation, confirmé. L'identifiant du disque physique est recalculé pour chaque fichier. Sous Windows (`platform/hardware.rs:90`), chaque appel ouvre `\\.\X:` et lance un `DeviceIoControl`, sans cache (Linux met en cache pendant 10 s). Pour 1 million de candidats, la recherche de doublons fait 3 millions d'ouvertures de périphérique.
20. `src/commands/ops.rs:393-400` — optimisation, plausible. Le rejeu d'une copie cherche chaque hash par recherche linéaire dans la liste des hash, soit un coût quadratique. Pour 50 000 fichiers copiés, cela fait environ 1,25 milliard de comparaisons de chaînes avant de commencer.
21. `src/commands/duplicates.rs:409-424` — optimisation, confirmé. La seconde passe (hash de fin) regroupe seulement par taille et hash de fin, sans rappeler le hash de début. Des fichiers de groupes de début différents qui partagent un même pied (zéros, signature) sont envoyés ensemble en hash complet, alors qu'aucun n'a de pair réel.
22. `src/commands/ops/trash_restore.rs:69` — optimisation, plausible. Sous Windows, le nom de restauration est résolu pour tous les éléments de la corbeille (un accès au fichier `$I` chacun), avant la sélection. Restaurer un seul fichier d'une corbeille de 50 000 éléments lit 50 000 fichiers.
23. `src/commands/ops/linux_trash.rs:84` — optimisation, confirmé. L'unicité de la sélection se vérifie par `contains` sur un vecteur, soit un coût quadratique. Restaurer 50 000 éléments représente environ 1,25 milliard de comparaisons.
24. `src/commands/search.rs:153` et `172` (aussi `460` et `539`) — optimisation, confirmé. Sous Linux, chaque entrée coûte deux `lstat` (filtre puis boucle), car walkdir ne met pas les métadonnées en cache. Pour la recherche de contenu, chaque candidat est ouvert deux fois (test binaire, puis lecture).
25. `src/commands/ops/linux_trash.rs:51` et `src/commands/io.rs:576` — optimisation, plausible. Chaque appel aux propriétés d'un élément de la corbeille relance un listage complet de la corbeille, avec lecture de tous les `.trashinfo` sous Linux. Scénario : Propriétés d'une sélection multiple dans la corbeille, soit N listages complets.
26. `src/commands/search.rs:280` — optimisation, plausible. La recherche rapide en mode texte normalise et met en minuscules la requête à chaque nom testé (plusieurs allocations par entrée, voir `criteria.rs:37-45`). Pour plusieurs millions d'entrées, la requête pourrait être préparée une seule fois.

## Bilan

Critique : 0, élevé : 1, moyen : 5, faible : 11, optimisation : 9 (26 constats).

<!-- LOT TERMINÉ -->
