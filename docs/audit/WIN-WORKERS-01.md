# Audit WIN-WORKERS-01

Résumé : 17 constats. 0 critique, 2 élevés, 7 moyens, 7 faibles, 1 optimisation. Le risque principal est la lecture de la provenance (flux `Zone.Identifier`) : elle ne tolère que `NotFound`. Sur FAT/exFAT et sur certains partages sans flux alternatifs, l'extraction est donc probablement bloquée.

## Fichiers lus en entier

- [x] `src/platform/windows_archive_io_worker.rs`
- [x] `src/platform/windows_archive_io_worker/timing.rs`
- [x] `src/platform/windows_archive_io_worker/transport.rs`
- [x] `src/platform/windows_archive_security.rs`
- [x] `src/platform/windows_archive_source.rs`
- [x] `src/platform/windows_archive_worker.rs`
- [x] `src/platform/windows_broker_cancellation.rs`
- [x] `src/platform/windows_cache.rs`
- [x] `src/platform/windows_cache_proof.rs`
- [x] `src/platform/windows_cache_proof/key.rs`
- [x] `src/platform/windows_download.rs`
- [x] `src/platform/windows_icon_bitmap.rs`
- [x] `src/platform/windows_icon_cache.rs`
- [x] `src/platform/windows_icon_persistence.rs`
- [x] `src/platform/windows_icon_pixels.rs`
- [x] `src/platform/windows_icon_source.rs`
- [x] `src/platform/windows_office_search.rs`
- [x] `src/platform/windows_pdf_session.rs`
- [x] `src/platform/windows_pdf_session/previews.rs`
- [x] `src/platform/windows_pdf_thumbnail.rs`
- [x] `src/platform/windows_preview_cache.rs`

## Constats

### 1. `src/platform/windows_archive_security.rs:90`
- Gravité : élevé. Confiance : plausible.
- Défaut : `read_provenance` ne traite comme absence de provenance que `NotFound`. Or FAT, exFAT et les partages SMB sans flux alternatifs renvoient `ERROR_INVALID_NAME` (123) à l'ouverture de `fichier:Zone.Identifier`, et ce code est propagé comme une erreur.
- Scénario : extraire une archive située sur une clé USB FAT32/exFAT (`systems/file_ops/extract.rs:137`, `commands/archive.rs:126`, `archive_seven_open`, `archive_tar_open`, `archive_iso_open`). La lecture de provenance échoue avant toute extraction, et l'opération est refusée.

### 2. `src/platform/windows_archive_security.rs:191`
- Gravité : élevé. Confiance : plausible.
- Défaut : même filtrage limité à `NotFound` dans `read_output_provenance`. `open_name` traduit le NTSTATUS par `RtlNtStatusToDosError` : `STATUS_OBJECT_NAME_INVALID` devient 123, et ce code n'est pas toléré.
- Scénario : `systems/archive_io.rs:147` appelle cette lecture à chaque décodage. Sur FAT/exFAT, toute extraction par le worker d'archives échoue, ainsi que l'ouverture d'un espace de travail d'archive (`archive_workspace.rs:169`).

### 3. `src/platform/windows_archive_security.rs:125` et `:164`
- Gravité : moyen. Confiance : plausible.
- Défaut : l'écriture de la provenance sur la sortie (`write_provenance`, `write_output_provenance`) est obligatoire. Aucun repli n'existe quand le volume de destination ne gère pas les flux alternatifs.
- Scénario : extraire une archive téléchargée (marquée Internet) vers une clé FAT32 ou un partage sans flux. Les fichiers sont écrits, puis l'ouverture du flux échoue et l'opération est signalée en erreur.

### 4. `src/platform/windows_pdf_session.rs:176`
- Gravité : moyen. Confiance : confirmé.
- Défaut : après un dépassement de délai dans `reply`, la session reste ouverte et le worker continue à rendre. La réponse tardive reste dans le pipe, et la page suivante lit la réponse de la page précédente. Si le délai expire au milieu d'une trame, le flux est désynchronisé pour la suite de la session.
- Scénario : une page lourde dépasse 15 s. Le Viewer (`ui/app/viewer/pdf.rs:167`) demande ensuite la page M et affiche l'image de la page N. Toutes les pages suivantes sont décalées d'une position ou en erreur, et le worker garde sa réserve de 1 Gio.

### 5. `src/platform/windows_pdf_thumbnail.rs:107`
- Gravité : moyen. Confiance : confirmé.
- Défaut : après `canonicalize`, le code retire les 4 premiers caractères de `\\?\`. Pour un chemin UNC, `\\?\UNC\serveur\partage\x.pdf` devient `UNC\serveur\partage\x.pdf`, un chemin invalide pour `StorageFile::GetFileFromPathAsync`.
- Scénario : un PDF sur un partage réseau ou sur un lecteur réseau mappé (que `canonicalize` résout en UNC), ou sous `\\wsl.localhost`. La miniature, l'aperçu et la session du Viewer échouent toujours. Le chemin rendu peut aussi dépasser `MAX_PATH` une fois le préfixe retiré.

### 6. `src/platform/windows_archive_worker.rs:19`
- Gravité : moyen. Confiance : confirmé.
- Défaut : la liste de navigation prend `SLOT` par un `lock()` bloquant et appelle `confined` avec un checkpoint toujours `Ok`. Ni l'attente ni le worker ne peuvent être annulés. `verify` (ligne 107) partage le même slot.
- Scénario : l'utilisateur ouvre une grosse archive lente puis navigue ailleurs. Le worker continue jusqu'à 120 s, et toute autre ouverture d'archive ou vérification de mot de passe attend derrière lui. Le résultat périmé est calculé pour rien.

### 7. `src/platform/windows_cache_proof/key.rs:113`
- Gravité : moyen. Confiance : confirmé.
- Défaut : le fichier de clé est créé de façon exclusive avant d'être sécurisé et rempli. Une erreur ou un arrêt entre la création et `sync_all` (ACL, `BCryptGenRandom`, DPAPI, crash) laisse un fichier vide ou mal protégé, et aucun chemin ne le supprime ni ne le régénère. Le même blocage survient si DPAPI ne peut plus déchiffrer la clé (mot de passe réinitialisé par un administrateur).
- Scénario : Oxyde est tué pendant la première création de la clé. Ensuite, chaque `cache_proof_key` (`archive_open_cache/recovery.rs:86`, `reclaim.rs:45`) échoue de façon permanente. Deux premiers appels concurrents produisent aussi une violation de partage transitoire chez le second appelant.

### 8. `src/platform/windows_cache_proof/key.rs:106`
- Gravité : moyen. Confiance : plausible.
- Défaut : la clé est ouverte par `windows_no_reparse::open`, qui refuse tout point de reparse parmi les ancêtres. Un test (`signing_key_lookup_refuses_an_ancestor_junction`) le vérifie.
- Scénario : un profil ou `%LOCALAPPDATA%` déplacé par jonction (par exemple `C:\Users\nom` vers `D:\nom`), ou un volume monté dans un dossier. Le cache d'ouverture d'archives devient indisponible en permanence, sans cause visible pour l'utilisateur.

### 9. `src/platform/windows_icon_bitmap.rs:245`
- Gravité : moyen. Confiance : plausible.
- Défaut : l'alpha est repris tel quel du DIB 32 bpp mis à zéro. Or `DrawIconEx` n'écrit pas de canal alpha pour les icônes héritées sans alpha (monochromes, 4/8/24 bpp avec masque AND). Ces pixels restent donc transparents. Le test de la ligne 317 ne vérifie que le RVB.
- Scénario : un ancien `.exe` ou `.ico` sans alpha donne une icône entièrement transparente. `icons.rs:171` ne trouve aucun contenu au-dessus du seuil, et l'icône s'affiche vide.

### 10. `src/platform/windows_archive_io_worker.rs:284`
- Gravité : faible. Confiance : plausible.
- Défaut : quand l'accusé `Paused` arrive plus d'une seconde après la demande, `paused_at` n'est pas renseigné. Toute la durée de la pause est alors comptée dans le délai de l'opération.
- Scénario : machine chargée ou pipe saturé au moment de la pause. L'utilisateur laisse l'opération en pause, et elle échoue au retour avec une limite de durée (`Seconds`) qu'elle n'a pas réellement consommée.

### 11. `src/platform/windows_archive_io_worker.rs:232`
- Gravité : faible. Confiance : plausible.
- Défaut : après `Finished`, le worker doit fermer stdout et se terminer en moins d'une seconde, sinon toute l'opération est déclarée invalide alors que les données sont complètes.
- Scénario : machine saturée, ou fin d'un worker 7z qui libère 2 Gio. Une compression terminée est signalée comme un échec de traitement d'archive.

### 12. `src/platform/windows_archive_io_worker.rs:85`
- Gravité : faible. Confiance : plausible.
- Défaut : un slot d'admission empoisonné renvoie `invalid()` pour toujours. Le profil n'est pas `panic = abort`, et le handler applicatif s'exécute avec le garde tenu. Même schéma pour `windows_archive_worker.rs:19`/`:77` et `windows_office_search.rs:26`.
- Scénario : une panique dans le handler, interceptée par `spawn_blocking`, empoisonne `BULK` ou `TARGETED`. Toutes les opérations d'archive de cette catégorie échouent jusqu'au redémarrage.

### 13. `src/platform/windows_icon_pixels.rs:96`
- Gravité : faible. Confiance : confirmé.
- Défaut : la clé du PNG persistant (`icons::cache_key`) ne dépend que du chemin et de la taille, sans date ni identité de fichier. Seul un vidage manuel du cache l'invalide.
- Scénario : un `.exe`, `.lnk` ou `.ico` est remplacé ou mis à jour au même chemin (nouvelle version d'un installeur dans Téléchargements). L'ancienne icône reste affichée, même après redémarrage.

### 14. `src/platform/windows_preview_cache.rs:11`
- Gravité : faible. Confiance : confirmé.
- Défaut : `ensure_parents` refuse tout ancêtre portant l'attribut de reparse, jusqu'à la racine du chemin du cache.
- Scénario : avec un profil déplacé par jonction ou symlink, ou un volume monté dans un dossier, toute persistance d'aperçus et d'icônes échoue silencieusement. Chaque démarrage relance donc les workers Shell.

### 15. `src/platform/windows_pdf_session.rs:42`
- Gravité : faible. Confiance : confirmé.
- Défaut : un chemin non Unicode (UTF-16 avec surrogate isolé, possible sur NTFS) fait sortir le worker par `?` avant toute trame. `windows_pdf_thumbnail::open` (ligne 106) convertit aussi le chemin avec `to_string_lossy`, ce qui donne un autre nom.
- Scénario : un PDF avec un nom non Unicode valide. Le parent ne reçoit qu'un « worker exited » générique au lieu d'une erreur explicite, et la miniature échoue.

### 16. `src/platform/windows_pdf_thumbnail.rs:130`
- Gravité : faible. Confiance : plausible.
- Défaut : `sizes_of` appelle `GetPage` et `Size` pour chaque page, jusqu'à 100 000, avant la première réponse. L'ouverture de session (`windows_pdf_session.rs:93`) n'attend que 15 s.
- Scénario : un PDF de plusieurs milliers de pages sur un disque lent ou un partage. L'ouverture dépasse le délai et le Viewer refuse le document.

### 17. `src/platform/windows_icon_pixels.rs:84`
- Gravité : optimisation. Confiance : plausible.
- Défaut : chaque accès réussi au cache clone l'image (jusqu'à 256 Kio) sous le mutex global. `windows_icon_cache.rs:42` fait de même.
- Scénario : un défilement rapide d'un dossier d'exécutables multiplie les copies et sérialise les chargements d'icônes concurrents sur ce verrou.

<!-- LOT TERMINÉ -->
