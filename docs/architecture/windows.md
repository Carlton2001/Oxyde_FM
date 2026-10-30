# Menus, infobulles et fenêtres secondaires

Sous Windows, les menus et infobulles utilisent des fenêtres GPUI possédées ; un échec natif doit réactiver le rendu intégré sans perdre l’état métier. Sous Wayland, les infobulles et aperçus de survol sont aussi des fenêtres séparées, mais ancrées (voir [Infobulles et géométrie](#infobulles-et-géométrie)). Les menus Linux et les infobulles en X11, qui refuse ces fenêtres, restent dans le rendu intégré.

## Choisir le point d’entrée

| Surface | API / règle |
| --- | --- |
| Infobulle ordinaire | `NativeTooltipExt::native_tooltip` ; aucun `.tooltip(...)` direct dans les vues |
| Survol de fichiers ou d’arbre | Sessions de `components::tooltip::hover` ; [aperçus](previews.md) |
| Menu interactif | `components::popup` avec factory, condition de validité et callback de fermeture |
| Suggestions de chemin | `popup::show_passive` ; focus et clavier restent dans le champ |
| Fenêtre secondaire | `components::tool_window` ; état métier dans la vue appelante |
| Invite modale (renommage, compression, lecteur réseau, confirmations, avis d’échec) | `DialogKind` fixe de `app/dialog_windows` ; `Modal::standalone`, jamais de surcouche dans la fenêtre principale |

Un identifiant réutilise la fenêtre existante ; une demande en attente est remplacée par la plus récente. La factory reconstruit l’état actuel, sans copie métier figée. À la fermeture, lire la spec actuelle plutôt qu’un callback capturé à la première ouverture.

## Placement et événements

- `PopupAnchor::point` fixe une position locale, `element` ancre sous un contrôle et `beside` place un menu latéral. Mesurer les bounds au rendu ; les menus ne suivent pas le curseur après ouverture.
- Les sous-menus utilisent les bounds réelles des lignes après défilement, sans l’ombre, et touchent leur ancre. Leur propriétaire logique est le menu parent.
- Router les actions métier avec `popup::with_owner`. Convertir coordonnées et rectangles par `owner_point`/`owner_bounds`, DPI compris. Les scrollbars gardent leur propriétaire et des identifiants distincts par fenêtre.
- Ne pas retransmettre le pointeur par `Window::dispatch_event` à la source : son hit testing ferait réagir les éléments sous le menu. Appeler uniquement les gestionnaires de glissement nécessaires après conversion ; la capture termine un glissement hors surface.
- Ne jamais appeler `Window::dispatch_event` dans le constructeur d’`App::open_window` : la racine n’est pas installée. Initialiser les modificateurs après retour via le handle non typé.

Les appels Win32 passent par `Platform`. Placement et activation susceptibles de rappeler GPUI sont différés et regroupés, jamais synchrones pendant un rendu ou `Window::update`. Créer près du propriétaire pour conserver un DPI initial cohérent.

## Invites modales fixes

Les invites sont de vraies fenêtres possédées, au gabarit des Propriétés (`tool_window::header`, pied commun, largeur fixe, hauteur ajustée au contenu). `Platform::configure_fixed_dialog` désactive le propriétaire tant qu’elles sont visibles et verrouille leur position au centre de celui-ci, bornée à sa zone de travail ; ni déplacement, ni redimensionnement, ni réduction. `follow_fixed_dialog_owner` les recentre quand la fenêtre principale bouge. Le propriétaire est réactivé avant le masquage pour que Windows lui rende l’activation. La fermeture par la barre de titre ou Alt+F4 passe par `request_fixed_close`, qui respecte les opérations occupées ; à la libération, le focus du propriétaire revient à `focus_handle` s’il pointait encore vers l’invite. Linux ouvre la même fenêtre sans verrouillage.

## Focus et fermeture

Enregistrer les actions via `popup::keyboard::item`, avec le même callback que le clic, en excluant les lignes désactivées. Préserver flèches, début/fin, Entrée/Espace, Tab/Maj+Tab et Échap ; les éditeurs gardent leurs handlers. Les raccourcis métier sont routés vers la source.

Toutes les surfaces `popup` conservent l’activation Windows et le focus natif dans leur formulaire propriétaire, sans variation d’ombre lors de l’ouverture. Les menus interactifs gardent leur focus logique GPUI ; un hook `WH_GETMESSAGE` limité au thread UI redirige les messages clavier vers le dernier menu interactif visible du formulaire, avant la traduction des accélérateurs GPUI. Cela couvre les sélecteurs, menus contextuels, paramètres, sous-menus et champs éditables. La fermeture d’un sous-menu rend le clavier au menu précédent ; la fermeture du dernier rend le clavier au formulaire et libère le hook. `show_passive` réserve le clavier au champ source pour les suggestions de chemin. La fermeture ne demande aucune activation native.

Restituer le focus au propriétaire logique seulement si la famille reste active. Préserver le nouveau focus posé par une action. Un clic propriétaire ferme les descendants avant son action ; changer d’application ferme les menus sans réactiver Oxyde.

Avant de détruire un propriétaire, fermer ses descendants avec `tool_window::close_transients` ; respecter d’abord un éventuel refus de fermeture d’un dialogue occupé. `on_window_closed` reste un filet de sécurité. Les suggestions s’annulent quand leur propriétaire devient inactif.

## Infobulles et géométrie

Les infobulles natives sont des `PopUp` transparentes, passives, sans activation ni barre des tâches ; les clics les traversent. Une seule ombre GPUI, sans décoration DWM ni cadre non client, sans statut globalement topmost. Préserver l’origine de prépaint fournie par la marge d’ombre.

Le cadre visible suit le pointeur à 12 pixels logiques sur X/Y, avec retournement et bornage sur la zone de travail. Les placements fixes gardent les bounds de l’élément. Une marge transparente d’ombre n’ajoute pas de décalage visible.

`ui/window_geometry.rs` mémorise la position restaurable et l’état maximisé ; `app/tool_geometry.rs` gère les familles secondaires via `AppConfig::tool_window_placements`. Conserver les positions négatives, ignorer la minimisation et borner une géométrie invalide aux écrans disponibles. Les écritures sont différées de 350 ms ; la fermeture force la dernière sauvegarde. Réinitialiser les préférences conserve ces positions.

### Infobulles sous Wayland

Une application n'y place pas ses fenêtres : `hover/anchored.rs` ouvre un `AnchoredPopup` (`xdg_popup`), une surface sans entrée dont le compositeur garde le cadre à l'écran. Il n'existe pas de popup masqué : la surface reste ouverte et vide entre deux éléments d'une session, et se ferme à sa fin, car en ouvrir une autre coûte plusieurs images. Un compositeur affiche chaque image : la bulle ne se vide qu'après 80 ms sans élément survolé, sinon elle clignoterait à chaque espace entre deux fichiers. Son ombre est celle des menus, plus marquée que sous Windows, pour se détacher des fenêtres du bureau. La copie de GPUI ([README](../../vendor/gpui_linux/README.oxyde.md)) ajoute `Window::reposition_popup`, qui déplace la surface sans la redessiner : avec un redessin, Mutter 50 la limitait à 20 images par seconde.

Quand la taille change, entre deux types de fichiers par exemple, l'image reste celle de l'élément précédent jusqu'à la réponse du compositeur, puis la suivante arrive d'un bloc à la bonne taille. Le contenu mesuré demande cette taille avant sa présentation (`MeasuredContent::prepaint`) ; GPUI ne dessine pas tant que la réponse manque et ne valide aucun déplacement seul tant que l'image n'est pas sur la surface : le compositeur étirerait sinon l'ancienne au nouveau format (`wp_viewport`).

Mutter ferme un popup qui ne touche plus sa fenêtre parente. Les marges d'ombre le gardent en contact, quel que soit le bord où se trouve le pointeur. Il peut aussi le fermer de lui-même : `native_window::sync` en ouvre alors un autre au mouvement suivant.

Les essais utiles et leurs limites figurent dans le [guide visuel](../guides/visual-checks.md) ; les résultats datés dans le [bilan des fenêtres](../reports/2026-10-15-windows.md).

## Glisser-déposer de fichiers

Sous Windows, le ghost utilise une fenêtre GPUI possédée, transparente aux clics,
sans activation et sans barre des tâches. Il suit le pointeur et sa zone de
travail, y compris hors du propriétaire. Il devient temporairement topmost pendant
la session OLE, puis est détruit au dépôt, à l'annulation ou à la fermeture du
propriétaire. Les placements natifs sont différés et regroupés via `Platform`.
Le curseur reste inchangé ; le ghost indique l'action négociée ou le refus.
Le relâchement ou l'annulation masque le ghost dès `QueryContinueDrag`, avant
l'appel au `Drop` étranger, qui peut durer. Ce masquage invalide les placements
en attente et les mesures tardives ; aucun message de placement ne peut rouvrir
la surface. La fin de session conserve séparément l'acquittement du transfert.

Les interactions internes restent GPUI. À la sortie de la fenêtre, lors du passage
sur une fenêtre étrangère superposée ou d'un changement d'application, les chemins
réels préalablement vérifiés en arrière-plan sont confiés à une session OLE,
lancée par message après libération des emprunts GPUI. Le drag interne est alors
retiré ; un retour dans Oxyde passe par le même dépôt externe que les autres
programmes. Les membres virtuels d'archives ne sont pas exportés sans préparation.

Sous Linux, en Wayland, GPUI confie le geste au compositeur quand le pointeur quitte
la fenêtre (`app/drag/outgoing.rs`). Il offre les chemins en `text/uri-list` ; le
récepteur copie ou déplace lui-même, comme depuis Nautilus, et Oxyde voit le résultat
par sa surveillance de dossiers. Le compositeur n'affiche alors aucun ghost, GPUI ne
lui en fournissant pas. Un retour dans Oxyde reprend le glisser interne, donc son
déplacement. Sélection refusée en entier s'il s'y trouve un membre d'archive, un nom
non UTF-8, ou depuis la corbeille : le geste reste alors interne. GPUI n'offre pas de
fichiers en X11, où il reste toujours interne.

`Platform::install_file_drop_target` remplace le récepteur GPUI Windows afin de
transmettre les modificateurs, les effets autorisés et le refus des zones sans
cible. Les dossiers, listes (détails/grille), entrées physiques de la barre
latérale et onglets partagent les cibles d'`app/drag/external.rs`. Les opérations
et conflits restent dans les parcours asynchrones habituels. Le dépôt sur l'espace
libre des onglets ouvre les dossiers après vérification en arrière-plan.

Oxyde effectue lui-même les déplacements entrants. Le récepteur signale un
déplacement optimisé (`Performed DropEffect = NONE`, retour `NONE`) pour que la
source étrangère ne supprime pas les fichiers une seconde fois. Pour un départ,
seule la confirmation explicite d'un déplacement non optimisé demande le retrait
des sources par le moteur d'opérations. La session transporte toute la sélection
dans un unique objet `CF_HDROP`, sans modifier le presse-papiers. Les éléments
virtuels fournis uniquement en flux par une autre application ne sont pas acceptés.

Linux conserve les événements entrants GPUI et les cibles communes. La session
sortante OLE et la fenêtre indépendante sont spécifiques à Windows.
