# Audit FILEOPS-02

Fichiers lus en entier :

- [x] `src/systems/file_ops/delete.rs`
- [x] `src/systems/file_ops/delete/trash_batch.rs`
- [x] `src/systems/file_ops/execution.rs`
- [x] `src/systems/file_ops/extract.rs`
- [x] `src/systems/file_ops/item_outcomes.rs`
- [x] `src/systems/file_ops/overview.rs`
- [x] `src/systems/file_ops/scheduler.rs`
- [x] `src/systems/file_ops/summaries.rs`
- [x] `src/systems/folder_sizes.rs`
- [x] `src/systems/folder_sizes/pools.rs`
- [x] `src/systems/history_delete.rs`
- [x] `src/systems/history_directories.rs`
- [x] `src/systems/history_directory_attributes.rs`
- [x] `src/systems/history_file_undo.rs`
- [x] `src/systems/history_links.rs`

Résumé : 0 critique, 0 élevé, 5 moyens, 9 faibles, 4 optimisations.

## Constats

1. `src/systems/file_ops/extract.rs:91-97` et `:45-53` (avec `file_ops.rs:388`) : moyen, confirmé.
   La boucle de création de la cible ou de publication du membre relance `unique_archive_path` après `AlreadyExists`. Or ce chemin se fie à `exists()`, qui suit les liens et rend faux sur un lien cassé : il renvoie le même chemin et la boucle ne finit jamais, sans point d'annulation.
   Scénario : sous Linux, un lien symbolique cassé `photos` à côté de `photos.zip` (ou `notes` à côté de `notes.gz`). Extraire tourne à 100 % CPU, l'opération reste « en cours » et ne s'annule pas. Sa réservation de chemins bloque les opérations qui se recouvrent.

2. `src/systems/file_ops/delete.rs:315-348` avec `execution.rs:88-98` : moyen, confirmé.
   Sous Linux, la corbeille n'inscrit aucun effet dans `deleted_items`. Une mise à la corbeille partielle (un élément refusé, donc statut Erreur) ou annulée n'a donc aucun « effet ». Aucune transaction n'est poussée, et les éléments déjà déplacés ne peuvent pas être annulés par Undo.
   Scénario : on met 10 fichiers à la corbeille, l'un est en lecture seule dans un dossier protégé. Les 9 autres partent à la corbeille et Ctrl+Z ne les restaure pas.

3. `src/systems/file_ops/scheduler.rs:37-58` et `:275-281` : moyen, confirmé.
   `acquire` recalcule, toutes les 25 ms et pour chaque opération en attente, le recouvrement entre toutes les attentes et toutes les opérations actives. Chaque comparaison de paire de chemins fait deux `format!`, et le tout tourne sous les verrous `waiting` et `running_paths`. Le coût est quadratique en nombre de sources. `enqueue`, appelé depuis le thread UI via `queue_operation_on`, attend ce même verrou.
   Scénario : une copie de 20 000 fichiers en cours et une suppression de 20 000 fichiers en file. Chaque passe compare jusqu'à 4·10⁸ paires avec allocation et monopolise un worker Tokio. Mettre en file une nouvelle opération fige l'interface.

4. `src/systems/file_ops/scheduler.rs:137-230` avec `execution.rs:76` : moyen, plausible.
   Le retrait de `waiting` et `running_paths` en fin de tâche n'est pas protégé par un garde `Drop`. Si la tâche panique, la réservation de chemins fuit pour toujours et le statut reste non final. Par exemple, `op.lock().unwrap()` sur un mutex empoisonné par une panique du travail bloquant qui tenait le verrou.
   Scénario : une panique pendant une copie qui tenait le verrou de l'opération. Toute opération ultérieure sur ces dossiers reste en attente jusqu'au redémarrage.

5. `src/systems/history_file_undo.rs:484-511` : moyen, confirmé.
   Chaque sortie à écarter reçoit son propre dossier `.oxyde-undo-<uuid>`, avec un pin ouvert et un nœud `Directories` conservés jusqu'au commit. Undo d'une copie de N fichiers crée et supprime N dossiers cachés et garde environ 2N handles ouverts.
   Scénario : annuler la copie de 50 000 petits fichiers. Cela fait 50 000 créations et suppressions de dossiers en plus et 100 000 handles tenus. C'est lent, et cela peut échouer contre les limites de descripteurs sous Linux (`ulimit -n`), puis basculer en rollback.

6. `src/systems/file_ops/trash_batch.rs:91-99` et `:116-121` avec `history_delete.rs:119-126` : faible, confirmé.
   Un reçu « completed » accompagné d'une erreur passe deux fois dans les compteurs. La fermeture `completed` incrémente `processed_files` et marque l'élément terminé. L'erreur renvoyée l'incrémente encore et le marque échoué.
   Scénario : le Shell rapporte un élément déplacé avec une erreur tardive. La progression dépasse 100 % (processed > total) et l'élément apparaît en échec alors qu'il est à la corbeille.

7. `src/systems/history_delete.rs:83-90` et `:216-226` : faible, plausible.
   Si l'identité d'un élément est indisponible, il est quand même mis à la corbeille, et sans vérification. `entries()` refuse alors tout journal qui contient une identité absente : la transaction entière devient impossible à annuler.
   Scénario : une sélection mixte dont un élément vient d'un volume sans identifiant de fichier. Undo refuse de restaurer tous les éléments, pas seulement celui-là.

8. `src/systems/history_links.rs:33-45` : faible, confirmé.
   Si `create_link` réussit mais que `refresh_from_file` échoue, le rollback ne retire que les liens précédents (`links[..index]`). Le lien qui vient d'être créé reste en place alors que Redo est signalé en échec.
   Scénario : une lecture de métadonnées refusée juste après la création du lien. Un lien orphelin reste, et le Redo suivant échoue sur « destination already exists ».

9. `src/systems/history_file_undo.rs:683-702` : faible, confirmé.
   Au commit, l'échec de suppression d'une sortie mise en staging ou d'un dossier de staging n'est que journalisé. Undo annonce un succès alors qu'une copie complète reste dans un dossier caché `.oxyde-undo-*`.
   Scénario : un antivirus garde ouverte la sortie déplacée en staging. L'utilisateur croit la copie annulée, mais l'espace disque n'est pas libéré et le dossier caché reste.

10. `src/systems/file_ops/delete.rs:148-169` : faible, confirmé.
    Un élément `NotFound`, déjà disparu, n'est ni compté en échec ni marqué terminé (`succeeded` est faux). Son issue finale est « Interrompu » au lieu de « Terminé ».
    Scénario : un fichier supprimé par un autre programme entre la sélection et l'exécution. La carte d'opération indique une interruption alors que l'opération a réussi.

11. `src/systems/file_ops/delete.rs:297-307` : faible, confirmé.
    `collapse_roots` écarte les doublons et les descendants d'un dossier sélectionné. Ces sources ne reçoivent jamais `item_started` ni `item_complete`, et leur issue reste « Non traité » alors qu'elles sont à la corbeille avec leur parent.
    Scénario : on sélectionne un dossier et l'un de ses fichiers, par exemple depuis des résultats de recherche. Le fichier est affiché « non traité ».

12. `src/systems/file_ops/scheduler.rs:236-273` : faible, plausible.
    La réservation est purement lexicale. `\\?\C:\x` et `C:\x`, un lecteur mappé et son UNC, ou un chemin traversant une jonction ne se recouvrent pas. Les destinations réelles renommées (« Garder les deux », cible unique de l'extraction) ne sont pas réservées non plus.
    Scénario : deux opérations sur le même dossier via deux graphies différentes, ou une extraction vers `X (1)` pendant une copie vers `X (1)`. Elles s'exécutent en parallèle sur les mêmes objets.

13. `src/systems/folder_sizes/pools.rs:59-60` : faible, confirmé.
    `OnceCell` mémorise aussi l'erreur de construction du pool. Un échec transitoire (création de thread refusée) rend ce couloir définitivement inutilisable.
    Scénario : un pic de ressources au premier calcul de taille. Ensuite, tous les calculs de taille tombés sur ce couloir échouent jusqu'au redémarrage.

14. `src/systems/file_ops/extract.rs:98-101` : faible, confirmé.
    Si `OwnedTree::created` échoue puis que `file_identity` échoue aussi, le `?` renvoie cette seconde erreur. Le dossier cible vide reste en place et la cause d'origine est perdue.
    Scénario : l'extraction échoue sur un volume réseau. L'utilisateur voit une erreur d'identité trompeuse et garde un dossier `X (1)` vide.

15. `src/systems/file_ops/item_outcomes.rs:102-143` : optimisation, confirmé.
    `item_directory`, appelé deux fois par dossier copié (`copy.rs:463/468/480`), et `item_directory_created` parcourent toutes les entrées `item_progress`, sous le verrou de l'opération. Le coût est en O(dossiers × sources).
    Scénario : copier 5 000 dossiers sélectionnés contenant 100 000 sous-dossiers fait environ 10⁹ `starts_with`. Le verrou est tenu pendant ces parcours et ralentit le sondage de progression de l'UI.

16. `src/systems/history_file_undo.rs:400` avec `:686-691` (`file_copy.rs:376`) : optimisation, confirmé.
    Undo d'une copie lit intégralement chaque fichier deux fois. Une fois pour le hash de la vérification préalable (`verified_outputs`), puis une autre au commit (`remove_verified_output_checked`), alors que l'identité est déjà vérifiée par handle.
    Scénario : annuler la copie de 100 Go relit 200 Go avant de libérer l'espace.

17. `src/systems/history_directories.rs:30-64` : optimisation, confirmé.
    `prepare` fait un `std::fs::metadata` par chemin avant de vérifier le cache `nodes`. Il teste aussi `removed.iter().any(starts_with)` à chaque ancêtre, ce qui est quadratique.
    Scénario : annuler un déplacement de 50 000 fichiers et 5 000 dossiers fait 50 000 stats redondantes et environ 10⁸ comparaisons de chemins avant la première action.

18. `src/systems/folder_sizes.rs:150-176` : optimisation, plausible.
    Chaque fichier coûte un `entry.metadata()`, un appel système par chemin dans jwalk, alors que l'énumération fournit déjà la taille sous Windows. Chaque sous-dossier passe aussi par `excluded`, qui alloue une chaîne et un `Vec`.
    Scénario : le calcul de taille d'un arbre d'un million de fichiers fait un million de stats et d'allocations supplémentaires.

<!-- LOT TERMINÉ -->
