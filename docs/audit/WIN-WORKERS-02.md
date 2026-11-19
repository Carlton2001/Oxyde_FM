# Audit WIN-WORKERS-02

Résumé : 0 critique, 0 élevé, 6 moyens, 9 faibles, 2 optimisations. Les risques principaux sont les suivants : les workers LPAC refusent tout chemin contenant un point de reparse ou un volume sans ACL, sans repli. Les sessions persistantes saturent le budget de 4 workers. Les requêtes mises en file ne peuvent pas être annulées.

## Fichiers lus en entier

- [x] `src/platform/windows_preview_quota.rs`
- [x] `src/platform/windows_preview_worker.rs`
- [x] `src/platform/windows_preview_worker/icon_session.rs`
- [x] `src/platform/windows_preview_worker/image_session.rs`
- [x] `src/platform/windows_thumbnail_bitmap.rs`
- [x] `src/platform/windows_worker_budget.rs`
- [x] `src/platform/windows_worker_sandbox.rs`

## Constats

### Moyens

- `src/platform/windows_worker_budget.rs:12` · moyen · plausible
  - **Défaut** : `MAX_WORKERS` vaut 4, mais cinq sessions persistantes réservent chacune un emplacement tant qu'elles sont inactives (de 15 à 60 s). Ce sont les sessions aperçu image, Viewer, pochette audio, icônes Shell et PDF.
  - **Scénario** : dans un dossier qui mêle images, PDF et MP3, quatre sessions restent vivantes. Un aperçu Office, une liste d'archive, une vidéo ou des métadonnées audio attendent alors jusqu'à l'échéance, puis échouent avec « Global worker budget is busy » ou un dépassement de délai.

- `src/platform/windows_worker_sandbox.rs:97` · moyen · confirmé
  - **Défaut** : `retain_parents` refuse tout ancêtre en point de reparse, pour l'exécutable comme pour le dossier temporaire (ligne 107). Il n'existe aucun repli : tous les workers LPAC échouent.
  - **Scénario** : Oxyde est installé par Scoop (jonction `current`), ou le profil, `%TEMP%` ou le dossier portable sont déplacés par jonction ou lien symbolique. Les aperçus d'images, les pochettes, Office et les métadonnées d'archives sont alors tous indisponibles.

- `src/platform/windows_worker_sandbox.rs:153` · moyen · plausible
  - **Défaut** : la préparation exige `SetFileSecurityW` (DACL protégée) puis `FileIdInfo` (ligne 201) sur le dossier temporaire. Sur FAT32 ou exFAT, ces appels ne sont pas pris en charge et aucun repli n'est prévu.
  - **Scénario** : en mode portable sur une clé USB exFAT (`data/temp`), ou avec `%TEMP%` sur un disque RAM FAT, chaque worker LPAC échoue.

- `src/platform/windows_worker_sandbox.rs:119` · moyen · confirmé
  - **Défaut** : le dossier `oxyde-archive-lpac-*`, qui contient une copie complète de l'exécutable (jusqu'à 128 Mio), est conservé avec `keep()`. Seul le `Drop` le supprime, et aucun balayage n'a lieu au démarrage (aucune autre occurrence du préfixe).
  - **Scénario** : Oxyde plante, est tué ou la session se ferme pendant qu'une session image est vivante, ou bien un faucheur attend encore à la sortie. Les copies s'accumulent dans `%TEMP%`.

- `src/platform/windows_preview_worker/icon_session.rs:117` · moyen · plausible
  - **Défaut** : chaque requête passe par un `sync_channel(1)` suivi d'un `recv` bloquant, sans annulation ni contrôle d'actualité. Les requêtes en file sont traitées en série, jusqu'à 15 s chacune.
  - **Scénario** : un défilement rapide met en file des icônes déjà hors écran. Les icônes visibles attendent derrière elles, et chaque appelant immobilise un thread d'arrière-plan.

- `src/platform/windows_preview_worker/image_session.rs:153` · moyen · plausible
  - **Défaut** : le même schéma bloquant sans annulation s'applique à la session Viewer, avec un délai de 30 s par requête.
  - **Scénario** : en naviguant vite entre de grandes images dans le Viewer, chaque image intermédiaire est décodée jusqu'au bout avant la dernière. Les résultats périmés retardent l'image affichée.

### Faibles

- `src/platform/windows_preview_quota.rs:274` · faible · plausible
  - **Défaut** : le bail exclusif (partage 0) est maintenu pendant le scan et la publication. Un écrivain concurrent reçoit `WouldBlock`, et l'écriture est abandonnée sans nouvelle tentative.
  - **Scénario** : `thumbnails.rs` et `preview_disk.rs` génèrent en parallèle des miniatures dans `thumbnails`. Une partie n'est jamais mise en cache : un avertissement est journalisé et la miniature est régénérée à la visite suivante.

- `src/platform/windows_preview_worker.rs:33` · faible · plausible
  - **Défaut** : `SetProcessDpiAwarenessContext`, disponible à partir de Windows 10 1703, est importé statiquement dans le binaire partagé. Le repli des lignes 35 à 40 n'est donc jamais atteint sur les builds plus anciennes.
  - **Scénario** : sur Windows 10 LTSB 2015 ou 2016 (10240 ou 14393), le chargeur refuse l'exécutable (point d'entrée introuvable), à moins qu'une autre dépendance n'impose déjà 1703.

- `src/platform/windows_preview_worker.rs:519` · faible · confirmé
  - **Défaut** : dans `supervise_stream`, l'horloge `progress` démarre avant l'admission au budget, qui peut attendre jusqu'à 30 s. Le point de contrôle de démarrage ne vérifie que l'annulation.
  - **Scénario** : si l'admission attend plus de 15 s, le processus vidéo est lancé puis tué immédiatement. L'erreur affichée est alors « Video produced no frame for 15 seconds ».

- `src/platform/windows_preview_worker.rs:180` · faible · confirmé
  - **Défaut** : pour un chemin non Unicode ou plus long que `MAX_INPUT`, le repli PDF lance un worker non LPAC sans prendre `SLOT`, contrairement au commentaire de la ligne 44.
  - **Scénario** : un dossier de PDF aux noms très longs lance plusieurs analyseurs PDF en parallèle. Seul le budget global les limite.

- `src/platform/windows_preview_worker/image_session.rs:129` · faible · confirmé
  - **Défaut** : `release_viewer` utilise `try_send`. Le message `Release` est perdu si le canal (capacité 1) contient déjà une requête.
  - **Scénario** : le Viewer passe à une vidéo pendant qu'une image est en file. L'enveloppe de 1 Gio reste réservée jusqu'à 60 s d'inactivité et bloque l'admission vidéo, qui demande aussi 1 Gio.

- `src/platform/windows_worker_sandbox.rs:258` · faible · plausible
  - **Défaut** : le dossier d'exécution est marqué pour suppression juste après la fermeture de l'exécutable. Avec la sémantique de suppression classique, un fichier dont la suppression est en attente le garde non vide, et la suppression échoue.
  - **Scénario** : l'antivirus analyse encore le `worker.exe` fraîchement écrit au moment du nettoyage. Le dossier `oxyde-archive-lpac-*` reste en place et un avertissement est journalisé.

- `src/platform/windows_worker_sandbox.rs:624` · faible · plausible
  - **Défaut** : l'attribut `ALL_APPLICATION_PACKAGES_POLICY` (LPAC) n'existe pas avant Windows 10 1703, et le code ne prévoit pas de repli.
  - **Scénario** : sur LTSB 2016, `UpdateProcThreadAttribute` échoue et tous les workers d'images, d'Office et d'archives échouent.

- `src/platform/windows_worker_sandbox.rs:45` · faible · plausible
  - **Défaut** : un profil AppContainer est créé à chaque worker. Il n'est supprimé qu'après la préparation de l'exécution (environ 1,4 s), et aucun nettoyage n'a lieu au démarrage.
  - **Scénario** : si Oxyde est tué pendant cette fenêtre, des profils `Oxyde.Archive.<uuid>` restent dans le registre et dans `%LOCALAPPDATA%\Packages`.

- `src/platform/windows_thumbnail_bitmap.rs:129` · faible · plausible
  - **Défaut** : le canal alpha du HBITMAP Shell est transmis tel quel, sans normalisation, alors que le cache JPEG le remplace par une opacité totale.
  - **Scénario** : un fournisseur de miniatures sans alpha significatif (alpha à 0) produit une miniature transparente à la première génération, puis opaque une fois relue depuis le cache.

### Optimisations

- `src/platform/windows_preview_quota.rs:290` · optimisation · confirmé
  - **Défaut** : chaque écriture du cache énumère tout le dossier, jusqu'à 4096 entrées et plus, sous le bail exclusif, même loin du quota. Le coût est en O(N) par écriture, donc en O(N²) pour remplir le cache.
  - **Scénario** : la première visite d'un dossier de milliers d'images refait un scan complet pour chaque miniature écrite.

- `src/platform/windows_worker_sandbox.rs:197` · optimisation · confirmé
  - **Défaut** : chaque démarrage LPAC copie l'exécutable, fait un `sync_all` sur cette copie temporaire, puis relit la source pour la hacher (lignes 226 à 240) au lieu de hacher pendant la copie.
  - **Scénario** : les workers ponctuels (texte Office, métadonnées audio, archives) paient à chaque requête une écriture forcée sur disque et une lecture supplémentaire de dizaines de Mio.

<!-- LOT TERMINÉ -->
