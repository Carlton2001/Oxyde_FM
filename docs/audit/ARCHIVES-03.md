# Audit ARCHIVES-03

Résumé : 14 constats. 3 moyens, 6 faibles, 5 optimisations. Aucun critique ni élevé. Le confinement (protocole, admission, propriété des sorties, publication groupée) tient. Les défauts sont fonctionnels (création de dossier, casse sous Linux), de ressources ou de coût d'E/S.

## Fichiers lus en entier

- [x] `src/systems/archive_open_cache/recovery/reclaim.rs`
- [x] `src/systems/archive_owned_tree.rs`
- [x] `src/systems/archive_recovery.rs`
- [x] `src/systems/archive_volumes.rs`
- [x] `src/systems/archive_workspace.rs`
- [x] `src/systems/archive_workspace/native_commit.rs`
- [x] `src/utils/archive.rs`
- [x] `src/utils/archive_codec.rs`
- [x] `src/utils/archive_io_failure.rs`
- [x] `src/utils/archive_io_protocol.rs`
- [x] `src/utils/archive_io_state.rs`
- [x] `src/utils/archive_io_stream.rs`

## Constats

1. `src/systems/archive_workspace.rs:455`
   - Gravité : moyen. Confiance : confirmé.
   - Défaut : l'erreur de `ArchiveWorkspace::create_directory` passe par une `String` puis par `io::Error::other`. Le genre `AlreadyExists` est donc perdu, alors que la boucle « Nouveau dossier (n) » de `src/ui/app/rename.rs:450` ne passe au nom suivant que sur ce genre.
   - Scénario : créer un nouveau dossier dans un ZIP qui contient déjà « Nouveau dossier ». L'opération échoue avec un toast d'erreur au lieu de créer « Nouveau dossier (1) ». Corrigé, chaque essai referait en plus une extraction et une reconstruction complètes.

2. `src/systems/archive_workspace.rs:448`
   - Gravité : moyen. Confiance : confirmé.
   - Défaut : `create_directory_inner` prend le verrou de mutation par un `lock()` bloquant, sans le point d'annulation de `lock_for_operation`. Il ouvre et reconstruit ensuite l'archive avec une `FileOperation` non enregistrée (`mutation_operation`), donc sans progression ni annulation possibles.
   - Scénario : créer un dossier dans une archive de plusieurs Go, ou pendant qu'un transfert long tient le verrou de la même archive. La tâche `spawn_blocking` attend ou travaille jusqu'à 2 × 30 min (budget WORKSPACE), sans retour visuel et sans moyen d'arrêt.

3. `src/utils/archive_io_state.rs:80`
   - Gravité : moyen. Confiance : confirmé.
   - Défaut : le refus des doublons compare les chemins après `to_lowercase()` sur toutes les cibles, Linux compris, où le système de fichiers distingue la casse. Le décodage de production (`systems/archive_io.rs:136`) l'utilise sur les deux OS, et `docs/architecture/archives.md` ne documente pas cette limite.
   - Scénario : sous Linux, une archive qui contient `Makefile` et `makefile` (ou `xt_DSCP.c` et `xt_dscp.c`) est refusée en entier avec « Invalid archive broker protocol », pour l'extraction comme pour un espace de travail. Sous Windows, `to_lowercase` diffère aussi de la table de majuscules de NTFS (ς/σ). Ces collisions échouent sans danger à la création, mais avec une erreur obscure.

4. `src/systems/archive_owned_tree.rs:463`
   - Gravité : optimisation. Confiance : confirmé.
   - Défaut : `remove_selected` filtre toutes les clés de fichiers et de dossiers, puis recalcule `name_bytes` sur toutes les entrées (ligne 509) à chaque appel. `archive_transfer.rs:205-208` l'appelle une fois par élément sélectionné.
   - Scénario : supprimer 10 000 éléments d'une archive de 100 000 entrées coûte de l'ordre de 10⁹ comparaisons de chemins avant même la reconstruction.

5. `src/systems/archive_workspace.rs:203`
   - Gravité : optimisation. Confiance : confirmé.
   - Défaut : la seconde empreinte complète de la source s'exécute aussi sous Windows, alors que le commentaire juste au-dessus la réserve aux autres systèmes (la source y est déjà protégée en écriture).
   - Scénario : modifier une archive de 20 Go sous Windows la lit trois fois en entier (deux à l'ouverture, une à la publication). La seconde lecture consomme aussi le budget de 30 min.

6. `src/systems/archive_workspace.rs:191`
   - Gravité : optimisation. Confiance : plausible.
   - Défaut : un `WalkDir` complet cherche des liens après un décodage de production. Pourtant `archive_io::decode`, via `OwnedTree`, ne crée que des fichiers et des dossiers : `Entry` ne porte aucun type lien.
   - Scénario : chaque ouverture d'espace de travail ajoute un parcours disque de jusqu'à 100 000 entrées, sans effet de sécurité en production.

7. `src/systems/archive_workspace.rs:414`
   - Gravité : optimisation. Confiance : confirmé.
   - Défaut : une source virtuelle de copie ou de glissement ouvre un espace de travail complet (`resolve_for_operation(source, false)` depuis `archive_transfer.rs:68`). Ce chemin ignore l'extraction ciblée qui existe déjà (`prepare_selection_owned`).
   - Scénario : copier un fichier de 1 Ko hors d'une archive de 30 Go décode les 30 Go dans le dossier temporaire. Au-delà des limites WORKSPACE (64 Gio, 100 000 entrées), la copie échoue.

8. `src/systems/archive_workspace.rs:592`
   - Gravité : optimisation. Confiance : confirmé.
   - Défaut : l'export copie le membre (CopyFileEx, données complètes) depuis le dossier de préparation situé dans le même dossier de destination, au lieu de le renommer sans remplacement.
   - Scénario : exporter un membre de 10 Go écrit 20 Go et double temporairement l'espace occupé.

9. `src/systems/archive_owned_tree.rs:13`
   - Gravité : faible. Confiance : plausible.
   - Défaut : chaque entrée possédée garde deux handles ouverts (le parent dupliqué et le handle de suivi) pendant toute la vie de l'arbre. Ils sont ajoutés à chaque création (lignes 154 et 226).
   - Scénario : un espace de travail Windows de 100 000 entrées garde environ 200 000 handles et 100 000 objets fichier noyau ouverts pendant tout le transfert. Cela charge le pool noyau et gêne les autres processus (antivirus, indexation) sur ces fichiers temporaires.

10. `src/utils/archive_codec.rs:85`
    - Gravité : faible. Confiance : confirmé.
    - Défaut : `XzDecoder::new_multi_decoder` crée le flux xz sans limite mémoire (u64::MAX), alors que LZMA-alone est plafonné à 1 Gio avant toute allocation.
    - Scénario : un `.xz` ou `.tar.xz` déclarant un dictionnaire de 1,5 Gio dépasse la limite dure du job (Windows) ou RLIMIT_DATA (Linux). L'échec remonte comme `ArchiveProcessingFailure` générique, et non comme une limite.

11. `src/utils/archive_codec.rs:87`
    - Gravité : faible. Confiance : confirmé.
    - Défaut : le décodeur zstd garde la fenêtre maximale par défaut (2^27). Aucun `window_log_max` n'est réglé et la limite n'est pas documentée.
    - Scénario : un `.tar.zst` produit avec `zstd --long=31` (sauvegardes volumineuses) est refusé avec une erreur de traitement générique.

12. `src/utils/archive.rs:112`
    - Gravité : faible. Confiance : confirmé.
    - Défaut : `resolve_virtual_path` fait un `is_file()` sur chaque ancêtre au suffixe d'archive, et `src/ui/components/context_menu.rs:358` l'appelle de façon synchrone en construisant le menu contextuel du panneau, donc sur le thread UI.
    - Scénario : un clic droit dans un dossier d'archive situé sur un partage réseau lent ou déconnecté fige l'interface pendant le délai d'attente SMB.

13. `src/systems/archive_volumes.rs:27`
    - Gravité : faible. Confiance : plausible.
    - Défaut : pour un jeu RAR, le `source` reçu de l'appelant est abandonné, et chaque volume est rouvert par son nom sans comparer l'identité du premier au handle déjà vérifié (empreinte et identité dans `ArchiveWorkspace`).
    - Scénario : sous Linux, où rien n'empêche le remplacement, `film.part1.rar` est substitué entre l'ouverture vérifiée et la réouverture. Le contenu copié vient alors d'un autre objet que celui qui a été contrôlé.

14. `src/systems/archive_workspace.rs:315`
    - Gravité : faible. Confiance : plausible.
    - Défaut : sous Linux, rien ne verrouille l'original entre la dernière empreinte (315) et le renommage en sauvegarde (333). La sauvegarde est ensuite supprimée (352) sans nouvelle vérification.
    - Scénario : un autre programme ajoute des données à l'archive juste après la vérification. La reconstruction publiée les ignore, et la sauvegarde qui les contenait est supprimée.

<!-- LOT TERMINÉ -->
