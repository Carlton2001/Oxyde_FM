# Parcours des archives

Sous Windows de production, les codecs sont exécutés dans des workers ; le parent possède les fichiers et décide des effets. Une panne de worker ne doit jamais déclencher de parsing local moins restrictif.

## Parcours et responsabilités

| Usage | Point d’entrée | Contrat |
| --- | --- | --- |
| Navigation et index de recherche | `Platform::list_archive_directory`, worker `oxyde-archive-list-worker` | Métadonnées validées, chemins internes relatifs ; 512 Mio, 120 s, réponse 32 Mio maximum |
| Extraction | `systems/archive_io.rs`, `archive_io::decode` | Courtier Decode ; lectures/écritures bornées et sorties possédées par le parent |
| Création et reconstruction | `systems/archive_encode.rs`, `archive_encode::run` | Même encodeur via `Platform::run_archive_io_worker` |
| Modification d’une archive | `archive_workspace/native_commit.rs` | Préparation, contrôles de l’original, publication avec backup et récupération |

Les workers Decode/Encode, listing/index sont en LPAC sans capacité. Le parent valide chemins, types, tailles et séquences ; il retient sources et sorties. Les erreurs typées sont traduites au rendu, sans exposer les codes internes.

Les variantes locales sous `cfg(any(not(target_os = "windows"), test))` servent Linux et les tests unitaires. Ceux-ci ne prouvent pas le parcours Windows de production (`cfg(all(target_os = "windows", not(test)))`). `tests/archive_io_pipeline.rs` lie la bibliothèque sans `cfg(test)` et exerce les véritables workers Windows.

## Formats et mutations

ZIP, 7z, TAR et TAR.ZST sont créables et modifiables via workspace. Les autres formats navigables restent en lecture/extraction. ISO et IMG ISO9660 sont virtuels en lecture seule ; sous Windows, les images non reconnues et VHD/VHDX passent par le montage natif via `Platform`.

Une mutation reconstruit dans le dossier de l’original. Le parent contrôle identité, taille, date, BLAKE3 et provenance avant remplacement. Une modification externe interrompt la publication. Les liens contenus dans l’archive ne sont pas suivis. Pause et annulation restent actives pendant lecture/reconstruction.

Pour un groupe, préparer toutes les archives puis publier en conservant les originaux jusqu’au succès complet ; restaurer le groupe sur échec. **Ce n’est ni une visibilité atomique ni une transaction durable après crash.** Conserver et signaler les backups non restaurables.

Un déplacement retire uniquement les sources effectivement copiées ; les conflits ignorés restent présents. Les mutations d’archives n’ajoutent pas d’Undo/Redo de système de fichiers. Une ouverture externe utilise une copie temporaire, sans réintégration automatique des éditions.

## Garanties à préserver

- Publication exclusive, contrôle des destinations et récupération des effets partiels dans le parent.
- Pas d’adoption ni de purge d’un objet étranger ou d’un cache ancien dépourvu de preuve suffisante.
- Le cache Windows récupérable utilise des preuves persistantes authentifiées ; clé perdue/corrompue et états incomplets restent protégés.
- La fidélité indépendante des contenus/métadonnées n’est établie que pour le périmètre TAR non compressé de R9. Les autres formats et résultats de workers n’ont pas cette garantie générale.
- Linux garde les codecs locaux ; le confinement Windows et ses validations ne s’y appliquent pas.

Lire les [limites R1–R9](../security/status.md) avant d’étendre une garantie. Les [bilans historiques](../reports/2026-10-13-security.md) donnent les révisions et preuves ; ils ne valent pas une nouvelle validation.
