# Menus, infobulles et fenêtres secondaires

Sous Windows, les menus et infobulles utilisent des fenêtres GPUI possédées. Linux conserve le rendu intégré ; un échec natif Windows doit réactiver ce repli sans perdre l’état métier.

## Choisir le point d’entrée

| Surface | API / règle |
| --- | --- |
| Infobulle ordinaire | `NativeTooltipExt::native_tooltip` ; aucun `.tooltip(...)` direct dans les vues |
| Survol de fichiers ou d’arbre | Sessions de `components::tooltip::hover` ; [aperçus](previews.md) |
| Menu interactif | `components::popup` avec factory, condition de validité et callback de fermeture |
| Suggestions de chemin | `popup::show_passive` ; focus et clavier restent dans le champ |
| Fenêtre secondaire | `components::tool_window` ; état métier dans la vue appelante |

Un identifiant réutilise la fenêtre existante ; une demande en attente est remplacée par la plus récente. La factory reconstruit l’état actuel, sans copie métier figée. À la fermeture, lire la spec actuelle plutôt qu’un callback capturé à la première ouverture.

## Placement et événements

- `PopupAnchor::point` fixe une position locale, `element` ancre sous un contrôle et `beside` place un menu latéral. Mesurer les bounds au rendu ; les menus ne suivent pas le curseur après ouverture.
- Les sous-menus utilisent les bounds réelles des lignes après défilement, sans l’ombre, et touchent leur ancre. Leur propriétaire logique est le menu parent.
- Router les actions métier avec `popup::with_owner`. Convertir coordonnées et rectangles par `owner_point`/`owner_bounds`, DPI compris. Les scrollbars gardent leur propriétaire et des identifiants distincts par fenêtre.
- Ne pas retransmettre le pointeur par `Window::dispatch_event` à la source : son hit testing ferait réagir les éléments sous le menu. Appeler uniquement les gestionnaires de glissement nécessaires après conversion ; la capture termine un glissement hors surface.
- Ne jamais appeler `Window::dispatch_event` dans le constructeur d’`App::open_window` : la racine n’est pas installée. Initialiser les modificateurs après retour via le handle non typé.

Les appels Win32 passent par `Platform`. Placement et activation susceptibles de rappeler GPUI sont différés et regroupés, jamais synchrones pendant un rendu ou `Window::update`. Créer près du propriétaire pour conserver un DPI initial cohérent.

## Focus et fermeture

Enregistrer les actions via `popup::keyboard::item`, avec le même callback que le clic, en excluant les lignes désactivées. Préserver flèches, début/fin, Entrée/Espace, Tab/Maj+Tab et Échap ; les éditeurs gardent leurs handlers. Les raccourcis métier sont routés vers la source.

Restituer le focus au propriétaire logique seulement si la famille reste active. Préserver le nouveau focus posé par une action. Un clic propriétaire ferme les descendants avant son action ; changer d’application ferme les menus sans réactiver Oxyde.

Avant de détruire un propriétaire, fermer ses descendants avec `tool_window::close_transients` ; respecter d’abord un éventuel refus de fermeture d’un dialogue occupé. `on_window_closed` reste un filet de sécurité. Les suggestions s’annulent quand leur propriétaire devient inactif.

## Infobulles et géométrie

Les infobulles natives sont des `PopUp` transparentes, passives, sans activation ni barre des tâches ; les clics les traversent. Une seule ombre GPUI, sans décoration DWM ni cadre non client, sans statut globalement topmost. Préserver l’origine de prépaint fournie par la marge d’ombre.

Le cadre visible suit le pointeur à 12 pixels logiques sur X/Y, avec retournement et bornage sur la zone de travail. Les placements fixes gardent les bounds de l’élément. Une marge transparente d’ombre n’ajoute pas de décalage visible.

`ui/window_geometry.rs` mémorise la position restaurable et l’état maximisé ; `app/tool_geometry.rs` gère les familles secondaires via `AppConfig::tool_window_placements`. Conserver les positions négatives, ignorer la minimisation et borner une géométrie invalide aux écrans disponibles. Les écritures sont différées de 350 ms ; la fermeture force la dernière sauvegarde. Réinitialiser les préférences conserve ces positions.

Les essais utiles et leurs limites figurent dans le [guide visuel](../guides/visual-checks.md) ; les résultats datés dans le [bilan des fenêtres](../reports/2026-10-15-windows.md).
