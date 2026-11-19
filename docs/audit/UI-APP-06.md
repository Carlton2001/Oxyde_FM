# Audit UI-APP-06

Fichiers lus en entier :

- [x] `src/ui/app/properties.rs`
- [x] `src/ui/app/properties/dialog.rs`
- [x] `src/ui/app/properties/general.rs`
- [x] `src/ui/app/properties/interactions.rs`
- [x] `src/ui/app/properties/loading.rs`
- [x] `src/ui/app/properties/permissions.rs`
- [x] `src/ui/app/properties/permissions/enclosed.rs`
- [x] `src/ui/app/properties/permissions/item.rs`
- [x] `src/ui/app/properties/permissions/save.rs`
- [x] `src/ui/app/properties/section.rs`
- [x] `src/ui/app/properties/selection.rs`
- [x] `src/ui/app/properties/shortcut.rs`
- [x] `src/ui/app/properties/tabs.rs`
- [x] `src/ui/app/properties/window.rs`
- [x] `src/ui/app/quit.rs`
- [x] `src/ui/app/remote.rs`
- [x] `src/ui/app/rename.rs`
- [x] `src/ui/app/render.rs`

Bilan : 0 critique, 0 élevé, 5 moyens, 9 faibles, 2 optimisations.

## Moyen

1. `src/ui/app/properties/shortcut.rs:136` (appelé depuis la ligne 107) — moyen — plausible.
   Dans le repli sans popup natif, le menu « Exécuter » est construit pendant le rendu de `PropertiesWindow` et relit cette même entité (lecture de la largeur du déclencheur) alors qu'elle est en cours de mise à jour, ce qui provoque un panic de double emprunt dans GPUI.
   Scénario : sous Windows, la création de la surface popup a échoué pour la fenêtre (`popup::supported` renvoie faux), l'utilisateur ouvre l'onglet Raccourci d'un `.lnk` puis déplie la liste « Exécuter ». Le chemin est rare, mais il est fatal pour toute l'application.

2. `src/ui/app/rename.rs:30` — moyen — confirmé.
   F2 (et l'action Renommer) prend le premier élément de `selected_paths`, qui est un `HashSet` sans ordre, au lieu du curseur clavier ou de l'ancre de sélection.
   Scénario : trois fichiers sont sélectionnés et l'utilisateur appuie sur F2. L'élément renommé est arbitraire, et il peut être un autre fichier que celui sur lequel se trouve le curseur.

3. `src/ui/app/remote.rs:117` (et 127-130) — moyen — confirmé.
   Quand l'onglet quitte l'adresse, `cancel_remote_connection` ne fait que lever le drapeau. La fenêtre d'identification déjà affichée reste ouverte, et le worker reste bloqué sur `blocking_recv` jusqu'à la réponse de l'utilisateur.
   Scénario : l'utilisateur ouvre `smb://A`, l'invite de mot de passe s'affiche, puis il clique sur `smb://B` dans le même onglet. L'invite de A reste à l'écran. La demande de B arrive, trouve `remote_login` occupé et est annulée d'office : B échoue avec Annulé et l'onglet revient en arrière. Si l'utilisateur saisit ensuite les identifiants de A, la connexion obsolète monte quand même le partage.

4. `src/ui/app/properties/permissions/save.rs:44-46` (avec `properties.rs:30-34` et `window.rs:95`) — moyen — plausible.
   Le drapeau d'annulation de l'application des droits est celui de la fenêtre, levé dans `Drop`. La fermeture système de la fenêtre pendant l'enregistrement n'est pas bloquée, car `on_window_should_close` renvoie toujours vrai. Seuls le bouton et Échap contrôlent `saving`.
   Scénario : un dossier avec l'option « contenu » est en cours d'application et l'utilisateur fait Alt+F4 ou ferme depuis la barre des tâches. La récursion s'arrête à mi-arbre sans aucun message, les droits sont appliqués à moitié, et la vue principale n'est pas rafraîchie.

5. `src/ui/app/rename.rs:26-41` (et 93) — moyen — plausible.
   Ni `execute_renommer` ni `begin_rename` ne refusent la corbeille. `keyboard.rs` ne garde que `pending_trash_delete`, alors que le collage est refusé explicitement dans `clipboard.rs:55`.
   Scénario : dans `trash://`, l'utilisateur sélectionne un élément et appuie sur F2. Le chemin renommé est le chemin physique du magasin de la corbeille ($R… sous Windows, fichier de `Trash/files` sous Linux). Renommer ce chemin désynchronise l'élément de ses métadonnées ($I ou .trashinfo), ce qui rend sa restauration incorrecte.

## Faible

6. `src/ui/app/properties/interactions.rs:148-173` — faible — confirmé.
   L'écriture du raccourci n'est pas annulable et se termine même si la fenêtre a disparu. Dans ce cas, `window_handle.update` échoue et `refresh_all` n'est jamais appelé.
   Scénario : OK sur un `.lnk` modifié, puis fermeture immédiate de la fenêtre par Alt+F4. Le fichier est bien modifié, mais la liste garde l'ancienne icône ou cible jusqu'au prochain événement du watcher.

7. `src/ui/app/properties/selection.rs:22-30` — faible — confirmé.
   Un seul chemin dont `get_file_properties` échoue fait échouer toute l'inspection : le résumé et les permissions sont perdus, et seule une erreur s'affiche. La chaîne « Cancelled » est de plus affichée telle quelle, en anglais.
   Scénario : 20 éléments sont sélectionnés, l'un est supprimé ou déplacé entre-temps, puis Alt+Entrée. La fenêtre n'affiche que « Échec de l'opération » sans aucune propriété.

8. `src/ui/app/rename.rs:407-410` — faible — plausible.
   `create_new_item` n'écarte que `search://`. Ctrl+Maj+N (keyboard.rs:96) n'est pas filtré selon l'emplacement. Sur `trash://`, `__network_vincinity__` ou un URI distant, le nom est joint à un chemin virtuel, ce qui donne un chemin relatif.
   Scénario : Ctrl+Maj+N dans la corbeille. La création est tentée sur un chemin relatif, résolu par rapport au répertoire courant du processus. Le meilleur cas est un toast d'erreur technique ; le pire est une création au mauvais endroit si le parent relatif existe.

9. `src/ui/app/properties/window.rs:30-38` — faible — plausible.
   Aucune fenêtre déjà ouverte pour les mêmes chemins n'est réutilisée, et il n'existe pas de registre des fenêtres Propriétés.
   Scénario : Alt+Entrée répété (ou Alt+Entrée suivi d'un clic) sur un gros dossier. Chaque appel ouvre une nouvelle fenêtre qui lance son propre calcul de taille, d'où N parcours concurrents du même arbre.

10. `src/ui/app/rename.rs:341-353` — faible — plausible.
    Après un renommage réussi, seuls la sélection, l'ancre et le curseur des onglets suivent le nouveau nom. Le chemin des onglets situés dans le dossier renommé, ou sur lui, n'est pas réécrit, pas plus que leurs piles précédent/suivant.
    Scénario : le dossier `A` est ouvert dans un deuxième panneau, puis renommé en `B` depuis le premier. Le second panneau garde `A` en chemin, et le prochain rafraîchissement échoue ou renvoie à l'accueil.

11. `src/ui/app/properties/interactions.rs:111` — faible — plausible.
    Une cible vide est refusée comme invalide. Or `get_shortcut_info` peut renvoyer une cible vide pour un raccourci qui ne vise pas le système de fichiers (élément du shell, application empaquetée).
    Scénario : l'utilisateur modifie seulement le commentaire d'un tel `.lnk`. OK affiche « cible invalide » et le raccourci ne peut plus être édité.

12. `src/ui/app/rename.rs:252-255` — faible — plausible.
    Le mot de passe est enregistré (ou oublié) pour la destination avant la mise en file de l'opération, donc avant tout conflit de nom.
    Scénario : l'utilisateur crée `docs.zip` alors qu'une archive protégée du même nom existe, puis annule au dialogue de conflit. Le mot de passe mémorisé de l'archive existante a été remplacé ou effacé, et un nouveau mot de passe lui est demandé.

13. `src/ui/app/properties/permissions/save.rs:97-98` — faible — confirmé.
    Après l'erreur « modifié depuis l'ouverture » (Changed), les droits chargés (`loaded`) ne sont pas relus, et la comparaison de l'application suivante repart des mêmes valeurs périmées.
    Scénario : `chmod` externe pendant que la fenêtre est ouverte, puis OK. L'erreur s'affiche à chaque nouvel essai ; seule la réouverture de la fenêtre corrige.

14. `src/ui/app/properties/dialog.rs:52` — faible — confirmé.
    `validation_error` n'est effacée qu'au lancement d'un enregistrement. La modification d'un champ ou le changement d'onglet ne la retire pas, et elle s'affiche sur tous les onglets.
    Scénario : cible de raccourci vidée puis OK (message affiché), cible corrigée, passage sur l'onglet Général. Le message d'erreur est toujours visible tant qu'on n'a pas relancé OK.

## Optimisation

15. `src/ui/app/properties/selection.rs:53-59` — optimisation — confirmé.
    La détection des éléments inclus dans un dossier sélectionné compare chaque élément à tous les autres (n²) avec un `starts_with` de chemin pour chaque dossier. Elle s'exécute sur le thread bloquant, mais elle retarde l'affichage du résumé.
    Scénario : Ctrl+A sur 50 000 fichiers, puis Alt+Entrée. Le calcul atteint plusieurs milliards de comparaisons avant que le premier total n'apparaisse ; un tri par chemin ou un ensemble de préfixes de dossiers suffirait.

16. `src/ui/app/render.rs:240` — optimisation — confirmé.
    Chaque rendu clone l'arbre complet des panneaux (`LayoutNode`), alors que `render_layout_node` ne prend que `&self`. Le clone copie, pour chaque onglet, `selected_paths`, les piles précédent/suivant, les filtres et les jeux de largeurs de colonnes. Seule la liste de fichiers est partagée par `Arc`.
    Scénario : une sélection de 200 000 éléments (Ctrl+A) dans un dossier. Chaque image, y compris pendant un défilement ou un survol, clone 200 000 chaînes. Emprunter `self.state.layout` directement supprime ce coût.

<!-- LOT TERMINÉ -->
