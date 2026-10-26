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

Les trois workers auxiliaires sont des `main` vides sous Linux : aucun confinement.
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
sans PR ni fusion). Suite Windows réussie jusqu'à `b84e5c3` ; `check-windows.sh` réussi
sur `7480eb6`. **Suite à relancer :** M07 (`drag.rs`, `drag/external.rs`,
`drive_watchers.rs`) et l'extraction du trait `Volumes` touchent du code commun.

## Reste à faire

Identifiants M/T : [audit de portabilité](../reports/2026-10-26-linux-portability-audit.md).

**Vérifications sur machine physique (jalon 2)**
- GPU Intel puis NVIDIA : backend Vulkan obtenu (la VM utilisait GL/llvmpipe), ombres, redimensionnement.
- Échelles 100/125/150/200 %, écrans mixtes, déplacement d'une fenêtre entre écrans.
- Session X11/Xwayland séparément de Wayland.
- **KDE Plasma** : KWin accepte les décorations serveur, que GPUI demande par défaut ;
  risque de double barre de titre. Demander `WindowDecorations::Client` sous Linux à
  chaque `open_window` (comme Soliton), puis vérifier ombre et redimensionnement.
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
- M03/M04 : opérations par chemin malgré les handles retenus ; déplacement exclusif
  émulé par lien physique puis suppression. Deux tests `history_file_undo` échouent
  sous Linux : l'annulation remplace un dossier substitué entre-temps.
- Six tests (`file_ops::item_outcomes`, `operations::files::icons`) exigent un dossier
  `./target` : ils échouent quand `CARGO_TARGET_DIR` pointe ailleurs. Les tests
  `preview_jobs` à délai de 2 s échouent par intermittence quand toute la suite tourne.
- M05 : noms non UTF-8 convertis avec perte ; M06 : fidélité des métadonnées
  (modes, ACL, xattrs, propriétaire) ; T04 : date de création exigée par l'historique.
- T05 liens symboliques, T06 ouvertures en lecture inutiles, T07 cache d'archives ouvertes,
  T08 règles de casse et `\` traité comme séparateur.

**Intégration (jalon 5)**
- M08 : presse-papiers de fichiers absent (Ctrl+C/X/V renvoient une erreur).
- Glisser vers d'autres applications, volumes et éjection UDisks2 (T02, T03),
  surveillance adaptée (T11). Les montages `/run/media` sont listés, mais `list_drives`
  reste une lecture brute de `/proc/mounts` : `/boot/efi` illisible y apparaît, sans
  capacité ni type amovible.
- T12 : favoris GTK/XDG, erreurs de `xdg-open`, « Propriétés » système lancée par `gio open`.
- T13 : mises à jour automatiques à désactiver ; T15 : icône, fichier `.desktop`, `app_id`.
- T10 : réseau SMB/GVfs ; « Plus d'options » (menu Shell) est masqué hors Windows.

**Avancé et distribution (jalons 6–7)**
- T09 : aperçus PDF et vidéo, lecteur intégré ; M09/R5 : workers confinés Linux.
- T14 : CI Linux distincte ; paquets RPM puis DEB, essai Silverblue.
