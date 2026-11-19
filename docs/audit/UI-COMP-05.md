# Audit UI-COMP-05

## Fichiers lus en entier

- [x] src/ui/components/settings/controls.rs
- [x] src/ui/components/settings/data_actions.rs
- [x] src/ui/components/settings/default_format.rs
- [x] src/ui/components/settings/fallback.rs
- [x] src/ui/components/settings/integration.rs
- [x] src/ui/components/settings/measure.rs
- [x] src/ui/components/settings/panel.rs
- [x] src/ui/components/settings/submenu.rs
- [x] src/ui/components/sidebar.rs
- [x] src/ui/components/sidebar/folder_row.rs
- [x] src/ui/components/sidebar/linux_row.rs
- [x] src/ui/components/sidebar/measure.rs
- [x] src/ui/components/sidebar/menus.rs
- [x] src/ui/components/sidebar/pinned.rs
- [x] src/ui/components/sidebar/rail.rs
- [x] src/ui/components/sidebar/reorder.rs
- [x] src/ui/components/sidebar/rows.rs
- [x] src/ui/components/sidebar/toolbar.rs
- [x] src/ui/components/sidebar/tree.rs
- [x] src/ui/components/sidebar/volume_row.rs
- [x] src/ui/components/tabs/mod.rs
- [x] src/ui/components/tabs/scroll.rs
- [x] src/ui/components/tabs/tab.rs
- [x] src/ui/components/tabs/tab_bar.rs
- [x] src/ui/components/title_bar.rs
- [x] src/ui/components/tool_window.rs

## Constats

Bilan : aucun défaut critique ou élevé. Le code des réglages, des onglets, de la barre de titre et des fenêtres outils est sain. Les entrées d'I/O système (gestionnaire de dossiers, sélecteur de fichiers, montage) passent par des tâches bloquantes avec garde de réentrance. Les défauts relevés se concentrent sur la barre latérale.

### 1. `src/ui/components/sidebar/reorder.rs:70` — moyen — plausible

L'overlay de réorganisation des dossiers épinglés renvoie rien dès que le dossier déplacé n'est plus dans la liste des épinglés. Or l'état de réorganisation n'est effacé que par le relâchement géré par cet overlay, et les gestionnaires de l'arbre sont retirés tant que l'état existe (`tree.rs:137`). L'état reste donc bloqué : plus aucune réorganisation possible, infobulles de la barre latérale désactivées (`tree.rs:105`), ligne cachée si le chemin revient.
Scénario : pendant le glissement, le watcher du Quick Access remplace la liste des épinglés (`shell_features.rs:144`) et retire le dossier tenu (désépinglé depuis l'Explorateur, ou dossier supprimé). Même effet si la barre est repliée pendant le glissement (le raccourci ou le bouton), l'arbre n'étant plus rendu. Aucune annulation (Échap, perte de focus) n'existe : après Alt+Tab pendant le glissement, le clic suivant valide un déplacement non voulu.

### 2. `src/ui/components/sidebar/tree.rs:35` (et `:172`, `rows.rs:49`) — optimisation — confirmé

À chaque rendu de l'arbre, la liste complète des éléments visibles est clonée en profondeur trois fois : filtrage avec copie (`tree.rs:35-41`), copie pour la fermeture de rendu (`tree.rs:172`), puis copie intégrale dans le rendu des lignes (`rows.rs:49`), alors que seules les lignes de la plage visible sont lues.
Scénario : après « Tout développer » sur un dossier volumineux (milliers de nœuds), chaque défilement à la molette, chaque survol qui notifie et chaque redimensionnement recopie toutes les chaînes (nom, chemin) trois fois, sur le thread UI.

### 3. `src/ui/components/sidebar/tree.rs:47` — optimisation — confirmé

Chaque rendu parcourt tous les éléments pour mesurer la largeur naturelle (cache de glyphes reconstruit à chaque rendu, allocations de chaînes d'affichage pour chaque élément), somme les hauteurs (`:72`), recalcule les lignes épinglables (`:22-34`) et les emplacements de réorganisation (`:119-129`), alors que seule une fenêtre de lignes est affichée.
Scénario : même arbre volumineux, coût linéaire en nombre de nœuds à chaque trame, et non en nombre de lignes visibles.

### 4. `src/ui/components/sidebar.rs:42` — faible — plausible

Le filtre de visibilité des dossiers de l'arbre examine seulement le nœud lui-même (caché ou système), alors que `refresh_sidebar` (`workspace/sidebar.rs:63-79`) développe les enfants de tout dossier déplié, caché ou non. Les enfants d'un dossier masqué restent affichés, sans leur parent, avec une indentation qui les rattache visuellement à la mauvaise ligne.
Scénario : dépliage de AppData avec « afficher les fichiers cachés » activé, puis désactivation de l'option : AppData disparaît, ses sous-dossiers non cachés restent visibles sous la ligne précédente.

### 5. `src/ui/components/sidebar/rail.rs:168` — faible — confirmé

Le rail (barre repliée) ignore les volumes non montés et les distributions WSL, et son menu contextuel de lecteur (`:226-232`) n'ajoute pas « Démonter l'image disque » que l'arbre propose (`rows.rs:256-263`). Ces actions ne sont plus joignables en mode replié.
Scénario : lecteur issu d'une image disque montée, ou volume non monté, avec la barre latérale repliée.

### 6. `src/ui/components/sidebar/pinned.rs:62` — faible — plausible

Les limites du bouton utilisées pour ignorer le clic « extérieur » du menu des épinglés (`:62`, `:143`) font 2 rem de côté, alors que la cellule du rail en fait 2,25 (`rail.rs:13`). Un clic sur la bande de 0,25 rem en bas ou à droite du bouton ferme le menu par le clic extérieur, puis le bouton le rouvre par sa bascule.
Scénario : menu des épinglés ouvert au clic ; un clic sur le bord du bouton ne le ferme pas, il le rouvre.

### 7. `src/ui/components/tabs/scroll.rs:44` — faible — plausible

Le défilement automatique pendant un glisser ne distingue pas la chaîne de trames encore en attente après un arrêt. Un arrêt suivi d'un redémarrage avant la trame suivante lance une seconde chaîne, la première reprenant dès qu'elle voit un état actif ; deux chaînes rejouent alors le déplacement de souris à chaque trame jusqu'à la fin du glisser.
Scénario : glisser un onglet en passant rapidement dans la zone de bord, en sortant, puis en y rentrant en moins d'une trame.

### 8. `src/ui/components/sidebar/rows.rs:168` — faible — plausible

Les lignes Réseau et Corbeille acceptent les dépôts internes (surbrillance, `execute_file_drop` vers leur identifiant virtuel) sans écarter ces cibles. Le chemin virtuel échoue ensuite à la validation côté traitement et l'utilisateur voit une erreur de préparation, au lieu d'une cible refusée dès le survol.
Scénario : glisser un fichier du volet sur « Corbeille » ou « Réseau » dans l'arbre.

### 9. `src/ui/components/settings/fallback.rs:18` — faible — plausible

Sans surfaces natives, le voile qui ferme le panneau ne gère que le bouton gauche et ne bloque pas les autres entrées. Le clic droit et la molette atteignent les éléments situés dessous pendant que le panneau est ouvert.
Scénario : panneau des réglages ouvert en mode repli, clic droit sur la liste de fichiers derrière : le menu contextuel de la liste s'ouvre sous le panneau.

<!-- LOT TERMINÉ -->
