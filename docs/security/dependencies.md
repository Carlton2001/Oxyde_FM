# Dépendances conservées

Décision utilisateur : conserver `paste 1.0.15`, `rustybuzz 0.20.1` et `ttf-parser 0.25.1` tant qu’aucun motif concret ne justifie une migration. Un avis de maintenance ne prouve pas une vulnérabilité ; il reste visible.

| Dépendance | Usage identifié lors de la revue | Piste de migration |
| --- | --- | --- |
| `paste` | Macro de compilation via OpenEXR/pulp | `pastey` quand la chaîne publiée compatible l’adopte ; ne pas retirer OpenEXR pour masquer l’avis |
| `rustybuzz` | Composition de texte SVG via GPUI/usvg | `harfrust`, avec mise à jour cohérente de la pile SVG |
| `ttf-parser` | Contrôle des glyphes GPUI et dépendances SVG | Fontations/`skrifa`, avec contrôle des changements d’API/rendu |

Ajouter une alternative au manifeste ne remplace pas une dépendance indirecte. Ne pas introduire implicitement un fork de GPUI ni une source Git pour effacer les avis.

La revue du **9 octobre 2026** a signalé trois avis de maintenance et aucune vulnérabilité publiée pour les filtres Windows x86_64 utilisés. Le rapport [RustSec conservé](../../security/RUSTSEC_WINDOWS.json) et les versions citées sont historiques, sans vérification nouvelle dans cette refonte documentaire.

Le retrait de l’encodeur AVIF inutilisé a désactivé le chemin actif `ravif/rav1e`, en conservant les décodeurs précédents. Le verrou peut encore contenir des packages absents du graphe actif : ne pas confondre ces inventaires.

## Quand réexaminer

- Une vulnérabilité publiée touche une version et un chemin effectivement utilisés.
- Une version compatible de GPUI/OpenEXR adopte les remplacements.
- Un défaut fonctionnel concret justifie une migration ou un backport.

Avant une migration de typographie, vérifier accents des cinq langues, ligatures, scripts complexes, bidirectionnel, polices de repli, emoji/glyphes manquants et dimensions/couleurs des SVG. Les tests de pixels ne remplacent pas ces essais.

`scripts/audit-security.ps1` et `.github/workflows/security.yml` produisent les contrôles de sécurité ; ils ne migrent pas automatiquement les dépendances. L’ancienne analyse complète est récupérable via l’[historique documentaire](../reports/README.md).
