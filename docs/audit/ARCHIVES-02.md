# Audit ARCHIVES-02

Fichiers lus en entier :

- [x] `src/commands/archive_rar.rs`
- [x] `src/commands/archive_seven_open.rs`
- [x] `src/commands/archive_tar_open.rs`
- [x] `src/commands/archive_verify.rs`
- [x] `src/commands/archive_worker.rs`
- [x] `src/systems/archive_encode.rs`
- [x] `src/systems/archive_io.rs`
- [x] `src/systems/archive_move_sources.rs`
- [x] `src/systems/archive_open_cache.rs`
- [x] `src/systems/archive_open_cache/recovery.rs`

Résumé : 11 constats (1 moyen, 9 faibles, 1 optimisation). Rien de critique ni d'élevé. Le confinement (noms, liens, limites, source et destination retenues) tient. Les points faibles se concentrent sur la vérification des mots de passe et sur l'auto-réparation du cache d'ouverture.

Remarques de périmètre : `archive_tar_open.rs` est déclaré `#[cfg(test)]` dans `commands/mod.rs`, et l'extraction ciblée de `archive_seven_open.rs` l'est aussi. Ces parties ne servent pas en production et ne font l'objet d'aucun constat. Aucun constat non plus pour `archive_worker.rs`, `archive_encode.rs` et `archive_move_sources.rs`. Pour `archive_encode.rs`, l'absence de vérification indépendante hors TAR est déjà documentée (R9).

## Constats

### 1. `src/systems/archive_open_cache.rs:392` (avec `archive_open_cache/recovery.rs:205`)
- Gravité : moyen. Confiance : plausible.
- Défaut : une entrée Windows (v4) non terminée est comptée comme « en préparation » chaque fois que la récupération refuse de la reprendre. Or, à chaque bloc, le journal est synchronisé avant l'écriture du contenu, alors que le contenu lui-même ne l'est qu'à la fin du fichier. Après une coupure, le fichier peut donc être plus court que le préfixe journalisé. La récupération le refuse (`reclaim.rs`), et l'entrée reste « en préparation » indéfiniment.
- Scénario : quatre coupures de courant ou écrans bleus pendant des ouvertures de fichiers d'archive laissent quatre entrées de ce type. `pending >= MAX_PENDING_ENTRIES` renvoie alors `Busy` à chaque préparation. Toute ouverture externe depuis une archive échoue après 15 s d'attente, jusqu'à un nettoyage manuel de `%TEMP%\oxyde-archive-open\v2`.

### 2. `src/systems/archive_open_cache.rs:371`
- Gravité : faible. Confiance : confirmé.
- Défaut : un seul élément de la racine du cache dont le nom ne commence pas par `entry-` (ou une entrée `entry-*` qui n'est pas un dossier) fait échouer `make_room` pour tout le cache. Il n'existe aucune voie de réparation automatique.
- Scénario : un outil tiers, un antivirus ou l'utilisateur dépose un fichier dans le dossier du cache, par exemple `Thumbs.db` ou `desktop.ini`. Toutes les ouvertures depuis une archive échouent alors avec `ArchiveCacheIssue::Unavailable`.

### 3. `src/systems/archive_open_cache.rs:516`
- Gravité : faible. Confiance : confirmé.
- Défaut : une copie modifiée par l'utilisateur n'est jamais évincée, même sous pression, et continue de compter dans le quota (512 entrées, 1 Gio). Le chemin Windows (`reclaim.rs`) applique la même règle de contenu.
- Scénario : un utilisateur qui édite régulièrement des documents ouverts depuis des archives finit par remplir le quota. Chaque nouvelle ouverture renvoie ensuite `Full` sans rien proposer, et le cache reste plein tant qu'il n'est pas vidé à la main.

### 4. `src/systems/archive_open_cache.rs:235`
- Gravité : faible. Confiance : plausible.
- Défaut : le verrou inter-processus du cache est tenu pendant le hachage complet de la copie (jusqu'à 256 Mio), puis pendant `make_room`. Sous pression, `make_room` re-hache chaque candidat évincé. Les autres préparations n'attendent ce verrou que 15 s.
- Scénario : sur un disque lent, ouvrir un gros fichier pendant qu'une éviction re-hache plusieurs copies de 256 Mio fait échouer en `Busy` les ouvertures lancées en parallèle, y compris depuis une autre fenêtre ou une autre instance d'Oxyde.

### 5. `src/systems/archive_open_cache/recovery.rs:199`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : chaque bloc de 256 Kio écrit dans le cache déclenche plusieurs opérations coûteuses : copie du bloc, sérialisation JSON de tout l'état, scellement, écriture et `sync_all`. Dans cet état, le bloc à venir (`next`) est un `Vec<u8>` encodé comme tableau de nombres, ce qui gonfle chaque bloc jusqu'à environ 1 Mio.
- Scénario : ouvrir un fichier de 256 Mio depuis une archive coûte environ 1 024 fsync et près de 1 Gio de JSON produit et scellé. L'ouverture est nettement ralentie, surtout sur HDD ou sur un volume chiffré.

### 6. `src/commands/archive_rar.rs:145`
- Gravité : faible. Confiance : confirmé.
- Défaut : la vérification du mot de passe choisit le plus petit membre chiffré, quelle que soit sa position. Dans un RAR solide, tous les membres qui le précèdent sont décodés vers un puits.
- Scénario : un RAR solide chiffré de plusieurs Gio dont le plus petit fichier est à la fin. La vérification dans la boîte de dialogue décode presque toute l'archive et peut dépasser l'échéance du worker (120 s). L'utilisateur reçoit alors une erreur de worker au lieu d'un verdict sur le mot de passe. Choisir le premier membre chiffré d'une archive solide suffirait.

### 7. `src/commands/archive_verify.rs:30` (avec `archive_rar.rs:122`)
- Gravité : faible. Confiance : plausible.
- Défaut : l'échantillon s'arrête à 1 Mio, avant le contrôle CRC ou HMAC du membre. Pour les formats sans vérificateur de clé fiable, un mauvais mot de passe passe donc tant que le décodage des données erronées ne lève pas d'erreur dans le premier Mio. C'est le cas des RAR 1.5 à 4, de ZipCrypto au-delà du test d'un octet, et de 7z en méthode Copy avec AES.
- Scénario : le plus petit membre chiffré d'un RAR 4 en mode stocké fait 5 Mio. Un mauvais mot de passe est accepté, la boîte de dialogue se ferme, puis l'action échoue avec une erreur d'archive générique au lieu de redemander le mot de passe.

### 8. `src/commands/archive_verify.rs:51`
- Gravité : faible. Confiance : plausible.
- Défaut : seul le texte « Invalid checksum » est traduit en mot de passe erroné. Un mot de passe ZipCrypto qui passe le test d'un octet sur un membre Deflate produit surtout une erreur de flux Deflate corrompu, qui est ignorée. Cela contredit `docs/architecture/archives.md` (ligne 64).
- Scénario : environ un mauvais mot de passe sur 256 est accepté par la boîte de dialogue. L'extraction échoue ensuite avec « archive corrompue ».

### 9. `src/commands/archive_verify.rs:65`
- Gravité : faible. Confiance : plausible.
- Défaut : la vérification 7z choisit le plus petit bloc sans s'assurer qu'il est chiffré. Le commentaire suppose que tous les blocs partagent la même clé.
- Scénario : une archive 7z chiffrée à laquelle on a ajouté plus tard des fichiers sans mot de passe contient un petit bloc non chiffré. N'importe quel mot de passe est alors accepté, et l'action échoue ensuite sur les blocs chiffrés.

### 10. `src/commands/archive_rar.rs:39`
- Gravité : faible. Confiance : confirmé.
- Défaut : `failure` ne type que les erreurs de mot de passe et les I/O. Les erreurs `rars` de type `ResourceLimit` (nombre ou taille des en-têtes, sortie par membre ou totale, dictionnaire) deviennent « Invalid RAR archive », au lieu d'un `ArchiveLimitFailure` traduit.
- Scénario : ouvrir un fichier situé après plus de 256 Mio de données dans un RAR solide, ou lister un RAR 7 dont le dictionnaire dépasse 1 Gio. L'utilisateur lit que l'archive est invalide alors qu'une limite volontaire a été atteinte.

### 11. `src/systems/archive_io.rs:171`
- Gravité : faible. Confiance : confirmé.
- Défaut : `Operation::Decode::password` est transmis pour RAR, alors que `docs/architecture/archives.md` (ligne 62) indique « accepted for ZIP and 7z only ». La documentation de sécurité ne reflète donc pas le chemin réel du secret.
- Scénario : une relecture de sécurité fondée sur la documentation sous-estime les opérations qui reçoivent le mot de passe. Il suffit de corriger la documentation.

<!-- LOT TERMINÉ -->
