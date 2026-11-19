# Audit UI-APP-03

Lot : fenêtre des doublons (sources, actions de titre, fenêtre) et fenêtre de sélection de fichiers (Open/Save).

## Fichiers lus en entier

- [x] `src/ui/app/duplicates/sources.rs`
- [x] `src/ui/app/duplicates/title_actions.rs`
- [x] `src/ui/app/duplicates/window.rs`
- [x] `src/ui/app/file_chooser.rs`
- [x] `src/ui/app/file_chooser/additions.rs`
- [x] `src/ui/app/file_chooser/arrange.rs`
- [x] `src/ui/app/file_chooser/band.rs`
- [x] `src/ui/app/file_chooser/browse.rs`
- [x] `src/ui/app/file_chooser/choices.rs`
- [x] `src/ui/app/file_chooser/create.rs`
- [x] `src/ui/app/file_chooser/crumbs.rs`
- [x] `src/ui/app/file_chooser/details.rs`
- [x] `src/ui/app/file_chooser/filter.rs`
- [x] `src/ui/app/file_chooser/footer.rs`
- [x] `src/ui/app/file_chooser/grid.rs`
- [x] `src/ui/app/file_chooser/keys.rs`
- [x] `src/ui/app/file_chooser/location.rs`
- [x] `src/ui/app/file_chooser/menus.rs`
- [x] `src/ui/app/file_chooser/navbar.rs`
- [x] `src/ui/app/file_chooser/overwrite.rs`
- [x] `src/ui/app/file_chooser/render.rs`

Aucun constat critique ni élevé. Pas d'unwrap, d'expect ni d'indexation atteignable dans ces fichiers. Les résultats asynchrones des listages sont bien écartés par le remplacement des tâches. Le renommage est validé côté commande (même dossier parent).

## Constats

### Moyen

1. `location.rs:118` — moyen — confirmé. Un clic gauche hors du champ du chemin le soumet toujours, même si le texte n'a pas changé. Le chemin résolu est alors le dossier courant, et `show_folder` recharge le dossier, vide la liste, efface la sélection et remet le défilement à zéro. Scénario : Ctrl+L (ou clic sur la zone vide de la barre), puis clic sur une entrée. La sélection est perdue après coup et la liste clignote. Un double clic ouvre une liste déjà vidée.

2. `browse.rs:312` — moyen — plausible. Entrée (via `activate`) agit sur `shown` et la sélection d'avant la fin du tri ou du filtre en arrière-plan. La première correspondance n'est sélectionnée qu'à l'application du résultat de `arrange`. Scénario : dans un gros dossier, saisir un filtre puis appuyer aussitôt sur Entrée. L'entrée précédemment sélectionnée, désormais masquée par le filtre, est ouverte ou renvoyée à l'application.

3. `browse.rs:100` — moyen — plausible. Quand un filtre de type nomme des types MIME, le listage n'est publié qu'après avoir lu le type de chaque fichier du dossier (stat, puis lecture des octets magiques). Cette tâche n'est pas annulable : un changement de dossier l'abandonne, mais elle s'exécute jusqu'au bout. Aucun cache n'existe entre deux visites. Scénario : portail Linux avec filtre MIME, dossier de dizaines de milliers de fichiers ou partage réseau. La fenêtre reste en « chargement » longtemps, et chaque Retour ou F5 recommence.

### Faible

4. `browse.rs:171` — faible — confirmé. `arrange` force l'état « Prêt » au retour de n'importe quel tri ou filtre. Dans un dossier inaccessible (état Échec), cliquer un en-tête de colonne ou changer le filtre de type remplace le message d'erreur (dont « accès refusé ») par « dossier vide ». Même effet pendant le chargement : un tri affiche « vide » avant l'arrivée du listage.

5. `browse.rs:312` — faible — confirmé. Entrée dans la liste ou le filtre passe par `activate`, puis `accept`, qui ne vérifie pas `can_accept`. Le bouton « attend une interaction » (`needs_interaction`) est donc contourné par le clavier. Scénario : en choix de dossier, un premier Entrée renvoie aussitôt le dossier courant.

6. `browse.rs:68` — faible — plausible. `go` supprime le champ du chemin, le champ de renommage et le filtre, sans redonner le focus à la fenêtre (pas d'accès à `window`). Si le champ supprimé avait le focus, le clavier ne répond plus (Échap, Entrée, flèches) jusqu'à un clic dans la liste. Scénario : édition du chemin ou renommage en cours, puis clic sur un fil d'Ariane, un lieu de la barre latérale ou le bouton souris Précédent. La soumission différée de ces champs retourne tôt car le champ n'existe plus, donc personne ne restaure le focus.

7. `browse.rs:70` — faible — plausible. `go` réinitialise le menu et le menu contextuel des miettes, mais pas `crumbs.request`. Scénario : clic sur un chevron (`navbar.rs:66`), listage lent, puis navigation ailleurs. À l'arrivée du résultat, la requête est jugée à jour et le menu de l'ancien chevron s'ouvre sur le nouveau dossier. En cas d'échec du listage, rien ne s'affiche et aucune erreur n'est signalée.

8. `navbar.rs:150` — faible — plausible. « Copier le chemin » copie le texte interne des miettes. Sous Linux, ce texte encode les octets non UTF-8 par NUL suivi de deux chiffres hexadécimaux, au lieu du chemin réel ou affichable. Scénario : dossier dont le nom n'est pas UTF-8, clic droit sur la miette, puis coller.

9. `navbar.rs:284` — faible — confirmé. Une première miette finissant par deux-points est traitée comme une lettre de lecteur sans test de plateforme. Sous Linux, pour `/data:/x`, la miette prend le chemin relatif `data:/`. Le clic sur la miette et le menu du chevron suivant naviguent alors vers un chemin relatif inexistant, et l'étiquette passe par `drive_label`.

10. `location.rs:39` — faible — plausible. `typed_path` conserve sous Windows un préfixe de lecteur seul (`D:`) sans racine. `target` et `list_dir` interprètent alors le dossier courant du lecteur D pour le processus, et non `D:\`. Scénario : saisir `D:` dans la barre de chemin. Le dossier affiché dépend du répertoire courant mémorisé pour ce lecteur. `find_duplicates` traite déjà ce cas à part.

11. `file_chooser.rs:368` — faible — plausible. Le retrait de la requête par l'application ferme la fenêtre sans `close_transients` (contrairement à `finish` et à la fermeture système) et sans `remember`. Scénario : menu du pied de page ou menu d'un chevron ouvert au moment où l'application annule. Une fenêtre de menu possédée peut survivre à la fenêtre, et les préférences de vue ne sont pas mémorisées.

12. `file_chooser.rs:387` — faible — confirmé. Le dossier initial (dernier dossier, dossier du client) n'est pas vérifié, et `remember` (ligne 496) mémorise le dossier courant même en cas d'échec de listage. Scénario : clé USB débranchée ou dossier supprimé. La fenêtre s'ouvre sur une erreur à chaque fois, au lieu de retomber sur le candidat suivant (dossier par défaut, dossier personnel).

13. `create.rs:34` — faible — plausible. La recherche d'un nom libre est une boucle sans borne, non annulable, dans un thread d'arrière-plan. Scénario : un système de fichiers qui répond « existe déjà » pour tous les noms (montage FUSE ou SMB défaillant) ou une chaîne de localisation sans numéro. La boucle tourne sans fin, même après la fermeture de la fenêtre.

14. `create.rs:53` — faible — plausible. Si la fenêtre est fermée pendant la création, la mise à jour est abandonnée. Le dossier existe mais n'est pas inscrit dans l'historique du fenêtre principale (pas d'annulation possible) et aucun message n'est affiché.

15. `create.rs:63` — faible — plausible. `begin_rename` donne le focus au champ de renommage juste après avoir lancé le re-listage, alors que la ligne portant ce champ n'existe pas encore. Les frappes faites avant l'arrivée du listage vont dans le vide. Si le dossier est masqué ou filtré, le renommage reste actif sans champ visible.

16. `band.rs:49` — faible — plausible. Le rectangle de sélection garde `initial` (positions de l'ancien tableau). `go` et `arrange` ne le réinitialisent pas. Scénario : navigation (bouton souris Précédent) ou Ctrl+H pendant le tracé, avec Ctrl ou Maj. Des positions étrangères, éventuellement au-delà de la liste, entrent dans la sélection. `can_accept` devient vrai avec des entrées fantômes, et `accept` ne renvoie rien ou renvoie le dossier courant en choix de dossier.

17. `overwrite.rs:36` — faible — plausible. `self.overwrite` n'est renseigné qu'après l'ouverture différée de la fenêtre. Une seconde confirmation ou une fermeture du sélecteur dans cet intervalle laisse une invite orpheline. Son bouton Remplacer échoue silencieusement car la fenêtre cible n'existe plus. Le cas est étroit.

18. `browse.rs:140` — faible — plausible. Les chevrons de sous-dossiers viennent de `get_sidebar_nodes`, qui ne décode pas le chemin interne et renvoie des chemins avec perte (`to_string_lossy`). Sous Linux, pour les dossiers ou sous-dossiers non UTF-8, la requête échoue ou les clés ne correspondent pas aux chemins des entrées. Aucun chevron n'apparaît (échec avalé par un `else return`).

19. `sources.rs:37` — faible — plausible. La sélection est indexée par chemin, et la ligne « courant » est ajoutée même si son chemin est celui d'un lecteur (`lifecycle.rs:129`, sans dédoublonnage). Scénario : onglet actif à la racine d'un lecteur. Deux lignes portent le même chemin et se cochent ensemble, et un clic sur l'une décoche aussi l'autre.

### Optimisation

20. `browse.rs:231` — optimisation — confirmé. `position_rows` alloue à chaque appui de flèche un vecteur de toutes les positions, puis un petit vecteur par ligne (une ligne par entrée en vue Détails). Coût O(n) en allocations sur le thread UI, répété à la cadence de répétition clavier (par exemple 100 000 entrées donnent 100 000 allocations par frappe). Un calcul direct par colonnes suffirait.

21. `details.rs:45` — optimisation — plausible. `measure_columns` forme sur le thread UI, dans le rendu, un texte par taille distincte du dossier (`shape_line`) à chaque changement de `column_revision`, donc à chaque dossier ou rechargement. Sur un dossier de dizaines de milliers de fichiers aux tailles distinctes, c'est un gel mesurable à chaque navigation.

22. `file_chooser.rs:377` — optimisation — plausible. L'observateur de la fenêtre principale reconstruit `Places::of` à chaque notification de `OxydeApp`. Cela clone les épingles, lecteurs et volumes, et appelle `dirs::home_dir()` (appel système sous Windows) sur le thread UI, pour des notifications qui ne changent presque jamais les lieux.

23. `browse.rs:102` — optimisation — confirmé. `list_dir` trie déjà le résultat, puis `arrange` le retrie. `list_dir_unsorted` existe pour ce besoin. De plus `arrange.rs:126-136` clone chaque entrée admise et recalcule les libellés de type (une allocation par entrée) pour tout le dossier à chaque frappe du filtre, alors que ces ensembles ne dépendent pas du filtre.

24. `navbar.rs:84` — optimisation — plausible. Le menu d'un chevron ne veut que des noms de sous-dossiers mais passe par `list_dir` (métadonnées de toutes les entrées, tri). `crumb_menu` clone ensuite tout le vecteur et reconstruit tous les éléments à chaque rendu tant que le menu est ouvert. Coût notable pour un dossier de dizaines de milliers d'entrées.

25. `browse.rs:133` — optimisation — plausible. Chaque listage relance `get_sidebar_nodes`, qui relit le dossier et lit un niveau de chaque sous-dossier pour les chevrons. Les lectures sont donc redondantes avec `list_dir` et repétées à chaque Retour, F5 et Ctrl+H. Les lectures déjà lancées ne sont pas interrompues par une navigation. Coût important sur un partage réseau.

26. `title_actions.rs:33` — optimisation — plausible. « Tout réduire » insère un indice par groupe dans l'ensemble `collapsed`, et `prepare_results` clone cet ensemble à chaque frappe du filtre. Coût O(groupes) sur le thread UI pour un grand nombre de groupes.

<!-- LOT TERMINÉ -->
