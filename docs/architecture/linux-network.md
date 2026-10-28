# Réseau Linux (SMB et GVfs)

**Statut : implémenté, vérification visuelle en attente** (T10 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md)). Windows ne change pas.

## Principe : comme Nautilus

Nautilus ne parle pas SMB lui-même : GIO demande à GVfs de monter l'emplacement, GVfs
demande les identifiants par une `GMountOperation`, puis le partage est lisible par URI
et, grâce à `gvfsd-fuse`, sous un chemin local
`$XDG_RUNTIME_DIR/gvfs/smb-share:server=…,share=…`. Oxyde fait de même et parcourt ce
chemin avec ses opérations de fichiers ordinaires : copies, corbeille, surveillance et
propriétés ne connaissent pas les URI.

## Couches

- `platform/remote.rs` : types communs (`RemotePrompt`, `RemoteAnswer`,
  `RemoteLocation`, `RemoteError`). `Platform::is_remote_uri` et
  `Platform::connect_remote` ; Windows garde ses défauts (rien n'est une URI distante).
- `platform/linux_gio.rs` : GIO chargé à l'exécution par `libloading`
  (`libgio-2.0.so.0`), sans crate ni lien obligatoire : sans GLib, seul le réseau manque.
  Les appels asynchrones tournent dans un `GMainContext` propre au fil appelant, qui
  répond à `ask-password` en bloquant jusqu'à la réponse du dialogue.
  - `connect` : `g_file_mount_enclosing_volume` (déjà monté : succès), puis
    `g_file_get_path` → `Local(chemin FUSE)`, sinon `Browse(URI)` (serveur, réseau).
    Le dossier FUSE peut arriver après GIO, surtout juste après un démontage du même
    partage : `connect` l'attend 2 s au plus.
  - `children` : énumère une URI montée ; une liste d'un serveur construit l'URI de chaque
    partage depuis le serveur, car `standard::target-uri` perd un port non standard et
    `g_file_get_child` d'un serveur donne `._partage`.
  - `Owned` ne garde jamais un pointeur nul : GLib signale un `g_object_unref(NULL)`.
    `network:///` suit les cibles (serveurs découverts par WS-Discovery, DNS-SD, SMB).
    Remonte d'abord sans jamais demander d'identifiants (actualisation, onglet restauré).
  - `mounts` : dossiers du répertoire FUSE de type réseau (`smb-share`, `sftp`, `ftp`,
    `dav`, `afp-volume`, `nfs`), nommés par `g_mount_get_name` (« partage sur serveur »).
    Téléphones et appareils photo (`mtp`, `gphoto2`) restent hors liste.
  - `unmount` : `g_mount_unmount_with_operation` ; une montée que GVfs ne connaît plus
    (démontée par Nautilus, dossier FUSE encore visible un instant) compte comme démontée.
- `linux_volumes` : les partages GVfs rejoignent la liste des lecteurs (`remote`,
  protocole SMB, SFTP…, URI dans `remote_path`), avec l'action Démonter du menu des
  lecteurs. Les signaux `Mounted`/`Unmounted` du suivi de montages GVfs (bus de session)
  incrémentent la révision des lecteurs : `mountinfo` ne change pas, l'unique montage FUSE
  restant en place. Les montages CIFS du noyau restent listés sans action.

## Interface

- « Réseau » paraît dans la barre latérale (réglage existant) et liste `network:///`.
- La barre d'adresse accepte `smb://serveur/partage` et les autres schémas GVfs.
- `request_tab_listing` connecte avant de lister toute URI distante (`app/remote.rs`) :
  un partage ouvre son chemin local dans le même onglet, sans entrée d'historique pour
  l'URI ; un serveur est listé par URI et retenu pour la session. Fermer le dialogue
  revient à l'emplacement précédent ; un échec affiche « Impossible de se connecter » et
  le détail de GVfs dans une notification.
- Dialogue fixe `RemoteLogin` (`components/remote_login_dialog.rs`), dont les valeurs
  sont gardées à la saisie : un champ qui rappelle Oxyde (Entrée) ne peut être relu
  pendant sa propre mise à jour.
  Il montre : message de GVfs
  sans sa ligne de titre, utilisateur, domaine et mot de passe selon les drapeaux,
  connexion anonyme et mémorisation (trousseau, « Remember forever » de GNOME) quand le
  serveur les permet ; « Identifiants refusés » au second essai.
- Le fil d'Ariane commence au partage, nommé comme dans la barre latérale ; l'onglet
  aussi.

## Limites

- Une connexion en cours ne s'annule pas : naviguer ailleurs ignore son résultat.
- Une seule demande d'identifiants à la fois ; une seconde connexion qui en demande
  pendant ce temps abandonne.
- `ask-question` (empreinte d'un hôte SFTP inconnu) n'a pas de réponse : le montage
  échoue. SMB n'en pose pas.
- Sans `gvfs-fuse`, un partage n'a pas de chemin local : seule la liste reste possible.
- Un onglet restauré sur un partage démonté depuis revient au dossier personnel.

## Essais

- `cargo test linux_gio` : schémas reconnus, montages réseau du dossier FUSE.
- Serveur Samba local, puis le test ignoré (mauvais mot de passe redemandé puis annulé,
  montage, sans nouvelle demande une fois monté, liste des partages, démontage double) :

  ```sh
  podman run -d --name oxyde-smb -p 4450:445 -e USER=oxyde -e PASS=secret \
    -v ~/oxyde-test/smb-share:/storage:Z docker.io/dockurr/samba:latest
  OXYDE_SMB_URI=smb://127.0.0.1:4450/Shared OXYDE_SMB_USER=oxyde OXYDE_SMB_PASSWORD=secret \
    cargo test --offline --lib linux_gio -- --include-ignored --test-threads=1
  ```
- `dbus-monitor --session "type='signal',interface='org.gtk.vfs.MountTracker'"` montre
  `Mounted` et `Unmounted` pour `gio mount`, donc pour Nautilus.
