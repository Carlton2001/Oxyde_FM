# Audit ARCHIVES-04

Résumé : 9 constats, 5 moyens et 4 faibles. Aucun critique ni élevé. Le confinement des noms tient (traversée, préfixes, flux, périphériques). Le principal coût est fonctionnel : des archives légitimes sont refusées entièrement, surtout sous Linux et pour les TAR courants.

## Fichiers lus en entier

- [x] `src/utils/archive_limits.rs`
- [x] `src/utils/archive_listing_protocol.rs`
- [x] `src/utils/archive_mutation_test.rs`
- [x] `src/utils/archive_password.rs`
- [x] `src/utils/archive_security.rs`
- [x] `src/utils/archive_volumes.rs`
- [x] `src/utils/disk_image.rs`
- [x] `src/utils/tar_fidelity.rs`

## Constats

### 1. `src/utils/archive_security.rs:17`

- Gravité : moyen. Confiance : confirmé.
- Défaut : les règles de nommage Windows s'appliquent aussi sous Linux : noms réservés même avec extension (`aux.c`, `con.h`), caractères `:?*"<>|`, barre oblique inverse, point ou espace final. Un seul nom de ce type fait échouer toute l'opération : le worker de listage refuse l'encodage entier (`archive_listing_protocol.rs:76` et `:120`), `ArchiveBudget::entry` et `tar_fidelity` échouent aussi.
- Scénario : sous Linux, l'archive des sources du noyau contient `drivers/gpu/drm/nouveau/nvkm/subdev/i2c/aux.c`. Elle ne peut être ni parcourue, ni indexée, ni extraite. Même effet avec un fichier `12:30.log` ou `a\b.txt`. L'erreur affichée est le texte anglais brut « Unsafe archive entry path ».

### 2. `src/utils/tar_fidelity.rs:241` (et `:106`)

- Gravité : moyen. Confiance : confirmé.
- Défaut : la vérification indépendante s'exécute avant toute extraction de TAR non compressé (`systems/archive_io.rs:124`). Elle refuse tout TAR qui contient :
  - un en-tête PAX global (`g`) ;
  - un lien symbolique (`2`), un lien physique (`1`) ou un `K` GNU ;
  - toute clé PAX autre que `path` et `size` (`mtime`, `atime`, `ctime`, `linkpath`, `SCHILY.xattr.*`, `LIBARCHIVE.*`, `uname`…).

  Les clés PAX inconnues produisent une erreur `Unsupported` en anglais. Elle n'est pas de type `FidelityFailure`, donc `source_error` ne la convertit pas en échec typé.
- Scénario : l'extraction d'un `.tar` produit par `git archive` (en-tête `g` qui porte le commit), ou par `tar` sur macOS avec attributs étendus, échoue entièrement. Un TAR qui contient un seul lien symbolique échoue de même, alors que les liens ne sont simplement pas suivis ailleurs.

### 3. `src/utils/tar_fidelity.rs:245` (et `:247`)

- Gravité : moyen. Confiance : confirmé.
- Défaut : le nom court de l'en-tête doit être de l'UTF-8 valide, même quand un nom long GNU (`L`) le remplace : `unwrap_or` reçoit une expression déjà évaluée avec `?`. De même, le préfixe ustar est validé alors qu'un `path` PAX le remplace ensuite.
- Scénario : GNU tar (format par défaut `gnu`) coupe le champ nom à 99 octets sans tenir compte des caractères. Un chemin accentué de plus de 100 octets, coupé au milieu d'un caractère multioctet, fait refuser l'extraction de l'archive entière, alors que le nom long `L` est valide.

### 4. `src/utils/archive_security.rs:79`

- Gravité : moyen. Confiance : confirmé.
- Défaut : `ExtractionRoot::new` exige que chaque ancêtre du chemin reçu soit un dossier ordinaire : `symlink_metadata` puis `O_NOFOLLOW` sous Linux, refus des points de reparse sous Windows. Or le cache d'ouverture lui passe un chemin non résolu (`systems/archive_open_cache.rs:142`, base issue de `app_paths::cache_dir()` ou du dossier temporaire).
- Scénario : sous Linux, `~/.cache` est un lien symbolique vers un autre disque (configuration courante). Sous Windows, `%TEMP%` ou le profil a été déplacé par jonction. Toute ouverture d'un fichier depuis une archive échoue avant l'extraction.

### 5. `src/utils/archive_password.rs:66`

- Gravité : moyen. Confiance : plausible.
- Défaut : la clé du magasin de mots de passe ne normalise que le préfixe `\\?\` et la casse sous Windows, et rien sous Linux. Le chemin affiché (mémorisé par la boîte de dialogue) et le chemin résolu (utilisé par les opérations, cf. `path_parents`) diffèrent quand la résolution passe par un lecteur réseau mappé, `subst`, une jonction ou un lien symbolique.
- Scénario : une archive chiffrée sur `Z:` (partage mappé) ou dans un dossier atteint par lien symbolique sous Linux. Le mot de passe est validé puis mémorisé sous `z:\…`, mais l'extraction cherche `\\srv\partage\…`. Le refus `PasswordRequired` revient et la boîte de dialogue se rouvre en boucle.

### 6. `src/utils/archive_listing_protocol.rs:242`

- Gravité : faible. Confiance : confirmé.
- Défaut : une réponse d'erreur du worker (statut 1) est publiée telle quelle dans `CommandError::ArchiveError` : jusqu'à 4096 octets, caractères de contrôle et bidirectionnels compris. Les erreurs d'E/S du même décodage sont pourtant réduites à `WorkerFailed`, sauf si elles sont typées.
- Scénario : un worker compromis par une archive malveillante (dans LPAC ou bubblewrap) affiche dans l'interface un message arbitraire et trompeur. Cas légitime : des erreurs internes anglaises s'affichent non traduites, contrairement à la règle des erreurs typées traduites au rendu.

### 7. `src/utils/archive_limits.rs:222`

- Gravité : faible. Confiance : plausible.
- Défaut : avec `OPEN_FILE`, la limite de flux décodé vaut 256 Mio plus 100 000 × 4 Kio (environ 646 Mio). Or l'ouverture d'un membre d'un TAR compressé décode tout le flux lors d'une première passe complète (`commands/archive_io_codec.rs:541`).
- Scénario : un double clic sur un petit README dans un `.tar.zst` de 2 Gio décompressés échoue sur la limite « Decoded », quelle que soit la taille du fichier choisi.

### 8. `src/utils/archive_volumes.rs:58` (et `:44`)

- Gravité : faible. Confiance : confirmé.
- Défaut : l'ancien nommage n'est reconnu qu'à partir du `.rar`. Un `.r00` n'est ni un volume détecté ni un format d'archive, alors que `docs/architecture/archives.md` affirme qu'un ensemble s'ouvre depuis n'importe lequel de ses volumes. La suite s'arrête aussi à `.r99`, alors que WinRAR continue en `.s00`.
- Scénario : un double clic sur `film.r03` n'ouvre pas l'ensemble. Un ancien ensemble de plus de 101 volumes est coupé et le décodage échoue sur un volume « manquant ».

### 9. `src/utils/archive_password.rs:28`

- Gravité : faible. Confiance : confirmé.
- Défaut : à la désérialisation, un mot de passe refusé par `ArchiveSecret::new` (plus de 1024 octets) est libéré sans effacement. La `String` a été sortie du `Zeroizing` avant l'appel.
- Scénario : une `Operation` dont le mot de passe dépasse la limite laisse ce secret en clair dans le tas libéré du processus, ce qui contredit la garantie « efface sa mémoire ».

<!-- LOT TERMINÉ -->
