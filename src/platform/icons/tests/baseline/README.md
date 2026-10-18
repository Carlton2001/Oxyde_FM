# Référence du benchmark des icônes

`icons.rs` et `cache.rs` sont les sources de production exactes du commit
`fcf724874a05fc4c58ab4f6f3093b9bafe13a645`, respectivement
`src/platform/icons.rs` et `src/platform/windows_icon_cache.rs`.
Seuls le commentaire de provenance initial et le retrait des blocs de tests
diffèrent. Garder ces fichiers figés pour préserver la comparaison.

`mod.rs` adapte leur résultat au parcours initial : décodage du PNG avant IPC,
ou second décodage après validation dans le cache disque du parent.
La conversion native de bitmap, le recadrage et le redimensionnement n'ont pas
changé entre les versions.

Lancer manuellement sous Windows, avec un TEMP/TMP privé et les permissions
natives de création des fichiers temporaires :

```powershell
cargo test --offline --locked --lib platform::icons::comparison_tests::compare_original_and_optimized_icons -- --ignored --exact --test-threads=1 --nocapture
```

`OXYDE_ICON_BENCH_REPORT` permet de choisir le fichier JSON des mesures brutes.
Par défaut : `target/native-tests/icon-comparison.json`.

Chaque cas alterne l'ordre des deux versions et écarte quatre échauffements.
Le cache Windows est chaud. Les caches Oxyde sont vidés avant chaque mesure
d'extraction, en dehors du chronométrage. Les lectures du cache disque utilisent
le cache système chaud. La mesure RAM porte sur le worker ; le cache RAM du
parent n'a pas changé. Les deux versions doivent produire les mêmes pixels.
Le benchmark n'inclut ni démarrage du processus, ni IPC, ni rendu GPUI.
