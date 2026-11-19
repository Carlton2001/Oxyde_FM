# Audit WIN-FS-01

Résumé : 22 constats, aucun critique ni élevé. 4 moyens (glisser-déposer pour gauchers, copie sur volumes sans gestionnaire de montage, reparse non-liens traités comme des liens, suppression à sémantique héritée), 15 faibles, 3 optimisations.

## Fichiers lus en entier

- [x] `src/platform/windows_clipboard_write.rs`
- [x] `src/platform/windows_copy_staging.rs`
- [x] `src/platform/windows_disk_images.rs`
- [x] `src/platform/windows_disk_images/native.rs`
- [x] `src/platform/windows_drives.rs`
- [x] `src/platform/windows_eject_holders.rs`
- [x] `src/platform/windows_file_clipboard.rs`
- [x] `src/platform/windows_file_copy.rs`
- [x] `src/platform/windows_file_drag.rs`
- [x] `src/platform/windows_file_drag/data.rs`
- [x] `src/platform/windows_file_drag/target.rs`
- [x] `src/platform/windows_file_kind.rs`
- [x] `src/platform/windows_file_moves.rs`
- [x] `src/platform/windows_file_names.rs`
- [x] `src/platform/windows_file_tree.rs`

## Constats

### Moyen

- `src/platform/windows_file_drag.rs:339` : moyen, confirmé. Le démarrage du glisser natif teste `GetAsyncKeyState(VK_LBUTTON)`, qui lit le bouton physique gauche et non le bouton logique principal (comportement documenté par Microsoft). Scénario : avec les boutons de souris inversés (gaucher), le bouton principal est le bouton physique droit. `run()` conclut que le bouton est relâché, ne lance jamais `DoDragDrop` et signale une annulation, donc aucun glisser vers l'Explorateur ou une autre application n'est possible.
- `src/platform/windows_file_copy.rs:124` (et `:128`) : moyen, plausible. La source et le parent de destination passent par `std::fs::canonicalize`, donc par `GetFinalPathNameByHandleW(VOLUME_NAME_DOS)`. Cet appel échoue sur les volumes absents du gestionnaire de montage (disques RAM ImDisk, certains volumes virtuels ou chiffrés, certains redirecteurs). Scénario : toute copie depuis ou vers un disque RAM échoue avec « Incorrect function », alors que la source et le parent sont déjà tenus par handle.
- `src/platform/windows_file_copy.rs:118`, `src/platform/windows_file_tree.rs:138` et `:249` : moyen, plausible. Tout objet portant `FILE_ATTRIBUTE_REPARSE_POINT` est traité comme un lien, quelle que soit son étiquette : WOF/CompactOS, déduplication, placeholders cloud quand ils sont exposés, AppExecLink. Scénario : copier un dossier de jeu compressé par `compact /exe` ou CompactGUI échoue avec « Copy source is not an ordinary file ». Supprimer un dossier non vide portant une étiquette non-lien (dossier cloud exposé) tente d'effacer l'entrée sans l'énumérer et échoue avec « répertoire non vide ».
- `src/platform/windows_file_moves.rs:270` : moyen, plausible. La suppression utilise `FileDispositionInfo` hérité : l'effacement reste en attente tant qu'un autre processus garde le fichier ouvert avec `FILE_SHARE_DELETE`. Scénario : pendant une suppression récursive (`windows_file_tree.rs:272`) ou le nettoyage du staging (`windows_copy_staging.rs:106`, après le `.pin` fraîchement créé), Defender, l'indexeur ou une miniature de l'Explorateur tient encore un enfant. Le parent n'est pas encore vide, l'opération échoue par intermittence avec ERROR_DIR_NOT_EMPTY et laisse un dossier partiellement supprimé ou un staging caché orphelin. `FileDispositionInfoEx` avec sémantique POSIX n'est pas tenté, même là où il existe (NTFS, Windows 10 1607+).

### Faible

- `src/platform/windows_file_tree.rs:254` : faible, confirmé. Un fichier en lecture seule fait échouer la suppression (refus voulu et testé), mais seulement après la suppression définitive des frères déjà parcourus. Scénario : supprimer définitivement un dépôt Git (les packs de `.git/objects` sont en lecture seule) laisse une arborescence à moitié effacée, là où l'Explorateur supprime tout.
- `src/platform/windows_clipboard_write.rs:88` : faible, confirmé. `OpenClipboard` n'est tenté qu'une fois, sans nouvelle tentative (aucune non plus dans `clipboard.rs`). Scénario : l'historique du presse-papiers Windows, Ditto ou Office tient brièvement le presse-papiers, et Copier ou Couper échoue avec « Cannot open clipboard ».
- `src/platform/windows_disk_images/native.rs:206` : faible, plausible. Dans la boucle d'attente qui suit l'attachement, un échec de `open_device` ou `device_number` interrompt aussitôt le montage par `?` au lieu de réessayer, alors que l'image reste attachée à vie (`PERMANENT_LIFETIME`). Scénario : le périphérique `\\.\PhysicalDriveN` n'est pas encore ouvrable juste après l'attachement. L'interface affiche une erreur alors que l'image est bien montée.
- `src/platform/windows_disk_images/native.rs:372` : faible, confirmé. `image_path(&handle)?` propage l'erreur d'un seul périphérique (pointeur hors tampon, nombre d'entrées invalide), ce qui interrompt toute l'énumération. Scénario : une réponse malformée d'un seul disque vide toute la liste des images montées.
- `src/platform/windows_disk_images/native.rs:272` (et `:381`) : faible, plausible. Le chemin d'image reconstruit prend le point de montage le plus court du volume hôte, puis compare en ignorant seulement la casse ASCII. Scénario : une ISO ouverte via un lecteur `subst`, un lecteur réseau mappé ou un volume monté dans un dossier est rapportée sous un autre chemin. Il en va de même pour un nom non ASCII dont seule la casse diffère. L'image n'est alors pas reconnue comme montée et le démontage n'est pas proposé.
- `src/platform/windows_disk_images/native.rs:144` (et `:207`) : faible, plausible. `drive_numbers` ouvre `\\.\X:` pour chaque lettre, lecteurs réseau mappés compris, et la boucle de montage le recalcule jusqu'à 20 fois. Scénario : avec un lecteur mappé vers un serveur injoignable, chaque ouverture attend le délai SMB, ce qui allonge fortement le montage et la liste des images.
- `src/platform/windows_drives.rs:433` : faible, plausible. `size as usize - 1` déborde si un fournisseur réseau renvoie une longueur nulle en cas de succès. Scénario : un fournisseur tiers (VirtualBox, NFS) qui met `lpnLength` à 0 provoque une panique dans la tâche de liste des lecteurs.
- `src/platform/windows_drives.rs:425` : faible, plausible. Seul le succès de `WNetGetConnectionW` est exploité. Pour une connexion persistante déconnectée (ERROR_CONNECTION_UNAVAIL), le nom distant fourni est ignoré. Scénario : un lecteur mappé hors ligne apparaît sous le libellé « Local Disk ».
- `src/platform/windows_drives.rs:7` : faible, confirmé. Le libellé reprend le deuxième segment du chemin distant. Scénario : un lecteur WebDAV mappé (`\\serveur@SSL\DavWWWRoot\dossier`) s'affiche « DavWWWRoot ».
- `src/platform/windows_drives.rs:383` : faible, plausible. `GetVolumeInformationW` et `GetDiskFreeSpaceExW` sont appelés séquentiellement pour chaque lettre, y compris distante. Scénario : un lecteur réseau injoignable bloque toute la liste détaillée pendant le délai SMB, sur le thread bloquant ; l'interface n'est pas figée, mais tous les détails des lecteurs arrivent en retard.
- `src/platform/windows_file_drag/data.rs:46` : faible, confirmé. Après un échec de `GlobalLock`, `GlobalFree` est appelé avant `Error::from_thread`, donc l'erreur rapportée est celle de `GlobalFree`. Scénario : diagnostic erroné lors d'un échec de mémoire pendant un glisser.
- `src/platform/windows_file_drag/target.rs:78` (et `:141`) : faible, plausible. `GetData(CF_HDROP)` est appelé sur le thread UI à chaque `DragEnter`, puis de nouveau au `Drop`, au lieu d'un simple `QueryGetData`. Scénario : une source à rendu différé (dossier zip de l'Explorateur, gestionnaire d'archives) extrait les fichiers en temporaire à chaque entrée du curseur dans la fenêtre, ce qui fige Oxyde.
- `src/platform/windows_file_drag/target.rs:194` : faible, confirmé. La cible de GPUI est révoquée avant l'enregistrement de la nouvelle. Si `RegisterDragDrop` échoue, la fenêtre n'a plus aucune cible de dépôt. Scénario : échec d'enregistrement (OLE non initialisé sur ce thread, ressources épuisées), après quoi plus aucun dépôt n'est accepté.
- `src/platform/windows_file_tree.rs:78` : faible, plausible. Seul STATUS_NO_MORE_FILES termine l'énumération. STATUS_NO_SUCH_FILE, renvoyé par la première requête sur un dossier vide par les systèmes de fichiers qui ne synthétisent pas `.` et `..` (certains redirecteurs, WinFsp ou Dokan), devient l'erreur « fichier introuvable ». Scénario : supprimer ou parcourir un dossier vide sur un tel volume échoue.
- `src/platform/windows_copy_staging.rs:129` : faible, plausible. `File::set_permissions` transmet les attributs via `FILE_BASIC_INFO`, où la valeur 0 signifie « inchangé ». Scénario : une sortie dont l'unique attribut est la lecture seule (fichier copié depuis une ISO sans attribut archive) garde cet attribut, sa suppression échoue, et le staging reste en place lors d'une annulation.

### Optimisation

- `src/platform/windows_file_tree.rs:72` : optimisation, plausible. `NtQueryDirectoryFile` est appelé avec `ReturnSingleEntry` dans un tampon de 64 Kio, plus une allocation par nom : un appel système par entrée, et potentiellement un aller-retour réseau par entrée sur SMB. Scénario : la suppression ou l'énumération de dossiers de dizaines de milliers d'entrées, surtout sur un partage réseau.
- `src/platform/windows_file_copy.rs:92` : optimisation, plausible. Chaque fichier copié rouvre toute la chaîne de ses parents (`HeldParents::new`), puis deux `canonicalize` ouvrent de nouveau les chemins. Scénario : une copie de nombreux petits fichiers dans une arborescence profonde multiplie les ouvertures en O(profondeur) par fichier.
- `src/platform/windows_file_tree.rs:169` : optimisation, plausible. `thread::scope` crée `workers` threads système pour chaque élément supprimé, alors que `delete.rs` traite déjà plusieurs éléments en parallèle en mode Turbo. Scénario : supprimer en Turbo une sélection de nombreux dossiers crée et détruit des threads en rafale et surcharge le processeur.

<!-- LOT TERMINÉ -->
