# Distribuer Oxyde sous Windows

Pour construire et lancer en développement : [guide de développement](development.md).

## Logo et icônes de l'application

Le logo source est `assets/icons/logo_oxyde.svg`. « À propos » et la barre de
titre affichent `logo_about.png` (144 px) et `logo_title.png` (40 px), rendus
pour les écrans 2x : GPUI réduit les images en une seule passe et crénèle une
source trop grande. Après modification du SVG, régénérer ses déclinaisons
avant de reconstruire l'application :

```powershell
./scripts/distribution-icon.ps1
```

Le script utilise le moteur SVG de GPUI et génère `assets/icons/logo.png`
pour les icônes natives ainsi que `assets/icons/oxyde.ico` (16 à 256 pixels)
pour l'exécutable, l'Explorateur, la barre des tâches et l'installateur.
Ces deux fichiers générés sont conservés dans Git. Chaque taille est rendue
depuis le SVG ; les icônes spécifiques des workers restent indépendantes.

## Construction de la distribution

Construire avec Rust MSVC x64 et les Build Tools Visual C++. Installer une fois
le générateur de licences :

```powershell
cargo install cargo-about --version 0.9.2 --locked --features cli
```

Lancer ensuite la distribution :

```powershell
./scripts/build-distribution.ps1 -PortableOnly
```

Sans `-PortableOnly`, le script construit aussi le Setup classique avec Inno Setup 6.3+.
`-SkipBuild` réutilise les exécutables release existants ; `-IsccPath` permet de préciser
Inno Setup. Le runtime C est lié statiquement (`.cargo/config.toml`) : aucune DLL Microsoft
n'est livrée, et le script refuse un exécutable qui en importerait encore une. Les SHA-256 sont fournis pour le ZIP portable.

## Version portable

Extraire `target/distribution/output/Oxyde_FM-<version>-x64-portable.zip`, puis lancer
`Oxyde_FM/Oxyde_FM.exe`. Garder les trois workers à côté de l'exécutable.
Le ZIP ne contient aucun installateur ni lanceur intermédiaire.
Le dossier `licenses/syntaxes` conserve les licences et notices des grammaires
SCSS/Sass et PowerShell ; il est également déployé par le Setup classique.
Le dossier `licenses/dependencies` contient les notices générées des crates Rust,
les licences du checkout GPUI/Zed et la notice Lucide. Il est inclus dans les deux
formats de distribution.

La présence du dossier `data` à côté de l'exécutable active automatiquement le mode portable,
quel que soit le répertoire courant. Il est fourni dans le ZIP et contient ensuite :

- `config.json` : préférences et état de la session (onglets, disposition, fenêtre).
- `oxyde.log`, éventuellement `oxyde.previous.log` : diagnostics.
- `cache/icons` et `cache/thumbnails` : caches générés et leurs fichiers de contrôle.
- `temp` : fichiers temporaires et espaces de travail des archives/workers.

Les variables TEMP, TMP et TMPDIR du processus sont redirigées vers `data/temp` avant le
lancement des threads ; les enfants les héritent. La configuration installée et les données
Tauri ne sont ni importées ni modifiées. Si `data` n'est pas utilisable, une erreur s'affiche :
il n'y a aucun repli silencieux vers AppData. Un support accessible en écriture est nécessaire.

Pour déplacer l'application, déplacer tout le dossier Oxyde_FM, y compris `data`.
Les chemins enregistrés vers les fichiers utilisateur ne sont pas réécrits lors du déplacement.
Supprimer `data` désactive le mode portable au prochain démarrage et rétablit les emplacements
habituels du profil utilisateur. Effacer le cache dans les paramètres conserve configuration,
logs et session. Aucun raccourci ni entrée de désinstallation n'est créé en mode portable.

Cette isolation concerne les fichiers gérés par Oxyde. Elle ne garantit pas l'absence de traces
produites par Windows, les outils externes ou les opérations de fichiers demandées par l'utilisateur.
Les profils AppContainer éphémères nécessaires à la protection des workers restent utilisés
et supprimés suivant leur cycle de vie actuel. Les protections ne sont pas réduites pour le mode portable.

## Setup classique

`Oxyde_FM_<version>_x64-setup.exe` installe par utilisateur dans
`%LOCALAPPDATA%\Programs\Oxyde_FM`, avec raccourci Démarrer « Oxyde », raccourci bureau
facultatif et entrée de désinstallation distincte. Identité : `Oxyde.GPUI.Windows`.
Le Setup ne déploie pas de dossier `data` : les chemins installés existants restent inchangés.
La désinstallation supprime `%LOCALAPPDATA%\Oxyde` (caches) et propose, sauf en mode silencieux,
de supprimer `%APPDATA%\Oxyde` (préférences, session, logs). Aucune association de fichiers,
modification de PATH ou remplacement de l'Explorateur Windows n'est effectué.

Lancé avec `/UPDATE` (par l'updater Tauri ou GPUI), le Setup saute ses pages puis relance
Oxyde. Les options NSIS ajoutées par l'updater Tauri (`/P /R /ARGS`) sont ignorées.

Transition depuis l'édition Tauri : après l'installation, le Setup désinstalle toute entrée
`Oxyde_FM` de l'éditeur `Oxyde` (MSI par machine avec élévation, ou NSIS par utilisateur),
puis supprime `%APPDATA%\com.oxyde.desktop` et `%LOCALAPPDATA%\com.oxyde.desktop`.
Cette section de `oxyde.iss` est temporaire.

## Mises à jour dans l'application

`systems/updates.rs` lit `latest.json` sur la dernière release publiée, compare la version
à `CARGO_PKG_VERSION` et vérifie la signature minisign du Setup avec la clé publique de
l'updater avant de l'écrire dans un dossier temporaire. Le téléchargement passe par
`Platform::download` (WinHTTP, proxy système, HTTPS uniquement, taille bornée).

Une vérification automatique a lieu 30 s après le démarrage. Chaque version n'est annoncée
qu'une fois (`last_notified_update` dans `config.json`), jamais en mode portable.
Paramètres › À propos permet de vérifier puis d'installer. L'installation lance le Setup avec
`/UPDATE /WAITPID=<pid>`, puis ferme Oxyde normalement ; le Setup attend la fin du processus
(60 s au plus), remplace les fichiers et relance Oxyde. En mode portable, le bouton ouvre la
page des releases.

## Version et tag

```powershell
./scripts/bump-version.ps1 -Version <x.y.z>
```

Depuis `main` sans modification locale, le script vérifie que la version dépasse l'actuelle
et que le tag `v<x.y.z>` n'existe ni localement ni sur `origin`. Il met à jour `Cargo.toml`
et `Cargo.lock`, commit `Release v<x.y.z>` et crée le tag annoté, sans pousser.
La publication se fait ensuite à la main : `git push origin main v<x.y.z>`.
Les tags `v1.x` existants appartiennent à l'édition Tauri : la première version GPUI
publiée doit être supérieure à 1.2.1.

## Automatisation et validation

Le workflow manuel `.github/workflows/distribution.yml` compile, teste les chemins portables
et publie le ZIP et son SHA-256 comme artefact GitHub Actions, sans publier de release.

Le push d'un tag `v<x.y.z>` lance `.github/workflows/release.yml`. Il vérifie que le tag
correspond à `Cargo.toml`, construit le Setup et le ZIP, signe le Setup avec la clé
minisign de l'updater (secrets `TAURI_SIGNING_PRIVATE_KEY` et `_PASSWORD`, partagés avec
l'édition Tauri), puis génère `latest.json` avec `scripts/write-update-manifest.ps1`.
Les trois clés Windows du manifeste pointent vers le Setup Inno. Le workflow crée une
release brouillon ; la publier à la main la rend visible des updaters, qui lisent
`releases/latest/download/latest.json`.

À chaque exécution de `build-distribution.ps1`, le script régénère les licences des crates
depuis `Cargo.lock`, avant la compilation et l'assemblage des paquets, même avec `-SkipBuild`.
Il inclut ensuite le rapport dans le ZIP et le Setup. Si la génération échoue, la distribution
s'arrête. Un accès réseau est nécessaire pour récupérer les notices amont manquantes.
Le workflow installe automatiquement le générateur. Pour lancer la génération séparément,
suivre le [guide des notices](../../licenses/dependencies/README.md).

Tests : `cargo test --lib app_paths::tests`. Vérifier aussi les caches et archives concernés.
Le script vérifie les fichiers requis dans le ZIP. Tester manuellement la session, les aperçus,
les archives et la fermeture depuis un dossier déplacé et une clé USB Windows 10/11.
Les exécutables ne sont pas signés. La documentation historique ne valide ni une distribution Linux ni une exécution récente du workflow hébergé.
