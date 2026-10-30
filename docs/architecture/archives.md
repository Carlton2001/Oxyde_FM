# Parcours des archives

Sous Windows de production, les codecs sont exécutés dans des workers ; le parent possède les fichiers et décide des effets. Une panne de worker ne doit jamais déclencher de parsing local moins restrictif.

## Parcours et responsabilités

| Usage | Point d’entrée | Contrat |
| --- | --- | --- |
| Navigation et index de recherche | `Platform::list_archive_directory`, worker `oxyde-archive-list-worker` | Métadonnées validées, chemins internes relatifs ; 512 Mio, 120 s, réponse 32 Mio maximum |
| Extraction | `systems/archive_io.rs`, `archive_io::decode` | Courtier Decode ; lectures/écritures bornées et sorties possédées par le parent. ZIP lit sa source via une fenêtre de 256 Kio conservée entre les repositionnements |
| Création et reconstruction | `systems/archive_encode.rs`, `archive_encode::run` | Même encodeur via `Platform::run_archive_io_worker` |
| Modification d’une archive | `archive_workspace/native_commit.rs` | Préparation, contrôles de l’original, publication avec backup et récupération |

Les workers Decode/Encode, listing/index sont en LPAC sans capacité. Le parent valide chemins, types, tailles et séquences ; il retient sources et sorties. Les erreurs typées sont traduites au rendu, sans exposer les codes internes.

Le transport Decode/Encode réveille le superviseur sur réception ou fin d’écriture, avec files de capacité un et un crédit par trame lue. Les helpers peuvent bloquer sur les pipes ; le superviseur garde ses contrôles de pause, annulation et délai. Son attente inactive demande au maximum 5 ms avant revérification, sans garantie absolue sur l’ordonnancement ni les callbacks natifs. Une trame partielle ne bloque pas cette surveillance. Les helpers ne sont jamais joints sans borne à la fermeture.

Les variantes locales sous `cfg(test)` ne servent que les tests unitaires, qui ne prouvent pas le parcours de production (`cfg(not(test))`). `tests/archive_io_pipeline.rs` et `tests/linux_archive_workers.rs` lient la bibliothèque sans `cfg(test)` et exercent les véritables workers de Windows et de Linux.

## Formats et mutations

ZIP, 7z, TAR et TAR.ZST sont créables et modifiables via workspace. Les autres formats navigables restent en lecture/extraction. ISO et IMG ISO9660 sont virtuels en lecture seule ; sous Windows, les images non reconnues et VHD/VHDX passent par le montage natif via `Platform`.

Le montage Windows utilise directement `virtdisk.dll` (`OpenVirtualDisk`,
`AttachVirtualDisk`, `DetachVirtualDisk`). Le suivi des images interroge les
périphériques disque/CD-ROM avec `GetStorageDependencyInformation` et associe
leurs lettres de lecteur par numéro et type de périphérique. Il détecte aussi
les images montées hors d’Oxyde, sans lancer PowerShell. Ces appels restent hors
du thread UI ; le rafraîchissement des lecteurs, le délai d’attribution des
lettres et la protection AutoPlay conservent leur fonctionnement. Les ISO/IMG
sont montés en lecture seule ; les montages persistent après fermeture du handle.

### Compression et Turbo sous Windows

Turbo conserve le niveau de qualité choisi et augmente le parallélisme, en plus de la priorité du processus. La limite CPU du job reste active ; Turbo ne promet pas une occupation de 100 % du processeur.

La création/reconstruction 7z réserve un job de 2 Gio, y compris sans Turbo pour permettre son activation en cours de route. Les autres parcours Decode/Encode gardent 1 Gio. Le budget global reste de 2,5 Gio, avec 512 Mio réservés aux petits workers de navigation ; une autre lecture ciblée d’archive nécessitant 1 Gio peut attendre jusqu’à 30 s, puis être refusée si sa réservation reste indisponible.

| Format | Travail parallèle et limites |
| --- | --- |
| ZIP | Jusqu’à 16 fichiers compressés simultanément selon le CPU, assemblés dans l’ordre sans recompression. Admission estimée à 768 Mio pour sources, résultats et encodeurs ; fichier de plus de 256 Mio traité en flux sur un thread. Un fichier unique n’est pas découpé en DEFLATE parallèle. |
| 7z | LZMA2 multithread, y compris pour les gros fichiers, nombre de threads réduit selon le dictionnaire et une enveloppe estimée de 1,5 Gio pour les encodeurs. Petits fichiers regroupés en blocs solides de 128 Mio maximum, séparés par les gros fichiers. Dictionnaires rapide/normal/meilleur : 1/16/32 Mio. |
| TAR.ZST | Jusqu’à 8 threads Zstandard, 2 au niveau meilleur, fenêtre de 8 Mio. Contrat : nombre fixé au démarrage du flux ; activer ou couper Turbo ensuite ne change que la priorité. |
| TAR | Pas de compression : parcours et écriture séquentiels, priorité Turbo conservée. |

Depuis sevenz-rust2 0.24 (lzma-rust2 0.22), l’encodeur LZMA2 multithread garde au plus un segment par thread en attente : la mémoire ne croît plus avec la taille d’un fichier, et la limite monothread de 256 Mio ne concerne plus que ZIP. Les répertoires sont enregistrés immédiatement, même si des fichiers précédents attendent encore leur bloc solide ; l’ordre interne des entrées 7z peut donc changer. Les estimations d’admission ne remplacent pas la limite dure de mémoire du job. Le parallélisme ZIP et 7z est réévalué aux frontières des fichiers/blocs ; les tâches déjà lancées peuvent terminer leur calcul après une désactivation de Turbo ou pendant une pause, mais les effets restent bloqués par le parent.

Une conversation de contrôle reste active pendant les calculs et finalisations sans I/O. Les codecs et leurs threads restent dans le worker isolé ; le protocole et la publication par le parent sont conservés. Ces changements ciblent le parcours Windows ; les encodeurs locaux Linux restent distincts et ne sont pas validés ici.

Une mutation reconstruit dans le dossier de l’original. Le parent contrôle identité, taille, date, BLAKE3 et provenance avant remplacement. Une modification externe interrompt la publication. Les liens contenus dans l’archive ne sont pas suivis. Pause et annulation restent actives pendant lecture/reconstruction.

Pour un groupe, préparer toutes les archives puis publier en conservant les originaux jusqu’au succès complet ; restaurer le groupe sur échec. **Ce n’est ni une visibilité atomique ni une transaction durable après crash.** Conserver et signaler les backups non restaurables.

Un déplacement retire uniquement les sources effectivement copiées ; les conflits ignorés restent présents. Les mutations d’archives n’ajoutent pas d’Undo/Redo de système de fichiers. Une ouverture externe utilise une copie temporaire, sans réintégration automatique des éditions.

## Garanties à préserver

- Publication exclusive, contrôle des destinations et récupération des effets partiels dans le parent.
- Pas d’adoption ni de purge d’un objet étranger ou d’un cache ancien dépourvu de preuve suffisante.
- Le cache Windows récupérable utilise des preuves persistantes authentifiées ; clé perdue/corrompue et états incomplets restent protégés.
- Le cache Linux n'évince qu'avec un bail d'écriture sur chaque fichier, preuve qu'aucun lecteur ne le tient ouvert : voir [cache Linux](linux-open-cache.md).
- La fidélité indépendante des contenus/métadonnées n’est établie que pour le périmètre TAR non compressé de R9. Les autres formats et résultats de workers n’ont pas cette garantie générale.
- Linux exécute les mêmes codecs et le même protocole dans des [workers confinés par bubblewrap](linux-previews.md#archives) ; le confinement Windows et ses validations ne s’y appliquent pas.

Lire les [limites R1–R9](../security/status.md) avant d’étendre une garantie. Les [bilans historiques](../reports/2026-10-13-security.md) donnent les révisions et preuves ; ils ne valent pas une nouvelle validation.
