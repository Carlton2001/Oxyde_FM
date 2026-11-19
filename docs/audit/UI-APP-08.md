# Audit UI-APP-08

Fichiers lus en entier :

- [x] `src/ui/app/trash_settings/linux/render.rs`
- [x] `src/ui/app/trash_settings/render.rs`
- [x] `src/ui/app/trash_settings/settings.rs`
- [x] `src/ui/app/type_select.rs`
- [x] `src/ui/app/updates.rs`
- [x] `src/ui/app/viewer.rs`
- [x] `src/ui/app/viewer/audio.rs`
- [x] `src/ui/app/viewer/autoscroll.rs`
- [x] `src/ui/app/viewer/content.rs`
- [x] `src/ui/app/viewer/content_render.rs`
- [x] `src/ui/app/viewer/deletion.rs`
- [x] `src/ui/app/viewer/display_options.rs`
- [x] `src/ui/app/viewer/fullscreen_controls.rs`
- [x] `src/ui/app/viewer/image_interaction.rs`
- [x] `src/ui/app/viewer/markdown.rs`
- [x] `src/ui/app/viewer/markdown/autolink.rs`
- [x] `src/ui/app/viewer/markdown/loading.rs`

Aucun constat critique ni élevé. Les entrées sont classées par gravité.

## Moyen

1. `src/ui/app/trash_settings/settings.rs:260` (bouton : `render.rs:243`, `linux/render.rs:230`) — moyen, confirmé. Le bouton « Tout supprimer » vide la corbeille de tous les volumes sans aucune confirmation, et l'appel Windows passe le drapeau SHERB_NOCONFIRMATION. Un clic accidentel dans la fenêtre des propriétés de la corbeille détruit définitivement tout son contenu.

2. `src/ui/app/trash_settings/render.rs:37` — moyen, plausible. L'ordre de tabulation contient toujours le champ de capacité, alors que ce champ n'est pas dessiné quand « ne pas déplacer vers la corbeille » est actif ou pendant un chargement ou une sauvegarde. Un Tab depuis Annuler met le focus sur une poignée absente de l'arbre : les raccourcis Échap, Tab et Entrée du dialogue ne reçoivent plus rien jusqu'à un clic. La version Linux (`linux/render.rs:81`) ne place dans l'ordre que les champs affichés.

3. `src/ui/app/viewer/markdown.rs:195` — moyen, confirmé. Le rendu retire les tabulations finales du texte d'une ligne de tableau, donc les cellules vides en fin de ligne disparaissent. Une ligne dont les dernières cellules sont vides affiche moins de colonnes que l'en-tête, et ses cellules s'étirent : le tableau est mal aligné.

4. `src/ui/app/viewer/markdown/loading.rs:34` — moyen, confirmé. Le fichier est décodé en UTF-8 avec remplacement, sans retirer le BOM et sans gérer l'UTF-16. Un .md enregistré avec BOM UTF-8 (Bloc-notes, PowerShell) affiche son premier titre comme du texte brut, car l'analyseur n'ignore pas le BOM. Un .md en UTF-16 s'affiche en caractères de remplacement.

5. `src/ui/app/viewer/markdown/loading.rs:19` — moyen, plausible. La limite de lecture est celle du texte brut, 512 Mio. Le fichier est entièrement copié en mémoire, puis recopié si le décodage UTF-8 échoue, avant l'analyse. Un très gros .md consomme de l'ordre du gigaoctet et occupe l'unique file de travaux du Viewer pendant toute l'analyse.

6. `src/ui/app/viewer/markdown/autolink.rs:67` — moyen, plausible. Le retrait des parenthèses fermantes en fin d'adresse recompte les deux types de parenthèses sur toute l'adresse à chaque caractère retiré, soit un coût quadratique. Un paragraphe formé d'une adresse suivie de centaines de milliers de « ) » fige la tâche d'analyse, qui ne peut pas être annulée à cet endroit. Comme la file du Viewer est de taille 1, plus aucun fichier ne s'ouvre pendant ce temps.

7. `src/ui/app/updates.rs:96` (fermeture : `updates.rs:109`) — moyen, plausible. L'installeur est lancé avant la demande de fermeture. Si la fermeture est refusée (confirmation de sortie, opérations en cours) ou si la fenêtre principale est absente, l'état reste « Downloading » indéfiniment. Le bouton ne revient jamais à un état utilisable alors que l'installeur attend la fermeture d'Oxyde.

## Faible

8. `src/ui/app/trash_settings/settings.rs:82` — faible, plausible. Le chargement est tout ou rien : si la configuration d'un seul volume amovible ou fixe échoue, aucun volume n'est listé et le dialogue n'affiche qu'une erreur. Un disque dont la corbeille est illisible empêche de régler les autres.

9. `src/ui/app/trash_settings/settings.rs:168` — faible, confirmé. Le changement de volume ne valide la saisie que si le champ n'est pas vide. Si l'utilisateur vide le champ puis clique un autre volume, la saisie est abandonnée sans erreur, alors qu'Enregistrer signale l'erreur pour le même état.

10. `src/ui/app/trash_settings/settings.rs:229` — faible, confirmé. Les écritures sont séquentielles et l'état initial n'est pas mis à jour après un échec partiel. Scénario : le volume A est modifié puis écrit, le volume B échoue. L'utilisateur remet A à sa valeur d'origine et réenregistre : A est jugé inchangé, donc non réécrit, et le système garde la valeur intermédiaire alors que l'interface affiche l'ancienne.

11. `src/ui/app/trash_settings/settings.rs:281` — faible, confirmé. Si le vidage échoue en cours de route, l'occupation affichée n'est pas relue. Le compteur et le bouton restent sur des valeurs périmées alors qu'une partie des éléments a été supprimée.

12. `src/ui/app/type_select.rs:38` — faible, confirmé. L'exception « un espace initial n'est pas une recherche » exige un préfixe vide. Or le préfixe n'est jamais vidé à l'expiration, il l'est seulement à la frappe suivante. Après une première lettre tapée plus tôt, un espace démarre une recherche sans résultat qui consomme la touche et les lettres suivantes pendant une seconde. La version du sélecteur de fichiers (`file_chooser/keys.rs:80`) teste correctement l'expiration.

13. `src/ui/app/updates.rs:79` — faible, plausible. Les opérations de fichiers en cours ne sont vérifiées qu'avant le téléchargement. Une opération démarrée pendant celui-ci n'est pas revue avant le lancement de l'installeur, qui ferme Oxyde.

14. `src/ui/app/viewer.rs:451` — faible, confirmé. L'extension est prise avec un découpage qui renvoie le nom entier quand il n'y a pas de point. Un fichier sans extension nommé comme une extension audio ou vidéo (« mp3 », « flac ») ou un fichier caché comme « .mp3 » est traité comme un média natif.

15. `src/ui/app/viewer.rs:196` — faible, plausible. Les suppressions en cours sont mémorisées par Viewer. Un Viewer créé après le début d'une suppression ne les connaît pas et peut ouvrir le fichier en cours de suppression, ce qui retient un handle côté worker et peut faire échouer la suppression.

16. `src/ui/app/viewer/fullscreen_controls.rs:154` — faible, confirmé pour le premier point. Le drapeau d'attente est levé avant la récupération du handle natif de la fenêtre. Si cette récupération échoue, la fonction retourne sans le baisser et la barre plein écran n'est plus jamais créée. Plausible pour le second point (ligne 207) : après un échec d'ouverture de la fenêtre, chaque placement suivant retente l'ouverture et journalise l'erreur.

17. `src/ui/app/viewer/markdown.rs:95` — faible, confirmé. Au plus 32 entrées d'images sont gardées, et les images prêtes, en échec ou en attente ne sont jamais retirées. Au-delà des 32 premières images affichées, les suivantes restent à jamais sur leur texte de remplacement, même après défilement.

18. `src/ui/app/viewer/markdown/loading.rs:13` (images : `markdown.rs:101`) — faible, plausible. La lecture du .md et le décodage des images passent par la file de chargement sans enregistrer le fichier dans le registre de suppression, contrairement aux autres contenus (`deletion.rs:47`). Une suppression ne les attend pas : le fichier ou une image que le worker lit peut encore être ouvert pendant la suppression.

## Optimisation

19. `src/ui/app/type_select.rs:72` — optimisation, confirmé. Chaque frappe reconstruit les lignes d'affichage et la liste des noms, puis passe les noms en minuscules (une allocation par nom) jusqu'à trois fois par recherche. Dans un dossier de 100 000 entrées, cela représente jusqu'à plusieurs centaines de milliers d'allocations par touche sur le thread UI.

20. `src/ui/app/viewer.rs:243` — optimisation, plausible. À chaque notification de l'application, le Viewer recapture la source et compare l'ensemble des sélections (coût proportionnel à la taille de la sélection), puis le clone entièrement s'il a changé. Avec « tout sélectionner » dans un très gros dossier et un Viewer ouvert, chaque notification et chaque pas de sélection élastique coûtent un parcours ou une copie de tous les chemins.

21. `src/ui/app/viewer/markdown.rs:162` — optimisation, confirmé. À chaque rendu d'une ligne d'en-tête de tableau visible, le texte de tout le tableau est assemblé pour le bouton de copie, même masqué. Chaque ligne de tableau visible remonte aussi jusqu'à son en-tête. Un bloc de code est copié deux fois par rendu (texte et source du bouton). Un grand tableau ou bloc visible alloue des mégaoctets à chaque image.

22. `src/ui/app/viewer/image_interaction.rs:101` — optimisation, confirmé. Chaque événement de molette lance deux tâches détachées avec minuterie (1,2 s et 180 ms). Un pavé tactile produit des centaines de tâches vivantes en quelques secondes pour mettre à jour deux champs.

23. `src/ui/app/viewer/image_interaction.rs:211` — optimisation, confirmé. Le calcul de l'image courante d'un GIF ou WebP animé alloue un vecteur de délais à chaque image peinte (60 fois par seconde), alors que les délais sont constants pour une image donnée.

24. `src/ui/app/viewer/content.rs:144` — optimisation, plausible. L'analyse du texte d'un document Office (`parse_document`, une allocation par bloc et par cellule) s'exécute dans la fermeture de construction, donc sur le thread UI, après le travail de fond. Pour un classeur volumineux, cela peut figer l'interface avant le premier affichage.

<!-- LOT TERMINÉ -->
