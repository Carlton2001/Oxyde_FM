# Icônes disponibles avant persistance — 18 octobre 2026

Le worker Shell renvoie désormais ses pixels sans encodage PNG ni écriture
disque. Après réception et validation de la réponse, le parent les met en cache
RAM puis propose une copie à une file de persistance dédiée. Le PNG conserve
les mêmes contrôles de publication, quota, synchronisation et renommage ; cette
étape ne retarde plus le retour des pixels.

## Mesures finales

Médianes de trois passages indépendants, sans compilation en parallèle, sur
`cmd.exe`, `reg.exe` et PowerShell. Chaque taille comporte 270 mesures par
version, après quatre paires d'échauffement par exécutable et passage.

| Taille | Avant : pixels disponibles après publication | Après : pixels disponibles dès admission | Réduction du délai | Après : publication terminée |
| --- | ---: | ---: | ---: | ---: |
| 32 px | 3,214 ms | 0,860 ms | 73,2 % | 3,211 ms |
| 96 px | 7,032 ms | 4,443 ms | 36,8 % | 7,070 ms |

Le travail disque reste effectué. Son coût est déplacé hors du délai de
disponibilité des pixels. Chaque publication différée a été attendue et le PNG
comparé aux pixels source hors du chronométrage de disponibilité. Aucun fichier
n'a été omis dans ces trois passages ; les images des deux versions sont
identiques.

## Protocole

Le parcours synchrone appelle l'extraction native avec son cache disque
synchrone. Le parcours différé appelle le point d'entrée réel du worker
`get_file_icon`, puis l'admission à la file du parent. Le contrôle de disponibilité
de la file est effectué avant chronométrage. Le test attend sa vidange après le
retour différé pour garantir que chaque fichier est publié avant la paire
suivante. Les caches applicatifs et le fichier PNG sont vidés avant chaque
version, hors chronométrage ; l'ordre des deux versions alterne.

Même binaire de tests, mêmes dépendances et profil de développement GPUI.
Windows Shell et cache de fichiers système préchauffés ; un seul thread de test.
La version différée inclut son encodage du chemin et sa clé de cache ; la clé
du helper synchrone est préparée avant chronométrage.

Le benchmark mesure le parcours natif et l'admission à la file, sans démarrage de
processus, IPC, insertion dans le cache RAM du parent ni rendu GPUI. Il ne mesure
pas l'affichage d'un dossier complet. Un premier passage pendant une compilation
a été écarté de l'agrégation finale ; les trois passages retenus utilisent les
fichiers `quiet-run-1.json`, `quiet-run-2.json` et `quiet-run-3.json`.

## Bornes et nettoyage

- Un seul thread d'écriture dans le parent ; 32 demandes et 4 Mio de pixels
  maximum en attente, plus une image active issue du protocole borné à 256 × 256.
- L'admission ne prend aucun verrou pendant l'encodage ou l'I/O. Si la file est
  pleine ou indisponible, l'image reste affichable, sans persistance garantie.
- Les accès RAM/disque existants ne sont pas republiés. Le worker conserve son
  cache de pixels et ses garanties GDI.
- Effacer le cache suspend les admissions, invalide les anciennes révisions,
  retire les demandes en attente et attend l'écriture active sur le thread de
  nettoyage. La suspension couvre ensuite la suppression des fichiers. Les
  réponses anciennes ne remplissent plus le cache RAM et ne programment aucune
  publication tardive.
- À la fermeture, la file refuse les demandes nouvelles et abandonne celles en
  attente. Elle attend au plus deux secondes la publication active avant la
  sortie native finale. Il s'agit d'un cache régénérable : les dernières images
  ne sont pas garanties sur disque à la fermeture.

## Vérifications

34 tests natifs ciblés passent : géométrie GDI, limites des caches, pixels et
transparence, jonctions, quotas, saturation de la file, nettoyage pendant une
écriture, révisions obsolètes, pauses imbriquées, panic du publisher et arrêt
borné. Le test natif de réutilisation de session passe également avec le binaire
`oxyde.exe` fraîchement construit : quatre réponses et le même processus
conservé, puis contrôle de son arrêt.

`cargo check --offline --locked --all-targets` passe sans avertissement du projet.
`cargo build --offline --locked --bins` a reconstruit l'application et ses trois
exécutables auxiliaires.

## Reproduire

```powershell
$fixtureRoot = Join-Path (Get-Location) 'target/native-tests/deferred-icons'
New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
$env:TEMP = $fixtureRoot
$env:TMP = $fixtureRoot
$env:OXYDE_ICON_BENCH_REPORT = Join-Path $fixtureRoot 'quiet-run-1.json'
cargo test --offline --locked --lib platform::icons::deferred_tests::compare_synchronous_and_deferred_persistence -- --ignored --exact --test-threads=1 --nocapture
```

Ce test nécessite les permissions natives d'accès au cache temporaire.
Mesures brutes et agrégation locales dans `target/native-tests/deferred-icons/`,
ignorées par Git. Source : `src/platform/icons/tests/deferred.rs`.

Les comparatifs [GPUI initial](2026-10-18-windows-icon-performance.md) et
[Tauri](2026-10-18-tauri-icon-performance.md) décrivent les étapes précédentes,
où la publication disque restait synchrone.
