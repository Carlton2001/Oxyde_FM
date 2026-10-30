# Distribuer Oxyde sous Linux

**Statut : paquets RPM et DEB construits et essayés, RPM vérifié dans l'interface, CI
réussie sur GitHub (T14 de l'[audit](../reports/2026-10-26-linux-portability-audit.md)).** La distribution Windows est décrite dans
[son propre guide](distribution.md) et ne change pas.

## Construire

```bash
bash scripts/build-linux-packages.sh --rpm   # sur Fedora
bash scripts/build-linux-packages.sh --deb   # partout : podman hors d'Ubuntu 24.04
```

Les fichiers et leur SHA-256 arrivent dans `target/linux-packages/output`. Outre les
dépendances de [développement](linux-development.md#installer-sur-fedora-workstation), il
faut sur Fedora `rpm-build`, `desktop-file-utils`, `appstream`, `python3` et `cargo-about`
0.9.2. Le DEB se construit dans l'image de `packaging/linux/debian/Containerfile`, avec son
propre dossier `target/ubuntu-24.04`, et `lintian` arrête la construction sur une erreur ;
`rpmlint` n'est qu'informatif. `--skip-build` reprend les exécutables release existants.
`--offline` garde les avis de la construction précédente : `cargo-about` lit les crates
de toutes les cibles, y compris macOS, qu'un cache Linux ne contient pas.

Un paquet ne s'installe que sur un système au moins aussi récent que celui qui l'a
construit (glibc) : le RPM vise Fedora 44 et suivantes, le DEB Ubuntu 24.04 et suivantes,
Debian 13 et suivantes. Pas d'autre architecture que x86-64, ni de signature ou de dépôt
pour l'instant. Le workflow `release.yml` ne publie pas ces paquets : une version reste
Windows seulement.

## Contenu des paquets

- `/usr/bin` : `oxyde`, `oxyde-archive-list-worker`, `oxyde-archive-io-worker`, côte à côte
  parce qu'Oxyde les cherche à côté de son exécutable. `oxyde-image-worker` est vide sous
  Linux et n'est pas livré.
- Entrée de bureau, icônes hicolor (SVG et PNG de 16 à 512), métadonnées AppStream. Les
  PNG sont conservés dans Git et régénérés avec les autres déclinaisons du logo :
  `cargo run --offline --locked --example generate_application_icons`.
- Licence et avis tiers, régénérés à chaque construction par
  `scripts/linux-third-party-notices.py`, l'équivalent de
  `update-third-party-licenses.ps1` ; le fichier conservé dans Git reste celui de Windows.
  Le champ `License` du RPM est calculé par `scripts/rpm-license.py` depuis les crates liées
  sous Linux et les ressources embarquées.
- `StartupNotify=false` : GPUI ne consomme pas le jeton de démarrage, et GNOME garderait
  le curseur occupé jusqu'à son délai.

Aucun script RPM : les déclencheurs des paquets de Fedora rafraîchissent les caches de
bureau et d'icônes, y compris sur Silverblue où seul `/usr` est livré.

## Dépendances

GIO, libseccomp, Vulkan, Wayland, Fontconfig, poppler et GStreamer sont chargés à
l'exécution, donc invisibles pour les outils de paquets, qui ne trouvent que `libxcb`,
`libxkbcommon` et la libc. Ils sont déclarés à la main :

| Rôle | Fedora | Debian/Ubuntu | Niveau |
| --- | --- | --- | --- |
| Confinement des décodeurs | `bubblewrap`, `libseccomp` | `bubblewrap`, `libseccomp2` | requis |
| GIO et réglages de la corbeille | `libgio-2.0.so.0` (`glib2`) | `libglib2.0-0t64`, `libglib2.0-bin` | requis |
| Rendu | `vulkan-loader`, `mesa-vulkan-drivers` (lavapipe en repli) | `libvulkan1`, `mesa-vulkan-drivers` | requis |
| Fenêtres et polices | `libwayland-client`, `libfontconfig`, `default-fonts-core-sans` | `libwayland-client0`, `libfontconfig1`, une police système | requis |
| Ouverture dans une autre application | `xdg-utils` | `xdg-utils` | requis |
| PDF | `poppler-glib` | `libpoppler-glib8t64` | recommandé |
| Vidéo et audio | `gstreamer1-plugins-base`, `-good` | `gstreamer1.0-plugins-base`, `-good`, `libgstreamer-plugins-base1.0-0` | recommandé |
| Réseau | `gvfs`, `gvfs-fuse`, `gvfs-smb` | `gvfs`, `gvfs-fuse`, `gvfs-backends` | recommandé |
| Volumes | `udisks2` | `udisks2` | recommandé |

Le confinement est requis parce qu'Oxyde décode sans lui, avec un simple avertissement au
journal. Les recommandés n'ôtent chacun que leur fonction.

## Ubuntu et AppArmor

Ubuntu 23.10 et suivantes refusent les espaces de noms utilisateur aux programmes sans
profil AppArmor qui les autorise : bubblewrap échoue et les décodeurs tourneraient sans
confinement. Comme Ubuntu le fait pour Nautilus, le DEB installe
`/etc/apparmor.d/io.github.Carlton2001.Oxyde`, un profil `unconfined` rattaché à
`/usr/bin/oxyde` qui ajoute seulement `userns`. Il ne vaut que pour ce chemin : un Oxyde
lancé d'ailleurs reste sans confinement sous Ubuntu.

Essai sur une machine virtuelle Ubuntu 24.04 (restriction active, 118 profils) : avec le
profil, aucun avertissement et l'archive s'ouvre ; profil retiré, `bwrap: loopback: Failed
RTM_NEWADDR` et « Preview workers run without a sandbox ». Le profil est retiré du noyau à
la désinstallation et le fichier à la purge. Un système sans restriction (Debian) n'en a
pas besoin et n'est pas gêné.

## Ouvrir un dossier

`Exec=oxyde %U` et `MimeType=inode/directory` placent Oxyde dans « Ouvrir avec » sans le
rendre gestionnaire par défaut, qui reste celui que le bureau désigne (Nautilus sous
GNOME). Comme une nouvelle fenêtre de Nautilus, le lancement n'affiche que
les emplacements reçus, un onglet par emplacement, et ne touche pas aux onglets enregistrés
pour le lancement suivant ; le code est dans `platform/linux_startup.rs`.

Limites : un lancement est un nouveau processus, sans instance unique, et son enregistrement
des préférences réécrit la session qu'il a lue au démarrage ; un fichier reçu ouvre son
dossier sans le sélectionner ; un nom non UTF-8 est ignoré.

## Intégration continue

`.github/workflows/linux.yml`, sur `main`, `linux-foundation` et leurs PR (premier passage réussi sur
`7583b40`, 33 minutes) :

- **tests** sur `ubuntu-24.04` : compilation sans avertissement, suite complète, puis
  `preview_jobs` à part, dont les délais de 2 s échouent quand toute la suite charge la
  machine. La restriction AppArmor est levée pour que les tests de confinement s'exécutent.
- **rpm**, dans un conteneur `fedora:44`, et **deb**, sur `ubuntu-24.04` : construction,
  installation avec les dépendances, artefact téléchargeable.

Rust est fixé à 1.99.1 et `cargo-about` vérifié par empreinte. Les workflows Windows ne
changent pas.

## Essais du 30 octobre 2026

- Interface : RPM installé sur Fedora 44 et jugé bon par l'utilisateur, avec comme points
  à essayer l'icône et le nom dans les Activités, « Ouvrir avec » depuis Nautilus, une
  archive, un PDF et le retour des onglets enregistrés.
- CI : `actionlint` sans remarque ; job **tests** rejoué à la main sur une machine virtuelle
  Ubuntu 24.04 (ext4), réussi, confinement compris, puis workflow réussi sur GitHub.
- RPM : installé dans un conteneur Fedora 44 avec ses dépendances, lancé sous Xvfb sur une
  URI `file://` accentuée, archive ouverte par les workers de `/usr/bin` ; couche
  `rpm-ostree` sur l'image Silverblue 44 acceptée, `bootc container lint` passé. Le
  conteneur interdit bubblewrap, et Silverblue n'a pas été démarré.
- DEB : construit dans un conteneur Ubuntu 24.04, mêmes essais dans la machine virtuelle
  ci-dessus, puis retrait, réinstallation et purge. Installé aussi avec ses dépendances dans
  des conteneurs Debian 13 et Ubuntu 26.04, bibliothèques chargées à l'exécution présentes.
- Dépendances requises seules (`--no-install-recommends`, `install_weak_deps=False`) :
  installation, lancement sous Xvfb et archive ouverts sur Fedora 44 et Ubuntu 24.04.
  `lintian` et `rpmlint` sans erreur ; restent les pages de manuel absentes.
