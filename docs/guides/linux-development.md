# Développer la version Linux

Le port Linux est en cours sur la branche `linux-foundation`. **Windows reste la
plateforme de référence et ne doit pas être altéré** : un changement Linux passe par
le trait `Platform` ou un module `cfg(not(target_os = "windows"))`. Un fichier commun
touché impose de vérifier Windows (voir [Préserver Windows](#préserver-windows)).

**La branche n'est pas fusionnée dans `main`**, qui porte les versions Windows publiées :
le port Linux doit pouvoir être abandonné sans rien défaire. La synchronisation va dans
un seul sens, `main` vers `linux-foundation`, à chaque version Windows, suivie de
`check-windows.sh`. Hors du pilote [volumes](../architecture/volumes.md), éviter de
restructurer du code Windows sur la branche : chaque déplacement devient un conflit
potentiel à la synchronisation suivante.
État vérifié et preuves : [bilan du 26 octobre](../reports/2026-10-26-linux-ui-trash.md).

## Installer sur Fedora Workstation

Rust Linux récent (validé avec Rust 1.99.1, Fedora 44), puis :

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  fontconfig-devel freetype-devel libxkbcommon-devel \
  libxkbcommon-x11-devel libxcb-devel wayland-devel vulkan-loader
```

Sur machine physique, vérifier le pilote Vulkan : `mesa-vulkan-drivers` pour Intel/AMD,
pilote NVIDIA propriétaire (RPM Fusion `akmod-nvidia`) pour NVIDIA. Le pin GPUI et
`Cargo.lock` sont communs à Windows : ne pas les mettre à jour pour un problème de SDK.

## Vérifier et lancer

```bash
bash scripts/check-linux.sh            # cargo check --locked --all-targets, sans avertissement
export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target"
cargo build --locked --bin oxyde
```

Lancer sur un **profil isolé** tant que les opérations de fichiers ne sont pas sûres
(jalon 4) : configuration, cache, corbeille et temporaires restent dans un dossier jetable.

```bash
P=~/oxyde-test; mkdir -p $P/{config,cache,data,tmp}
XDG_CONFIG_HOME=$P/config XDG_CACHE_HOME=$P/cache XDG_DATA_HOME=$P/data TMPDIR=$P/tmp \
  RUST_LOG=info "$CARGO_TARGET_DIR/debug/oxyde"
```

Le journal est écrit dans `$P/config/Oxyde/oxyde.log` (adaptateur GPU choisi, décorations).
Une corbeille isolée ne reçoit que les suppressions faites sur le même système de
fichiers que `$P`. **Les réglages GNOME de la corbeille (`gsettings`) restent globaux.**
Ne lancer aucune écriture (suppression, déplacement, collage, glisser) sur des données réelles.

Les trois workers auxiliaires sont des `main` vides sous Linux. Les rendus PDF et vidéo
passent par un worker d'Oxyde lui-même, confiné par bubblewrap et seccomp ; voir les
[aperçus Linux](../architecture/linux-previews.md#confinement-bubblewrap). Après modification, reconstruire avec
`cargo build --offline --bins`.
WSL reste utilisable pour `check-linux.sh` ; il ne valide ni GPU, ni bureau, ni volumes.

## Préserver Windows

Après tout changement de fichier commun, vérifier depuis Linux que Windows compile :

```bash
bash scripts/check-windows.sh          # cargo xwin check, cible MSVC, toutes les cibles
```

Il détecte `cfg` mal placés et imports cassés, sans lier ni exécuter. Installation unique :
`sudo dnf install llvm lld clang`, `rustup target add x86_64-pc-windows-msvc`,
`cargo install --locked cargo-xwin`, puis un premier lancement avec `XWIN_ACCEPT_LICENSE=1`,
qui accepte la licence Microsoft du CRT et du SDK et les télécharge (1,2 Go dans
`~/.cache/cargo-xwin`). `scripts/llvm-rc-crate-root.sh` fait chercher à `llvm-rc` les
ressources depuis la racine de chaque crate, comme sous Windows.

Les tests restent à lancer sous Windows : `./scripts/test-windows.ps1 -Offline`
([procédure](development.md#vérifier-un-changement)), ou sans redémarrer par le
déclenchement manuel du workflow `tests.yml` sur la branche poussée (`workflow_dispatch`,
sans PR ni fusion). Suite Windows réussie sur GitHub jusqu'à `7583b40`, `tests/archive_io_pipeline.rs` compris.

## Reste à faire

Identifiants M/T : [audit de portabilité](../reports/2026-10-26-linux-portability-audit.md).

**Vérifications sur machine physique (jalon 2)**
- GPU Intel puis NVIDIA : backend Vulkan obtenu (la VM utilisait GL/llvmpipe), ombres, redimensionnement.
  Fait sur portable hybride pour le réveil : au lancement, les pilotes Vulkan des cartes sans
  écran connecté sont exclus (`VK_LOADER_DRIVERS_DISABLE`, `platform/linux_gpu.rs`), sauf si
  l'utilisateur ou le bureau choisit déjà un GPU, et les applications ouvertes par Oxyde ne
  l'héritent pas. Mesuré sur Intel + RTX 4070 : la NVIDIA reste `suspended` pendant et après
  l'exécution ; avec un choix explicite, elle passe `active`. Reste : écran externe branché
  sur la NVIDIA, Oxyde déjà lancé.
- Échelles 100/125/150/200 %, écrans mixtes, déplacement d'une fenêtre entre écrans.
- Session X11/Xwayland séparément de Wayland.
- **KDE Plasma** : les fenêtres à barre de titre demandent `WindowDecorations::Client`
  (`ui::window_decorations`). Double barre reproduite puis disparue sous KWin 6 en X11 et
  Wayland imbriqués ; déplacement, redimensionnement et maximisation vérifiés sous KWin X11.
  Reste : une vraie session Plasma, ombres et fenêtres secondaires.
- Infobulles : leur bornage utilise encore la taille de fenêtre et peut mordre de
  quelques pixels sur la marge d'ombre (`tooltip/position.rs`).

**Corbeille**
- KDE : essai réel des limites `ktrashrc`. KDE ne les applique qu'aux suppressions faites
  par KIO ; Oxyde ne les fait pas encore respecter quand il met à la corbeille.
- Restauration entre systèmes de fichiers (`EXDEV`) refusée : à traiter si une corbeille
  domicile reçoit des éléments d'un autre volume.
- Copier/couper/glisser depuis la corbeille désactivés : contrat de transfert à définir.

**Opérations sur les fichiers (jalon 4, avant tout usage réel)**
- M07, corrigé : un dépôt entrant est toujours une copie, puisque GPUI ne négocie que
  Copy en Wayland et X11 ; Ctrl et Maj restent à la source, qui garde le clavier. Les
  volumes se distinguent par le point de montage le plus profond (`/proc/self/mountinfo`,
  relu en arrière-plan), comparé sans résoudre les liens symboliques. Essayé sous GNOME :
  Nautilus vers Oxyde, glisser interne vers `/tmp`, puis vers une clé USB en lecture seule
  (copie proposée, refus propre). Reste : une clé inscriptible.
- Six tests (`file_ops::item_outcomes`, `operations::files::icons`) exigent un dossier
  `./target` : ils échouent quand `CARGO_TARGET_DIR` pointe ailleurs. Les tests
  `preview_jobs` à délai de 2 s échouent par intermittence quand toute la suite tourne.
- M05, fait : [noms non UTF-8](../architecture/non-utf8-names.md) distincts de leurs
  sosies, marqués comme dans Nautilus, renommables ; les autres opérations les refusent.
- M03/M06, faits pour la copie : [copies](../architecture/copies.md) depuis la source
  vérifiée, par le noyau, avec modes, dates, `user.*` et ACL ; un dossier déplacé sur son
  disque est renommé entier et renommer fonctionne sur ntfs-3g : voir
  [déplacements](../architecture/moves.md).
- T04, fait : l'identité n'exige plus de date de naissance hors Windows.
- T05, fait : [liens symboliques](../architecture/links.md), sauf dans les archives.
- T08, fait : [noms et chemins Linux](../architecture/linux-names.md), `\` caractère de
  nom et casse par dossier (FAT, exFAT, SMB, `casefold`).
- T06, fait : [accès sans lecture et suppression](../architecture/linux-access.md),
  déplacer sans lire, dossiers traversables, pause et annulation à chaque élément ;
  Turbo supprime en parallèle, sauf sur Btrfs.
- T07, fait : [cache des fichiers ouverts depuis une archive](../architecture/linux-open-cache.md),
  éviction sous bail d'écriture, dossier de l'utilisateur.
- M03/M04, faits : [effets par dossiers tenus](../architecture/linux-held-effects.md),
  renommage et suppression de l'objet vérifié où qu'il soit, sans jamais toucher un
  remplaçant ; publication exclusive, y compris sur ntfs-3g.

**Intégration (jalon 5)**
- M08, fait : [presse-papiers](../architecture/clipboard.md) partagé avec Nautilus.
- T11, fait : [surveillance](../architecture/watchers.md) par inotify hors partages réseau.
- Glisser vers d'autres applications. Volumes
  faits (T02, T03) : inventaire UDisks2, notifications, démontage et éjection, volumes non
  montés, disques physiques et SSD ; voir le [pilote volumes](../architecture/volumes.md).
- T12, fait : [accès rapide](../architecture/quick-access.md) partagé avec Nautilus,
  erreurs de `xdg-open` signalées, Propriétés d'Oxyde seules.
- T13, fait : aucune mise à jour dans l'application, laissée au gestionnaire de paquets
  comme pour Nautilus ; voir la [distribution](distribution.md#mises-à-jour-dans-lapplication).
- T15, fait : [identité de bureau](../architecture/desktop-identity.md) ;
  `bash scripts/install-linux-desktop.sh --exec <binaire> --profile ~/oxyde-test`
  installe l'entrée et l'icône pour l'utilisateur.
- Fait : [Propriétés Linux](../architecture/linux-properties.md) comme Nautilus, avec
  les onglets Droits d’accès (groupe, niveaux, exécution, sélection,
  propriétaire pour root) et Contenu (éléments contenus), et le type MIME selon GIO. Les tests de root se lancent dans
  un espace de noms utilisateur : `unshare --map-auto --map-root-user <binaire de test>
  linux_permissions`.
- T10, implémenté : [réseau](../architecture/linux-network.md) par GVfs comme Nautilus,
  partages montés au chemin FUSE, identifiants demandés, liste `network:///` ;
  vérification visuelle en attente. « Plus d'options » (menu Shell) est masqué hors Windows.

**Avancé et distribution (jalons 6–7)**
- T09 : [poppler et GStreamer](../architecture/linux-previews.md) dans un worker ; PDF,
  survol vidéo et lecteur du Viewer vérifiés.
  M09/R5 : tous les décodeurs tournent dans des workers confinés par bubblewrap et
  seccomp, archives comprises ; vérifiés dans l'interface.
- T14, fait : [paquets RPM et DEB et CI Linux](linux-distribution.md), ouverture des
  dossiers reçus et profil AppArmor pour Ubuntu ; RPM vérifié dans l'interface, CI réussie.
