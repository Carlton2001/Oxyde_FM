# Transport des archives : correction et mesures

Mesures locales Windows x64, ZIP 9.0.4, profil de développement optimisé du projet. Dossier ControlCenter_5 : 195 fichiers, 51 dossiers, 274 152 940 octets. Un passage par mode, cache réchauffé par le manifeste ; les témoins directs omettent le confinement et les gardes de production.

| Parcours | Avant | Après | Taille ZIP |
| --- | ---: | ---: | ---: |
| Production Rapide | 111,34 s | 4,56 s | 284 931 629 octets |
| Production Rapide + Turbo | 106,76 s | 4,03 s | 284 931 629 octets |

Le témoin direct Rapide prend 2,65 s après correction, contre 3,41 s dans la passe initiale ; cette variation rappelle la portée locale des chronométrages. Le petit jeu généré passe également de 7,49 s à 0,29 s en production Rapide. Aucun chronométrage de l’Explorateur n’est revendiqué.

## Correction

Les lectures et fins d’écriture réveillent le superviseur par une file d’événements bornée, au lieu du polling avec sommeil de 1 ms. Deux helpers traitent les pipes bloquants ; un crédit autorise chaque lecture de trame. Un flot de requêtes ne peut pas affamer les notifications de fin d’écriture. Session, séquences et tailles sont vérifiées avant allocation des corps de trames.

Les 34 361 trames, 4 354 lectures, 9 689 écritures et 19 578 checkpoints du cas réel sont conservés. Les attentes cumulées du superviseur passent de 104,60 s à 3,34 s ; elles peuvent chevaucher le calcul du worker et ne représentent pas du temps CPU. Le niveau de compression, les permissions LPAC, jobs, contrôles d’effets et admissions restent inchangés.

La correction concerne le transport partagé Decode/Encode : extraction, compression et reconstruction. Les workers de listing, d’images, d’icônes et de vidéo ont des transports distincts et ne sont pas modifiés par cette correction.

## Validation et limites

`./scripts/test-windows.ps1 -Offline` : compilation de toutes les cibles et reconstruction des binaires sans avertissement du projet ; **1 161 tests distincts réussis, 21 ignorés, aucun échec**. Les quatre nouveaux tests unitaires couvrent fragmentation/EOF, identité/séquences/tailles, réveils et crédit de lecture. Un nouveau test natif vérifie délai et annulation avec en-tête ou corps de trame incomplet, puis le succès d’une nouvelle session.

Les tests natifs existants de pause, annulation, writer bloqué, fausses sessions, rejeu, données après résultat terminal et erreurs mémoire passent ; les 29 tests du pipeline de production passent également. Les ZIP produits par le benchmark ont été décompressés et vérifiés par inventaire et BLAKE3, puis supprimés. Les deux ZIP originaux contiennent les mêmes fichiers, vérifiés séparément par SHA-256.

Journaux non versionnés : `target/archive-transport-validation.log`, `archive-transport-controlcenter.log`, `archive-transport-smoke.log` et anciennes mesures `target/archive-measure-controlcenter.log`. Ils peuvent disparaître avec `target/`. Outil et interprétation : [mesurer la compression ZIP](../diagnostics/archive-compression.md).

Aucun essai visuel GPUI ni exécution Linux n’est ajouté. Le superviseur revérifie ses contrôles après une attente inactive demandée de 5 ms ; cela ne promet pas de délai absolu pour l’ordonnancement ou les I/O natives du parent. Les limites générales de sécurité restent distinctes de ce gain de performance.
