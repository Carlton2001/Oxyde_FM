# Audit UI-COMP-04

Fichiers lus en entier :

- [x] `src/ui/components/operations/items/icons.rs`
- [x] `src/ui/components/operations/items/outcome.rs`
- [x] `src/ui/components/operations/items/table.rs`
- [x] `src/ui/components/operations/lifecycle.rs`
- [x] `src/ui/components/operations/list.rs`
- [x] `src/ui/components/operations/view.rs`
- [x] `src/ui/components/popup/dropdown.rs`
- [x] `src/ui/components/popup/hover.rs`
- [x] `src/ui/components/popup/keyboard.rs`
- [x] `src/ui/components/popup/mod.rs`
- [x] `src/ui/components/popup/wayland.rs`
- [x] `src/ui/components/popup/window.rs`
- [x] `src/ui/components/popup/window/frame.rs`
- [x] `src/ui/components/quit_confirmation.rs`
- [x] `src/ui/components/remote_login_dialog.rs`
- [x] `src/ui/components/rename_dialog.rs`
- [x] `src/ui/components/scroll_area.rs`
- [x] `src/ui/components/scrollbar.rs`
- [x] `src/ui/components/scrollbar/scroll_top.rs`
- [x] `src/ui/components/select_menu.rs`
- [x] `src/ui/components/settings.rs`
- [x] `src/ui/components/settings/about.rs`
- [x] `src/ui/components/settings/columns.rs`
- [x] `src/ui/components/settings/compression.rs`

## Constats

### Moyen

- `src/ui/components/operations/view.rs:161` — moyen, confirmé. Chaque rendu du panneau demande à la bibliothèque un instantané complet de l'opération sélectionnée. Cet instantané reconstruit une table d'issues avec un chemin cloné par source et clone aussi la liste des sources. Il le fait sous le verrou de l'opération, que les workers prennent pour publier leur progression. Le panneau observe toute l'application (`view.rs:25`) et se redessine donc à chaque notification, y compris les ticks de progression. Scénario : suppression ou copie de plusieurs dizaines de milliers d'éléments avec la fenêtre des opérations ouverte. Le thread UI refait des copies de plusieurs mégaoctets plusieurs fois par seconde et ralentit le worker, qui attend le verrou.

- `src/ui/components/operations/items/table.rs:115` (cause dans `items.rs:232-250`, hors lot) — moyen, confirmé. Le chargement initial des lignes capture le filtre, le tri et le sens au lancement de la tâche, puis applique son résultat sans les relire. Un filtre saisi ou un en-tête cliqué pendant le chargement relance un tri sur des lignes encore vides. Le résultat du chargement, calculé avec les anciens critères, écrase ensuite la liste. Scénario : opération sur une grosse sélection, dont le chargement fait un accès disque par source. L'utilisateur tape un filtre ou clique sur « Taille » avant la fin du chargement. La liste apparaît non filtrée ou dans l'ancien ordre, alors que le champ et la flèche de tri indiquent le nouveau.

### Faible

- `src/ui/components/operations/view.rs:163` (boucle dans `items.rs:233-246`, `icons.rs:14`) — faible, confirmé. Le chargement des lignes est une boucle synchrone d'accès disque dans une seule tâche d'arrière-plan, sans point d'annulation. Remplacer ou abandonner la tâche (changement d'opération, réouverture) n'arrête donc pas le travail déjà lancé. Scénario : naviguer rapidement entre plusieurs grosses opérations laisse autant de threads à énumérer des dizaines de milliers de chemins, parfois sur des volumes réseau lents, et le résultat est jeté.

- `src/ui/components/operations/items/icons.rs:14` — faible, plausible. Pour un déplacement ou une copie dont la source a disparu, la ligne se résout sur « dossier de destination + nom d'origine ». Si un conflit a été résolu en conservant les deux, l'élément a un autre nom au final. Scénario : déplacer `a.txt` vers un dossier qui en contient déjà un, en choisissant de conserver les deux. La ligne affiche la taille, le type et l'icône du fichier préexistant, et le double-clic (`items.rs:309`) révèle ce fichier au lieu de l'élément déplacé.

- `src/ui/components/operations/items/table.rs:139` — faible, plausible. Les lignes (entrée disque, taille, type, icône) sont résolues une seule fois à la sélection de l'opération. L'ordre de tri par État est calculé avec les issues de cet instant. Seuls les badges d'état sont relus à chaque rendu. Scénario : ouvrir la fenêtre pendant une suppression ou un déplacement, avec un tri par État. L'ordre ne bouge plus quand les éléments passent à « Terminé ». La taille et le type des éléments déjà déplacés ou supprimés restent figés jusqu'à la réouverture de la fenêtre.

- `src/ui/components/popup/hover.rs:38` — faible, confirmé. Si la fenêtre se ferme avant la fin du délai, la mise à jour échoue en silence et l'intention en attente n'est jamais effacée. Le test d'identité de la cible (`hover.rs:29`) ignore alors toute nouvelle intention de même cible. Scénario : survoler la ligne « Langue » des réglages, fermer le panneau en moins de 250 ms, le rouvrir et survoler la même ligne. Le sous-menu ne s'ouvre pas au survol tant que le pointeur n'a pas touché une autre ligne.

- `src/ui/components/operations/list.rs:255` — faible, confirmé. Le commentaire annonce que la largeur reste dans les bornes affichées, mais seule la borne basse est appliquée. Un glissement très à droite enregistre une largeur arbitrairement grande dans les préférences. Scénario : tirer la séparation jusqu'au bord de l'écran, puis agrandir la fenêtre ou la rouvrir sur un écran plus large. La liste prend alors presque toute la largeur et laisse au détail le minimum.

- `src/ui/components/popup/keyboard.rs:81` — faible, plausible. Entrée et Espace déclenchent la ligne 0 quand aucune ligne n'est sélectionnée au clavier. Le survol à la souris efface la sélection (`pointer`) et ne désigne aucune ligne. Scénario : ouvrir un menu contextuel de texte au clic droit, survoler « Copier », puis appuyer sur Entrée. L'action de la première ligne (par exemple « Couper ») s'exécute, pas celle de la ligne survolée.

- `src/ui/components/rename_dialog.rs:279` — faible, confirmé. Un nom saisi qui se termine par un point (`rapport.`) a pour extension une chaîne vide selon la bibliothèque standard. L'avertissement affiche alors « extension changée de txt en (vide) » au lieu de « supprimée ». Scénario : renommer `rapport.txt` en `rapport.` pendant la frappe.

### Optimisation

- `src/ui/components/operations/items/table.rs:235` — optimisation, confirmé. À chaque rendu, la table des issues est clonée une seconde fois dans un `Rc`. La barre d'outils la parcourt ensuite neuf fois (`table.rs:194`, un parcours par état) rien que pour compter. Scénario : opération de dizaines de milliers d'éléments, fenêtre ouverte. Cela s'ajoute au coût de `view.rs:161` à chaque notification. Un seul parcours avec un compteur par état suffirait, et le partage de la table évite le clone.

- `src/ui/components/popup/window/frame.rs:90` — optimisation, confirmé. À chaque peinture d'une surface de menu, la table des bornes de lignes est clonée en entier pour la comparer à la précédente. Scénario : menu de plusieurs centaines d'entrées (liste de langues ou de formats), repeint à chaque survol ou animation. Il suffirait de comparer sans cloner et de ne copier qu'en cas de changement.

<!-- LOT TERMINÉ -->
