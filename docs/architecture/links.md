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
- La sélection des sources ouvre l'entrée sans suivre le lien (`O_PATH | O_NOFOLLOW`) et
  refuse tout ce qui n'est ni fichier ni dossier : copier, déplacer ou compresser un lien
  échoue avec un message de source modifiée. **Le parcours d'un dossier sélectionné fait
  de même** (vérifié) : copier, déplacer ou compresser un dossier qui contient un seul lien
  échoue en entier, même sur un même volume, car un déplacement reconstruit le dossier
  fichier par fichier.
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
2. **Copier un lien comme un lien**, Linux seul, Windows inchangé :
   - *Sélection* : un lien, choisi ou rencontré dans un dossier, devient une source
     « lien » : identité du lien lui-même (périphérique, inode, par le descripteur
     `O_PATH`) et texte de sa cible (`readlinkat` sur le parent retenu). Jamais suivi.
   - *Plan* : les liens sont listés à part, comptés comme éléments de 0 octet ; un conflit
     se résout comme pour un fichier (Remplacer, Ignorer).
   - *Exécution*, après les dossiers : identité et cible revérifiées, puis `symlinkat` dans
     le parent de destination retenu, qui refuse un nom existant. Remplacer crée le lien
     dans le dossier temporaire de copie puis le renomme par-dessus la destination, comme
     un fichier.
   - *Historique* : le journal note les liens créés avec leur cible ; annuler ne retire un
     lien que s'il est toujours un lien vers la même cible, sinon il est conservé et
     signalé.
3. **Déplacer un lien**, Linux seul : même volume, renommage du lien entre parents retenus
   (`renameat2` sans remplacement, déjà utilisé par le renommage) ; autre volume, copie
   comme à l'étape 2 puis retrait du lien source après revérification. Annuler renomme ou
   recrée en sens inverse, avec les mêmes contrôles.
4. **Compresser** : un lien est archivé comme lien (tar, zip, 7z le savent), à étudier
   avec les [limites des archives](archives.md) ; reporté si le format l'interdit.
5. **Bilan.**
