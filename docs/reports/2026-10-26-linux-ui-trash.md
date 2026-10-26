# Premiers essais graphiques Linux et corbeille freedesktop

Oxyde se lance et se manipule sous **Fedora 44 Workstation, GNOME/Wayland, dans une
VM VMware** ; la corbeille Linux est séparée de celle de Windows. **Windows n'a pas été
recompilé** : sa suite native reste à exécuter avant fusion.

Date : 26 octobre 2026. Branche `linux-foundation`, commits `93e9ee3` (fenêtres),
`4f24ca4` (corbeille) et le correctif de l'annulation d'une suppression qui les suit.
Reste à faire et procédure : [développement Linux](../guides/linux-development.md).

## Environnement

VM VMware, 4 vCPU, 7 Go, adaptateur SVGA II. GPUI choisit **GL sur SVGA3D** ; Vulkan
n'expose que llvmpipe. GNOME refuse les décorations serveur : GPUI passe en décorations
client. Ces résultats ne valident ni Intel, ni NVIDIA, ni KDE.

## Fenêtres (`93e9ee3`)

- `src/ui/components/client_decorations.rs` : déplacement par la barre de titre (au
  premier mouvement, double-clic pour agrandir, clic droit pour le menu de fenêtre),
  marge transparente de 10 px portant ombre, bord et zones de redimensionnement avec
  curseurs ; aucune marge sur les côtés collés, agrandis ou en plein écran.
- Le contenu se dimensionne sur `content_bounds` au lieu de la taille de fenêtre : liste,
  notifications, modales, sous-menus, menus de texte et hauteur des fenêtres d'outils.
- Dialogues et Propriétés ouverts en `WindowKind::Floating`, rattachés à leur propriétaire
  comme sous Windows ; visionneuse, recherche, doublons et opérations restent libres.
- « Plus d'options » (menu Shell Windows) masqué hors Windows.

## Corbeille (`4f24ca4` et correctif)

Code Windows inchangé, compilé seulement pour Windows. Implémentation Linux distincte :
`src/platform/linux_trash.rs`, `src/commands/ops/linux_trash.rs`,
`src/ui/app/trash_settings/linux.rs`.

- Contenu `files/X` et fiche `info/X.trashinfo` distingués : types, tailles, aperçus et
  Propriétés portent sur le contenu ; fiche sans contenu ignorée.
- Sélection exacte (`Rapport.txt` ≠ `rapport.txt`) ; chemin d'origine accepté seulement
  s'il désigne un élément unique, pour l'annulation d'une suppression.
- Restauration par `renameat2(RENAME_NOREPLACE)`, sans écraser ni recréer le dossier
  d'origine ; purge des liens sans leur cible ; cache `directorysizes` mis à jour.
- Sortie de la corbeille uniquement par restauration : Couper masqué, transferts refusés.
- Propriétés : résumé, Vider, confirmation de suppression, et réglages du bureau :
  GNOME `org.gnome.desktop.privacy`, KDE `ktrashrc` (limites par corbeille).

## Validation réellement effectuée

| Environnement / vérification | Résultat |
| --- | --- |
| `bash scripts/check-linux.sh` | Toutes les cibles, sans avertissement du projet |
| Tests ciblés Linux : cadre, menus, modales, conflits, notifications, langues, corbeille | Réussis |
| Essai manuel, profil isolé : rendu, navigation, menus, clavier AZERTY | Correct |
| Essai manuel : déplacement, redimensionnement, curseurs, ombre, dialogues rattachés | Correct |
| Essai manuel, corbeille isolée (`XDG_DATA_HOME`) : listage, casse, restauration refusée sans dossier d'origine, suppression puis restauration, Vider, absence de Couper | Correct |
| Suite Windows, KDE, GPU physiques, échelles fractionnaires | **Non exécutés** |

## Limites

L'annulation d'une suppression sous Linux n'a pas été essayée à la main après son
correctif. Les opérations de fichiers Linux gardent les risques M03–M07 de l'audit.
Les réglages GNOME modifiés depuis Oxyde sont globaux, même avec un profil isolé.
