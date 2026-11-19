# Audit UI-COMP-01

## Fichiers lus en entier

- [x] `src/ui/components/archive_password_dialog.rs`
- [x] `src/ui/components/backdrop.rs`
- [x] `src/ui/components/calendar.rs`
- [x] `src/ui/components/client_decorations.rs`
- [x] `src/ui/components/confirmation.rs`
- [x] `src/ui/components/conflict_dialog.rs`
- [x] `src/ui/components/conflict_dialog/advanced.rs`
- [x] `src/ui/components/conflict_dialog/comparison.rs`
- [x] `src/ui/components/conflict_dialog/controls.rs`
- [x] `src/ui/components/conflict_dialog/frame.rs`
- [x] `src/ui/components/conflict_dialog/rules.rs`
- [x] `src/ui/components/context_menu.rs`
- [x] `src/ui/components/context_menu/grid.rs`
- [x] `src/ui/components/context_menu/item.rs`
- [x] `src/ui/components/dialog.rs`
- [x] `src/ui/components/dropdown.rs`
- [x] `src/ui/components/file_list.rs`
- [x] `src/ui/components/file_list/card_body.rs`

## Constats

Aucun constat critique. Les `expect` et indexations du lot sont protégés par les appelants : la fenêtre de dialogue se ferme avant le rendu quand l'état est vidé, et la file de conflits n'est créée que si la réponse est non vide. Le double Échap n'a pas lieu, car l'action d'Échap du champ de saisie court-circuite le gestionnaire de touches du cadre.

### Élevé

1. `conflict_dialog/controls.rs:126-136` (utilisé par `comparison.rs:266-281`), élevé, confirmé. Les boutons Ignorer et Remplacer agissent dès l'appui de la souris et à chaque événement clavier Entrée ou Espace, sans test de répétition ni de nombre de clics. En mode simple, chaque choix fait aussitôt passer au conflit suivant. Scénario : un double-clic sur Remplacer, ou Entrée maintenue sur le bouton focalisé, applique le choix à des conflits suivants que l'utilisateur n'a pas vus, et peut écraser des fichiers cibles sans qu'il les ait vus.

### Moyen

2. `conflict_dialog/rules.rs:12-29`, moyen, plausible (comportement voulu et couvert par un test, mais risqué pour les données). En mode avancé, le premier choix explicite affecte l'action opposée à tous les autres conflits non décidés, y compris les enfants du dossier choisi. Le bouton Appliquer devient alors actif tout de suite (`frame.rs:54-57`), sans confirmation. Scénario : l'utilisateur clique sur Ignorer pour un seul fichier, puis sur Appliquer. Tous les autres conflits sont remplacés, seul un badge « automatique » le signale, et la cible peut être plus récente que la source.

### Faible

3. `archive_password_dialog.rs:68-72`, avec `app/archive_password.rs:176-181, 187, 203`, faible, plausible. Annuler pendant la vérification du mot de passe ne retire pas le secret mémorisé au lancement de la vérification. Le retour de la tâche s'arrête avant l'oubli quand l'invite a disparu. Scénario : l'utilisateur saisit un mauvais mot de passe, valide, puis annule pendant la vérification. Le mauvais secret reste dans le magasin et sera réutilisé tel quel pour cette archive.

4. `conflict_dialog/rules.rs:71-81`, avec `comparison.rs:66`, faible, plausible. La catégorie « identique » ne repose que sur la date et la taille, et « plus récent » sur une comparaison stricte sans tolérance. Un dossier dont les deux côtés ont la même date et une taille non calculée est annoncé identique sans comparaison du contenu. Une copie vers un système de fichiers à date arrondie (FAT, exFAT) apparaît toujours « plus récente » de 1 à 2 secondes. Scénario : le filtre « Identiques » regroupe des éléments différents, et l'utilisateur les ignore en pensant qu'ils sont identiques.

5. `conflict_dialog/advanced.rs:135-137`, faible, plausible. L'indentation d'une ligne vaut 1 rem par niveau de profondeur, sans plafond, alors que la liste fait au moins 10 rems de large. Scénario : un conflit à 12 niveaux sous la racine commune laisse peu ou pas de place au nom, qui est tronqué ou invisible.

6. `conflict_dialog/frame.rs:187-189`, avec `advanced.rs:151-153`, faible, confirmé. Le cadre avale toutes les touches sauf Tab, et les lignes de la liste avancée ne sont ni focalisables ni pilotables au clavier. Scénario : au clavier seul, on ne peut ni choisir une ligne ni replier un dossier en vue avancée, et les flèches ne font rien.

7. `context_menu.rs:358`, faible, plausible. `resolve_virtual_path` fait des appels `is_file` sur les ancêtres au nom d'archive, pendant la construction du menu de fond, sur le thread UI. Scénario : un clic droit sur le fond d'un dossier dont un ancêtre porte un suffixe d'archive (par exemple `x.zip`) sur un partage réseau lent ou déconnecté fige l'interface le temps du délai réseau.

### Optimisation

8. `conflict_dialog/advanced.rs:123-243`, avec `conflict_dialog.rs:114-120`, `rules.rs:100-115` et `frame.rs:54-57`, optimisation, plausible. La liste avancée construit une ligne par conflit visible à chaque rendu, sans virtualisation, avec icône, copie du chemin et recherche de décision par ancêtre. Chaque rendu refait aussi un filtrage avec mise en minuscules de tous les noms, le calcul de la profondeur minimale sur tous les conflits, trois comptages par catégorie et le test « tout résolu ». Scénario : fusionner deux arbres de dizaines de milliers d'entrées rend chaque survol ou chaque saisie dans la recherche coûteux sur le thread UI.

9. `conflict_dialog/controls.rs:46-48`, avec `rules.rs:34-42`, optimisation, plausible. Dans « Tout remplacer » ou « Tout ignorer » en mode simple, chaque dossier appelle `record_choice`, qui parcourt tous les conflits. Le coût est de N fois D recherches de chaînes sur le thread UI. Scénario : 30 000 conflits dont 5 000 dossiers font plus de 100 millions de recherches et figent la fenêtre plusieurs secondes au clic.

10. `context_menu/item.rs:119` et `item.rs:185`, optimisation, plausible. « Ouvrir dans un nouvel onglet » et « Épingler » testent chaque fichier du dossier avec `contains` sur un `Vec<String>` de la sélection, soit un coût en F fois S. Pour « Ouvrir dans un nouvel onglet », le test précède le filtre sur le type. Scénario : « Tout sélectionner » dans un dossier de 20 000 entrées, puis ce menu, donne environ 4×10^8 comparaisons de chaînes sur le thread UI.

11. `conflict_dialog/frame.rs:37`, optimisation, plausible. `set_window_title` est appelé à chaque rendu du dialogue, donc un appel système par image (survol, saisie, redimensionnement du séparateur), alors que le titre ne change qu'au changement d'étape ou de mode.

12. `dropdown.rs:252-297`, `326-336`, `395` et `417`, optimisation, plausible. À chaque rendu, le menu façonne le texte de toutes les entrées, fait jusqu'à douze recherches de traduction par entrée pour le raccourci (deux fois), clone deux fois le vecteur d'entrées et construit toutes les lignes, même hors de la zone visible. Scénario : un menu long (valeurs de filtre de colonne, « Ouvrir avec ») est recalculé en entier à chaque rafraîchissement du menu.

13. `file_list/card_body.rs:54, 92, 105`, optimisation, plausible. Chaque carte visible fabrique à chaque rendu deux identifiants par `format!` contenant le chemin complet, et un type en majuscules (nouvelle chaîne). Scénario : le défilement ou le survol d'une grille de plusieurs centaines de cartes visibles produit des milliers d'allocations par image.

<!-- LOT TERMINÉ -->
