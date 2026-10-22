# Mémoire du Viewer et des aperçus

## Capturer une session

Fermer Oxyde, puis reconstruire les exécutables, y compris les workers :

```powershell
cargo build --offline --bins
./scripts/diagnose-viewer-memory.ps1
```

Le lanceur démarre `target/debug/oxyde.exe` avec
`OXYDE_MEMORY_DIAGNOSTICS=1` uniquement dans son environnement. Équivalent :

```powershell
$env:OXYDE_MEMORY_DIAGNOSTICS = '1'
cargo run --offline
```

Dans une session neuve : attendre quelques secondes, parcourir les mêmes images,
fermer le Viewer, attendre 30 secondes, puis répéter le parcours. Garder le même
état des aperçus de tooltips et de la grille entre les passages.

Les lignes `memory-sample:` vont dans `oxyde.log`, à côté de `config.json`
(`%APPDATA%/Oxyde` habituellement, `data` en portable). Elles contiennent le
démarrage, l'activité regroupée au plus une fois par seconde, puis le repos à
5 s, 30 s et toutes les minutes. Les événements sont regroupés : `last_event`
est le dernier observé, pas un journal exhaustif. Le registre est plafonné à
4096 images et n'en possède que des références faibles. Les sondages et
l'écriture de leurs lignes sont hors du thread UI. Hors activation, aucune
image n'est enregistrée et aucun moniteur ne démarre.

## Lire les compteurs

- `process_bytes` : Private Bytes, Working Set et régions virtuelles Windows
  engagées privées, mappées et exécutables ; espace réservé séparé. Les régions
  engagées ne sont pas toutes résidentes. Ce sondage concerne uniquement le
  processus principal, pas ses workers.
- `viewer_pixels.live_bytes` : pixels des images du Viewer encore référencées,
  y compris les chargements en attente de publication et les anciennes frames.
  `pending_bytes` désigne les résultats décodés pas encore publiés ;
  `retired_bytes` ceux retirés du contenu mais dont une référence forte subsiste.
- `preview_budget_entries_bytes` : pixels vivants inscrits au budget partagé
  des aperçus ; les caches et frames peuvent en partager la propriété.
- `tooltip_cache_entries_bytes` : nombre d'entrées et octets des caches image
  puis PDF ; `audio_cache_unique_entries_bytes` déduplique les pochettes par
  identité d'image. `icon_cache_entries_bytes` mesure le cache natif du parent.

Les valeurs sont des **octets**, sauf les nombres d'entrées. Les mesures se
succèdent et ne forment pas un instantané atomique. Les périmètres se recouvrent :
ne pas additionner les caches, le budget et les pixels du Viewer, ni soustraire
leur somme des Private Bytes. `None` signifie verrou occupé ou mesure indisponible,
pas zéro. `untracked_registrations > 0` indique une couverture incomplète
(contention ou saturation) ; une valeur nulle de pixels ne prouve alors pas leur
absence. Le suivi couvre les images pleine résolution, pages PDF et images
Markdown dès leur décodage, puis à leur publication et à leur retrait. Les
pochettes audio sont enregistrées à la publication du contenu. Les aperçus de
repli du Viewer restent dans le budget partagé des aperçus ; ils ne sont pas
tous inscrits dans ce registre. Ce suivi n'est pas un inventaire de tous les
objets GPUI ou de tous les tampons de transport.

Si les pixels du Viewer retombent à zéro et que les caches restent petits,
le reliquat ne s'explique pas par ces propriétaires de pixels. L'attribution
restante entre tas/allocateur, atlas GPU, pilote, code chargé et autres objets
nécessite un profil natif ou des captures complémentaires. Cette instrumentation
ne mesure ni les textures individuelles ni l'espace libre interne des tas et
ne prouve pas, à elle seule, qu'un reliquat est légitime.

## Initialisation des grammaires

Le 22 octobre 2026, un test Windows isolé a mesuré 25 501 696 octets (24,3 Mio)
de Private Bytes supplémentaires pendant l'initialisation de Syntect et des
grammaires embarquées. Ce delta inclut les allocations d'initialisation et
leur éventuelle rétention par l'allocateur ; ce n'est pas la taille exacte du
SyntaxSet. Le Viewer ne les précharge plus lors d'un parcours d'images : elles
sont initialisées hors UI lorsqu'une analyse syntaxique est demandée.

Rejouer seul, dans un processus neuf :

```powershell
cargo test --offline --locked --bin oxyde syntax_initialization_memory -- --ignored --test-threads=1 --nocapture
```

Les tests du registre vérifient le partage, les frames animées, les résultats
obsolètes, la libération des références et la saturation. Le test natif de
nettoyage vérifie les références aux pixels après deux rendus ; il ne mesure
pas la libération des textures par le pilote.

## Backend Windows et upload initial

Le backend Windows GPUI est une copie locale de la même révision épinglée,
avec une correction ciblée de l'atlas : une nouvelle texture dont les deux axes
font au moins 1024 px reçoit directement ses pixels à la création. Les tailles
partielles et les textures déjà présentes conservent UpdateSubresource. Ce
changement évite sur l'Intel testé deux gros tampons natifs retenus après les
uploads. Il n'ajoute aucune purge mémoire ni attente GPU au rendu de production.
Provenance : `vendor/gpui_windows/README.oxyde.md`.

Le lanceur active aussi les logs `gpu-atlas-create`, `gpu-atlas-remove` et
`gpu-atlas-destroy`. Pour les obtenir avec cargo run, ajouter
`$env:RUST_LOG = 'warn,gpui_windows::directx_atlas=debug'`. Le retrait d'une clé
et la destruction d'une texture sont distincts si plusieurs clés la partagent.
Ces événements complètent les compteurs de pixels sans mesurer directement les
allocations conservées par le pilote après les Release COM.

Probes manuelles, à exécuter chacune seule :

```powershell
cargo test --offline --locked --lib directx_image_upload_memory_lifecycle -- --ignored --test-threads=1 --nocapture
cargo test --offline --locked --lib directx_image_initial_data_memory_lifecycle -- --ignored --test-threads=1 --nocapture
cargo test --offline --locked --bin oxyde native_viewer_large_image_graphics_memory -- --ignored --test-threads=1 --nocapture
```

Les deux premières utilisent uniquement des données privées synthétiques et
peuvent appeler ClearState/Flush/Trim pour le diagnostic ; ces appels ne font pas
partie de la correction du rendu. Le probe GPUI affiche huit images synthétiques,
attend 30 s après leur retrait et ne force aucune purge du pilote. Les quantités
mesurées dépendent du pilote et ne sont pas des assertions portables.

Tests purs des bornes et des longueurs avant passage de données à D3D11 :

```powershell
rustc --test vendor/gpui_windows/src/atlas_upload.rs --edition 2024 -o target/atlas-upload-tests.exe
./target/atlas-upload-tests.exe
```

La compilation directe teste le helper utilisé en production sans activer
`gpui/test-support`, qui ajoute des dépendances Git de test non requises par
Oxyde. Voir les [mesures du 22 octobre](../reports/2026-10-22-viewer-memory.md).
