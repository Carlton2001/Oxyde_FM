# FILEOPS-03 : historique des déplacements, renommage, sélection des sources, staging, mises à jour

## Fichiers lus en entier

- [x] `src/systems/history_move.rs`
- [x] `src/systems/history_move_redo.rs`
- [x] `src/systems/history_renamed_dirs.rs`
- [x] `src/systems/item_rename.rs`
- [x] `src/systems/link_moves.rs`
- [x] `src/systems/mod.rs`
- [x] `src/systems/new_folder_history.rs`
- [x] `src/systems/path_parents.rs`
- [x] `src/systems/quick_access_watcher.rs`
- [x] `src/systems/source_directories.rs`
- [x] `src/systems/source_path_set.rs`
- [x] `src/systems/source_selection.rs`
- [x] `src/systems/staging_journal.rs`
- [x] `src/systems/transfer_naming.rs`
- [x] `src/systems/updates.rs`

## Constats

### `src/systems/source_selection.rs:197` : élevé, plausible
Chaque dossier parcouru garde son descripteur ouvert (`BoundDirectory.file`) tant qu'un fichier de son sous-arbre est vivant. Or `file_ops/copy.rs` accumule tous les fichiers dans `files_to_process` avant d'exécuter, et le projet ne relève jamais `RLIMIT_NOFILE` (aucun `setrlimit` hors `Resource::Data` dans `linux_worker.rs`). Les ancêtres tenus par `HeldParents` s'y ajoutent.
Scénario : sous Linux, avec la limite souple par défaut de 1024, copier ou déplacer une arborescence de plus d'environ 1000 dossiers contenant des fichiers (un projet `node_modules`, une photothèque) fait échouer toute la planification en EMFILE.

### `src/systems/path_parents.rs:25` : moyen, plausible
`HeldParents::new`, préalable de tout renommage, nouveau dossier, déplacement et suppression, dépend de `std::fs::canonicalize` (GetFinalPathNameByHandleW). Cet appel échoue sur certains volumes absents du gestionnaire de montage.
Scénario : sur un disque RAM ImDisk, un dossier partagé VirtualBox ou certains montages WebDAV, toutes les opérations de fichiers échouent avec « Incorrect function », alors que la lecture du dossier fonctionne.

### `src/systems/history_renamed_dirs.rs:61` : faible, confirmé
Quand la remise en place d'un dossier échoue, `revert` remplace l'erreur typée par un `io::Error` textuel. `CommandError::from` (`models/error.rs:622`) ne retrouve alors plus le `HistoryMoveFailure`.
Scénario : un Rétablir de déplacement échoue après avoir renommé des dossiers, et l'un d'eux ne peut pas reprendre son nom. L'utilisateur reçoit une erreur générique au lieu de `Incomplete`, alors que l'état est partiellement appliqué.

### `src/systems/history_move_redo.rs:334` : faible, plausible
`dedup` après un tri par profondeur seule ne supprime que les doublons adjacents. Deux occurrences d'un même chemin séparées par un chemin de même profondeur subsistent dans `created` (ligne 335) ou dans `removed` (ligne 341).
Scénario : si le journal contient deux fois un dossier créé, le second `create` reçoit AlreadyExists. Rétablir échoue alors sur un faux conflit et annule toute l'opération.

### `src/systems/history_move_redo.rs:419` : optimisation, confirmé
La validation préalable hache entièrement chaque fichier déplacé par copie. `move_bound_controlled` le hache ensuite à nouveau (`history_move.rs:233`), puis, entre volumes, hache aussi la copie et relit la source (`history_move.rs:265`).
Scénario : rétablir le déplacement entre volumes d'une vidéo de 20 Go lit 60 Go sur la source et 20 Go sur la destination.

### `src/systems/history_move.rs:57` : faible, confirmé
`NativeIo::copy` renvoie la taille du fichier source et non celle de la copie, si bien que le contrôle « Incomplete history move copy » (ligne 261) compare la source à elle-même et ne détecte jamais rien. Seul le hachage qui suit protège réellement.
Scénario : une copie tronquée n'est détectée que par la comparaison des empreintes, après une relecture complète inutile.

### `src/systems/link_moves.rs:17` : faible, plausible
Avec `replace`, la destination est écrasée (renommage à la ligne 17 ou `create_link` à la ligne 34) avant que la vérification après déplacement ou le retrait de la source soient acquis. En cas d'échec, l'opération est signalée en échec alors que l'élément remplacé est déjà détruit.
Scénario : un déplacement avec remplacement d'un lien symbolique entre volumes, où le retrait de la source échoue (lien modifié entre-temps, droits). La destination d'origine est perdue et le message dit seulement « it replaced the destination ».

### `src/systems/link_moves.rs:40` : faible, confirmé
Le `?` sur la capture d'identité du lien créé quitte la fonction avec cette erreur. L'erreur de retrait de la source est perdue, et le message ne mentionne pas la copie laissée à la destination.
Scénario : déplacement d'un lien entre volumes dont le retrait de la source échoue, puis échec de la lecture d'identité. Le lien existe en double sans que l'utilisateur en soit informé.

### `src/systems/staging_journal.rs:137` : faible, confirmé
La note est supprimée quel que soit le résultat de `clean`. C'est aussi le cas quand le volume est absent (NotFound à la ligne 154 traité comme « déjà parti ») ou injoignable (erreur traitée comme `kept`), et quand la note est vide ou illisible.
Scénario : Oxyde est tué pendant une copie vers une clé USB, puis redémarre clé débranchée. La note est effacée, et le dossier `.oxyde-…` caché contenant la sortie partielle reste définitivement sur la clé.

### `src/systems/staging_journal.rs:78` : optimisation, plausible
Chaque staging, donc chaque fichier copié (`file_copy.rs:292`), crée dans le dossier de configuration une note : `create_dir_all`, création avec verrou, écriture JSON, puis suppression.
Scénario : copier 10 000 petits fichiers ajoute environ 40 000 opérations de métadonnées dans le profil utilisateur, éventuellement itinérant ou sur un disque lent.

### `src/systems/source_selection.rs:357` : faible, plausible
Un enfant qui disparaît entre l'énumération et sa capture fait échouer `SelectedEntry::child` en NotFound. Le `?` interrompt alors tout le parcours au lieu d'ignorer l'entrée disparue.
Scénario : copier ou compresser un dossier actif (cache de navigateur, dossier de compilation) échoue entièrement à cause d'un fichier temporaire supprimé pendant la planification.

### `src/systems/path_parents.rs:54` : optimisation, confirmé
`with_bound_directory` duplique tous les handles de la racine, et `BoundDirectory::parents` (`source_selection.rs:33`) toute la chaîne d'ancêtres, à chaque appel. Ces appels ont lieu une fois par fichier déplacé ou copié (`SelectedFile::parents`, `open_copy`).
Scénario : déplacer 50 000 fichiers situés à 15 niveaux de profondeur coûte environ un million de duplications de handles ou de descripteurs.

### `src/systems/transfer_naming.rs:46` : faible, plausible
Le nom de copie généré n'est pas borné à la longueur maximale d'un composant (255). `symlink_metadata` renvoie alors InvalidFilename ou ENAMETOOLONG, que `copy_name` propage.
Scénario : dupliquer sur place un fichier dont le nom fait environ 250 caractères fait échouer toute la copie avec une erreur système brute.

### `src/systems/updates.rs:67` : faible, confirmé
Le dossier temporaire est conservé (`keep()`) et n'est supprimé qu'en cas d'échec d'écriture ou de vérification. Rien ne le nettoie si le lancement échoue ou si l'installation n'a pas lieu, et chaque tentative en crée un nouveau.
Scénario : des téléchargements répétés après un échec de `launch_update` accumulent dans le dossier temporaire des installateurs pouvant atteindre 512 Mo chacun.

### `src/systems/updates.rs:62` : optimisation, confirmé
L'installateur est téléchargé entièrement en mémoire (jusqu'à 512 Mo) avant d'être écrit sur le disque.
Scénario : une mise à jour volumineuse provoque un pic mémoire de la taille du paquet pendant le téléchargement.

<!-- LOT TERMINÉ -->
