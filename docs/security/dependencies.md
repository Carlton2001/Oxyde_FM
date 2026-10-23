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

## Exécuter l’audit Windows

```powershell
./scripts/audit-security.ps1
```

Sous Windows, le script utilise le binaire x64 officiel `cargo-audit` 0.22.2,
installé dans `target/security-tools/bin`. L’installateur vérifie les SHA-256 fixés
de l’archive avant extraction et du binaire avant exécution, puis sa version.
L’empreinte de l’archive est publiée dans la [release RustSec officielle](https://github.com/rustsec/rustsec/releases/tag/cargo-audit/v0.22.2).
Un binaire déjà présent avec l’empreinte attendue est réutilisé sans téléchargement ;
sinon le script télécharge et vérifie son remplacement. Aucune compilation de
l’outil ni mise à jour de Rust n’est nécessaire dans le workflow Windows.
Sur les autres systèmes, le parcours existant par `cargo install` est conservé.

La base RustSec et l’index des versions retirées sont actualisés à chaque audit :
aucun résultat ni base d’avis n’est mis en cache par le workflow. Les filtres
Windows x86_64, le blocage des versions `yanked`, les avis de maintenance visibles,
le rapport JSON et sa conservation comme artifact restent présents.
Le workflow conserve le rapport avec `actions/upload-artifact` 7.0.2, épinglé sur un
commit immuable et utilisant Node.js 24 ; cela retire l'action Node.js 20 dépréciée.
Le nom de l'artefact, le chemin du JSON et l'envoi après échec (`if: always()`) sont conservés.
Le script affiche le temps d’installation de l’outil, puis celui de l’actualisation et de
l’audit. Les [mesures datées](../reports/2026-10-22-windows-ci.md) distinguent les
anciennes durées GitHub des nouvelles validations locales.

Pour vérifier l’installateur hors réseau avec une copie intacte du ZIP officiel :

```powershell
./scripts/tests/install-cargo-audit.tests.ps1 -ArchivePath C:\chemin\cargo-audit-x86_64-pc-windows-msvc-v0.22.2.zip
```
