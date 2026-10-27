# Volumes

**Statut : pilote en cours, étapes 1 à 5 faites** (trait extrait, inventaire UDisks2,
notifications de montage, démontage et éjection, volumes non montés). Ce document deviendra le contrat du domaine une fois le pilote terminé.

Le pilote sort les volumes du trait `Platform` dans un trait de domaine et livre, sous
Linux, l'équivalent de Nautilus : partitions montées et non montées, montage au clic,
démontage et éjection avec authentification, rafraîchissement par signaux. Il mesure le
coût d'une migration progressive de `Platform` par domaine avant de l'appliquer ailleurs. **Windows garde un comportement identique** : son code est
déplacé, pas réécrit.

## Périmètre

| Dans le pilote | Hors pilote |
| --- | --- |
| `list_drives`, `list_drive_roots`, `mount_points`, `watch_drive_changes`, `eject_drive`, `is_ssd`, `physical_disk_id` | Lecteurs réseau (`map_network_drive`, `is_network_path`…) : futur domaine Réseau |
| `mounted_disk_images`, `mount_disk_image`, `unmount_disk_image` (déplacés ; Linux refuse toujours) | `install_disk_image_autoplay_handler` : intégration de fenêtre, reste dans `Platform` |
| Nouveau : volumes non montés, monter, démonter | Politique de corbeille par lecteur (`trash_settings`) : inchangée |

## Interface

`Platform` expose `fn volumes(&self) -> &dyn Volumes`, défini dans `src/platform/volumes.rs`,
implémenté par `windows_volumes.rs` et `linux_volumes.rs`. Interface visée (les méthodes
nouvelles arrivent aux étapes 4 et 5) :

```rust
pub trait Volumes: Send + Sync {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo>; // ex-list_drives
    fn roots(&self) -> Vec<DriveInfo> { self.list(true) }       // ex-list_drive_roots
    fn mount_points(&self) -> Vec<String> { Vec::new() }
    fn watch(&self, revision: Arc<AtomicU64>) -> Result<Option<Box<dyn Send>>, String>; // ex-watch_drive_changes
    fn unmounted(&self) -> Vec<UnmountedVolume> { Vec::new() }
    fn actions(&self, drive: &DriveInfo) -> DriveActions;         // pur, sans appel système
    fn mount(&self, id: &str) -> Result<String, VolumeError>;     // chemin monté
    fn unmount(&self, path: &str) -> Result<(), VolumeError>;
    fn eject(&self, path: &str) -> Result<(), VolumeError>;       // ex-eject_drive
    fn is_ssd(&self, path: &Path) -> bool;
    fn physical_disk_id(&self, path: &Path) -> u64;
    // + les trois méthodes d'images disque, inchangées
}
```

- `DriveInfo` reste identifié par son chemin. `file_system` donne le format affiché dans les
  Propriétés (NTFS, FAT32, exFAT, ext4…) : `GetVolumeInformationW` sous Windows, `IdType`
  et `IdVersion` d'UDisks2 sous Linux, rien pour un montage FUSE.
- `actions(drive) -> DriveActions { unmount, eject }` décide du menu à partir du lecteur
  listé, sans champ de plus dans `DriveInfo` : l'implémentation par défaut, celle de
  Windows, donne `eject = is_usb()` et `unmount = false`, soit le menu actuel, y compris
  quand le type USB n'arrive qu'avec les détails matériels. Linux suit la barre latérale
  GTK : un lecteur amovible ou optique s'éjecte, un autre volume se démonte ; ni `/`, ni
  les montages réseau, ni le repli sans UDisks2 n'offrent d'action.
- `UnmountedVolume { id, label, size, removable, file_system }` forme une liste séparée dans
  l'état (`unmounted_volumes`) ; Windows la renvoie vide. Sous Linux, `id` est l'objet bloc
  UDisks2 ; `mount` refuse tout autre chemin D-Bus.
- `VolumeError` distingue `Cancelled` (authentification annulée : aucun message), `NotAuthorized`,
  `Busy(applications)` (fichiers ouverts), `WindowsHibernated` et `Other(detail)`.

## Linux : `src/platform/linux_volumes/`

- `udisks.rs` : client `zbus::blocking` vers `org.freedesktop.UDisks2` (`GetManagedObjects`).
  `zbus` 5.19 et `rustix` 1.1 sont déjà verrouillés par GPUI : leur déclaration Linux
  ajoute deux arêtes à `Cargo.lock`, aucun crate ; `blocking-api` active l'API bloquante.
- `inventory.rs` : fonction pure objets UDisks2 → `DriveInfo`, testée sur fixtures, comme
  GVfs : `HintIgnore` masque EFI, récupération et swap ; un volume monté n'apparaît que
  sous `/run/media/$USER/`, `/media/` ou le dossier personnel, sans composant caché, et `/`
  reste en tête (« Root »), suivi des disques internes, puis amovibles, puis optiques,
  chacun par nom (tri naturel) ; les volumes non montés suivent le même ordre, internes
  d'abord. `/boot` disparaît donc. `HintSystem` ne sert pas : UDisks2 le
  pose sur tout disque interne, `Data` compris. Optique → `cdrom`, amovible ou USB →
  `removable`. Capacité par `rustix::fs::statvfs` en mode détaillé, sauf montages réseau.
  `media_type` reste vide : le renseigner basculerait la surveillance du sondage vers
  inotify (T11).
- `actions.rs` : `Filesystem.Unmount` avec des options vides, donc
  `auth.no_user_interaction = false` : polkit fait afficher son dialogue par GNOME s'il le
  faut, et l'appel bloque le fil de fond jusqu'à la réponse. Éjection = démontage de toutes
  les partitions montées du disque puis `Drive.Eject` pour un support optique,
  `Drive.PowerOff` sinon quand le disque le permet (retrait sûr, comme sous Windows),
  `Drive.Eject` à défaut. Les erreurs UDisks2 `NotAuthorizedDismissed` → `Cancelled`,
  `NotAuthorized*` → `NotAuthorized`, `DeviceBusy` → `Busy`. `Mount` arrive à l'étape 5.
- `holders.rs` : sur `Busy`, comme le dialogue « Volume occupé » de GNOME, parcourt `/proc`
  (dossier courant, racine, exécutable, descripteurs, projections) et nomme les processus
  de l'utilisateur qui gardent le volume ouvert ; le message ajoute « Ouvert dans :
  nautilus. ». Nautilus n'est prévenu que des démontages lancés par GNOME : sinon, il
  garde ses dossiers ouverts.
- `watch.rs` : un fil attend par `poll` le drapeau `POLLPRI` que le noyau lève sur
  `/proc/self/mountinfo` à chaque montage ou démontage de l'espace de noms, qu'il vienne
  d'UDisks2, de `mount` ou d'un client réseau ; chaque changement incrémente la révision.
  La liste se rafraîchit aussitôt, le sondage passe de 2 s à 30 s et ne reste qu'en repli.
  Fermer le tube du garde termine le fil. Fonctionne aussi sans UDisks2.
- `signals.rs` : `InterfacesAdded`/`InterfacesRemoved` d'UDisks2, et ses
  `PropertiesChanged` sur `Filesystem` ou `Block` (points de montage, libellés), incrémentent
  la même révision : les volumes branchés sans être montés, que la table des montages
  ignore, et l'état qu'UDisks2 enregistre après le noyau. Progression des tâches et
  mesures SMART sont ignorées.
  Le fil a sa propre connexion ; un itérateur bloquant ne se réveille pas, il s'arrête donc
  au signal suivant la libération du garde.
- `inventory::unmounted` : systèmes de fichiers (`IdUsage = filesystem`) sans point de
  montage, comme le moniteur de volumes GVfs : `HintIgnore` les masque, un disque entier
  partitionné cède la place à ses partitions, une entrée `/etc/fstab` hors des lieux de
  l'utilisateur reste masquée sauf `x-gvfs-show` (`x-gvfs-hide` masque toujours). Libellé
  vide sans étiquette de système de fichiers. `Filesystem.Mount` sans
  option renvoie le point de montage ; ntfs-3g signale un Windows en veille prolongée ou
  arrêté en démarrage rapide (`WindowsHibernated`).
- `fallback.rs` : sans UDisks2 (WSL, conteneur), tous les montages non virtuels de
  `mountinfo` (`mounts.rs`, échappements décodés), sans montage ni éjection ; avec UDisks2,
  il n'ajoute que les montages réseau.

## Windows

`WindowsVolumes` dans `windows_drives.rs` reçoit les délégations aujourd'hui dans
`windows_impl.rs`, sans changer de logique ; `eject` enveloppe son erreur dans
`VolumeError::Other`. Les appels communs
(`drive_watchers`, `network`, `disk_images`, `trash_settings`, `commands/system`, `utils`,
`file_ops/copy`) passent de `platform().list_drives(…)` à `platform().volumes().list(…)`.

## Interface utilisateur

- Linux nomme un lecteur par son seul libellé (« Root », « Data »), comme Nautilus et
  Dolphin, dans la barre latérale, les onglets, les propriétés et la liste de la
  corbeille (`format::drive_name`) ; l'infobulle et la ligne « Emplacement » des propriétés
  donnent le point de montage. Sans étiquette, le volume, monté ou non, s'appelle
  « Volume de 487 Go » d'après la taille de sa partition (`DriveInfo::volume_bytes`, clé
  `unlabeled-volume`), comme Nautilus, plutôt que par l'UUID qui nomme son dossier de
  montage ; les volumes nommés passent avant. Windows garde « Nom (C:) ».
- « Propriétés système » n'existe que sous Windows : Linux n'a pas de dialogue équivalent.
- Volumes non montés (`SidebarItem::Volume`, `sidebar/volume_row.rs`) dans la section des
  lecteurs, après les montés, icône et libellé atténués, infobulle « format · taille ».
  Clic gauche : montage en arrière-plan (`mount_volume`), puis navigation vers le chemin
  obtenu. Curseur inchangé, pas de menu contextuel, absents du rail replié.
- Menu : « Démonter » (icône `eject`) si `actions.unmount`, « Éjecter » (`unplug`) si
  `actions.eject`. `release_drive` remplace `eject_usb_drive` pour les deux : les
  surveillances sont relâchées avant l'appel, sinon Linux répond `Busy` ; une
  authentification annulée n'affiche rien.
- Clés `drive-unmount`, `drive-unmount-failed`, `drive-holders`, `drive-mount-failed` et
  `drive-mount-hibernated` dans les cinq catalogues.

## Étapes

Chaque étape passe `check-linux.sh`, `check-windows.sh` et ses tests ; les étapes 1 et 4,
qui touchent Windows, passent aussi la suite Windows, sous Windows ou par déclenchement
manuel de `tests.yml` sur la branche (elle n'est pas fusionnée dans `main`).

1. **Extraire le trait**, déplacements seuls, Linux inchangé. *Fait* : 7 fichiers communs,
   uniquement les appels ; suite Windows encore à passer.
2. **Inventaire UDisks2** et repli. *Fait* : aucun fichier commun touché ; Root, `Data` et
   la clé avec leurs capacités, `/boot` et `/boot/efi` masqués.
3. **Notifications** à la place du sondage. *Fait* par la table des montages : elle couvre
   tout ce que la liste affiche ; les signaux UDisks2 rejoignent l'étape 5, la seule à
   montrer des volumes non montés.
4. **Démonter et éjecter** avec authentification. *Fait* : fichiers communs touchés
   `volumes.rs`, `windows_volumes.rs` (type d'erreur), `drive_watchers.rs`, le menu des
   lecteurs et les catalogues ; suite Windows à passer.
5. **Volumes non montés** et montage au clic, avec les signaux UDisks2. *Fait* : fichiers
   communs touchés `volumes.rs` (types), l'état et la barre latérale (`workspace`,
   `sidebar`), `drive_watchers.rs` et les catalogues ; Windows n'en liste aucun.
6. *Facultatif* : `is_ssd` (`/sys/block/*/queue/rotational`) et `physical_disk_id` Linux,
   utilisés par le parallélisme des copies.
7. **Bilan** : fichiers communs touchés par étape, décision sur le domaine suivant.

**Critère d'arrêt :** si l'étape 1 déborde des appels listés, ou si les étapes 2 à 5 doivent
retoucher du code commun au-delà de l'interface utilisateur prévue, le pilote s'arrête et
la suite du portage reste dans la structure actuelle.

## Décisions

- Partitions affichées : celles que montre Nautilus, selon les indications UDisks2, y compris
  pour les partitions Windows non marquées `HintSystem`.
- `/` garde le nom « Root », comme dans Dolphin. Les usages Linux priment sur ceux de Windows.
