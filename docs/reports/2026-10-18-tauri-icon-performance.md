# Icônes Windows : méthode Tauri contre méthode GPUI — 18 octobre 2026

Mesures de l'étape avec persistance GPUI synchrone. Le [parcours différé](2026-10-18-deferred-icon-persistence.md) décrit le changement livré ensuite et ses mesures.

Le parcours Tauri est plus rapide à cache applicatif vide quand chaque méthode
utilise son écriture disque propre. L'extraction seule est proche. L'écart
principal se réduit fortement lorsque Tauri utilise la publication disque de
GPUI. Sur les lectures de cache mesurées, GPUI est plus rapide.

## Médianes agrégées

Trois passages indépendants, trois exécutables, deux tailles. Chaque version
contribue le même nombre d'échantillons par exécutable. Une variation positive
signifie que le temps GPUI est supérieur au temps Tauri.

| Parcours | Taille | Tauri | GPUI | Variation GPUI / Tauri | Mesures par version |
| --- | --- | ---: | ---: | ---: | ---: |
| Extraction seule, sans disque, sortie PNG identique | 32 px | 0,748 ms | 0,740 ms | −1,1 % | 270 |
| Extraction seule, sans disque, sortie PNG identique | 96 px | 4,303 ms | 4,432 ms | +3,0 % | 270 |
| Cache applicatif vide, persistance propre à chaque méthode | 32 px | 1,140 ms | 3,573 ms | +213,4 % | 270 |
| Cache applicatif vide, persistance propre à chaque méthode | 96 px | 4,688 ms | 7,455 ms | +59,0 % | 270 |
| Cache applicatif vide, même publication disque GPUI | 32 px | 2,892 ms | 3,001 ms | +3,8 % | 270 |
| Cache applicatif vide, même publication disque GPUI | 96 px | 6,647 ms | 6,910 ms | +4,0 % | 270 |
| Cache disque du backend Tauri / du parent GPUI | 32 px | 319,7 µs | 45,6 µs | −85,7 % | 2 700 |
| Cache disque du backend Tauri / du parent GPUI | 96 px | 341,1 µs | 88,2 µs | −74,1 % | 2 700 |
| Cache mémoire du backend Tauri / du worker GPUI | 32 px | 242,5 µs | ≈0,3 µs | ≈−99,9 % | 9 000 |
| Cache mémoire du backend Tauri / du worker GPUI | 96 px | 256,6 µs | ≈1,0 µs | ≈−99,6 % | 9 000 |

Les sorties ont les mêmes dimensions et les mêmes pixels sur toutes les paires,
y compris les échauffements. Aucun résultat Tauri vide ni différence de canal
n'a été observé sur le corpus.

## Source Tauri et adaptations

Référence : projet local `Biluf_FM_Clean`, fichier
`src-tauri/src/commands/icons.rs`, révision
`54b8b4c2586908c29d0e60af0c4ec8d7c5c49dd4`. Le fichier était sans modification
locale. SHA-256 du fichier original complet :
`3d3800a27412f351da198ba9f611f235272214580b87b737074add5d10861a37`.

La copie figée est dans `src/platform/icons/tests/tauri/source.rs`. Seuls les
imports liés au point d'entrée Tauri, ses attributs de commande et la fonction
recevant `AppHandle` ont été retirés. L'extraction, le GDI, le recadrage, les
filtres, le PNG et le cache sont ceux du projet Tauri. La conformité de la copie
a été vérifiée après ces seules adaptations. Le projet Tauri n'a pas été modifié.

Les deux méthodes tournent dans le même binaire, avec le profil de développement
GPUI (`opt-level = 1`, dépendances `opt-level = 2`) et ses dépendances, notamment
les bindings Windows et `image`. Cela compare les méthodes à compilation égale,
pas les performances des applications complètes ni de leurs builds historiques.

## Protocole

Corpus identique au [premier comparatif](2026-10-18-windows-icon-performance.md) :
`cmd.exe`, `reg.exe`, `WindowsPowerShell/v1.0/powershell.exe`, sous
`%SystemRoot%/System32`, en 32 et 96 pixels.

Un seul thread de test, COM initialisé en MTA, cache Shell Windows et cache de
fichiers du système préchauffés. Exécution native hors du bac à sable pour les
opérations de cache. Pour chaque exécutable, taille et passage :

- Quatre paires d'échauffement exclues ; ordre Tauri/GPUI alterné ensuite.
- 30 paires pour chacune des trois phases d'extraction/persistance.
- 300 paires pour les lectures disque, caches mémoire applicatifs vidés avant
  chaque version.
- 1 000 paires pour les accès mémoire, caches applicatifs déjà remplis.
- Vidage des caches et retrait des fichiers avant chaque version hors du
  chronométrage. Comparaison des pixels également hors chronométrage.

L'extraction seule produit un PNG dans les deux versions ; GPUI effectue donc
explicitement cet encodage pour rendre les sorties comparables. Le parcours
Tauri sans disque conserve son insertion native en cache mémoire ; GPUI appelle
son générateur sans passer par le cache applicatif.

Dans les autres phases, le backend Tauri retourne ses octets PNG et GPUI ses
pixels, comme dans leur parcours réel. Le décodage PNG Tauri utilisé uniquement
pour comparer les pixels est hors chronométrage ; son décodage WebView réel n'est
donc pas inclus. GPUI inclut son décodage lors d'une lecture disque.

La phase avec politique identique conserve l'extraction/PNG Tauri mais publie
ensuite ces octets avec `Platform::write_preview_cache`, comme GPUI, au lieu de
l'écriture directe Tauri. Les fichiers de ce test sont temporaires.

## Ce qui explique les écarts

Les deux extracteurs utilisent `SHGetFileInfoW`, `SHGetImageList` et
`IImageList::GetIcon`, avec les mêmes tailles, recadrage, CatmullRom et Lanczos3.
La conversion GDI diffère : Tauri utilise notamment un DC d'écran et une gestion
manuelle des ressources ; GPUI possède une conversion bornée, initialise le DIB
et synchronise le dessin avant lecture des pixels.

Tauri crée le répertoire et écrit le PNG directement avec `std::fs::write`.
GPUI publie via des handles contrôlés, un fichier temporaire, une synchronisation
disque, l'admission au quota et un renommage. Les médianes à cache vide sont donc
environ **3,13 fois plus élevées pour GPUI en 32 px** et **1,59 fois en 96 px**
avec leurs politiques respectives. L'écart tombe à environ 4 % avec la même
publication. Cette expérience pointe surtout le coût de persistance ; elle
n'isole pas individuellement la synchronisation, les contrôles et le quota.

Tauri appelle le Shell avant de consulter son cache, même sur un accès RAM.
GPUI peut lire son cache directement. Le résultat favorise donc GPUI sur les
caches du backend mesurés, y compris lorsque le PNG Tauri n'est pas décodé dans
le temps relevé.

L'extraction seule ne montre pas de gain stable de Tauri sur tous les fichiers :
les écarts individuels varient avec le cas et le passage. Le ressenti de rapidité
Tauri est compatible avec le parcours initial plus court et sa publication
disque plus légère, plutôt qu'avec une API Shell différente.

## Limites et reproduction

Pas de mesure du démarrage des processus, des sessions IPC, de la concurrence,
des files, des caches d'interface, du rendu GPUI ni du WebView. L'accès mémoire
mesuré n'est pas le cache d'images déjà affichées dans l'interface. Les durées
RAM GPUI très courtes sont approximatives à cette résolution de chronométrage.
Le premier appel Shell après lancement de Windows et les dossiers massifs
d'exécutables ne sont pas couverts. Aucun changement du parcours de production
n'a été effectué pour ce comparatif.

```powershell
$fixtureRoot = Join-Path (Get-Location) 'target/native-tests/tauri-icon-comparison'
New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
$env:TEMP = $fixtureRoot
$env:TMP = $fixtureRoot
$env:OXYDE_ICON_BENCH_REPORT = Join-Path $fixtureRoot 'run-1.json'
cargo test --offline --locked --lib platform::icons::tauri_comparison_tests::compare_tauri_and_gpui_icons -- --ignored --exact --test-threads=1 --nocapture
```

Les trois passages ont réussi. Données brutes locales dans
`target/native-tests/tauri-icon-comparison/run-{1,2,3}.json`, agrégation dans
`summary.json`, ignorées par Git. Le [README de la référence Tauri](../../src/platform/icons/tests/tauri/README.md)
détaille les adaptations. `cargo check --offline --locked --all-targets` passe
sans avertissement du projet avec le nouveau benchmark.
