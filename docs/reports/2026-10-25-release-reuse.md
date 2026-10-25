# Release : construction unique et validations du commit exact — 25 octobre 2026

La release ne compile plus. `distribution.yml` construit une fois le Setup et le ZIP
pour le commit de `main` ; `release.yml` attend sur ce commit exact la réussite des tests
natifs, de l'audit et de cette distribution, vérifie les fichiers, signe et crée le brouillon.
Cela traite A09 sans relancer de contrôle. **Aucune exécution hébergée n'a encore validé
ce parcours** : les gains ci-dessous sont des mesures anciennes ou des estimations signalées.

## Mesures hébergées disponibles

Dernières exécutions publiques, toutes sur `167dba72` (Release v2.0.1, 23 octobre) ;
aucune exécution plus récente n'existe. Les 14 commits locaux suivants, dont l'isolation
Rustup, ne sont pas poussés : son effet sur GitHub reste **non mesuré**.

| Workflow | Durée | Étapes principales |
| --- | --- | --- |
| [Tests natifs](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079033) | 15 min 39 | cache 48 s (hit), suite 14 min 00 |
| [Portable](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079036) | 8 min 49 | setup + cache 50 s (hit), tests portables 3 min 19, licences + build + ZIP 4 min 20 |
| [Release](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079017) | 11 min 55 | setup 15 s (pas de cache), tests portables 7 min 12, build Setup + ZIP 3 min 52, signature 12 s |
| [Sécurité](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079123) | 27 s | — |

Durées d'étapes lues dans l'API publique des jobs ; les journaux détaillés exigent une
authentification et n'ont pas été relus. Les deux distributions compilaient le même code
avec la même toolchain, le même profil et la même cible, sur le même commit :
**11 min 04 d'étapes Windows de la release refaisaient le travail du portable.**

## Changements

1. **Construction unique** (`distribution.yml`) : construit aussi le Setup (Inno Setup
   préinstallé, installation de secours conservée), écrit `build-info.json` (commit,
   version, SHA-256 du Setup, du ZIP et de son fichier de contrôle) et publie l'artefact
   `Oxyde_FM-setup-x64` à côté de `Oxyde_FM-portable-x64`, inchangé. Tests portables,
   construction explicite de l'application et des trois workers, licences et vérifications
   du ZIP sont inchangés.
2. **Release dépendante des validations** (`release.yml`, `scripts/wait-release-checks.ps1`) :
   après le contrôle tag/version, attend `tests.yml`, `security.yml` et `distribution.yml`
   pour `GITHUB_SHA` sur `main` (push ou lancement manuel). Dernière exécution de ce commit
   retenue ; échec, annulation, absence après 5 min ou attente de plus de 120 min arrêtent
   la release avant téléchargement et signature. Autre commit, branche ou pull request ignorés.
3. **Réutilisation vérifiée** (`scripts/release-build-info.ps1`) : téléchargement des artefacts
   de l'exécution de distribution retenue (`run-id`), puis refus si commit, version, liste
   de fichiers, SHA-256 ou fichier `.sha256` du ZIP ne correspondent pas. Signature minisign,
   `latest.json`, SHA-256 du Setup et brouillon restent identiques ; publication manuelle.
4. **Runner** : l'attente et la signature tournent sur `ubuntu-latest`, sans cache ni Rust
   compilé. `download-artifact` 8.0.2 est épinglé sur `9000827c…` (Node.js 24).

Le commit de `bump-version.ps1` modifie `Cargo.toml` et `Cargo.lock` : il déclenche toujours
les trois workflows. Un tag posé sur un commit qui ne les a pas déclenchés échoue avec un
message demandant de les relancer sur `main`.

## Caches

- Distribution sur `main` : hit observé (1,37 Go restaurés en ~33 s) ; un hit n'enregistre
  rien, donc pas de nouvel envoi. La version de l'application dans `Cargo.lock` ne change pas
  le suffixe des dépendances (`8d3a2d45` identique sur `main` et le tag v2.0.1).
- Release : le cache manqué (`No cache found`) n'a plus d'effet puisque la release ne compile plus.
  L'isolation Rustup reste utile pour stabiliser la clé du portable sur `main` face aux
  changements de Rust stable du runner ; son efficacité reste à constater après push.
- Tests natifs : hit observé (1,64 Go en 48 s). La clé suit `rustup update stable` et changera
  à chaque version stable ; changer de toolchain uniquement pour le cache est exclu.
- Aucun autre gain de cache justifié n'a été identifié : la compilation restante concerne le
  crate de l'application, jamais en cache, et les licences (~74 s) restent régénérées par contrôle.

## Gains

| Gain | Statut |
| --- | --- |
| Calcul Windows supprimé : tests portables + build de la release (11 min 04 d'étapes en v2.0.1) | Mesure de l'ancien parcours ; suppression effective à confirmer |
| Ajout au portable : Inno Setup 9,7 s, `build-info.json`, second envoi (~15 Mo) | Inno mesuré en v2.0.1 ; le reste est estimé |
| Release Linux : checkout, attente, téléchargement, signature, brouillon | Non mesuré ; occupe un runner pendant l'attente |
| Attente jusqu'au brouillon : environ la durée des tests natifs (15 min 39 en v2.0.1) + ~1 min, contre 11 min 55 | **Estimation** ; le brouillon arrive plus tard, mais déjà validé |

Le temps de calcul total baisse ; le temps jusqu'au brouillon augmente d'environ 4 minutes,
mais la publication manuelle devait de toute façon attendre la fin des tests.

## Validation locale

- `scripts/tests/release-reuse.tests.ps1` : 12 scénarios avec API GitHub simulée — attente
  d'exécutions en cours, re-run réussi remplaçant un échec, audit en échec, tests annulés,
  autre commit/branche/PR, délai dépassé ; fichiers conformes acceptés ; autre commit, autre
  version, Setup modifié, `.sha256` du ZIP périmé et `build-info.json` absent refusés.
- `actionlint` 1.7.12 sur les quatre workflows : aucune erreur.
- `build-distribution.ps1` complet en local sur `93df2fd`, comme l'étape du workflow : licences
  20,1 s, build Rust 136,7 s (cache local), Inno Setup 10,1 s, ZIP 0,9 s ; puis écriture et
  vérification réelles de `build-info.json` pour 2.0.1 réussies. Journal :
  `target/release-reuse-tests/local-distribution.log`. Durées locales, non comparables au runner.
- Non simulé localement : téléchargement inter-exécutions, droits `actions: read` du jeton,
  `cargo metadata --offline` et signature sur runner Linux.

## À confirmer sur GitHub

Après un push explicitement demandé : une exécution **Windows portable distribution** doit
publier les deux artefacts ; la release suivante doit attendre les trois workflows, télécharger
les fichiers de ce commit et produire un brouillon équivalent. Relever alors les durées
complètes, la durée d'attente et l'effet de l'isolation Rustup sur la clé du portable.
