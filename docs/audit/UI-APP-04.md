# Audit UI-APP-04

Lot : sélecteur de fichiers (save, scroll, selection, setting, sidebar, status, tree, types), opérations, préparation, filtres, instance, clavier, disposition. Lecture seule.

## Fichiers lus en entier

- [x] src/ui/app/file_chooser/save.rs
- [x] src/ui/app/file_chooser/scroll.rs
- [x] src/ui/app/file_chooser/selection.rs
- [x] src/ui/app/file_chooser/setting.rs
- [x] src/ui/app/file_chooser/sidebar.rs
- [x] src/ui/app/file_chooser/status.rs
- [x] src/ui/app/file_chooser/tree.rs
- [x] src/ui/app/file_chooser/types.rs
- [x] src/ui/app/file_operations.rs
- [x] src/ui/app/file_preparation.rs
- [x] src/ui/app/filters.rs
- [x] src/ui/app/filters/grid_menu.rs
- [x] src/ui/app/filters/panel.rs
- [x] src/ui/app/folder_handler.rs
- [x] src/ui/app/group_selection.rs
- [x] src/ui/app/grouping.rs
- [x] src/ui/app/instance.rs
- [x] src/ui/app/item_pointer.rs
- [x] src/ui/app/keyboard.rs
- [x] src/ui/app/keyboard_menu.rs
- [x] src/ui/app/layout.rs
- [x] src/ui/app/layout/geometry.rs
- [x] src/ui/app/layout/measurements.rs
- [x] src/ui/app/layout/pane.rs
- [x] src/ui/app/layout/scroll.rs
- [x] src/ui/app/layout/split_drop.rs

## Constats

### Élevé

- `src/ui/app/filters.rs:43-50` (changement de filtre), élevé, confirmé. Un changement de filtre ne touche pas à la sélection de l'onglet : les chemins sélectionnés mais masqués par le filtre restent sélectionnés, alors que Supprimer (`trash_settings/actions.rs:76`) et Copier/Couper (`clipboard.rs:29`) lisent directement l'ensemble des chemins sélectionnés, sans l'intersecter avec la liste filtrée. Scénario : Ctrl+A dans un dossier, filtre de nom qui ne laisse qu'un fichier, Suppr : tous les fichiers masqués partent aussi à la corbeille (ou sont supprimés avec Maj), l'utilisateur n'en voyant qu'un. Aucun élagage de la sélection n'existe dans la préparation ni dans les filtres.

### Faible

- `src/ui/app/file_chooser/save.rs:222-229` (résultat de l'enregistrement), faible, plausible. Le résultat de la tâche en arrière-plan est appliqué sans vérifier que le dossier affiché est resté le même ; la navigation ne l'annule pas. Scénario : Entrée avec un nom qui désigne un sous-dossier sur un partage lent, puis clic sur un autre dossier avant la réponse : la fenêtre saute vers le sous-dossier de l'ancien dossier et vide le champ de nom.
- `src/ui/app/file_chooser/save.rs:92-94` (refus des fichiers en lecture seule), faible, plausible. Le test ne regarde que l'attribut ou les bits d'écriture globaux ; il ne reflète pas le droit d'écriture réel. Scénario sous Linux : fichier en 0644 appartenant à un autre utilisateur, non refusé alors que l'écriture échouera, malgré la demande « refuser la lecture seule » de l'application.
- `src/ui/app/file_operations.rs:268-278` et `176-183` (échec d'opération), faible, confirmé. Chaque échec écrase l'avis en cours et l'éventuelle reprise de suppression (qui devient vide si le second échec n'est pas une suppression). Scénario : deux suppressions qui échouent à la suite (partage réseau coupé) : la première liste de fichiers à réessayer et son message sont perdus.
- `src/ui/app/file_operations.rs:357` (reprise après échec de suppression), faible, confirmé. L'existence d'un chemin est testée en suivant les liens : un lien symbolique brisé dont la suppression a échoué est jugé inexistant et exclu de la reprise. Scénario : suppression partielle contenant un lien brisé sur un dossier protégé ; le bouton Réessayer ne le propose pas.
- `src/ui/app/file_chooser/setting.rs:13-18` et `46-49`, faible, plausible. Une lecture d'état lancée avant la bascule, mais terminée après elle, remplace l'état tout juste écrit par une valeur ancienne (le drapeau de changement est déjà retombé). En cas d'échec de la tâche bloquante, l'état est remis à l'ancienne valeur alors que le changement a pu aboutir. Scénario : ouverture des réglages puis bascule rapide pendant que la lecture du portail est lente.
- `src/ui/app/folder_handler.rs:13-18` et `54-57`, faible, plausible. Même course que ci-dessus pour l'application par défaut des dossiers : une lecture antérieure peut écraser le résultat de la bascule, et l'échec de la tâche rétablit une valeur potentiellement fausse.
- `src/ui/app/file_chooser/sidebar.rs:52-63` (montage d'un volume), faible, plausible. La mise à jour de la fenêtre principale (lecteurs, volumes non montés, surveillants) est faite à l'intérieur de la mise à jour de la fenêtre du sélecteur ; si celle-ci est fermée pendant le montage (boîte d'authentification longue), le volume reste monté mais la fenêtre principale continue de le proposer comme non monté.
- `src/ui/app/file_chooser/tree.rs:93-103` (chargement d'un dossier de l'arbre), faible, plausible. Le résultat d'un chargement lancé avant un nouveau listage du dossier est inséré tel quel, sans tenir compte de la mise à jour intermédiaire faite par la mise à jour de l'arbre. Scénario : dossier créé entre le début du chargement et sa fin ; l'arbre n'affiche pas le nouveau dossier jusqu'au prochain relistage.
- `src/ui/app/instance.rs:13-15` (éléments reçus d'un autre processus), faible, confirmé. Un élément sans parent (racine de lecteur ou `/`) est ignoré sans trace ; un chemin relatif donne un dossier vide qui ouvre un onglet sur un chemin vide. Scénario : un appel ShowItems sur une racine de lecteur ne fait rien.
- `src/ui/app/instance.rs:25-31` (comparaison des dossiers déjà ouverts), faible, plausible. Sous Windows, seuls la casse et les séparateurs de fin sont normalisés : un préfixe `\\?\` ou un mélange de `/` et `\` fait ouvrir un doublon d'onglet au lieu de réutiliser l'onglet actif.

### Optimisation

- `src/ui/app/file_preparation.rs:119-123` (calcul de préparation), optimisation, plausible. Le calcul de tri et de filtre sur le fil bloquant n'est pas annulable ; l'abandon d'une tâche libère l'interface mais le calcul continue en gardant son permis (deux au total). Scénario : frappe rapide dans le filtre de nom sur un dossier de 100 000 entrées : jusqu'à deux calculs périmés occupent tous les permis et retardent le calcul utile.
- `src/ui/app/layout/measurements.rs:21-25` (mesure des colonnes), optimisation, plausible. La largeur de la colonne Taille est obtenue en mettant en forme chaque taille distincte du dossier sur le fil de l'interface, et la mesure est refaite après chaque préparation (tri, filtre, rafraîchissement). Scénario : dossier avec des dizaines de milliers de tailles distinctes : gel visible à chaque frappe de filtre. Seules les plus longues chaînes par unité suffiraient.
- `src/ui/app/file_chooser/sidebar.rs:201-209` et `249` (rendu des lieux), optimisation, confirmé. À chaque rendu, la liste dépliée de l'arbre est reconstruite (avec clonage des nœuds) pour les lignes, puis de nouveau pour la hauteur du contenu, puis pour le test de débordement (qui appelle encore la hauteur) ; le défilement vers le dossier la reconstruit une fois de plus. Un seul calcul par rendu suffirait.
- `src/ui/app/file_chooser/save.rs:215-219` et `111-119` (résolution du nom), optimisation, confirmé. Le chemin est testé jusqu'à quatre fois sur le disque (type strict, résolution, version étendue, lecture seule). Sur un partage réseau, chaque appel est un aller-retour ; un seul résultat réutilisé suffirait.
- `src/ui/app/item_pointer.rs:30-34` (début de pression sur un élément), optimisation, confirmé. L'ensemble complet de la sélection est cloné à chaque clic simple, même lorsque le geste ne devient jamais un rectangle de sélection. Avec une très grande sélection, c'est une copie proportionnelle à sa taille à chaque clic.

<!-- LOT TERMINÉ -->
