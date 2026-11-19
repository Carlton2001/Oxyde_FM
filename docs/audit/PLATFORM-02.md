# PLATFORM-02 : plateforme Linux (éléments retenus, liens, MIME, PDF, média, permissions, portail)

## Fichiers lus en entier

- [x] `src/platform/linux_held.rs`
- [x] `src/platform/linux_hidden_list.rs`
- [x] `src/platform/linux_impl.rs`
- [x] `src/platform/linux_instance.rs`
- [x] `src/platform/linux_library.rs`
- [x] `src/platform/linux_links.rs`
- [x] `src/platform/linux_media.rs`
- [x] `src/platform/linux_media/worker.rs`
- [x] `src/platform/linux_mime.rs`
- [x] `src/platform/linux_mime/magic.rs`
- [x] `src/platform/linux_pdf.rs`
- [x] `src/platform/linux_pdf/library.rs`
- [x] `src/platform/linux_pdf/session.rs`
- [x] `src/platform/linux_permissions.rs`
- [x] `src/platform/linux_permissions/enclosed.rs`
- [x] `src/platform/linux_portal.rs`
- [x] `src/platform/linux_portal/chooser.rs`
- [x] `src/platform/linux_portal/conf.rs`
- [x] `src/platform/linux_portal/forward.rs`
- [x] `src/platform/linux_portal/guard.rs`
- [x] `src/platform/linux_portal/pending.rs`
- [x] `src/platform/linux_preview.rs`

## Constats

### `src/platform/linux_hidden_list.rs:11` : moyen, confirmé
Le fichier `.hidden` est ouvert en lecture, en suivant les liens, avant toute vérification de type. Une FIFO nommée `.hidden`, ou un lien vers une FIFO, bloque donc `open` jusqu'à ce qu'un écrivain se présente, ce qui peut ne jamais arriver.
Scénario : un utilisateur local exécute `mkfifo /tmp/x/.hidden`. Ouvrir `/tmp/x` dans Oxyde, ou le déplier dans la barre latérale, appelle `DirectoryAttributes::load` (`commands/io.rs`, `commands/sidebar.rs`). Le listage reste bloqué pour toujours, sans annulation possible, et chaque nouvelle visite immobilise un fil bloquant de plus.

### `src/platform/linux_impl.rs:34` : faible, confirmé
`open_archive_source` ouvre le chemin en lecture, liens suivis, et ne vérifie qu'ensuite qu'il s'agit d'un fichier ordinaire. Le contrôle arrive trop tard pour une FIFO.
Scénario : une FIFO, ou un lien vers une FIFO, nommée `x.zip` ou `x.7z` est ouverte comme archive ou extraite. Le fil de l'archive reste bloqué dans `open` sans délai ni annulation.

### `src/platform/linux_held.rs:102` : faible, plausible
`open_copy` ouvre le nom en lecture sans en vérifier d'abord le type, et sans `O_NONBLOCK`. `open_file` (l. 89-94) vérifie le type puis ouvre, ce qui laisse une fenêtre de course. La protection annoncée contre les FIFO peut donc être contournée.
Scénario : pendant la vérification d'une copie, un autre processus remplace le nom de destination par une FIFO. Le fil de copie reste bloqué indéfiniment dans `openat`.

### `src/platform/linux_permissions/enclosed.rs:104` : faible, plausible
Le parcours récursif ne compare que `st_dev`. Un montage lié (bind mount) du même système de fichiers est donc parcouru comme un dossier ordinaire, alors que le module annonce ne jamais entrer dans un autre montage.
Scénario : un dossier contient un montage lié vers une autre partie du même volume, ou vers l'un de ses propres ancêtres. « Appliquer aux éléments contenus » change alors les droits hors de l'arborescence choisie. Dans le cas d'un ancêtre, le parcours tourne en boucle jusqu'à épuiser les descripteurs.

### `src/platform/linux_permissions.rs:103` : faible, plausible
Le verrou global du cache des comptes reste tenu pendant toute l'exécution du processus `getent` dans `name`.
Scénario : NSS interroge un annuaire LDAP ou SSSD injoignable. Une fenêtre Propriétés attend l'expiration NSS, et toutes les autres lectures de permissions des autres fenêtres restent bloquées pendant ce temps.

### `src/platform/linux_permissions.rs:33` : optimisation, confirmé
Le nom de chaque groupe de l'utilisateur, puis le propriétaire et le groupe de l'élément, sont résolus par un processus `getent` distinct. Pourtant, `getent` accepte plusieurs clés et `getent()` sait déjà les passer.
Scénario : un utilisateur membre d'une vingtaine de groupes ouvre l'onglet Permissions après l'expiration du cache, qui ne dure que 10 s. Une vingtaine de processus sont lancés à la suite.

### `src/platform/linux_mime.rs:184` : optimisation, confirmé
Pour chaque fichier, `by_name` parcourt linéairement toutes les globs, d'abord pour les noms exacts, puis jusqu'à deux fois pour les suffixes et les motifs. `by_content` recalcule en plus `magic::extent` sur toutes les règles de toutes les sections (l. 227) à chaque appel.
Scénario : le sélecteur de fichiers (`file_chooser/browse.rs`, `content_types`) type chaque fichier d'un dossier qui en contient des milliers. Cela fait des millions de correspondances de motifs, là où une table de littéraux et de suffixes, plus une étendue calculée une seule fois, suffiraient.

### `src/platform/linux_portal.rs:239` : faible, confirmé
Sans configuration système du portail, activer puis désactiver le choix de fichiers laisse dans le dossier de l'utilisateur un fichier qui ne contient qu'une section `[preferred]` vide. Le fichier créé n'a pas l'en-tête « Written by Oxyde », donc `creation` ne le reconnaît pas et ne le supprime pas.
Scénario : sur une session où aucun `*-portals.conf` n'existe, l'utilisateur active puis désactive l'option. Le fichier vide reste et masque toute configuration que le système installera plus tard, puisque le portail lit en premier le dossier de l'utilisateur.

### `src/platform/linux_portal.rs:270` : faible, confirmé
Pour prendre en compte la configuration, Oxyde redémarre `xdg-desktop-portal.service`. Cela se produit à chaque changement de l'option, et au démarrage quand le fichier `.portal` a changé (l. 318). Ce redémarrage met fin aux sessions de portail des autres applications.
Scénario : pendant un partage d'écran Wayland (visio, OBS) ou alors qu'un dialogue de portail est ouvert dans une autre application, l'utilisateur coche l'option, ou lance Oxyde juste après une mise à jour. Le partage ou le dialogue est interrompu.

### `src/platform/linux_portal/chooser.rs:286` : faible, plausible
`forward` appelle `fallback()`, qui lit `portals.conf` et les fichiers `.portal` de façon synchrone sur l'exécuteur du bus. Le module évite pourtant explicitement les lectures disque sur cet exécuteur (`first_folder` passe par `unblock`).
Scénario : avec un dossier personnel sur NFS lent, un appel transmis au portail suivant bloque pendant ce temps les autres appels servis par la connexion, dont FileManager1 et les autres demandes du portail.

### `src/platform/linux_media/worker.rs:114` : faible, plausible
La demande de déplacement (seek) est marquée comme appliquée avant même de savoir si `player.seek` a réussi. Un échec est donc silencieux et ne sera jamais retenté.
Scénario : l'utilisateur déplace le curseur alors que le flux n'accepte pas encore le seek (mise en tampon, flux réseau). La lecture continue à l'ancienne position et la barre de progression revient en arrière, puisque les rapports portent déjà le nouvel identifiant de seek.

### `src/platform/linux_impl.rs:588` : faible, plausible
Les chemins reçus en `String` par `pdf_thumbnail`, `pdf_grid_thumbnail`, `video_grid_thumbnail`, `video_frames`, `decode_video_worker`, `open_item` (l. 825) et `open_terminal` sont passés tels quels à `Path::new` ou à la ligne de commande, sans `path_text::decode`. Les appelants de `content_type` et de `mime_type`, eux, décodent.
Scénario : un fichier PDF ou vidéo dont le nom n'est pas en UTF-8 est représenté avec des échappements NUL. Le lancement du worker ou de `xdg-open` échoue (« nul byte »), et l'aperçu ou l'ouverture n'aboutit pas.

### `src/platform/linux_preview.rs:84` : faible, confirmé
Les modes `thumbnail`, `office` et `office-text` refusent tout chemin qui n'est pas en Unicode, alors que les autres modes du même worker ouvrent le `Path` directement.
Scénario : une image ou un document Office dont le nom contient un octet invalide en UTF-8 (ancienne archive en Latin-1) n'a ni miniature ni aperçu texte, et le message d'erreur est « Preview path is not Unicode ».

<!-- LOT TERMINÉ -->
