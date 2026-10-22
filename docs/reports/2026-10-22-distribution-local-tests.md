# Distribution Windows — validation locale du 22 octobre 2026

Sur ce poste, le cumul **tests portables + construction Rust** passe de **8 min 12 s
à 4 min 08 s** avec des dossiers de compilation vides. En conservant seulement les
dépendances compilées, il passe de **2 min 47 s à 2 min 09 s**. Ces résultats locaux
ne prédisent pas la durée d'une release GitHub.

## Conditions et preuves

- Sources Rust de `f5b09bac`, avec l'outillage local de distribution ; copie isolée
  de 889 fichiers, inventaire SHA-256 enregistré avant les essais.
- Windows 11 Professionnel, Intel Core i9-13900HX, 32 processeurs logiques,
  63,7 Gio de RAM ; Rust/Cargo 1.98.1, compilateur `48a229cea`, cible MSVC x64.
- Commandes séquentielles, mêmes sources et compilateur, `--offline --locked`.
  Les sources Cargo étaient déjà présentes ; aucun téléchargement de dépendances
  n'est mesuré. Le cache disque de Windows n'est pas purgé.
- Une mesure par cas, avec le temps du processus Cargo, tests compris. Le parcours
  ancien est mesuré avant l'optimisé ; ce n'est pas un benchmark statistique.
- Preuves locales non versionnées : `target/local-distribution-tests/20261022-113233/`
  (`results.json`, journaux par commande, `source-hashes.json`, `hardware.json`).
  Le `target` de développement et ses exécutables existants sont conservés.

L'ancien parcours utilise `cargo test --lib app_paths::tests`, puis
`cargo build --release --target x86_64-pc-windows-msvc --bins`. L'optimisé utilise
les tests en `--release --target x86_64-pc-windows-msvc`, puis sélectionne seulement
`oxyde` et ses trois workers avec quatre options `--bin`.

## Mesures Rust

| Cache de compilation | Ancien : tests / build | Optimisé : tests / build | Cumul ancien / optimisé |
| --- | --- | --- | --- |
| Vide | 304,805 / 186,856 s | 173,749 / 73,778 s | 491,661 / 247,527 s |
| Intégral, sources inchangées | 3,881 / 0,809 s | 3,536 / 0,886 s | 4,690 / 4,422 s |
| Dépendances seulement | 92,201 / 74,815 s | 55,807 / 73,154 s | 167,016 / 128,961 s |

À cache vide, la seconde commande produit 418 lignes Cargo `Compiling` dans
l'ancien parcours, contre une seule (`oxyde`) dans l'optimisé. Les six tests passent
dans les six exécutions filtrées, dont le scénario démarrant un processus enfant.
L'exécution des tests reste de l'ordre de trois secondes ici ; ces temps incluent
notamment la copie et le lancement de l'exécutable enfant et diffèrent des 0,26 s
de la [référence hébergée](2026-10-22-windows-distribution.md).

Le cas « dépendances seulement » supprime les artefacts du seul paquet `oxyde`
avec `cargo clean -p oxyde` dans chaque dossier isolé, pour les profils/cibles concernés,
puis reconstruit. Les dépendances sont conservées. Il approche le principe du cache
de distribution, sans reproduire le nettoyage exact de Swatinem, son archive ou son
service de stockage. Les temps de nettoyage (moins d'une seconde par commande)
sont exclus du tableau.

Un premier essai sous bac à sable a échoué sur un remplacement d'archive native
MSVC (`LNK1114`, accès refusé). Il est exclu des mesures ; les dossiers mesurés
ont été créés neufs pour les reprises hors du bac à sable.

## Outillage et contrôles de distribution

Les onze contrôles de `scripts/tests/install-cargo-about.tests.ps1` passent :
installation réelle du binaire de fixture, version, cache sans réseau, refus d'un
hash d'archive incorrect, réparation d'un cache corrompu, deux emplacements par
défaut, restauration sans métadonnées Cargo dans un chemin avec espaces,
téléchargement interrompu préservant l'ancien fichier, extraction en erreur,
binaire extrait altéré et nettoyage après échec. Un téléchargement réseau réel
supplémentaire prend environ 1 s ; sa réutilisation est inférieure à 0,1 s ici.

Les contrôles locaux de packaging refusent aussi une génération de licences en
erreur ou vide avant toute compilation/assemblage, refusent un compilateur Inno
absent, exigent un fichier de signature pour le manifeste et vérifient ses trois
clés Windows, ses métadonnées et sa date. Les fixtures restent dans le dossier de preuves.

Le packaging complet est exécuté avec les quatre nouveaux binaires, sans `-SkipBuild`.
Le compilateur [Inno Setup officiel 6.7.1](https://github.com/jrsoftware/issrc/releases/tag/is-6_7_1)
est préparé dans les fixtures avec son mode portable, sans raccourcis ni désinstalleur ;
l'empreinte de téléchargement et la signature Authenticode sont vérifiées. Dans cet
essai à compilation déjà chaude : licences 20,1 s, Rust 0,9 s, Setup 8,8 s, ZIP 0,8 s.
Les 16 entrées du ZIP sont comparées au payload par SHA-256, en plus des contrôles
du script de distribution et de l'empreinte du ZIP.

Le CLI Tauri 2.10.1 utilisé par le workflow signe le nouveau Setup avec une **clé
jetable**. Le vérificateur `minisign-verify` 0.2.5 accepte le fichier original et
refuse une copie altérée d'un octet. Le manifeste réel issu de cette signature est
vérifié pour les trois clés Windows. La clé privée et son journal de génération sont
ensuite supprimés. Ces artefacts de test ne doivent pas être publiés : leur signature
n'utilise pas la clé de l'updater Oxyde. La signature publiée du Setup Tauri 1.2.1 est
également vérifiée par le test Rust existant, lancé explicitement malgré son attribut
`ignore` : un test découvert, un succès.

## Suite native et paquet déplacé

`scripts/test-windows.ps1 -Offline` passe : `cargo check --all-targets`, construction
de tous les binaires puis tests de toutes les cibles en série, avec TEMP/TMP privés.
Le journal ne contient aucun avertissement. Résultat : **1 338 succès, zéro échec,
35 tests ignorés** (883 succès dans la bibliothèque, 391 dans le binaire principal,
64 dans les intégrations). Les cibles sans test sont compilées, sans être comptées
comme des scénarios exécutés. `native-test-summary.json` conserve le bilan par cible,
sans recompter le résultat d'un processus enfant affiché dans le journal du parent.

Quatre tests PDF/icônes normalement ignorés passent ensuite avec l'exécutable du
ZIP extrait dans un nouveau dossier contenant des espaces. Ils contrôlent réutilisation
du processus, rendu, limites, documents invalides et arrêt des sessions, avec des
fixtures générées. Avec le test de signature publiée, cinq des 35 tests ignorés
sont donc exécutés séparément avec succès ; les 30 autres restent non exécutés.

Le premier lancement de la suite native avait été interrompu avant les tests par
un relais PowerShell du harnais retirant le séparateur `--`. La validation finale
exécute directement le script du dépôt ; les journaux de cet essai rejeté sont conservés
séparément. Actionlint 1.7.12, l'analyse syntaxique PowerShell et `git diff --check`
passent également. Les sources Rust et manifests du poste correspondent encore aux
empreintes prises avant la copie isolée.

## Limites

Les mesures Rust excluent licences, installation de Rust, transfert du cache,
Setup, ZIP et signature. Les mécanismes de cache entre `main` et tags, leur coût
réseau et les durées du runner nécessitent encore les deux exécutions GitHub
décrites dans le [guide de distribution](../guides/distribution.md).
L'installation/désinstallation d'Oxyde, les parcours UI interactifs, les tests ignorés
restants et Linux ne sont pas validés. Aucun push, tag, release ou installation
d'Oxyde n'est effectué par ces essais.
