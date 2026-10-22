# Temps de distribution Windows — 22 octobre 2026

La release GPUI v2.0.0 a duré **33 min 20 s**. Les six tests portables eux-mêmes
prennent **0,26 s** ; l'essentiel du temps est consacré à leur préparation/construction
et à une seconde construction en release. Aucun gain des optimisations locales n'est
encore mesuré sur GitHub.

## Référence hébergée

Source : [Release v2.0.0, exécution 39676783523](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39676783523),
[job release](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39676783523/job/119488600579).
Commit `83befe3f6133e5989cc1440dcd803f0b63399c02`, tag `v2.0.0`, 21 octobre 2026.
Logs détaillés consultés après authentification le 22 octobre. Runner Windows Server 2025,
image `windows-2025-vs2026` / `20261007.264.1`, Rust stable 1.99.1.

| Mesure | Durée | Portée |
| --- | --- | --- |
| Exécution complète | 33 min 20 s | Durée GitHub, job de 33 min 17 s |
| Installation de cargo-about 0.9.2 | 3 min 34 s | Étape complète ; Cargo annonce 3 min 33 s, téléchargements compris |
| Tests du stockage portable | 17 min 10 s | Étape complète |
| Préparation/construction des tests | 17 min 08 s | `Finished test profile`, dépendances et téléchargements compris |
| Exécution des six tests | 0,26 s | `test result: ok. 6 passed`, 906 autres tests filtrés |
| Construction et packaging | 11 min 26 s | Licences, Rust, Setup et ZIP ensemble |
| Construction Cargo release | 9 min 13 s | `Finished release profile` dans cette étape |
| Inno Setup | 15,078 s | Durée annoncée par son compilateur |
| Signature et manifeste | 17 s | Étape complète, téléchargement du CLI inclus |

Repères UTC du job :

- `cargo test` démarre à `21:16:01.478` ; premier `Compiling` à `21:18:20.201` ;
  `Finished test profile` à `21:33:10.568` ; résultat des tests à `21:33:11.047`.
- `build-distribution.ps1` démarre à `21:33:11.208` ; premier `Compiling` à
  `21:35:07.771` ; `Finished release profile` à `21:44:20.509`.
- Inno Setup annonce sa fin à `21:44:36.785` ; le ZIP est annoncé à `21:44:37.915`.

L'intervalle d'environ 1 min 56 s avant le premier `Compiling` de la distribution
inclut licences et orchestration ; il ne constitue pas une mesure isolée des licences.
De même, les lignes `Compiling` ne permettent pas d'isoler le seul temps CPU du compilateur.
Le ZIP n'a pas encore de chronométrage propre dans cette révision.

## Changement évalué et suite

Le commit local `0945330e` ajoute le cache partagé, fixe Rust 1.98.1, aligne profil
et cible des tests avec les exécutables et sélectionne les quatre binaires livrés.
La suite remplace la compilation de cargo-about par son binaire officiel 0.9.2,
avec empreintes fixées de l'archive et de l'exécutable, contrôle de version et nouvelle
clé de cache `cargo-about-0.9.2-prebuilt-v2`.

La [validation locale complémentaire](2026-10-22-distribution-local-tests.md) compare
les parcours Rust avec trois états du cache, vérifie l'installateur de cargo-about,
le packaging complet et la signature avec une clé jetable. Elle ne mesure pas le
service de cache ni la durée des workflows GitHub. Le chemin du générateur officiel
est imposé dans le harnais de packaging : Cargo peut autrement sélectionner un ancien
générateur déjà installé sur le poste.

Le protocole hébergé et la maintenance des versions/empreintes sont dans le
[guide de distribution](../guides/distribution.md). Au constat initial de ce travail,
`main` distant pointe encore sur `83befe3f` : les nouvelles mesures attendent la publication
des changements et deux exécutions sur le même commit. Aucun push, tag ou release n'est
effectué par ce travail. La refactorisation des tests reste différée jusqu'aux mesures
avec cache ; les six scénarios, dont le processus enfant, restent inchangés.
