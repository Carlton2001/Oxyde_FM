# Volumes

**Statut : pilote en cours, étapes 1 à 3 faites** (trait extrait, inventaire UDisks2,
notifications de montage). Ce document deviendra le contrat du domaine une fois le pilote terminé.

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
nouvelles et `VolumeError` arrivent aux étapes 4 et 5) :

```rust
pub trait Volumes: Send + Sync {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo>; // ex-list_drives
    fn roots(&self) -> Vec<DriveInfo> { self.list(true) }       // ex-list_drive_roots
    fn mount_points(&self) -> Vec<String> { Vec::new() }
    fn watch(&self, revision: Arc<AtomicU64>) -> Result<Option<Box<dyn Send>>, String>; // ex-watch_drive_changes
    fn unmounted(&self) -> Vec<UnmountedVolume> { Vec::new() }
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
  et `IdVersion` d'UDisks2 sous Linux, rien pour un montage FUSE. Il gagnera
  `actions: DriveActions { unmount, eject }` ;
  Windows calcule `eject = is_usb()` et `unmount = false`, soit le menu actuel.
- `UnmountedVolume { id, label, size, removable, fs_type }` forme une liste séparée dans l'état ;
  Windows la renvoie vide.
- `VolumeError` distingue `Cancelled` (authentification annulée : aucun message), `NotAuthorized`,
  `Busy` (fichiers ouverts) et `Other(detail)`.

## Linux : `src/platform/linux_volumes/`

- `udisks.rs` : client `zbus::blocking` vers `org.freedesktop.UDisks2` (`GetManagedObjects`).
  `zbus` 5.19 et `rustix` 1.1 sont déjà verrouillés par GPUI : leur déclaration Linux
  ajoute deux arêtes à `Cargo.lock`, aucun crate ; `blocking-api` active l'API bloquante.
- `inventory.rs` : fonction pure objets UDisks2 → `DriveInfo`, testée sur fixtures, comme
  GVfs : `HintIgnore` masque EFI, récupération et swap ; un volume monté n'apparaît que
  sous `/run/media/$USER/`, `/media/` ou le dossier personnel, sans composant caché, et `/`
  reste en tête (« Root »). `/boot` disparaît donc. `HintSystem` ne sert pas : UDisks2 le
  pose sur tout disque interne, `Data` compris. Optique → `cdrom`, amovible ou USB →
  `removable`. Capacité par `rustix::fs::statvfs` en mode détaillé, sauf montages réseau.
  `media_type` reste vide : le renseigner basculerait la surveillance du sondage vers
  inotify (T11).
- `actions.rs` : `Filesystem.Mount`/`Unmount` avec `auth.no_user_interaction = false` (GNOME
  affiche son dialogue polkit) ; éjection USB = démontage de toutes les partitions du disque
  puis `Drive.PowerOff`, ou `Drive.Eject` pour un support optique.
- `watch.rs` : un fil attend par `poll` le drapeau `POLLPRI` que le noyau lève sur
  `/proc/self/mountinfo` à chaque montage ou démontage de l'espace de noms, qu'il vienne
  d'UDisks2, de `mount` ou d'un client réseau ; chaque changement incrémente la révision.
  La liste se rafraîchit aussitôt, le sondage passe de 2 s à 30 s et ne reste qu'en repli.
  Fermer le tube du garde termine le fil. Fonctionne aussi sans UDisks2.
- `signals.rs` (étape 5) : `InterfacesAdded`/`InterfacesRemoved` d'UDisks2 signaleront les
  volumes branchés mais non montés, que la table des montages ignore.
- `fallback.rs` : sans UDisks2 (WSL, conteneur), tous les montages non virtuels de
  `mountinfo` (`mounts.rs`, échappements décodés), sans montage ni éjection ; avec UDisks2,
  il n'ajoute que les montages réseau.

## Windows

`WindowsVolumes` dans `windows_drives.rs` reçoit les délégations aujourd'hui dans
`windows_impl.rs`, sans changer de logique. Seule modification de construction : le champ
`actions` dans les deux `DriveInfo` de `windows_drives.rs`. Les appels communs
(`drive_watchers`, `network`, `disk_images`, `trash_settings`, `commands/system`, `utils`,
`file_ops/copy`) passent de `platform().list_drives(…)` à `platform().volumes().list(…)`.

## Interface utilisateur

- Linux nomme un lecteur par son seul libellé (« Root », « Data »), comme Nautilus et
  Dolphin, dans la barre latérale, les propriétés et la liste de la corbeille ;
  l'infobulle et la ligne « Emplacement » des propriétés donnent le point de montage.
  Windows garde « Nom (C:) ».
- « Propriétés système » n'existe que sous Windows : Linux n'a pas de dialogue équivalent.
- Partitions non montées dans la section des lecteurs, après les montées, icône atténuée.
  Clic gauche : montage en arrière-plan, puis navigation vers le chemin obtenu. Curseur inchangé.
- Menu : « Démonter » si `actions.unmount`, « Éjecter » si `actions.eject`. `eject_usb_drive`
  devient une libération commune : les surveillances sont relâchées avant l'appel, sinon
  Linux répond `Busy`.
- Nouvelles clés dans les cinq catalogues : `drive-unmount`, `drive-unmount-failed`,
  `drive-mount-failed` (avec le cas d'un NTFS Windows en veille prolongée).

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
4. **Démonter et éjecter** avec authentification ; champ `actions` côté Windows.
5. **Volumes non montés** et montage au clic, avec les signaux UDisks2.
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
