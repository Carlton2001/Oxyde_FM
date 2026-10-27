# Liens symboliques

**Statut : domaine terminé** (cinquième domaine de l'option B, après les
[volumes](volumes.md), le [presse-papiers](clipboard.md), la [surveillance](watchers.md) et
l'[accès rapide](quick-access.md)) ; bilan ci-dessous, suite Windows encore à passer. Ce
document est le contrat du domaine.

## Périmètre

Afficher, ouvrir, renommer, supprimer, copier et déplacer un lien symbolique sous Linux
(point T05 de l'audit). Les jonctions et points de reparse Windows gardent leur
comportement : sous Windows, un lien vers un dossier porte déjà l'attribut dossier, et les
transferts refusent les points de reparse.

## Constat Linux

- Le listage décrit le lien lui-même (`symlink_metadata`) : un lien vers un dossier
  apparaît comme un fichier, et l'ouvrir le passe à `xdg-open`, qui ouvre la cible dans
  Nautilus. Un lien vers un fichier affiche la longueur du chemin cible comme taille.
- *Avant l'étape 2* : la sélection des sources ouvrait l'entrée sans suivre le lien
  (`O_PATH | O_NOFOLLOW`) et refusait tout ce qui n'est ni fichier ni dossier : copier,
  déplacer ou compresser un lien échouait avec un message de source modifiée. **Le
  parcours d'un dossier sélectionné faisait de même** (vérifié) : copier, déplacer ou
  compresser un dossier qui contient un seul lien échouait en entier, même sur un même
  volume, car un déplacement reconstruit le dossier fichier par fichier.
- Supprimer (corbeille ou définitivement) et renommer agissent déjà sur le lien, jamais sur
  sa cible : la corbeille ne résout que le dossier parent, la suppression définitive lit
  `symlink_metadata`, le renommage ouvre sans suivre.

## Règles

- **Afficher** : un lien vers un dossier se comporte comme un dossier (navigation dans le
  chemin du lien, comme Nautilus), un lien vers un fichier montre la taille du fichier ; un
  lien cassé reste un lien. Comme dans Nautilus, l'icône dit où mène le lien (dossier,
  fichier, nulle part) et le type « Lien brisé » remplace « Lien symbolique » pour un lien
  cassé. Les propriétés montrent la cible du lien et décrivent l'objet visé ; un lien
  cassé y décrit le lien lui-même. Windows garde une seule icône de lien.
- **Tailles de dossiers** : un lien n'est jamais mesuré, ni parcouru dans une mesure.
- **Copier, déplacer** (étapes 2 et 3) : comme `cp -P` et Nautilus, le lien est reproduit avec le
  même texte de cible, jamais suivi. Création exclusive par `symlinkat`, qui refuse un nom
  existant ; déplacement sur le même volume par renommage du lien.

## Étapes

1. *Fait.* **Afficher et ouvrir** : `file_entry_from_metadata` suit le lien hors Windows
   pour le type et la taille.
2. *Fait.* **Copier un lien comme un lien**, Linux seul, Windows inchangé :
   - *Sélection* : un lien, choisi ou rencontré dans un dossier, devient une source
     « lien » (`SelectedLink`) : identité du lien lui-même, par le descripteur `O_PATH`, et
     texte de sa cible, lu sur ce même descripteur (`readlinkat` à chemin vide), donc sur
     l'objet capturé et non sur le nom. Jamais suivi. Sous Windows,
     `Platform::source_link_target` renvoie `None` et le refus des points de reparse reste.
   - *Plan* : les liens sont listés à part, comptés comme éléments de 0 octet et validés
     avec les fichiers contre les collisions de noms. Un lien cassé à la destination occupe
     son nom : c'est un conflit (Remplacer, Ignorer), y compris dans l'analyse des conflits,
     où un lien vers un dossier est un seul élément et non un dossier à fusionner.
   - *Exécution*, après les dossiers et avant les fichiers : identité et cible revérifiées
     par le parent source retenu, puis `symlinkat` dans le parent de destination retenu,
     qui refuse un nom existant. Remplacer crée le lien sous un nom privé du même dossier
     puis le renomme par-dessus la destination ; ce renommage échoue sur un dossier, qui
     n'est donc jamais remplacé. Un système sans liens (FAT, exFAT) fait échouer la copie
     avec l'erreur du système, comme Nautilus.
   - *Historique* : le journal note chaque lien créé avec sa source, sa cible et son
     identité (`HistoryCreatedLink`). Annuler met le lien de côté sous un nom privé, ne le
     retire que si c'est toujours le même lien vers la même cible, sinon le remet en place
     et échoue ; un retour arrière le recrée. Rétablir recrée les liens depuis la même
     source, si elle pointe toujours vers la même cible et si la destination est libre. Un
     remplacement bloque l'annulation, comme pour un fichier.
   - *Pas encore* : compresser ou copier vers une archive un lien, ou un dossier qui en
     contient, est refusé avant tout changement avec un message explicite (étape 4).
3. *Fait.* **Déplacer un lien**, Linux seul (`link_moves::move_link`) : lien revérifié,
   puis renommé entre parents retenus sans remplacement (`renameat2` `NOREPLACE`, ou nom
   vérifié puis renommage sur ntfs-3g qui refuse l'option) et revérifié une fois déplacé ;
   un lien substitué entre-temps est remis à sa place. Sur un autre volume, le lien est
   recréé à la destination puis la source n'est retirée que si c'est toujours le même lien
   ; sinon la copie est retirée et le déplacement échoue. Le journal note les liens
   déplacés (`moved_links`). Annuler exige un emplacement d'origine libre et le même lien
   à la destination, recrée les dossiers d'origine et le ramène par le même chemin ;
   Rétablir refait le déplacement avec les mêmes contrôles. Les retours arrière déplacent
   en sens inverse et mettent à jour l'identité d'un lien recréé.
4. *Reporté, par choix de sécurité.* **Compresser** : les formats savent stocker un lien
   (tar, zip par le mode Unix, 7z), mais Oxyde refuse les entrées-liens à l'ouverture, à
   l'extraction et dans l'espace de travail des archives, car un lien extrait puis suivi
   par une entrée suivante peut écrire hors de la destination. Une archive contenant un
   lien serait donc illisible par Oxyde lui-même ; archiver la cible à la place
   recopierait silencieusement des données extérieures (un lien vers `Documents`
   embarquerait tout le dossier). Compresser ou copier vers une archive un lien, ou un
   dossier qui en contient, est donc refusé avant toute écriture, avec un message traduit
   qui l'explique (`ArchiveLinkRefusal`). Lever ce refus demande un chantier propre :
   extraire les liens en dernier, sans jamais les suivre, avec une cible confinée à la
   destination.
5. *Fait.* **Bilan**, ci-dessous.

## Bilan

**Résultat.** Sous Linux, un lien se montre, s'ouvre, se renomme, se supprime, se copie
et se déplace comme dans Nautilus : un lien vers un dossier s'ouvre comme un dossier,
l'icône dit où il mène, un lien brisé est signalé, les propriétés montrent sa cible, et
copier ou déplacer reproduit le lien sans jamais le suivre, sur un même volume comme d'un
volume à l'autre (Btrfs et NTFS vérifiés à la main), avec Annuler et Rétablir. Avant ce
domaine, copier, déplacer ou compresser un dossier contenant un seul lien échouait en
entier. Le code propre aux liens tient en 320 lignes de production environ
(`linux_links.rs`, `link_moves.rs`, `copy/links.rs`, `history_links.rs`) et 340 de tests.

**Fichiers communs touchés** (hors modules propres aux liens, catalogues et icônes) :

| Étape | Fichiers communs | Nature |
| --- | --- | --- |
| 1. Afficher | `models/file_entry.rs` | Linux seul, par `cfg` |
| 1 bis. Icônes, type, propriétés | `ui/components/icons.rs`, `icons/colors.rs`, `file_list/presentation.rs`, `filters.rs`, `layout/measurements.rs`, `properties/general.rs`, `properties/loading.rs`, `commands/io.rs` | Linux seul, sauf un champ de plus |
| 2. Copier | `platform/mod.rs`, `linux_impl.rs`, `source_selection.rs`, `file_ops/copy.rs`, `commands/ops/conflicts.rs`, `commands/ops.rs`, `history_file_undo.rs`, `execution.rs` | méthodes `Platform` par défaut refusées sous Windows |
| 2–3. Journal | `models/transaction.rs`, `file_ops.rs` et chaque constructeur de `TransactionDetails` | deux champs `created_links`, `moved_links` |
| 3. Déplacer | `history_file_undo.rs`, `history_move_redo.rs` | nouveaux effets, rollback compris |
| 4. Archives | `compression_sources.rs`, `models/error.rs`, `ui/format.rs` | refus typé et traduit |

**Leçons.**
- Lire la cible sur le descripteur `O_PATH` du lien capturé (`readlinkat` à chemin vide)
  plutôt que sur son nom : une substitution entre deux vérifications ne change pas le lien
  lu.
- Un lien brisé occupe toujours son nom : `exists()` le manque, `symlink_metadata` non.
  L'analyse des conflits et la détection de remplacement doivent utiliser le second.
- ntfs-3g refuse `RENAME_NOREPLACE` aussi pour les liens ; le repli vérifie le nom libre
  puis renomme, avec une fenêtre de course impossible à fermer sur ce système.
- `TransactionDetails` sans valeur par défaut impose de toucher chaque constructeur à
  chaque nouveau champ : un `Default` mériterait d'être envisagé.

**Reporté.** Liens dans les archives (étape 4). Sous Windows, les liens symboliques et
jonctions restent refusés par les transferts, sans changement.
