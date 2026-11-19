# Audit FILEOPS-01

Fichiers lus en entier :

- [x] `src/systems/compression_output.rs`
- [x] `src/systems/compression_sources.rs`
- [x] `src/systems/copied_source.rs`
- [x] `src/systems/copy_benchmark.rs`
- [x] `src/systems/copy_diagnostics.rs`
- [x] `src/systems/destination_tree.rs`
- [x] `src/systems/file_copy.rs`
- [x] `src/systems/file_ops.rs`
- [x] `src/systems/file_ops/archive_transfer.rs`
- [x] `src/systems/file_ops/compress.rs`
- [x] `src/systems/file_ops/copy.rs`
- [x] `src/systems/file_ops/copy/destinations.rs`
- [x] `src/systems/file_ops/copy/directories.rs`
- [x] `src/systems/file_ops/copy/links.rs`
- [x] `src/systems/file_ops/copy/whole_moves.rs`
- [x] `src/systems/file_ops/copy/worker.rs`

Synthèse : 2 élevés, 3 moyens, 10 faibles, 6 optimisations. Aucun constat critique. Les invariants de publication exclusive, de suppression du seul objet retenu et de rejet des points de reparse tiennent dans le périmètre lu.

## Constats

### compress.rs:268 (élevé, confirmé)
- Défaut : la boucle de publication ne sort sur `AlreadyExists` qu'en demandant un nouveau nom à `unique_archive_path` (`file_ops.rs:389`). Or ce nom repose sur `Path::exists()`, qui suit les liens et renvoie faux sur toute erreur. Si le nom reste occupé par un objet qu'`exists()` ne voit pas, la fonction renvoie le même nom et la boucle tourne sans fin, sans point d'annulation.
- Scénario : on compresse « photos » dans un dossier qui contient un lien symbolique cassé `photos.zip` (ou une entrée dont les métadonnées sont refusées). La publication exclusive échoue sans fin et un cœur reste à 100 %. L'opération ne peut pas être annulée et garde l'un des deux créneaux du planificateur.

### destination_tree.rs:150 (élevé, plausible)
- Défaut : une copie garde ouverts en même temps un descripteur par dossier source contenant un fichier planifié (les `SelectedFile` retenus dans `files_to_process` gardent leur `BoundDirectory` jusqu'à `copy.rs:685`) et un descripteur par dossier créé à destination (cache de `DestinationTree`, libéré en fin d'opération). `Destinations` (`destinations.rs:119`) y ajoute un descripteur par dossier existant fusionné. Aucune limite ni hausse de `RLIMIT_NOFILE` n'existe dans le projet.
- Scénario : sous Linux, avec la limite douce habituelle de 1024 descripteurs, copier une arborescence d'environ 500 dossiers non vides (bibliothèque photo, `node_modules`) échoue en cours de route avec « Too many open files ».

### worker.rs:263 (moyen, plausible)
- Défaut : pour un déplacement vers ou depuis une archive, chaque fichier copié garde jusqu'au commit sa source, sa sortie et les deux chaînes complètes de dossiers parents (`HeldParents`). Cela fait environ 2 × (profondeur + 2) descripteurs par fichier.
- Scénario : sous Linux (limite de 1024), déplacer une centaine de fichiers situés à trois niveaux de profondeur dans une archive épuise les descripteurs avant le commit, et l'opération échoue.

### copy_diagnostics.rs:160 (moyen, confirmé)
- Défaut : chaque fichier copié écrit deux lignes `info` (ici et `worker.rs:143`) avec les chemins complets. Le filtre `oxyde_lib=info` de `main.rs:67` les laisse passer, et le journal de session est plafonné à 32 Mio (`main.rs:37`).
- Scénario : une copie d'environ 40 000 petits fichiers épuise le budget de journal. Tous les avertissements, erreurs et paniques ultérieurs de la session sont perdus, et chaque fichier coûte en plus une écriture synchrone dans le journal.

### archive_transfer.rs:46 (moyen, plausible)
- Défaut : le refus « archive dans elle-même » ne compare que l'égalité exacte entre une source et l'archive cible. Un dossier source qui contient l'archive cible passe la vérification.
- Scénario : on glisse le dossier `Projet` dans `Projet/sauvegarde.zip`. L'archive reconstruite contient une copie de son ancienne version. En déplacement, les sources sont ensuite supprimées jusqu'à ce que la suppression de `sauvegarde.zip`, remplacée entre-temps, échoue et laisse l'opération en erreur à mi-chemin.

### compress.rs:271 (faible, confirmé)
- Défaut : après une collision, le nom suivant est dérivé du dernier candidat et non du nom d'origine.
- Scénario : si `a.zip` et `a (1).zip` apparaissent pendant l'encodage, l'archive est publiée sous `a (1) (1).zip` au lieu de `a (2).zip`.

### file_ops.rs:395 (faible, confirmé)
- Défaut : la reconnaissance des extensions composées (`.tar.gz`…) est sensible à la casse.
- Scénario : si `Backup.TAR.GZ` existe déjà, le nom proposé devient `Backup.TAR (1).GZ`.

### file_copy.rs:289 (faible, confirmé)
- Défaut : quand le remplacement est demandé, `check_replacement` exige que la cible existe encore. Si elle a disparu entre la planification et la copie, l'erreur `NotFound` fait échouer toute l'opération, alors que la publication aurait simplement créé le fichier.
- Scénario : l'utilisateur choisit « Remplacer » pour `a.txt`, puis une autre application supprime `a.txt` avant que le worker l'atteigne. La copie entière s'arrête en erreur.

### copy.rs:387 (faible, confirmé)
- Défaut : le conflit d'un fichier est détecté par `exists()`, qui suit les liens. Un lien cassé à destination n'est donc pas vu comme un conflit (contrairement au traitement des liens, ligne 373).
- Scénario : un lien symbolique cassé `rapport.pdf` occupe la destination. Aucune question n'est posée, la publication exclusive échoue avec `AlreadyExists` et l'opération s'arrête après une partie des fichiers.

### copy.rs:255 (faible, confirmé)
- Défaut : `shared_names` compte les noms de façon sensible à la casse. Deux dossiers `Docs` et `docs`, sélectionnés à deux endroits, passent tous deux en déplacement « dossier entier ». `destinations::validate`, qui compare selon les règles de casse du volume, refuse alors l'opération pour collision au lieu de fusionner, comme il le ferait pour deux noms identiques.
- Scénario : sous Windows, déplacer `A\Docs` et `B\docs` vers `T` échoue avec une erreur de collision.

### copy.rs:273 (faible, confirmé)
- Défaut : `same_entry` compare l'identité de fichier. Un lien physique du même fichier, présent sous le même nom dans la destination, est donc traité comme un auto-transfert.
- Scénario : déplacer `A\a.txt` vers `B`, où `B\a.txt` est un lien physique du même fichier, est ignoré sans conflit (la source reste en place). En copie, `a - Copie.txt` est créé au lieu de proposer un remplacement.

### archive_transfer.rs:171 (faible, plausible)
- Défaut : après succès, tout le journal d'effets est vidé, y compris les fichiers créés sur le vrai système de fichiers quand la destination n'est pas une archive.
- Scénario : on extrait par glisser des fichiers d'une archive vers un dossier réel. L'opération ne laisse aucune entrée annulable, alors que les fichiers publiés sont bien réels.

### compression_sources.rs:135 (faible, confirmé)
- Défaut : la déduplication des noms d'archive se fait en minuscules, quel que soit le système de fichiers source.
- Scénario : sous Linux, compresser un dossier contenant `README` et `readme` échoue avec « Ambiguous archive source names ».

### compression_sources.rs:98 (faible, confirmé)
- Défaut : une source sans nom de fichier est refusée.
- Scénario : compresser la racine d'un lecteur (`E:\`, clé USB) échoue avec « Invalid compression source name ».

### copy.rs:561 (faible, plausible)
- Défaut : sous Windows, la détection SSD est mise en cache par préfixe de lecteur, ce qui ignore les volumes montés dans un dossier. Sous Linux, elle fait un `stat` par fichier source.
- Scénario : sous Windows, des sources sur un disque dur monté dans `C:\mnt\hdd` héritent du statut SSD de `C:` et reçoivent 4 workers en mode Turbo, ce qui provoque des déplacements de tête. Sous Linux, 100 000 fichiers coûtent 100 000 `stat` avant la copie.

### file_copy.rs:282 (optimisation, confirmé)
- Défaut : pour chaque fichier, les parents de la source sont résolus de nouveau par chemin (deux `canonicalize` et une ouverture par ancêtre), alors que le worker détient déjà la source et la chaîne de ses parents (`open_copy`, `open_selected`).
- Scénario : 100 000 petits fichiers à une profondeur de 8 entraînent environ un million d'ouvertures de dossiers redondantes.

### copy.rs:315 (optimisation, confirmé)
- Défaut : sous Windows, chaque entrée parcourue est ouverte (`open_archive`, avec clonage de la chaîne des parents) et `verify_archive_item` verrouille l'opération, même sans archive en jeu.
- Scénario : un déplacement sur le même volume de 100 000 fichiers, qui ne devrait faire que des renommages, ouvre aussi en lecture chaque fichier pendant la planification.

### destination_tree.rs:134 (optimisation, confirmé)
- Défaut : `ensure_directory` repart de la racine pour chaque fichier. Chaque niveau coûte un `try_clone`, un `metadata`, une prise du verrou et un point de contrôle qui verrouille une ou deux fois le mutex de l'opération.
- Scénario : une arborescence profonde de petits fichiers multiplie les appels système et la contention sur le mutex que l'interface lit à chaque rendu.

### copy.rs:166 (optimisation, confirmé)
- Défaut : `source_name` cherche linéairement dans `trash_entries`. Elle est appelée quatre fois par source, ce qui donne O(S × T).
- Scénario : restaurer 10 000 éléments de la corbeille vers un dossier fait environ 4 × 10⁸ comparaisons de chemins avant le début du transfert.

### file_ops.rs:494 (optimisation, plausible)
- Défaut : `get_operation`, appelé dans `render` du panneau des opérations (`ui/components/operations/view.rs:161`), clone à chaque image toutes les sources et les résultats par élément, sous les deux verrous.
- Scénario : une opération lancée sur 50 000 éléments sélectionnés clone 50 000 `PathBuf` à chaque rafraîchissement du panneau.

### file_copy.rs:52 (optimisation, plausible)
- Défaut : chaque fichier copié crée puis supprime un dossier de staging, un fichier pin et une note JSON verrouillée dans le dossier de configuration (`staging_journal::record`).
- Scénario : pour des milliers de petits fichiers, ces créations et suppressions dépassent le coût des données, et chacune est examinée par l'antivirus.

<!-- LOT TERMINÉ -->
