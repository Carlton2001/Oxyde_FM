# Liens symboliques

**Statut : en cours** (cinquième domaine de l'option B, après les [volumes](volumes.md), le
[presse-papiers](clipboard.md), la [surveillance](watchers.md) et l'[accès
rapide](quick-access.md)). Ce document deviendra le contrat du domaine une fois terminé.

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
  lien cassé reste un lien. L'icône et le type « Lien symbolique » restent ceux du lien.
- **Tailles de dossiers** : un lien n'est jamais mesuré, ni parcouru dans une mesure.
- **Copier, déplacer** (étape 2) : comme `cp -P` et Nautilus, le lien est reproduit avec le
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
   - *Pas encore* : déplacer, compresser ou copier vers une archive un lien, ou un dossier
     qui en contient, est refusé avant tout changement avec un message explicite (étapes 3
     et 4).
3. **Déplacer un lien**, Linux seul : même volume, renommage du lien entre parents retenus
   (`renameat2` sans remplacement, déjà utilisé par le renommage) ; autre volume, copie
   comme à l'étape 2 puis retrait du lien source après revérification. Annuler renomme ou
   recrée en sens inverse, avec les mêmes contrôles.
4. **Compresser** : un lien est archivé comme lien (tar, zip, 7z le savent), à étudier
   avec les [limites des archives](archives.md) ; reporté si le format l'interdit.
5. **Bilan.**
