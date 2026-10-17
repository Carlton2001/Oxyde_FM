# Mesurer la compression ZIP

Les logs `oxyde_lib::archive_timing` distinguent préparation du plan, encodage et publication. Le superviseur Windows ajoute démarrage du worker, temps des handlers du parent, contrôles, attentes d’événements et compteurs de trames/lectures/écritures/checkpoints. Les compteurs ne changent ni limites ni protocole.

La [mesure avant/après du 17 octobre](../reports/2026-10-17-archive-transport.md) documente la correction du transport partagé Decode/Encode.

## Comparaison reproductible

Fermer Oxyde avant de reconstruire les binaires partagés :

```powershell
cargo build --offline --bins --example archive_compression_benchmark
$env:RUST_LOG = "warn,oxyde_lib::archive_timing=info"
cargo run --offline --example archive_compression_benchmark -- "C:\donnees\dossier"
```

Sans argument, l’outil génère un petit jeu privé sous `target/`. Avec un dossier, il le lit sans y écrire ; liens et points de réanalyse sont refusés. Les sorties sont créées dans un nouveau `target/archive-compression-<uuid>/`.

| Mode | Ce qui est mesuré |
| --- | --- |
| `production-fast` | Parcours Windows de production, worker isolé, niveau ZIP 1, sans Turbo |
| `production-fast-turbo` | Même parcours et niveau 1, avec Turbo |
| `direct-fast` | Même bibliothèque ZIP et niveau 1, écriture directe dans l’outil de mesure |
| `direct-normal` | Même écriture directe au niveau 6 |

Les archives sont décompressées et leur inventaire/contenu vérifiés contre les empreintes BLAKE3 des sources. Cette vérification et la préparation du manifeste sont hors chronométrage. Les ZIP vérifiés sont supprimés ; le rapport JSON et les fixtures générées restent disponibles.

## Interprétation

- La comparaison directe est un témoin expérimental, sans les gardes de sources, le confinement ni les échanges du parcours de production. Aucun repli direct n’est ajouté à l’application.
- Un passage unique, avec cache réchauffé par le manifeste et ordre fixe, ne donne pas un débit garanti. Le ZIP direct n’est pas le moteur de l’Explorateur Windows.
- `transport_wait_s` mesure le temps réellement passé à attendre les événements du transport. Il peut chevaucher le calcul du worker : ce n’est ni du temps CPU ni une mesure exacte du surcoût IPC. Les anciennes mesures de polling utilisaient `poll_sleep_s`.
- `handler_s` couvre les callbacks du parent ; `controls_s` couvre les contrôles dans la boucle. Le reste comprend notamment encodage/décodage des trames, communication et travail du worker. Ne pas le nommer « compression pure ».
- `write_bytes` compte les octets transmis, y compris les réécritures d’en-têtes ; il peut dépasser la taille finale. Le nombre et la taille moyenne des échanges aident à repérer une granularité trop fine.
- Les temps de préparation/publication et les compteurs doivent être lus avant d’attribuer toute la durée au codec ou à l’isolation.

Pour une mesure dans l’interface, reconstruire tous les workers avec `cargo build --offline --bins`, puis lancer `cargo run --offline`. Filtrer `archive_timing` dans `oxyde.log` ; ne pas déduire une validation visuelle du benchmark sans interface.
