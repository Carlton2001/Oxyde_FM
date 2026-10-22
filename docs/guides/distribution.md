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

Les curseurs du Viewer (`assets/cursors/search.cur` et `hand-grab.cur`, 32 à
96 pixels) proviennent des SVG Lucide `search` et `hand-grab`, teintés en blanc
sans modifier leur tracé. Ils sont conservés dans Git ; les régénérer avec
Node.js et `sharp` (`npm install --no-save sharp`) :

```powershell
node scripts/generate-viewer-cursors.cjs
```

## Construction de la distribution

Construire avec Rust MSVC x64 et les Build Tools Visual C++. Installer une fois
le générateur de licences, depuis le binaire officiel Windows x64 :

```powershell
./scripts/install-cargo-about.ps1
```

Le script installe `cargo-about` 0.9.2 dans `$CARGO_HOME/bin` (ou `~/.cargo/bin`),
qui doit être dans le PATH. Il vérifie les SHA-256 fixés de l'archive officielle
avant extraction et du binaire avant exécution, puis contrôle sa version. Un binaire
déjà présent avec l'empreinte attendue est réutilisé sans accès réseau ; sinon il est
remplacé après vérification du téléchargement. Les empreintes proviennent de la
[release officielle](https://github.com/EmbarkStudios/cargo-about/releases/tag/0.9.2).
La compilation locale reste possible avec
`cargo install cargo-about --version 0.9.2 --locked --features cli`.

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

Le workflow `.github/workflows/distribution.yml` compile, teste les chemins portables
et publie le ZIP et son SHA-256 comme artefact GitHub Actions, sans publier de release.
Il peut être lancé manuellement. Il se lance aussi sur `main` après modification de
`Cargo.toml`, `Cargo.lock`, `.cargo/`, des scripts et réglages de licences, ou des workflows
et de l'action de distribution, pour préparer le cache des dépendances utilisé par les
prochaines releases.

Les deux workflows de distribution utilisent l'action locale
`.github/actions/setup-distribution/action.yml` : Rust **1.98.1** (version fixée pour
ces workflows uniquement), cache partagé des dépendances compilées et de `cargo-about`
**0.9.2**. Mettre à jour la toolchain dans cette action après validation d'une nouvelle
version de Rust. `cargo-about` est téléchargé précompilé par `scripts/install-cargo-about.ps1`
en l'absence d'un binaire vérifié dans le cache Cargo ; aucune compilation du générateur
n'est nécessaire. Lors d'une mise à jour, adapter version, empreintes de l'archive et du
binaire dans ce script, puis la version dans la clé partagée du cache. Les tests portables
utilisent `--release --target x86_64-pc-windows-msvc`,
comme les exécutables livrés, pour partager la compilation des dépendances. La suite
native de `tests.yml` conserve son profil et ses vérifications propres.

Le cache est enregistré uniquement sur la branche par défaut : GitHub permet aux tags
de le restaurer, mais ne partage pas les caches entre deux tags distincts. La première
exécution sur `main` prépare ce cache ; un changement de toolchain, de dépendances ou
une éviction peut imposer une reconstruction. Une release déclenchée avant la fin de
cette préparation peut encore travailler sans cache. Pour le remplir ou le rafraîchir
manuellement, lancer **Windows portable distribution** sur `main` avant le tag.
Le cache ne remplace aucun test ni la régénération des licences.
La compilation de distribution sélectionne uniquement l'application et ses trois workers ;
les outils internes comme `copy_benchmark` ne sont pas construits pour le paquet.

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
Le script affiche séparément les durées des licences, de la compilation Rust, d'Inno Setup
et de la création/vérification du ZIP, pour identifier les prochains gains possibles.

Pour mesurer le cache, terminer une première exécution **Windows portable distribution**
sur `main`, puis en lancer une deuxième sur le même commit. Relever les clés et résultats
de restauration, le volume et la durée des transferts/restauration/enregistrement du cache,
l'installation de `cargo-about`, la préparation/construction des tests (ligne Cargo `Finished`),
leur exécution (`test result`), les licences, la compilation Rust et le ZIP. Ce workflow
ne mesure ni Inno Setup ni la signature. Ne pas créer de tag ou de release pour mesurer.
Les [mesures de référence](../reports/2026-10-22-windows-distribution.md) concernent
la release v2.0.0 ; aucun gain hébergé des optimisations n'est encore mesuré.
La [comparaison locale des parcours Rust](../reports/2026-10-22-distribution-local-tests.md)
utilise des sources figées et des dossiers de compilation isolés, avec caches vide,
intégral et limité aux dépendances. Elle exclut les transferts du cache GitHub.

Pour vérifier l'installateur avec une archive officielle téléchargée sans modification :

```powershell
./scripts/tests/install-cargo-about.tests.ps1 -ArchivePath <cargo-about-0.9.2-x86_64-pc-windows-msvc.tar.gz>
```

Ces tests utilisent l'archive comme fixture locale et contrôlent installation, version,
réutilisation sans réseau, rejet d'un hash incorrect, remplacement d'un cache corrompu
et emplacements par défaut (`CARGO_HOME/bin` ou `USERPROFILE/.cargo/bin`). Ils couvrent
aussi les chemins contenant des espaces, une restauration sans métadonnées Cargo,
un téléchargement interrompu, une extraction en erreur ou altérée et le nettoyage
des fichiers temporaires après échec, sans remplacer l'installation Cargo de l'utilisateur.
Ils ne mesurent pas le transfert du cache GitHub.

Tests : `cargo test --lib app_paths::tests`. Vérifier aussi les caches et archives concernés.
Le script vérifie les fichiers requis dans le ZIP. Tester manuellement la session, les aperçus,
les archives et la fermeture depuis un dossier déplacé et une clé USB Windows 10/11.
Les exécutables ne sont pas signés. La documentation historique ne valide ni une distribution Linux ni une exécution récente du workflow hébergé.
