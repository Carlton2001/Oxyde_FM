# Turbo et compression — 17 octobre 2026

Mesures Windows MSVC, profil de développement habituel, sur les changements suivant `123f2a3`. Source `ControlCenter_5` : 195 fichiers, 51 répertoires, 274 152 940 octets. Un passage par configuration, après lecture des empreintes des sources ; préparation, encodage et publication sont chronométrés. Vérification de l’inventaire et des contenus BLAKE3 hors chronométrage ; toutes les archives vérifiées ont été supprimées du dossier privé de mesure.

## Résultats avec Turbo

| Format | Qualité | Temps | Taille |
| --- | --- | --- | --- |
| ZIP | Rapide | 4,52 s | 271,73 Mio |
| ZIP | Normal | 7,78 s | 257,14 Mio |
| 7z | Rapide | 13,17 s | 255,91 Mio |
| 7z | Normal | 32,17 s | 246,26 Mio |
| TAR.ZST | Rapide | 1,39 s | 257,50 Mio |
| TAR.ZST | Normal | 1,12 s | 257,07 Mio |
| TAR | Sans compression | 1,37–1,38 s | 261,64 Mio |

Le test utilisateur précédent donnait environ une minute et 256 Mio pour le 7z rapide avec Turbo. Sa référence NanaZip reste meilleure : 12 s, 241,6 Mio, niveau 5, dictionnaire 32 Mio, 32 threads configurés. Oxyde ne rejoint donc pas encore son compromis temps/taille en 7z Normal. Ces passages uniques ne garantissent pas un débit ni un classement entre qualités.

## Changement et bornes

ZIP compresse plusieurs fichiers en parallèle et conserve le niveau choisi. 7z utilise LZMA2 multithread et des blocs solides bornés ; TAR.ZST active les threads Zstandard. Les tampons de 256 Kio réduisent les petits échanges du transport. [Contrat et limites](../architecture/archives.md#compression-et-turbo-sous-windows).

Le 7z Normal a atteint **1 414 397 952 octets de mémoire engagée** : le plafond individuel antérieur de 1 Gio provoquait un échec sur ce cas réel. La création 7z réserve désormais 2 Gio ; le budget global reste à 2,5 Gio avec 512 Mio pour les petits workers de navigation. ZIP/Decode/TAR/TAR.ZST restent à 1 Gio. Les lectures ciblées d’archives peuvent attendre cette réservation.

ZIP parallélise les fichiers, sans découper un fichier unique. Au-delà de 256 Mio par fichier, ZIP et 7z gardent un encodage en flux monothread. Les encodeurs Linux locaux et le rendu de l’interface ne sont pas validés par ces mesures.

Les logs et rapports JSON bruts sont sous `target/archive-turbo-verified-*.log` et `target/archive-compression-*/report.json`, hors Git. [Reproduire la mesure](../diagnostics/archive-compression.md).

## Validation

Une première suite Windows complète a passé 1 166 tests, avec 21 ignorés. Après l’ajustement du plafond 7z et de l’admission ZIP : compilation de toutes les cibles sans avertissement du projet, reconstruction de tous les binaires et **103 tests ciblés réussis** (34 codecs/protocole/transport, 8 budgets, 61 intégrations archives/aperçus/recherche). Les scénarios incluent niveaux ZIP conservés, blocs solides, pause et annulation après mise en mémoire, refus des workers défaillants, récupération et réserve de navigation. Les huit archives du tableau ont passé la vérification complète d’inventaire/contenu.
