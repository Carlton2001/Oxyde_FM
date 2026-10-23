# Distribution : clé de cache et actions Node.js — 23 octobre 2026

La release v2.0.1 réussit en **11 min 55**, contre 33 min 20 pour v2.0.0, mais
sans restaurer le cache. La correction locale isole Rustup pour éviter qu'une
toolchain inutilisée du runner change la clé. Elle doit encore être exécutée sur GitHub.

## Mesures hébergées

Sources : commit `167dba72a004b3091e78ec2a56c072d19467a488`, exécutions du 23 octobre
en heure de Paris. Les durées complètes incluent l'orchestration du workflow.

| Workflow | Référence précédente | Nouvelle durée complète |
| --- | --- | --- |
| [Release v2.0.1](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079017) | 33 min 20, v2.0.0 | 11 min 55 |
| [Tests natifs](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079033) | 30 min 05, avant cache | 15 min 39 |
| [Sécurité](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079123) | 6 min 27, outil compilé | 27 s |
| [Portable](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39873079036) | 18 min 25, première préparation | 8 min 49 |

Ces quatre exécutions réussissent. Les sources et l'état des caches diffèrent des
références ; il ne s'agit pas d'une comparaison répétée sur un même commit.
Le cache natif de 1 644 343 405 octets est restauré dans une étape de 48 s.
Le portable restaure 1 374 205 385 octets, entre le hit à 23:38:35 UTC et la fin de
restauration à 23:39:08 UTC. La release annonce `No cache found`.

Dans la release : `cargo-about` est installé en 0,9 s, préparation/construction
des tests portables en 7 min 11, puis les six scénarios passent en 0,16 s.
Les phases suivantes annoncent licences 74,4 s, construction Rust 146,5 s,
Inno Setup 9,7 s et création/vérification du ZIP 0,7 s.

## Cause et correction

Les journaux montrent Rust 1.98.1 sélectionné dans les deux distributions, mais
une autre version installée de 1.99.0 dans la release contre 1.99.1 dans le portable.
L'action [rust-cache épinglée](https://github.com/Swatinem/rust-cache/blob/d1527c8f67ef9af357290475d4af4b0fab8f12e0/src/config.ts#L357)
inclut toutes les toolchains installées dans sa clé. Le suffixe des dépendances
reste `8d3a2d45`, mais la partie environnement diffère : `62e1f3fb` contre `5077c390`.

L'action composite installe désormais `1.98.1-x86_64-pc-windows-msvc` dans un
`RUSTUP_HOME` dédié sous `RUNNER_TOOL_CACHE`, avec un chemin stable par version/cible.
Elle refuse un inventaire contenant une autre toolchain, puis exporte ce répertoire
et la toolchain pour les étapes suivantes. Le Rustup préinstallé n'est ni supprimé
ni modifié. Les manifestes, le verrou, la configuration et le compilateur choisi
continuent de participer à la clé ; aucun contrôle d'environnement n'est désactivé.
La règle d'enregistrement sur la branche par défaut reste présente.

## Avertissement Node.js 20

L'avertissement du job `rustsec` vient de `actions/upload-artifact` 4.6.2.
Ses deux usages, sécurité et portable, passent à la [release officielle 7.0.2](https://github.com/actions/upload-artifact/releases/tag/v7.0.2),
commit `cf430e030ddbb5b0abf93d22962f4752f3646cd9`. Son `action.yml` utilise
`node24`. Les paramètres d'envoi restent identiques et l'archivage reste activé
par défaut. Checkout et rust-cache épinglés utilisent également Node.js 24.

## Validation locale et suite

- Le test lit le bloc PowerShell réel de l'action : les défauts 1.99.0/1.99.1 du
  runner produisent le même environnement isolé. Installation en erreur, inventaire
  en erreur, toolchain étrangère ou répertoire du runner absent arrêtent le setup.
- Une installation réelle de Rust 1.98.1 dans un dossier de test avec espaces,
  l'appel à Rust/Cargo et une seconde initialisation réussissent. Le sysroot est
  bien sous le répertoire isolé et l'inventaire Rustup du poste reste identique.
  Un programme autonome sans dépendance est également compilé et exécuté en x64.
  Le premier essai utilisait un chemin de fixture trop long pour MSVC (321 caractères
  pour une bibliothèque) ; un chemin court, comme sur le runner, passe. Le mode
  `-RealToolchain` utilise désormais un répertoire court sous `target/rt <identifiant>`.
- Le calcul `getRustVersions` du code officiel épinglé est rejoué localement :
  l'ancien inventaire reproduit deux clés différentes ; le vrai inventaire isolé
  produit la même empreinte dans les deux cas. Un autre compilateur sélectionné
  change encore l'empreinte. Ce test ne simule pas le service de cache GitHub.
- Les quatre workflows passent `actionlint` 1.7.12. L'analyse syntaxique du test
  et des deux blocs PowerShell de l'action, ainsi que `git diff --check`, réussissent.
- Les métadonnées officielles des trois actions épinglées confirment `node24`.
  Les options d'envoi utilisées existent dans upload-artifact 7.0.2 et l'archivage
  reste activé par défaut. Aucun envoi d'artefact n'est simulé en local.

Les fixtures et traces sont sous `target/distribution-cache-regression/` et
`target/distribution-setup-tests/` ; les toolchains réelles sont sous `target/rt *`,
hors Git. Après publication des changements,
préparer la nouvelle clé sur `main` avec **Windows portable distribution** avant
un tag. La première préparation reconstruira les dépendances ; aucun nouveau
temps de release n'est prédit. Aucun push ou déclenchement distant n'est effectué ici.
