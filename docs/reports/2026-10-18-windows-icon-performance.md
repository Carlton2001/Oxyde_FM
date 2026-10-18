# Comparaison des performances des icônes Windows — 18 octobre 2026

Mesures de l'étape avec persistance synchrone. Le [parcours différé](2026-10-18-deferred-icon-persistence.md) décrit le changement livré ensuite et ses mesures.

La suppression des décodages PNG redondants accélère les caches mesurés. Elle
n'apporte pas de gain stable à la première extraction d'un exécutable : le temps
médian augmente légèrement dans ce test. Les mesures ne couvrent pas le temps
d'affichage dans Oxyde.

## Résultats

Médianes de tous les échantillons, à poids égal pour trois exécutables et trois
passages indépendants. La variation indique `(nouveau / initial − 1) × 100` :
une valeur négative correspond à une réduction du temps.

| Parcours | Taille | Initial | Nouveau | Variation | Mesures par version |
| --- | --- | ---: | ---: | ---: | ---: |
| Extraction native + persistance, cache Oxyde vide | 32 px | 3,339 ms | 3,400 ms | +1,8 % | 270 |
| Extraction native + persistance, cache Oxyde vide | 96 px | 7,472 ms | 7,500 ms | +0,4 % | 270 |
| Cache disque du parent | 32 px | 55,3 µs | 44,8 µs | −19,0 % | 2 700 |
| Cache disque du parent | 96 px | 119,4 µs | 77,3 µs | −35,3 % | 2 700 |
| Cache mémoire du worker | 32 px | 9,9 µs | ≈0,2 µs | ≈−98,0 % | 27 000 |
| Cache mémoire du worker | 96 px | 40,6 µs | ≈0,7 µs | ≈−98,3 % | 27 000 |

Sur les médianes individuelles par exécutable, taille et passage :

- Cache disque 32 px : réduction de 17,8 à 19,8 %.
- Cache disque 96 px : réduction de 32,4 à 42,1 %.
- Cache mémoire du worker : réduction de 97,7 à 98,5 %.
- Extraction 32 px : de 0,8 % plus rapide à 6,4 % plus lente.
- Extraction 96 px : de 3,3 % plus rapide à 3,7 % plus lente.

## Protocole et référence

Référence initiale : commit `fcf724874a05fc4c58ab4f6f3093b9bafe13a645`.
Les sources de production de `src/platform/icons.rs` et
`src/platform/windows_icon_cache.rs` sont figées dans
`src/platform/icons/tests/baseline/`. Leur conformité au contenu Git a été
vérifiée, hors ajout du commentaire de provenance et retrait des blocs de tests.
Le code natif de conversion du bitmap est commun aux deux versions et n'a pas
été modifié par l'optimisation.

Le même binaire de tests compile les deux versions avec le même profil de
développement (`opt-level = 1`, dépendances `opt-level = 2`). Un seul thread de
test ; exécution Windows x64 hors du bac à sable pour les accès natifs au cache.

Corpus : `cmd.exe`, `reg.exe`, `WindowsPowerShell/v1.0/powershell.exe`, sous
`%SystemRoot%/System32`, chacun en 32 et 96 pixels.

Pour chaque exécutable, taille et passage :

- Quatre paires d'échauffement exclues, puis ordre initial/nouveau alterné.
- 30 paires avec les caches Oxyde vidés avant chaque version, hors chronométrage.
- 300 paires lisant le même PNG du cache disque, sans cache mémoire Oxyde.
- 3 000 paires avec le cache mémoire du worker déjà rempli.
- Égalité des pixels vérifiée après chaque paire, hors chronométrage.

Le cache Shell Windows et le cache de fichiers du système sont préchauffés.
« Cache Oxyde vide » ne signifie donc pas un démarrage à froid de Windows.
La phase d'extraction inclut le recadrage, le redimensionnement, la persistance
PNG et le décodage supplémentaire de l'ancienne version avant IPC.
La lecture disque initiale comprend ses deux décodages ; la nouvelle en comprend
un. Le cache mémoire initial du worker contient des PNG ; le nouveau conserve
les pixels. Le cache mémoire du parent, déjà constitué de pixels avant la
modification, n'est pas mesuré et n'est pas accéléré par ce changement.

## Limites et conclusion

Les mesures excluent le démarrage et le recyclage des processus, l'IPC, la
conversion BGRA, les files de chargement et le rendu GPUI. La file dédiée aux
icônes est couverte par un test fonctionnel de saturation, sans mesure de latence
en interface. Les durées RAM très courtes sont proches de la résolution du
chronomètre ; leur valeur absolue et leur facteur d'accélération sont indicatifs.

Le gain du cache disque est stable dans ce corpus. Le gain RAM du worker est
important, mais ne s'applique pas aux icônes déjà retenues dans le parent/GPUI.
Les petites variations d'extraction ne permettent pas de revendiquer un gain
à la découverte de nouveaux `.exe`. Ce résultat ne mesure pas le premier appel
Shell après lancement ni un dossier contenant des centaines d'exécutables.

## Reproduction et données

Le benchmark manuel est
`platform::icons::comparison_tests::compare_original_and_optimized_icons`.
Sa [référence](../../src/platform/icons/tests/baseline/README.md) documente le
code initial et le périmètre.

```powershell
$fixtureRoot = Join-Path (Get-Location) 'target/native-tests/icon-comparison'
New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
$env:TEMP = $fixtureRoot
$env:TMP = $fixtureRoot
$env:OXYDE_ICON_BENCH_REPORT = Join-Path $fixtureRoot 'run-1.json'
cargo test --offline --locked --lib platform::icons::comparison_tests::compare_original_and_optimized_icons -- --ignored --exact --test-threads=1 --nocapture
```

Les trois passages ont réussi. Les données brutes locales, en microsecondes,
sont conservées dans `target/native-tests/icon-comparison/run-{1,2,3}.json` ;
le tableau agrégé dans `target/native-tests/icon-comparison/summary.json`.
Ces artefacts de mesure sont ignorés par Git. `cargo check --offline --locked
--all-targets` passe sans avertissement du projet après ajout du benchmark.
