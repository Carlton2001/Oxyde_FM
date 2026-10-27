# Copies Linux

**Statut : domaine terminé** (sixième domaine de l'option B, après les
[volumes](volumes.md), le [presse-papiers](clipboard.md), la [surveillance](watchers.md),
l'[accès rapide](quick-access.md) et les [liens symboliques](links.md)) ; bilan
ci-dessous, suite Windows encore à passer. Ce document est le contrat du domaine.

## Périmètre

La copie d'un fichier sous Linux, y compris celle d'un déplacement d'un volume à l'autre
(points M03 et M06 de l'audit) : d'où elle lit, comment elle écrit, et ce qu'elle garde
du fichier et des dossiers d'origine. La mise en attente, l'empreinte BLAKE3 relue sur la
sortie, la publication exclusive et l'historique ne changent pas. Windows garde son
comportement : sa copie native passe déjà par `CopyFileEx`.

## Constat Linux

- `copy_file_with_progress` rouvre la source **par son nom** et ignore le descripteur que
  la sélection a vérifié (`_held_source`) : un fichier substitué entre la vérification et
  la copie serait copié à sa place (M03).
- Les octets passent par une boucle de lecture et d'écriture de 1 Mio. Sur Btrfs ou XFS,
  `cp` partage les blocs (reflink) et copie un gros fichier presque instantanément ; Oxyde
  réécrit tout.
- Un fichier garde son mode et sa date de modification ; pas sa date d'accès, ni ses
  attributs étendus, ni ses ACL. Les bits setuid et setgid sont recopiés même quand la
  copie change de propriétaire.
- Un dossier copié est créé avec les droits par défaut (umask) : un dossier privé
  (`700`) devient lisible par tous. Ses dates ne sont pas reprises (M06).

## Règles

- Lire uniquement par le descripteur vérifié, quand l'appelant en fournit un.
- Laisser le noyau copier (`copy_file_range`) : partage de blocs quand le système de
  fichiers le permet, copie côté noyau sinon ; repli sur la boucle actuelle pour les
  systèmes qui refusent (ntfs-3g, FAT, anciens noyaux). La progression, la pause et
  l'annulation restent vérifiées entre deux tranches.
- Garder mode, dates d'accès et de modification et ACL, comme `cp -p`, et les attributs
  étendus `user.*`, comme Nautilus ; pas le propriétaire, qui exige d'être administrateur.
  Le contexte SELinux et les autres espaces d'attributs ne sont pas recopiés : la
  destination reçoit les siens, comme avec `cp`. Une source sans ACL garde dans la copie
  l'ACL par défaut héritée de la destination, comme Nautilus.
- Les attributs étendus s'écrivent avant le mode : écrire `user.*` exige le droit
  d'écriture, et le mode appliqué ensuite tient l'ACL à jour.
- setuid et setgid ne sont gardés que si la copie appartient au même propriétaire que
  l'original.
- Un dossier créé par la copie reçoit les attributs de l'original une fois son contenu
  copié, du plus profond au plus haut. Son propriétaire garde toujours lecture, écriture
  et parcours (`mode | 700`, appliqué après l'ACL), sinon ni Annuler ni la suite de la
  copie ne pourraient y écrire ; les droits du groupe et des autres sont repris tels
  quels. Un dossier existant
  dans lequel on fusionne n'est jamais modifié.
- Un attribut que la destination ne sait pas garder (FAT, certains montages) n'empêche
  pas la copie : il est signalé dans le journal, avec son nom.
- Annuler et Rétablir redonnent ces attributs aux dossiers qu'ils recréent, à partir du
  journal de la transaction.

## Limites connues

- La date d'accès reprise ne survit qu'à la première lecture, comme avec `cp -p` : la
  copie a forcément une date de changement plus récente, et `relatime` met alors l'accès
  à jour. Nautilus lit le fichier dès l'ouverture de ses Propriétés. Oxyde relit ses
  propres copies (vérification, Annuler) sans toucher cette date (`O_NOATIME`), et la
  remet après la vérification : ntfs-3g ignore `O_NOATIME`.
- NTFS monté par ntfs-3g sans l'option `permissions` ne garde aucun mode : tous les
  fichiers y apparaissent en `777`, donc exécutables, quel que soit l'original. Il garde
  les attributs `user.*` (en flux nommés), pas les ACL. FAT ne garde ni mode ni
  attributs étendus.
- Un dossier contenant un sous-dossier en lecture seule (`550`) ne peut être déplacé que
  s'il est renommé entier, sur son montage : voir [déplacements](moves.md). D'un volume à
  l'autre, ses fichiers ne peuvent pas quitter la source, comme avec Nautilus.

## Étapes

1. **Copier depuis la source vérifiée, par le noyau** : `copy_file_with_progress` lit le
   descripteur retenu à des positions explicites, essaie `copy_file_range` par tranches,
   puis la boucle actuelle ; date d'accès reprise ; setuid et setgid filtrés.
2. **Droits et dates des dossiers** : la copie note mode et dates de chaque dossier source
   au plan, avant de le lister, et les applique à la fin aux dossiers qu'elle a créés, par
   leur descripteur retenu (méthode `Platform` sans effet sous Windows). Vaut aussi pour
   un déplacement, qui recrée les dossiers même sur un seul volume.
3. **Annuler et Rétablir** : la copie inscrit au journal mode et dates de chaque dossier
   qu'elle a créé. Les dossiers que l'historique recrée (source d'un déplacement annulé,
   destination d'un déplacement ou d'une copie rétablis) les reprennent une fois remplis,
   par le même module (`history_directory_attributes`).
4. **Attributs étendus et ACL** : `user.*` et ACL d'accès et par défaut, lus sur le
   descripteur source et écrits sur la sortie encore privée pour un fichier, notés au plan
   puis au journal pour un dossier (`linux_extended_attributes.rs`).
5. **Bilan.**

## Bilan

**Résultat.** Une copie Linux lit le fichier que la sélection a vérifié, laisse le noyau
copier (partage de blocs sur Btrfs : 1 Go copié en une fraction de seconde), et garde du
fichier et des dossiers ce que gardent `cp -p` et Nautilus : mode (setuid et setgid
filtrés), dates, attributs `user.*` et ACL. Les dossiers, que ni la copie ni le
déplacement ne gardaient (un dossier `700` devenait `755`, même déplacé sur un seul
disque), les gardent maintenant, à travers Annuler et Rétablir. Vérifié par les tests
sur tmpfs, Btrfs et une image NTFS montée par ntfs-3g ; Btrfs et NTFS essayés à la main.
Code propre au domaine : 290 lignes de production environ (`linux_file_copy.rs`,
`linux_extended_attributes.rs`, `copy/directories.rs`, `history_directory_attributes.rs`)
et 460 de tests.

**Fichiers communs touchés** (hors modules propres au domaine) :

| Étape | Fichiers communs | Nature |
| --- | --- | --- |
| 1. Copie noyau | `platform/linux_impl.rs`, `systems/file_copy.rs` | ouverture `O_NOATIME`, date d'accès remise après vérification (méthode `Platform` sans effet sous Windows) |
| 2. Dossiers | `platform/mod.rs`, `source_selection.rs`, `destination_tree.rs`, `file_ops/copy.rs` | relevé au plan, application par le descripteur retenu |
| 3. Historique | `models/transaction.rs`, `file_ops.rs`, `execution.rs`, `history_file_undo.rs`, `history_move_redo.rs`, `commands/ops.rs`, chaque constructeur de `TransactionDetails` | champ `directory_attributes` |
| 4. Attributs étendus | `platform/mod.rs` | types `ExtendedAttributes`, `DirectoryAttributes` |

**Leçons.**
- Relire une copie pour la vérifier change sa date d'accès : `relatime` la met à jour dès
  que la date de changement est plus récente, ce qui est toujours le cas d'une copie.
  `O_NOATIME` suffit sur Btrfs et tmpfs, pas sur ntfs-3g.
- Un déplacement sur un seul volume recrée les dossiers : tout ce qui vaut pour la copie
  des dossiers vaut aussi pour lui.
- Écrire les ACL avant le mode évite d'analyser leur format : le noyau recalcule les
  entrées propriétaire, masque et autres au `chmod`, ce qui garde l'accès complet du
  propriétaire.
- Les essais NTFS se font sur une image : `udisksctl loop-setup` puis le montage
  automatique de GNOME, sans toucher aux disques de l'utilisateur.

**Reporté.** Le déplacement d'un dossier entier par renommage, fait depuis dans les
[déplacements](moves.md).
