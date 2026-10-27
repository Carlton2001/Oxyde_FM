# Copies Linux

**Statut : en cours** (sixième domaine de l'option B, après les [volumes](volumes.md), le
[presse-papiers](clipboard.md), la [surveillance](watchers.md), l'[accès
rapide](quick-access.md) et les [liens symboliques](links.md)). Ce document deviendra le
contrat du domaine une fois terminé.

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
- Garder ce que garde `cp -p`, sauf le propriétaire, qui exige d'être administrateur :
  mode, dates d'accès et de modification, attributs étendus `user.*` et ACL. Le contexte
  SELinux n'est pas recopié : la destination reçoit le sien, comme avec `cp`.
- setuid et setgid ne sont gardés que si la copie appartient au même propriétaire que
  l'original.
- Un dossier créé par la copie reçoit mode et dates de l'original une fois son contenu
  copié, du plus profond au plus haut. Son propriétaire garde toujours lecture, écriture
  et parcours (`mode | 700`), sinon ni Annuler ni la suite de la copie ne pourraient y
  écrire ; les droits du groupe et des autres sont repris tels quels. Un dossier existant
  dans lequel on fusionne n'est jamais modifié.
- Un attribut que la destination ne sait pas garder (FAT, certains montages) n'empêche
  pas la copie : il est signalé dans le journal.

## Limites connues

- La date d'accès reprise ne survit qu'à la première lecture, comme avec `cp -p` : la
  copie a forcément une date de changement plus récente, et `relatime` met alors l'accès
  à jour. Nautilus lit le fichier dès l'ouverture de ses Propriétés. Oxyde relit ses
  propres copies (vérification, Annuler) sans toucher cette date (`O_NOATIME`).
- NTFS monté par ntfs-3g sans l'option `permissions` ne garde aucun mode : tous les
  fichiers y apparaissent en `777`, donc exécutables, quel que soit l'original. FAT n'en
  garde pas non plus.

## Étapes

1. **Copier depuis la source vérifiée, par le noyau** : `copy_file_with_progress` lit le
   descripteur retenu à des positions explicites, essaie `copy_file_range` par tranches,
   puis la boucle actuelle ; date d'accès reprise ; setuid et setgid filtrés.
2. **Droits et dates des dossiers** : la copie note mode et dates de chaque dossier source
   au plan, et les applique à la fin aux dossiers qu'elle a créés, par leur descripteur
   retenu (méthode `Platform` sans effet sous Windows).
3. **Attributs étendus et ACL**, fichiers et dossiers, sur la sortie encore privée pour un
   fichier.
4. **Bilan.**
