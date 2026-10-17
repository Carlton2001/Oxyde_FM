# Audit du code : corrections intégrées

Périmètre : audit du 16 octobre 2026, corrections et validations jusqu’au 17 octobre, intégrées à `main` à `3befdc5`. Ce bilan résume les résultats rapportés ; les tests n’ont pas été relancés pour la refonte documentaire.

## Résultats

| ID | Correction livrée |
| --- | --- |
| A01 | Réservation de toutes les destinations du plan avant transfert : refus des collisions internes entre sources |
| A02 | Intégrations workers raccordées à la bibliothèque de production ; script et CI Windows communs |
| A03 | Miniatures indexées par révision du fichier et libérées avec leur dernier propriétaire visible |
| A04–A05 | Critères de recherche d’archives et visibilité des fichiers harmonisés |
| A06–A07 | Renommage changeant seulement la casse ; distinction dossier réel / chemin d’archive |
| A08–A09 | Graphe conservé après fin d’opération ; recherche de texte brut à mémoire bornée |
| A10 | Documentation alignée sur les codecs Windows isolés, sans prétendre valider Linux |
| O01–O04 | Instantanés UI réutilisés, métadonnées réutilisées, préparation de recherche en arrière-plan, agrégation des conflits et validation asynchrone des chemins |

Les constats initiaux ne doivent pas être lus comme des défauts encore ouverts. Les contrats actifs sont dans [opérations](../architecture/operations.md), [aperçus](../architecture/previews.md) et [archives](../architecture/archives.md).

## Validation rapportée

`./scripts/test-windows.ps1 -Offline` : compilation de toutes les cibles et construction de tous les binaires sans avertissement, **1 160 tests distincts réussis, 21 ignorés, aucun échec**. Les scénarios antérieurs ont été comparés aux nouveaux sans suppression ni changement de statut. Contrôle final supplémentaire des sources et 277 tests UI réussis (un ignoré).

A10 : `cargo check --offline --locked --all-targets` sans avertissement et 29 tests `archive_io_pipeline` réussis le 17 octobre. Les journaux locaux sont sous `target/o01-o04-validation/` et `target/a10-validation/` ; ils peuvent disparaître avec `target`.

## Limites

Les mesures sont locales à la préparation des données. Elles ne prouvent ni un gain global de dessin/latence GPUI, ni une borne RSS/GPU, ni le comportement d’un partage SMB indisponible. La première présentation et certaines interactions restent linéaires ; les résultats peuvent encore nécessiter un tri complet et des copies temporaires.

Aucun nouvel essai visuel ou Linux n’est rapporté. La passe ne clôture pas les [limites de sécurité](../security/status.md).

Deux incidents intermédiaires d’A09 restent sans cause établie : rapport de récupération inattendu lors d’une annulation d’archive, et `DirectoryNotEmpty` au nettoyage d’un staging retenu. Les rejeux et la suite finale ont réussi, sans correctif de production attribué à ces incidents. Les journaux sont référencés dans le rapport complet ; une réussite ultérieure ne clôture pas leur diagnostic.

Rapport complet (reproductions, microbenchmarks, noms et résultats des tests) : révision `3befdc51a689e907d99b7c0603bbf8d666ae8976`, chemin `docs/CODE_AUDIT_2026-10-16.md`. Voir la [commande de récupération](README.md).
