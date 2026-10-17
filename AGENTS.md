# Oxyde — consignes pour les agents

Explorateur de fichiers natif en Rust, GPUI et Tokio. Documentation : [index](docs/README.md).

## Règles permanentes

- Garder `cargo check` sans avertissement du projet. Utiliser `rg`, jamais `grep`.
- Garder les calculs coûteux et les opérations de fichiers hors du thread UI.
- Toute API système passe par le trait `Platform` (`src/platform/mod.rs`), avec implémentations et dépendances conditionnées par cible.
- Privilégier les interactions natives et le clic gauche pour les actions principales.
- Utiliser les SVG Lucide officiels de `lucide-icons/lucide/icons/<nom>.svg`, sans modifier leur tracé. Si la source est inaccessible, signaler le blocage.
- Ne créer ni modifier un changement de curseur sans demande explicite de l’utilisateur. Les nouvelles interactions gardent le curseur par défaut.
- Écrire les messages de commit en anglais. **Chaque push vers GitHub exige une demande explicite pour cette opération**, y compris branches et tags, quel que soit l’outil. Une demande de modification, commit ou fusion ne l’autorise pas ; une autorisation antérieure ne vaut pas pour le push suivant.
- Après modification des workers ou de leur code partagé, prévenir qu’ils doivent être reconstruits. Fournir : fermer Oxyde, `cargo build --offline --bins`, puis `cargo run --offline`. `cargo run` seul ne reconstruit pas les exécutables auxiliaires.

## Avant de modifier

| Périmètre | Référence à lire |
| --- | --- |
| Interface | [Conventions UI](docs/architecture/interface.md) |
| Menus, infobulles, fenêtres secondaires | [Fenêtres](docs/architecture/windows.md) |
| Aperçus, miniatures et sessions | [Aperçus](docs/architecture/previews.md) |
| Archives, confinement et garanties | [Archives](docs/architecture/archives.md) et [limites de sécurité](docs/security/status.md) |
| Sortie finale du processus | [Fermeture PDF/Intel](docs/diagnostics/shutdown.md) |
| Découpage et tests | [Développement](docs/guides/development.md) |

## Organisation du code

- `src/ui/app/` possède l’état métier et les actions ; `components/` compose l’interface ; `elements/` fournit les primitives.
- Une responsabilité identifiable par module. Les façades déclarent et assemblent ; elles n’accumulent pas les parcours et rendus.
- Viser 200–600 lignes de production ; au-delà de 800, examiner le découpage avant une extension significative et justifier le maintien. Ne pas découper artificiellement ni compter les tests comme production.
- Extraire les sections cohérentes ; partager un composant seulement si plusieurs usages partagent présentation ou comportement. Préserver visibilité minimale, identifiants GPUI et cycles de vie. Limiter la refactorisation au changement.
- Ranger les nouveaux tests unitaires dans le `tests/` du module, sans changer leur parent logique Rust ni élargir les visibilités. Préserver scénarios, attributs et découverte lors d’un déplacement ; détails dans le guide de développement.
- Utiliser les couleurs sémantiques et les catalogues des cinq langues. Garder les décisions métier dans la vue appelante.

Les sources de `reference_zed/` et le checkout partiel Zed `v1.21.0` dans `C:\Users\Biluf\Downloads\zed-1.21.0` servent de référence GPUI pour le rendu/style ; ne pas reprendre leurs dépendances.
