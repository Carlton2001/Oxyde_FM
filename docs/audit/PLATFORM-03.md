# Audit PLATFORM-03

## Fichiers lus en entier

- [x] `src/platform/linux_quick_access.rs`
- [x] `src/platform/linux_quick_access/bookmarks.rs`
- [x] `src/platform/linux_quick_access/tests.rs`
- [x] `src/platform/linux_rename.rs`
- [x] `src/platform/linux_sandbox.rs`
- [x] `src/platform/linux_sandbox/landlock.rs`
- [x] `src/platform/linux_sandbox/seccomp.rs`
- [x] `src/platform/linux_settings.rs`
- [x] `src/platform/linux_settings/tests.rs`
- [x] `src/platform/linux_signature.rs`
- [x] `src/platform/linux_source_selection.rs`
- [x] `src/platform/linux_startup.rs`
- [x] `src/platform/linux_terminal.rs`
- [x] `src/platform/linux_terminal/tests.rs`
- [x] `src/platform/linux_trash.rs`
- [x] `src/platform/linux_trash/transfer.rs`
- [x] `src/platform/linux_update.rs`
- [x] `src/platform/linux_video.rs`
- [x] `src/platform/linux_video/stream.rs`
- [x] `src/platform/linux_volumes.rs`
- [x] `src/platform/linux_volumes/actions.rs`
- [x] `src/platform/linux_volumes/disks.rs`
- [x] `src/platform/linux_volumes/fallback.rs`
- [x] `src/platform/linux_volumes/holders.rs`
- [x] `src/platform/linux_volumes/images.rs`
- [x] `src/platform/linux_volumes/inventory.rs`
- [x] `src/platform/linux_volumes/mounts.rs`
- [x] `src/platform/linux_volumes/signals.rs`
- [x] `src/platform/linux_volumes/udisks.rs`
- [x] `src/platform/linux_volumes/watch.rs`
- [x] `src/platform/linux_window_controls.rs`
- [x] `src/platform/linux_window_controls/tests.rs`
- [x] `src/platform/linux_worker.rs`
- [x] `src/platform/memory.rs`

## Constats

### `src/platform/linux_source_selection.rs:16`
- Gravité : moyen. Confiance : confirmé.
- Défaut : les drapeaux d'ouverture sont des valeurs numériques x86 codées en dur (O_NOFOLLOW 0x20000, O_DIRECTORY 0x10000) ; sur aarch64 ces valeurs signifient O_LARGEFILE et O_DIRECT, donc ni « ne pas suivre les liens » ni « dossier exigé » ne s'appliquent.
- Scénario : Oxyde compilé depuis les sources sur aarch64 (Raspberry Pi, Asahi) ; la copie ou le déplacement d'un dossier contenant un lien symbolique vers un dossier ouvre la cible au lieu du lien (`open_source_directory`, `track_source_item`), recopie l'arborescence cible, et O_DIRECT peut faire échouer l'ouverture (EINVAL) sur un système de fichiers sans E/S directe.

### `src/platform/linux_volumes.rs:127` (avec `linux_volumes/images.rs:97`)
- Gravité : moyen. Confiance : confirmé.
- Défaut : dès qu'un volume appartient à une image montée, le démontage passe uniquement par `images::unmount`, qui canonicalise le chemin d'image rapporté par UDisks ; si l'image a été supprimée, le noyau rapporte « chemin (deleted) », la canonicalisation échoue et l'erreur est renvoyée sans repli sur le démontage ordinaire.
- Scénario : l'utilisateur monte un ISO, supprime ou vide l'ISO de la corbeille, puis choisit « Démonter » sur le volume : Oxyde renvoie « No such file or directory » et le volume reste monté, ainsi que le périphérique loop.

### `src/platform/linux_volumes/fallback.rs:84` (avec `linux_volumes.rs:74`)
- Gravité : moyen. Confiance : plausible.
- Défaut : sans UDisks2, seuls nfs, cifs, smb sont classés « remote » ; fuse.sshfs, rclone ou 9p deviennent « fixed », reçoivent un `statvfs` et ne sont pas sondés comme partages, alors que `disks.rs` reconnaît bien fuse.sshfs comme distant.
- Scénario : session sans udisks2 (WSL, conteneur, gestionnaire de fenêtres minimal) avec un montage sshfs dont le serveur ne répond plus : `statvfs` bloque indéfiniment et la liste des lecteurs ne se rafraîchit plus.

### `src/platform/linux_sandbox.rs:294`
- Gravité : moyen. Confiance : plausible.
- Défaut : la sonde de disponibilité de bubblewrap ne teste que les espaces de noms et `/usr`, pas `--proc`, `--dev`, `--seccomp` ni `--ro-bind-fd` qu'utilisent les lancements réels ; un refus de ces options n'active jamais le repli Landlock.
- Scénario : Oxyde lancé dans une toolbox ou distrobox (podman sans privilèges) où bwrap ne peut pas monter `/proc` (« Can't mount proc ») : la sonde réussit, chaque worker échoue au démarrage et aucun aperçu, PDF, vidéo ni archive ne fonctionne.

### `src/platform/linux_sandbox/seccomp.rs:38`
- Gravité : faible. Confiance : confirmé.
- Défaut : le filtre refuse `unshare` et `setns`, mais pas `clone` ni `clone3` avec CLONE_NEWUSER ; seul le `--disable-userns` de bubblewrap couvre ce cas, pas le repli Landlock qui utilise ce même filtre.
- Scénario : machine sans bubblewrap utilisable ; un fichier hostile qui compromet un décodeur peut créer un espace de noms utilisateur et exposer la surface noyau que Flatpak bloque précisément par ce biais.

### `src/platform/linux_sandbox.rs:132` (avec `linux_sandbox.rs:227`)
- Gravité : faible. Confiance : plausible.
- Défaut : les deux confinements n'ouvrent que l'arborescence FHS (`/usr`, `/lib*`, `/bin`) et l'exécutable seul ; un système dont l'interpréteur ELF et les bibliothèques vivent ailleurs n'est pas couvert.
- Scénario : NixOS ou Guix : l'interpréteur et les bibliothèques dans `/nix/store` sont hors d'atteinte sous bwrap comme sous Landlock, et tous les workers échouent à l'exécution.

### `src/platform/linux_volumes/holders.rs:35`
- Gravité : faible. Confiance : confirmé.
- Défaut : le chemin de `/proc/<pid>/maps` est pris comme sixième mot séparé par des blancs, donc tronqué au premier espace.
- Scénario : clé montée par UDisks sous `/run/media/<user>/Ma Clé`, un processus ne la retient que par un fichier mappé (police, bibliothèque, vidéo en mmap) : la boîte « volume occupé » ne nomme personne.

### `src/platform/linux_trash.rs:72`
- Gravité : faible. Confiance : plausible.
- Défaut : la restauration refuse EXDEV au lieu de copier puis supprimer, alors que des éléments de la corbeille personnelle peuvent provenir d'un autre système de fichiers.
- Scénario : KDE place un fichier d'une clé USB FAT dans la corbeille personnelle faute de pouvoir créer `.Trash-1000` sur la clé ; Oxyde le liste mais ne peut jamais le restaurer.

### `src/platform/linux_trash.rs:33` (avec `linux_trash/transfer.rs:33`)
- Gravité : faible. Confiance : confirmé.
- Défaut : tout dossier nommé exactement `Trash` qui contient `files` et `info` est pris pour une corbeille, et ses éléments ne peuvent quitter `files` qu'en tant qu'entrées listées par la corbeille système.
- Scénario : sauvegarde d'un ancien `~/.local/share` copiée dans `~/Sauvegarde/.local/share/Trash` : couper-coller ou glisser un fichier hors de `files` échoue avec « Only whole items can leave the trash ».

### `src/platform/linux_update.rs:35`
- Gravité : faible. Confiance : plausible.
- Défaut : tout `/usr/bin/oxyde` qui ne vient pas de dpkg est supposé installé par RPM.
- Scénario : paquet AUR, Nix ou copie manuelle dans `/usr/bin` : la mise à jour propose le paquet RPM, inutilisable sur cette distribution.

### `src/platform/linux_quick_access.rs:35`
- Gravité : faible. Confiance : confirmé.
- Défaut : la surveillance des signets n'est installée que si `~/.config/gtk-3.0` existe au démarrage ; le premier épinglage crée ce dossier (`bookmarks.rs:37`) sans démarrer de surveillance.
- Scénario : profil neuf ; après le premier épinglage dans Oxyde, les signets ajoutés ou retirés dans Nautilus ou une boîte de dialogue GTK n'apparaissent pas avant le redémarrage d'Oxyde.

### `src/platform/linux_volumes/fallback.rs:48`
- Gravité : faible. Confiance : confirmé.
- Défaut : sans UDisks2, chaque montage non virtuel devient un lecteur, sans dédoublonnage par périphérique ni masquage des points système.
- Scénario : openSUSE ou Fedora en Btrfs sans udisks2 : `/boot`, `/boot/efi`, `/home`, `/var/log`, `/.snapshots`… apparaissent comme autant de lecteurs distincts.

### `src/platform/linux_trash.rs:51` (avec `linux_trash.rs:94`)
- Gravité : optimisation. Confiance : confirmé.
- Défaut : la purge relit le cache `directorysizes` complet pour chaque élément, puis le réécrit avec un fsync pour chaque dossier, soit un coût quadratique.
- Scénario : vider une corbeille de plusieurs milliers de dossiers relit le cache des milliers de fois et provoque autant de fsync, ce qui rallonge nettement l'opération sur un disque dur ou une clé.

<!-- LOT TERMINÉ -->
