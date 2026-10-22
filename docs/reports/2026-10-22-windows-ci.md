# Workflows Windows : mesures et optimisation du 22 octobre 2026

Les journaux authentifiés du commit GitHub `83befe3f` montrent que les constructions
dominent la CI native et l’installation de l’outil domine l’audit de dépendances.
Les modifications locales décrites ici partent de `6c38a7ed`. Aucun nouveau workflow
n’a été exécuté sur GitHub et aucun gain distant n’est encore mesuré.

## Référence GitHub du 21 octobre

| Workflow / phase | Durée observée |
| --- | --- |
| [Windows native tests](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39676783586/job/119488600155), workflow complet | 30 min 05 |
| Mise à jour de Rust stable | 22 s |
| `cargo check --all-targets` | 7 min 59 |
| `cargo build --bins` | 12 min 01 |
| Compilation de `cargo test --all-targets` | 5 min 34 |
| Exécution des tests, lancements compris | environ 3 min 37 |
| [Windows dependency security](https://github.com/Carlton2001/Oxyde_FM/actions/runs/39676783611/job/119488600481), workflow complet | 6 min 27 |
| Mise à jour de Rust stable | 22 s |
| Étape d’installation et d’audit | 5 min 48 |
| Installation de `cargo-audit` par compilation | 5 min 40 |

Les trois durées Cargo natives proviennent de leurs messages `Finished` ; elles
incluent leur préparation et les téléchargements éventuels, notamment dans le
premier `check`. Le temps d’exécution vient des horodatages après la compilation
des tests. La suite GitHub exécute 1 323 tests avec succès, sans échec, et en ignore
35. Le résultat du processus enfant portable ne s’ajoute pas une seconde fois au
total de son parent. Aucun cache Rust n’est restauré par ces deux anciens workflows.

L’ancien audit affiche son résultat environ six secondes après l’installation du
binaire ; ce délai inclut l’actualisation et le traitement du rapport, sans mesure
isolée de l’audit. Il ne signale aucune vulnérabilité et affiche huit avis de
maintenance à cette date.

## Changements

- La CI native restaure les dépendances et leurs compilations via `rust-cache`
  épinglé, sous une clé distincte des distributions. Seule la branche par défaut
  enregistre le cache. Rust stable courant, le profil dev, le check de toutes les
  cibles, la construction de tous les binaires et les tests en série sont conservés.
- L’audit utilise le ZIP Windows x64 officiel de `cargo-audit` 0.22.2, avec SHA-256
  fixés pour l’archive et le binaire. Il vérifie également un binaire local avant
  de le réutiliser. La mise à jour de Rust est retirée de ce workflow, qui ne compile
  plus l’outil. Aucun cache Actions supplémentaire n’est nécessaire pour l’audit.
- Les scripts affichent leurs phases chronométrées. L’audit conserve l’actualisation
  des avis et de l’index, les filtres Windows x86_64, `--deny yanked`, les messages
  de maintenance, le code d’échec et le rapport JSON conservé par le workflow.

## Validations locales

- Huit scénarios d’installation avec le vrai ZIP et le vrai exécutable officiels :
  installation dans un chemin avec espaces, réutilisation sans réseau, refus d’un
  mauvais hash d’archive, réparation d’un binaire corrompu, interruption sans
  écrasement de l’installation précédente, refus d’un mauvais hash de binaire,
  destination par défaut indépendante du répertoire courant et nettoyage temporaire.
- Audit réel avec téléchargement : installation 4,2 s, actualisation et audit
  5,7 s. Second passage : binaire vérifié réutilisé en moins de 0,1 s ;
  actualisation et audit 2,3 s. Ce sont des mesures locales uniques.
- Base actualisée à `640bf5a9f06e66d8721e8772766531fe77c8630d`, 1 349 avis :
  aucune vulnérabilité pour le verrou du projet et huit avis de maintenance visibles.
- Deux verrous isolés vérifient les échecs réels : `smallvec` 0.6.9 vulnérable et
  `futures-util` 0.3.30 retiré sans vulnérabilité signalée. Dans les deux cas, le
  script rend le code 1 et écrit le JSON.
- Le contrôle du script natif vérifie les trois commandes et leurs arguments,
  l’arrêt après échec de chaque phase et la restauration de TEMP/TMP et du répertoire.
- Suite native complète rejouée avec le script modifié dans une copie isolée des
  mêmes sources Rust, sous Rust 1.98.1 : 1 338 réussites, aucun échec, 35 ignorés.
  Les quinze tests supplémentaires par rapport à l’ancien run GitHub sont déjà
  dans les sources locales antérieures à ce changement. Le check de toutes les
  cibles reste sans avertissement. Avec les compilations locales intégralement
  présentes : check 2,4 s, binaires 0,9 s, compilation et exécution des tests 192,0 s.
  Ce passage ne simule pas un transfert de cache GitHub ni une reconstruction
  limitée aux seules dépendances.
- Analyse syntaxique PowerShell, `actionlint` 1.7.12 et `git diff --check` réussis.

Les traces locales restent sous `target/ci-workflow-tests/`, hors Git.

## Mesure restante

Après mise à disposition sur GitHub, exécuter la CI native sur la branche par
défaut pour préparer le cache, puis une seconde fois sur le même commit. Comparer
les restaurations, la compression et le transfert, les trois phases Cargo et le
temps total. Une modification de code seule doit réutiliser les dépendances, mais
une nouvelle version de Rust stable ou un environnement différent peut les invalider.
Vérifier aussi le téléchargement officiel et le rapport de sécurité sur un runner
neuf. Ces optimisations ne changent pas le protocole de publication des releases.
