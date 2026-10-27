# Déplacements Linux

**Statut : domaine terminé** (septième domaine de l'option B, après les
[copies](copies.md)) ; bilan ci-dessous, suite Windows encore à passer. Ce document est le
contrat du domaine.

## Périmètre

Déplacer et renommer sous Linux, comme Nautilus : un dossier déplacé sur son volume, un
renommage ou une restauration de corbeille sur un volume FUSE. Les déplacements d'un
volume à l'autre, qui copient puis suppriment, et la fusion dans un dossier existant
gardent le chemin actuel. Windows garde son comportement.

## Constat Linux

- Oxyde déplace un dossier en recréant son arborescence à la destination, en renommant ses
  fichiers un à un, puis en supprimant les dossiers source vidés. Nautilus (GIO
  `g_file_move`) renomme le dossier entier en un appel quand la destination est libre sur
  le même volume. Conséquences : un dossier contenant un sous-dossier en lecture seule
  (`550`) ne peut pas être déplacé, un gros dossier demande autant d'appels que de
  fichiers, et un dossier ouvert ailleurs change d'identité.
- Renommer (F2), déplacer un dossier ou restaurer depuis la corbeille exige
  `RENAME_NOREPLACE`, que ntfs-3g refuse (`EINVAL`) : ces actions échouent sur un disque
  NTFS. Seuls les fichiers et les liens ont un repli.

## Règles

- Un dossier sélectionné est renommé en entier quand son nom est libre dans la
  destination et qu'il partage avec elle le montage et le système de fichiers (même
  identifiant de montage et même périphérique, par `statx`) : c'est la condition pour que
  le noyau accepte le renommage. Le plan le décide, et le renommage n'a lieu qu'après la
  validation de toutes les destinations, comme les autres effets. La fusion dans un
  dossier existant, deux dossiers sélectionnés du même nom, un autre volume, la corbeille
  et les archives gardent le chemin élément par élément.
- Le dossier renommé est celui que la sélection a vérifié : son identité est contrôlée
  avant et suivie après, comme pour un renommage.
- Annuler et Rétablir renomment le dossier entier par la même primitive, par identité,
  sans jamais remplacer.
- Sur un volume qui refuse `RENAME_NOREPLACE`, le renommage vérifie que le nom est libre
  puis renomme : une fenêtre de course reste possible, que ce système ne permet pas de
  fermer. Les fichiers gardent leur repli par lien physique, qui n'en a pas.

## Étapes

1. **Renommer sans remplacer sur ntfs-3g** : un module partagé (`linux_rename.rs`) pour le
   renommage d'élément, la restauration de corbeille et les liens.
2. **Dossier entier** : renommage du dossier sélectionné dans un déplacement sur un même
   montage, journal `renamed_dirs`, Annuler et Rétablir (`history_renamed_dirs.rs`), qui
   remettent en place les dossiers déjà renommés si la suite échoue.
3. **Bilan.**

## Bilan

**Résultat.** Un dossier déplacé sur son disque est renommé en un appel, comme dans
Nautilus : il garde son identité, ses attributs et son contenu tel quel, un sous-dossier
en lecture seule ne bloque plus rien, et Annuler ou Rétablir le renomment de même, sans
jamais écraser un nom repris entre-temps. Renommer, restaurer depuis la corbeille et
déplacer un dossier fonctionnent maintenant sur un disque NTFS, où ils échouaient. Vérifié
par les tests sur tmpfs, Btrfs et une image NTFS (`OXYDE_TEST_NTFS` pour les tests de
renommage). Code propre au domaine : 160 lignes de production environ (`linux_rename.rs`,
`copy/whole_moves.rs`, `history_renamed_dirs.rs`) et 150 de tests.

**Fichiers communs touchés** :

| Étape | Fichiers communs | Nature |
| --- | --- | --- |
| 1. ntfs-3g | `platform/linux_impl.rs`, `linux_links.rs`, `linux_trash.rs` | un seul renommage exclusif |
| 2. Dossier entier | `platform/mod.rs`, `source_selection.rs`, `destination_tree.rs`, `file_ops/copy.rs`, `execution.rs` | méthode `same_mount` à `false` sous Windows |
| 2. Historique | `models/transaction.rs`, `file_ops.rs`, `history_file_undo.rs`, `history_move_redo.rs`, chaque constructeur de `TransactionDetails` | champ `renamed_dirs` |

**Leçons.**
- `st_dev` ne suffit pas à prédire `EXDEV` : les sous-volumes Btrfs partagent un montage
  avec des périphériques distincts, les montages liés l'inverse. `statx` donne les deux.
- Un effet décidé pendant le plan doit attendre la validation de toutes les
  destinations : un premier essai qui renommait pendant le plan laissait un dossier
  déplacé quand une autre source entrait en collision.
- Deux dossiers sélectionnés du même nom fusionnent ; les renommer en entier ferait
  entrer le second en collision avec le premier.

**Reporté.** Sous Windows, les dossiers se déplacent toujours élément par élément.
