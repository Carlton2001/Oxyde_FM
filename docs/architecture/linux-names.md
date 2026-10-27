# Noms et chemins Linux

**Statut : domaine terminé** (onzième domaine de l'option B, T08 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md), après les
[Propriétés](linux-properties.md)) ; bilan ci-dessous. Ce document est le contrat du
domaine.

## Périmètre

Ce que vaut un nom de fichier sous Linux dans le code commun : le seul séparateur est
`/`, `\` est un caractère de nom comme un autre, et deux noms qui ne diffèrent que par
la casse sont deux éléments, sauf sur les systèmes de fichiers qui l'ignorent. Les noms
internes des archives gardent leur politique compatible Windows.

## Constat Linux

- Fil d'Ariane, titre d'onglet, défilement du fil, complétion de la barre d'adresse,
  couleur d'icône et ancêtres des conflits découpent les chemins sur `/` et `\`. Un
  dossier `a\b` apparaît comme deux étapes `a` et `b`.
- `paths_match` (accès rapide épinglé, fil des ancêtres, images disque) compare sans
  la casse et avec `\` : `Notes` épinglé marque aussi `notes`.
- Le glisser depuis d'autres applications lit `\` comme séparateur pour refuser de
  déposer un dossier dans lui-même.
- Le nom d'une archive à créer refuse `\ : * ? " < > |` et un point ou une espace finale,
  qui sont des noms Linux valides.
- `file_name_rules` répond toujours « sensible à la casse ». Or vfat et exFAT (clés USB,
  cartes SD), les partages SMB et les dossiers `casefold` d'ext4 ou f2fs comparent sans
  la casse : copier `A.txt` et `a.txt` vers une clé FAT vise un seul fichier.

Mesuré sur des images montées par UDisks (Fedora 44) : vfat (`utf8`) et exFAT
résolvent `ABC.txt` et `ÉTÉ.txt` vers `abc.txt` et `été.txt`. Renommer `abc.txt` en
`Abc.txt` y échoue (`EEXIST` avec `RENAME_NOREPLACE`) ; `gio rename` échoue de même
(« ce nom est déjà utilisé »).

## Règles

- **Séparateur** : `std::path::is_separator` pour tout chemin natif, soit `/` seul hors
  Windows. Windows garde `/` et `\`, sans changement.
- **Comparaison** : exacte hors Windows, octet pour octet, comme la barre latérale le
  fait déjà. Windows garde sa comparaison sans casse.
- **Nom d'archive à créer** : hors Windows, refus du vide, de `.`, `..` et de `/`, comme
  le dialogue de compression de Nautilus ; le reste est permis.
- **Casse par dossier**, pour les collisions préparées avant une copie :
  - dossier marqué `casefold` (ext4, f2fs, tmpfs) : sans casse ni différence de
    normalisation Unicode ;
  - vfat, exFAT, SMB/CIFS : sans casse ;
  - tout autre système, FUSE compris (ntfs-3g) : sensible, la norme Linux.
  Une erreur dans le sens prudent (sans casse) ne fait qu'annoncer un conflit de trop.
- **Renommer en changeant seulement la casse** sur un système sans casse : refus, comme
  Nautilus (GIO échoue de même) ; aucun renommage en deux temps.
- **Archives** : les noms internes gardent la politique actuelle (`\` lu comme `/`,
  noms incompatibles Windows refusés) ; l'accepter serait une décision séparée, avec
  tests Windows.

## Étapes

1. **Séparateurs et comparaisons** dans l'interface.
2. **Casse par dossier** derrière `Platform::file_name_rules`.
3. **Bilan.**

## Bilan

**Résultat.** Hors Windows, `\` est un caractère de nom : un dossier `a\b` est une seule
étape du fil d'Ariane, un seul titre d'onglet, une seule étape de complétion, et un
conflit ne le prend pas pour un enfant de `a`. Deux chemins qui ne diffèrent que par la
casse restent deux éléments pour l'accès rapide et la sélection. Le nom d'une archive à
créer suit Nautilus. La préparation d'une copie connaît la casse du dossier cible : vers
une clé FAT ou exFAT, un partage SMB ou un dossier `casefold`, `Report.txt` et
`report.txt` sont refusés ensemble avant toute écriture, au lieu d'échouer à mi-chemin
après avoir copié le premier.

Vérifié par des tests :
- séparateurs : fil, complétion, conflits ;
- casse : dossier ordinaire, majuscules d'un caractère pour un, pliage avec
  normalisation ;
- sur des images montées par UDisks : vfat, exFAT, ext4 `-O casefold` avec un dossier
  `+F` et un sans, descripteur `O_PATH` compris ;
- copie de deux noms de casse différente vers ces trois cibles : refus sans effet
  (avant : `File exists` après une première copie) ;
- renommer `notes.txt` en `Notes.txt` sur ces cibles : refus sans effet, comme GIO.

Les tests sur images se lancent avec `OXYDE_TEST_CASELESS=<montages>` et
`OXYDE_TEST_CASEFOLD=<dossier +F>:<dossier ordinaire>`. Une image ext4 casefold
appartenant à l'utilisateur se crée avec `mkfs.ext4 -O casefold -E
root_owner=$(id -u):$(id -g)`, puis `chattr +F` sur un dossier vide.

**Écarts restant.**
- Renommer en changeant seulement la casse échoue sur FAT et exFAT, comme dans
  Nautilus.
- ntfs3 monté avec `nocase` et lowntfs-3g avec `ignore_case` sont traités comme
  sensibles : rares, et l'erreur serait celle d'avant (échec à la copie, sans perte).
- Les noms internes des archives gardent la politique compatible Windows.
- La suppression avec historique est propre à Windows : non concernée.

**Fichiers communs touchés** : `paths_match` (comparaison exacte hors Windows), le fil
d'Ariane et son défilement, les onglets, la complétion, la couleur d'icône, les règles
de conflit, le dépôt externe et le nom d'archive à créer. Windows garde exactement ses
séparateurs, sa casse et ses tests, regroupés sous `cfg(windows)`.

**Leçons.**
- vfat monté par UDisks (`utf8`) ignore aussi la casse des lettres accentuées.
- `FS_IOC_GETFLAGS` refuse un descripteur `O_PATH` : rouvrir `.` en lecture suffit
  pour lire le drapeau `casefold`.
