# Volumes

**Statut : pilote en cours, étape 1 faite** (trait extrait, comportement inchangé). Ce
document deviendra le contrat du domaine une fois le pilote terminé.

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

- `DriveInfo` reste identifié par son chemin. Il gagne `actions: DriveActions { unmount, eject }` ;
  Windows calcule `eject = is_usb()` et `unmount = false`, soit le menu actuel.
- `UnmountedVolume { id, label, size, removable, fs_type }` forme une liste séparée dans l'état ;
  Windows la renvoie vide.
- `VolumeError` distingue `Cancelled` (authentification annulée : aucun message), `NotAuthorized`,
  `Busy` (fichiers ouverts) et `Other(detail)`.

## Linux : `src/platform/linux_volumes/`

- `udisks.rs` : client `zbus::blocking` vers `org.freedesktop.UDisks2` (`GetManagedObjects`).
  `zbus` 5.19 est déjà verrouillé par GPUI avec son API bloquante : la déclaration Linux
  ajoute une arête à `Cargo.lock`, aucun crate. À vérifier par `cargo tree` avant/après.
- `inventory.rs` : fonction pure objets UDisks2 → `DriveInfo`/`UnmountedVolume`, testée sur
  fixtures. Suit les indications UDisks2 comme Nautilus (`HintIgnore`, `HintSystem`, amovible) :
  `/boot` et `/boot/efi` disparaissent, `/` reste. Capacité par `statvfs` en mode détaillé.
- `actions.rs` : `Filesystem.Mount`/`Unmount` avec `auth.no_user_interaction = false` (GNOME
  affiche son dialogue polkit) ; éjection USB = démontage de toutes les partitions du disque
  puis `Drive.PowerOff`, ou `Drive.Eject` pour un support optique.
- `signals.rs` : `InterfacesAdded`/`InterfacesRemoved`/`PropertiesChanged` incrémentent la
  révision ; le sondage de 2 s ne reste qu'en repli.
- Repli sans UDisks2 (WSL, conteneur) : lecture actuelle de `/proc/mounts` et `mountinfo`
  (`linux_volumes/mounts.rs`), sans montage ni éjection.

## Windows

`WindowsVolumes` dans `windows_drives.rs` reçoit les délégations aujourd'hui dans
`windows_impl.rs`, sans changer de logique. Seule modification de construction : le champ
`actions` dans les deux `DriveInfo` de `windows_drives.rs`. Les appels communs
(`drive_watchers`, `network`, `disk_images`, `trash_settings`, `commands/system`, `utils`,
`file_ops/copy`) passent de `platform().list_drives(…)` à `platform().volumes().list(…)`.

## Interface utilisateur

- Partitions non montées dans la section des lecteurs, après les montées, icône atténuée.
  Clic gauche : montage en arrière-plan, puis navigation vers le chemin obtenu. Curseur inchangé.
- Menu : « Démonter » si `actions.unmount`, « Éjecter » si `actions.eject`. `eject_usb_drive`
  devient une libération commune : les surveillances sont relâchées avant l'appel, sinon
  Linux répond `Busy`.
- Nouvelles clés dans les cinq catalogues : `drive-unmount`, `drive-unmount-failed`,
  `drive-mount-failed` (avec le cas d'un NTFS Windows en veille prolongée).

## Étapes

Chaque étape passe `check-linux.sh`, `check-windows.sh` et ses tests ; les étapes 1 et 4,
qui touchent Windows, passent aussi la CI Windows sur une PR.

1. **Extraire le trait**, déplacements seuls, Linux inchangé. *Fait* : 7 fichiers communs,
   uniquement les appels ; CI Windows sur PR encore à passer.
2. **Inventaire UDisks2** et repli. Essai : `Data`, clé, `/boot/efi` masqué, capacités.
3. **Signaux** UDisks2 à la place du sondage.
4. **Démonter et éjecter** avec authentification ; champ `actions` côté Windows.
5. **Volumes non montés** et montage au clic.
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
