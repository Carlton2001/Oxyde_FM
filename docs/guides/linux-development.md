# Préparer la compilation Linux

Le port Linux est en cours. Windows reste la plateforme de référence ; compiler
sous Linux ne valide ni les opérations de fichiers, ni la corbeille, ni les
intégrations du bureau. Les essais graphiques sur VM restent à effectuer.
Voir l'[audit de portabilité](../reports/2026-10-26-linux-portability-audit.md)
pour les fonctions incomplètes et les risques à traiter avant utilisation réelle.

## Environnement Fedora

Utiliser une toolchain Rust Linux récente, distincte de Rust MSVC. La première
validation utilise Fedora 44 dans WSL et Rust 1.98.1. Ce couple ne définit pas
encore une version minimale de Rust ou de Fedora supportée par le produit.

Paquets de développement nécessaires à la préparation du SDK :

```bash
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  fontconfig-devel freetype-devel libxkbcommon-devel \
  libxkbcommon-x11-devel libxcb-devel wayland-devel
```

Le pin GPUI et `Cargo.lock` sont communs à Windows et Linux. Ne pas les mettre
à jour pour résoudre un simple problème de SDK. La présence de Vulkan, d'une
session graphique et des portails se vérifie séparément lors des essais UI.

## Vérifier dans WSL

Depuis PowerShell, entrer dans Fedora :

```powershell
wsl -d FedoraLinux-44
```

Puis dans le shell Linux :

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /mnt/c/chemin/vers/Oxyde-GPUI  # checkout Windows vu depuis WSL
bash scripts/check-linux.sh
```

Après la récupération initiale des sources Cargo/Git, le mode hors ligne est
disponible : `bash scripts/check-linux.sh --offline`.

Le script exécute `cargo check --locked --all-targets`. Il ne lance ni Oxyde ni
les tests. Son cache par défaut est `${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target`,
sur le système de fichiers Linux sous WSL, distinct du `target/` Windows.
Un `CARGO_TARGET_DIR` explicite reste prioritaire ; choisir un dossier Linux
distinct pour une autre copie du projet si nécessaire.

Pour vérifier aussi le linkage du binaire, utiliser le même cache :

```bash
export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target"
cargo build --offline --locked --bin oxyde
```

Les trois workers auxiliaires sont encore Windows uniquement : leurs `main`
Linux ne fournissent pas de confinement. Leur construction ne valide pas les
garanties Windows sous Linux. Ne pas lancer la suite complète sur le profil réel,
en particulier les scénarios impliquant corbeille, suppression ou volumes.

## Préserver Windows

Après un changement partagé, vérifier sous Windows avec la
[procédure de développement](development.md#vérifier-un-changement), notamment
`./scripts/test-windows.ps1 -Offline` pour la suite native et ses fixtures privées.
Conserver les modules natifs Windows, leurs tests et le patch GPUI Windows.
Limiter les adaptations de cible aux usages réellement spécifiques.

Pour une future validation graphique, employer des fixtures et un profil de test
distincts. WSL ne remplace pas une VM Fedora Workstation/Silverblue pour les
volumes, les services du bureau et le comportement Intel/NVIDIA.
