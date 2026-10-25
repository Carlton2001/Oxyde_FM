# Workflows : contrôles uniques et release sur tag — 25 octobre 2026

Le tag `v<x.y.z>` exécute en une seule fois tests natifs, audit et distribution de son commit,
puis signe **les fichiers de cette exécution** et crée le brouillon. Le commit de version poussé
sur `main` ne répète pas ces contrôles. Cela traite A09. **Aucune exécution hébergée n'a encore
validé ce parcours** : les gains sont des mesures de l'ancien parcours ou des estimations signalées.

Une première version, sur `main` en `3add8f6`…`1e2f7b4`, faisait attendre la release en
interrogeant l'API pour les exécutions de `main`. Elle est remplacée sur la branche
`ci/tag-release` : le tag local reste la référence de l'utilisateur, et un seul workflow avec
`needs` évite polling, délai de grâce et couplage par noms de fichiers.

## Mesures hébergées disponibles

Dernières exécutions publiques, toutes sur `167dba72` (Release v2.0.1, 23 octobre) ;
les commits locaux suivants, dont l'isolation Rustup, ne sont pas poussés.

| Workflow | Durée | Étapes principales |
| --- | --- | --- |
| [Tests natifs](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079033) | 15 min 39 | cache 48 s (hit), suite 14 min 00 |
| [Portable](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079036) | 8 min 49 | setup + cache 50 s (hit), tests portables 3 min 19, licences + build + ZIP 4 min 20 |
| [Release](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079017) | 11 min 55 | setup 15 s (pas de cache), tests portables 7 min 12, build 3 min 52, signature 12 s |
| [Sécurité](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079123) | 27 s | — |

Durées d'étapes lues dans l'API publique des jobs. Pour un même commit, la release refaisait
11 min 04 d'étapes Windows du portable, tandis que tests, audit et portable tournaient aussi sur `main`.

## Organisation

| Fichier | Rôle |
| --- | --- |
| `_tests.yml`, `_audit.yml`, `_distribution.yml` | Contrôles réutilisables, définis une fois |
| `_release-commit.yml` + `scripts/detect-release-commit.ps1` | Reconnaît le commit `Release v<x.y.z>` dont la version change |
| `tests.yml`, `security.yml` | `main` et PR selon les chemins ; audit hebdomadaire conservé |
| `distribution.yml` | `main` seulement si `Cargo.toml`, `Cargo.lock`, `.cargo/` ou l'action de toolchain changent ; manuel sinon |
| `release.yml` | Tag : version, tests, audit, distribution en parallèle ; `release` dépend des quatre |

Conservés : suite native avec construction explicite des workers, contrôles de licences, tests
portables, packaging, signature minisign, `latest.json`, brouillon publié à la main.
Ajouts : `concurrency` (annulation seulement sur les PR), environment `release` pour les secrets.

## Les deux cas

- **Push normal de `main`** : seuls les contrôles concernés tournent. La distribution prépare le
  cache que les tags peuvent restaurer ; elle ne tourne plus pour un simple changement de script.
- **`git push origin main vX.Y.Z`** : sur `main`, une détection Linux de quelques secondes ignore les
  contrôles du commit de version. Le tag teste, audite, construit, puis signe et crée le brouillon.
  L'ordre des deux pushes est indifférent.

## Caches

- Distribution : hit observé sur `main` (1,37 Go, ~33 s) ; un hit n'enregistre rien. Le changement
  de version dans `Cargo.lock` ne modifie pas le suffixe des dépendances (`8d3a2d45` sur `main` et
  sur le tag v2.0.1). Le tag restaure ce cache sans l'enregistrer.
- Le `No cache found` de la release v2.0.1 venait de la clé d'environnement ; l'isolation Rustup
  doit le corriger, **non mesuré** tant que ces commits ne sont pas poussés.
- Tests natifs : hit observé (1,64 Go, 48 s), restauré aussi par le tag.
- Aucun autre gain de cache justifié : la compilation restante concerne le crate de l'application
  et les licences (~74 s) restent régénérées par contrôle.

## Gains

| Gain | Statut |
| --- | --- |
| Release : 11 min 04 de build Windows en double supprimés | Mesure de l'ancien parcours ; à confirmer |
| Commit de version : tests, audit et portable sur `main` plus exécutés (≈ 15 min 39 + 27 s + 8 min 49 de runners en v2.0.1) | Mesures de l'ancien parcours ; à confirmer |
| Ajout au tag : Inno Setup (~10 s) et détection Linux sur `main` | Inno mesuré ; détection estimée à quelques secondes |
| Temps jusqu'au brouillon : environ la durée des tests natifs + ~1 min (contre 11 min 55) | **Estimation** ; brouillon plus tardif mais validé |

## Validation locale

- `actionlint` 1.7.12 sur les huit workflows : aucune erreur.
- `scripts/tests/detect-release-commit.tests.ps1` : 6 scénarios réussis (commit racine, commit de
  version, sujet sans changement de version, version changée avec un autre sujet, sujet incohérent,
  clone de profondeur 2). Le script reconnaît aussi `167dba7` et `83befe3f` (v2.0.1, v2.0.0) et
  pas `a1b9918c`.
- `build-distribution.ps1` complet en local sur `93df2fd` : licences 20,1 s, build Rust 136,7 s
  (cache local), Inno Setup 10,1 s, ZIP 0,9 s. Le build réutilisable exécute ce même script.
- Non simulé localement : appel des workflows réutilisables, `needs` et conditions sur GitHub,
  téléchargement des artefacts, environment `release`, `cargo metadata --offline` sur Linux.

## À confirmer sur GitHub

Après un push explicitement demandé : un push de `main` doit lancer les contrôles concernés ;
le commit de version doit les ignorer ; le tag doit produire un brouillon équivalent aux précédents.
Relever alors les durées complètes, le temps jusqu'au brouillon et la restauration des caches.
