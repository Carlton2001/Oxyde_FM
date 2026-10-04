# Oxyde — consignes pour les agents

## Projet

Oxyde est un explorateur de fichiers natif écrit en Rust, avec GPUI pour l'interface et Tokio pour les opérations asynchrones. Le projet a migré depuis une interface React/Tauri ; le backend Rust existant a été conservé et l'interface WebView a été remplacée par une interface native.

## Documentation à consulter

- **[SHUTDOWN_DIAGNOSTICS.md](SHUTDOWN_DIAGNOSTICS.md)** conserve le diagnostic et la résolution du blocage de fermeture après les aperçus PDF sous Windows (`SHUTDOWN-PDF-INTEL-2026-10-04`). Le consulter si ce problème revient ou avant de modifier la sortie finale du processus.
- **[UI_ARCHITECTURE.md](UI_ARCHITECTURE.md)** définit les règles actuelles pour les thèmes, l'internationalisation, les composants partagés, les préférences et la validation de l'interface. Le consulter avant de modifier l'UI.
- `src/ui/app/` contient l'état et l'assemblage des vues ; `src/ui/components/` les composants d'interface ; `src/ui/elements/` les éléments réutilisables de bas niveau.
- `src/platform/` est la couche d'abstraction des API système.

## Règles de développement

1. Garder `cargo check` sans avertissement du projet.
2. Privilégier les interactions natives, avec le clic gauche pour les actions principales.
3. Garder la navigation réactive ; les calculs coûteux et les opérations de fichiers doivent rester en arrière-plan.
4. Ne pas utiliser `grep` ; utiliser `rg` pour rechercher dans le dépôt.
5. Toute API propre à un système d'exploitation passe par le trait `Platform` de `src/platform/mod.rs`. Implémenter le comportement dans `windows_impl.rs` et `linux_impl.rs` selon les cibles, et garder les modules et dépendances spécifiques derrière les conditions de compilation appropriées.
6. Pour toute icône Lucide, récupérer et utiliser le SVG officiel depuis `lucide-icons/lucide/icons/<nom>.svg`. Ne jamais dessiner, redessiner ou approximer une icône à la main, ni modifier son tracé. Si le SVG officiel n'est pas accessible, signaler le blocage au lieu de créer une variante.
7. Seul l'utilisateur décide des cas de changement de curseur. Ne pas ajouter ni modifier un changement de curseur sans sa demande explicite, y compris le curseur en main sur les éléments cliquables. Conserver le curseur par défaut pour les nouvelles interactions, sauf instruction explicite de l'utilisateur.

## Références GPUI

`reference_zed/` contient des sources à consulter pour le rendu et le style GPUI uniquement ; ne pas reprendre leurs dépendances. Un checkout partiel de Zed `v1.21.0` est disponible dans `C:\Users\Biluf\Downloads\zed-1.21.0` (`gpui`, `gpui_platform`, `gpui_util`, `gpui_macros`). D'autres chemins peuvent être ajoutés au sparse checkout si nécessaire.

## Thèmes, langues et composants

Suivre les conventions et les exemples de [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) pour tout changement d'interface. En résumé : utiliser les couleurs sémantiques du thème, placer les textes traduisibles dans les catalogues, et factoriser un composant lorsque ses usages partagent réellement présentation ou comportement. Garder l'état métier et les actions dans la vue appelante.
