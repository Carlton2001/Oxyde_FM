# Distribuer Oxyde sous Windows

Pour construire et lancer en développement : [guide de développement](development.md).

## Construction de la distribution

Construire avec Rust MSVC x64 et les Build Tools Visual C++ :

```powershell
./scripts/build-distribution.ps1 -PortableOnly
```

Sans `-PortableOnly`, le script construit aussi le Setup classique avec Inno Setup 6.3+.
`-SkipBuild` réutilise les exécutables release existants ; `-IsccPath` et `-CrtDirectory`
permettent de préciser les outils locaux. Les SHA-256 sont fournis pour le ZIP portable.

## Version portable

Extraire `target/distribution/output/Oxyde_GPUI-<version>-x64-portable.zip`, puis lancer
`Oxyde_GPUI/Oxyde_GPUI.exe`. Garder les DLL et les trois workers à côté de l'exécutable.
Le ZIP ne contient aucun installateur ni lanceur intermédiaire.
Le dossier `licenses/syntaxes` conserve les licences et notices des grammaires
SCSS/Sass et PowerShell ; il est également déployé par le Setup classique.

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

Pour déplacer l'application, déplacer tout le dossier Oxyde_GPUI, y compris `data`.
Les chemins enregistrés vers les fichiers utilisateur ne sont pas réécrits lors du déplacement.
Supprimer `data` désactive le mode portable au prochain démarrage et rétablit les emplacements
habituels du profil utilisateur. Effacer le cache dans les paramètres conserve configuration,
logs et session. Aucun raccourci ni entrée de désinstallation n'est créé en mode portable.

Cette isolation concerne les fichiers gérés par Oxyde. Elle ne garantit pas l'absence de traces
produites par Windows, les outils externes ou les opérations de fichiers demandées par l'utilisateur.
Les profils AppContainer éphémères nécessaires à la protection des workers restent utilisés
et supprimés suivant leur cycle de vie actuel. Les protections ne sont pas réduites pour le mode portable.

## Setup classique

`Oxyde_GPUI_Setup.exe` installe par utilisateur dans
`%LOCALAPPDATA%\Programs\Oxyde_GPUI`, avec raccourci Démarrer « Oxyde GPUI », raccourci bureau
facultatif et entrée de désinstallation distincte. Identité : `Oxyde.GPUI.Windows`.
Le Setup ne déploie pas de dossier `data` : les chemins installés existants restent inchangés.
La désinstallation conserve la configuration et les caches. Aucune association de fichiers,
modification de PATH ou remplacement de l'Explorateur Windows n'est effectué.

## Automatisation et validation

Le workflow manuel `.github/workflows/distribution.yml` compile, teste les chemins portables
et publie le ZIP et son SHA-256 comme artefact GitHub Actions, sans publier de release.

Tests : `cargo test --lib app_paths::tests`. Vérifier aussi les caches et archives concernés.
Le script vérifie les fichiers requis dans le ZIP. Tester manuellement la session, les aperçus,
les archives et la fermeture depuis un dossier déplacé et une clé USB Windows 10/11.
Les exécutables ne sont pas signés. La documentation historique ne valide ni une distribution Linux ni une exécution récente du workflow hébergé.
