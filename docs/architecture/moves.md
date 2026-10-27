# Déplacements Linux

**Statut : en cours** (septième domaine de l'option B, après les [copies](copies.md)). Ce
document deviendra le contrat du domaine une fois terminé.

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
  destination et que le volume est le même ; le noyau tranche (`EXDEV`), et le chemin
  actuel reprend alors pour ce dossier seul. La fusion dans un dossier existant garde le
  chemin actuel, comme Nautilus qui fusionne élément par élément.
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
   volume, journal `renamed_dirs`, Annuler et Rétablir.
3. **Bilan.**
