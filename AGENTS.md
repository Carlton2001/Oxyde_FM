# Oxyde — consignes pour les agents

## Projet

Oxyde est un explorateur de fichiers natif écrit en Rust, avec GPUI pour l'interface et Tokio pour les opérations asynchrones. Le projet a migré depuis une interface React/Tauri ; le backend Rust existant a été conservé et l'interface WebView a été remplacée par une interface native.

## Documentation à consulter

- **[POPUP_WINDOWS_AUDIT.md](POPUP_WINDOWS_AUDIT.md)** décrit la migration des infobulles et menus vers les fenêtres Windows et ses limites de validation. Consulter également la section « Fenêtres transitoires » de `UI_ARCHITECTURE.md` avant de modifier ces surfaces.

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
8. Toujours rédiger les messages de commit en anglais.
9. Lorsqu’un changement modifie les workers ou du code partagé qu’ils utilisent, prévenir explicitement l’utilisateur qu’ils doivent être recompilés et fournir la commande Cargo à lancer. Par défaut : `cargo build --offline --bins` (fermer Oxyde avant de reconstruire), puis `cargo run --offline`. Ne pas laisser entendre que `cargo run` seul reconstruit les exécutables auxiliaires.

## Factorisation et taille des modules

La factorisation fait partie du développement courant : avant d'ajouter du code à un fichier déjà chargé, chercher une responsabilité cohérente à isoler ou un composant existant à réutiliser. Éviter de recréer des fichiers monolithiques après les refactorisations.

- Viser généralement **200 à 600 lignes de production par fichier** lorsqu'une responsabilité le permet. Au-delà de **800 lignes**, examiner le découpage avant une extension significative et justifier le maintien d'un fichier plus grand. Ces nombres sont des repères, pas des limites rigides ; distinguer les tests du code de production et ne pas découper artificiellement un ensemble cohérent.
- Donner à chaque module une responsabilité identifiable. Les fichiers racines et `mod.rs` servent principalement de façade, de déclaration des modules et d'assemblage ; éviter d'y accumuler les parcours fonctionnels et les grands blocs de rendu.
- Découper les grandes fonctions de rendu par sections ou interactions cohérentes, même si elles n'ont qu'un usage. Créer un composant partagé seulement lorsque plusieurs usages partagent réellement présentation ou comportement.
- Organiser les composants complexes en sous-modules privés nommés selon leurs responsabilités. Suivre les règles détaillées de [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) pour les façades, l'état, les callbacks et les dépendances.
- Préserver les imports utilisés ailleurs par des réexportations ciblées et garder la visibilité minimale. Ne pas élargir les accès ni déplacer l'état métier uniquement pour faciliter le découpage.
- Limiter une extraction au périmètre du changement, procéder par étapes vérifiables et préserver comportement, rendu, identifiants GPUI et cycles de vie. Un changement local ne justifie pas une refactorisation générale de modules voisins.

## Organisation des tests des modules

Les tests de non-régression sont destinés à être conservés. Ranger les nouveaux tests unitaires dans un **sous-répertoire `tests/` dédié au module concerné**, pour garder le code de production lisible : par exemple `src/commands/search/tests/` pour `src/commands/search.rs`. Pour un module déjà organisé en répertoire, placer les tests dans son propre `tests/` ou dans celui du sous-module auquel ils appartiennent.

- Répartir les fichiers de tests par responsabilité ou scénario lorsque leur volume le justifie, avec des noms explicites. Garder les fixtures et helpers près des tests qui les utilisent ; partager uniquement ceux réellement communs.
- Le rangement physique ne doit pas changer la hiérarchie logique Rust lors d'une extraction. Conserver le nom et le parent de chaque module de tests, au besoin avec `#[path]`, pour préserver les chemins complets des tests et les accès privés via `super::`.
- Garder les déclarations derrière `#[cfg(test)]` et conserver toutes les conditions de cible et tous les attributs existants (`ignore`, `should_panic`, attributs Tokio, etc.). Ne pas ajouter un niveau `mod tests` à l'intérieur du fichier extrait : il contient le corps de l'ancien bloc.
- Ne pas élargir les visibilités du code de production pour faciliter le déplacement. Le dossier racine `tests/` est réservé aux tests d'intégration des interfaces publiques ; ne pas y transférer des tests unitaires dépendant d'éléments privés.
- Lors d'un déplacement, préserver assertions, scénarios, synchronisation, fixtures et nettoyage. Vérifier les chemins relatifs et macros dépendant de l'emplacement source (`include_str!`, `include_bytes!`, `file!`, `#[path]`). Les helpers `#[cfg(test)]` hors des blocs peuvent rester dans le module parent si leur déplacement change les accès.
- Vérifier la découverte des mêmes tests avant/après, compiler et exécuter les tests concernés avec les mêmes configurations disponibles. Un filtre qui découvre zéro test ne constitue pas une validation. Signaler les cibles non vérifiées.
- Appliquer cette organisation aux nouveaux tests et lors des extractions utiles ; ne pas lancer une migration générale des petits blocs existants dans une tâche sans rapport.

Exemple : le module déclaré dans `src/commands/search.rs` reste un enfant direct de `search`, même si son fichier est rangé dans `search/tests/` :

```rust
#[cfg(test)]
#[path = "search/tests/content_tests.rs"]
mod content_tests;
```

## Références GPUI

`reference_zed/` contient des sources à consulter pour le rendu et le style GPUI uniquement ; ne pas reprendre leurs dépendances. Un checkout partiel de Zed `v1.21.0` est disponible dans `C:\Users\Biluf\Downloads\zed-1.21.0` (`gpui`, `gpui_platform`, `gpui_util`, `gpui_macros`). D'autres chemins peuvent être ajoutés au sparse checkout si nécessaire.

## Thèmes, langues et composants

Suivre les conventions et les exemples de [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) pour tout changement d'interface. En résumé : utiliser les couleurs sémantiques du thème, placer les textes traduisibles dans les catalogues, et factoriser un composant lorsque ses usages partagent réellement présentation ou comportement. Garder l'état métier et les actions dans la vue appelante.
