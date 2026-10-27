# Propriétés Linux

**Statut : étapes 1 et 2 faites, essai visuel attendu** (dixième domaine de l'option B, après les [noms non
UTF-8](non-utf8-names.md)). Ce document est le contrat du domaine.

## Périmètre

La fenêtre Propriétés d'un élément sous Linux : droits d'accès Unix (propriétaire,
groupe, accès, exécution) et informations propres à Linux, sur le modèle de Nautilus.
Windows garde ses onglets (Général, Raccourci) et son bouton vers les propriétés du
système.

## Constat Linux

- L'onglet Général montre nom, type par extension, emplacement, taille et dates. Les
  droits, le propriétaire et le groupe sont absents, et rien ne permet de rendre un
  script exécutable sans terminal.
- Linux n'a pas de fenêtre Propriétés du système vers laquelle renvoyer.

## Règles (Nautilus)

- **Onglet Droits d'accès** pour un élément seul, comme la page Permissions de Nautilus :
  propriétaire, groupe et autres, chacun avec son niveau d'accès.
  - Fichier : Aucun, Lecture seule, Lecture et écriture. Le propriétaire n'a pas
    « Aucun ». La case « Autoriser l'exécution du fichier comme un programme » règle
    les trois bits d'exécution.
  - Dossier : Aucun, Lister les fichiers uniquement, Accéder aux fichiers, Créer et
    supprimer des fichiers.
  - Une combinaison qui ne correspond à aucun niveau s'affiche telle quelle (`rw-`, …)
    et reste jusqu'à ce qu'un niveau soit choisi.
- **Qui peut modifier** : le propriétaire, ou root. Sinon l'onglet est en lecture seule,
  avec le message de Nautilus. Le groupe se choisit parmi ceux de l'utilisateur ; changer
  de propriétaire demande root, et Oxyde ne fait que l'afficher.
- **Application** au bouton OK, comme le reste de la fenêtre, et hors du thread UI. Elle
  vise l'objet ouvert au chargement (même `dev`/inode, mêmes droits et groupe). Si
  l'élément a changé entre-temps, rien n'est appliqué et un message invite à rouvrir.
  Les bits setuid, setgid et sticky sont conservés, sauf quand le noyau les retire à un
  changement de groupe.
- **Noms des comptes** par NSS (`getent`), comme les appels de GLib : comptes locaux,
  SSSD et LDAP. À défaut, Oxyde affiche le numéro.
- Un lien symbolique décrit sa cible, comme le reste des Propriétés ; un lien cassé n'a
  pas d'onglet Droits d'accès.
- **Système de fichiers sans droits Unix** : ntfs-3g (sans son option `permissions`) et
  FAT acceptent `chmod` sans rien garder. Oxyde relit l'objet après le changement et le
  signale au lieu de fermer la fenêtre comme si tout avait réussi.
- **Type** comme Nautilus, « image PNG (image/png) » : nom de fichier selon `globs2` de
  shared-mime-info (noms exacts, puis poids, puis longueur du motif), sinon contenu
  (texte UTF-8 ou inconnu). Description dans la langue de l'interface, sinon celle de
  la base. Les dossiers gardent le libellé d'Oxyde, souvent absent de la base. Tubes,
  sockets et périphériques sont nommés par leur nature, sans être ouverts.

## Étapes

1. **Droits d'un élément** : modèle des niveaux, lecture et écriture derrière
   `Platform`, onglet Droits d'accès.
2. **Type MIME** : type et description du système (shared-mime-info) dans Général.
3. **Bilan.**
