# Propriétés Linux

**Statut : domaine terminé** (dixième domaine de l'option B, après les [noms non
UTF-8](non-utf8-names.md)) ; bilan ci-dessous. Ce document est le contrat du domaine.

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
    les trois bits d'exécution. Elle n'apparaît, comme dans Nautilus (règle de GIO), que
    pour un type qui peut s'exécuter : sous-type de `application/x-executable` ou de
    `text/plain`.
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
- **Éléments contenus** (dossier) : la page « Modifier les droits des éléments
  contenus… » reprend le dialogue de Nautilus. Pour chaque classe, on choisit un niveau
  pour les fichiers et un pour les dossiers, « Inchangé » par défaut. OK l'applique à
  toute profondeur, avant le changement du dossier lui-même :
  - chaque dossier change après son contenu, si bien qu'un dossier qui se ferme à son
    propriétaire est quand même parcouru ;
  - un dossier fermé que le changement ouvre est changé d'abord, puis parcouru ;
  - les liens ne sont jamais suivis et les autres systèmes de fichiers jamais visités ;
  - chaque élément est changé par le descripteur dont le type a été vérifié ;
  - les échecs sont comptés, le premier est nommé, les autres éléments changent.
- **Système de fichiers sans droits Unix** : ntfs-3g (sans son option `permissions`) et
  FAT acceptent `chmod` sans rien garder. Oxyde relit l'objet après le changement et le
  signale au lieu de fermer la fenêtre comme si tout avait réussi.
- **Type** comme Nautilus, « image PNG (image/png) », selon l'algorithme de GIO
  (xdgmime) sur la base shared-mime-info :
  - par le nom (`globs2`) : nom exact, sinon le plus long suffixe `*.ext`, puis les
    autres motifs s'il reste moins de deux types ; la casse exacte d'abord ;
  - un seul type par le nom suffit ; sinon le contenu (`magic`) tranche : une règle de
    priorité 80 ou plus l'emporte, sinon le premier type du nom qui en dérive ;
  - sans nom reconnu : le contenu, sinon texte si les 128 premiers octets n'ont pas de
    caractère de contrôle, sinon inconnu ; un fichier vide est `application/x-zerosize`.

  Description dans la langue de l'interface, sinon celle de la base. Les dossiers
  gardent le libellé d'Oxyde, souvent absent de la base. Tubes, sockets et
  périphériques sont nommés par leur nature, sans être ouverts.

## Étapes

1. **Droits d'un élément** : modèle des niveaux, lecture et écriture derrière
   `Platform`, onglet Droits d'accès.
2. **Type MIME** : type et description du système (shared-mime-info) dans Général.
3. **Éléments contenus** : application récursive, case d'exécution limitée aux types
   exécutables, retour à la ligne des libellés.
4. **Bilan.**

## Bilan

**Résultat.** La fenêtre Propriétés d'un élément Linux a un onglet Droits d'accès
modelé sur Nautilus. On y règle le niveau d'accès du propriétaire, du groupe et des
autres, le groupe parmi ceux de l'utilisateur et l'exécution d'un programme ou d'un
script ; pour un dossier, les droits de tout son contenu. L'onglet Général donne le
type du système, par exemple « image PNG (image/png) ». Validé à l'écran sur Fedora 44.

Vérifié par des tests sur Btrfs, tmpfs et ntfs-3g :
- niveaux, bits spéciaux et exécution ;
- changement de groupe ;
- lien vers sa cible ;
- refus d'un élément modifié ou remplacé, élément d'un autre utilisateur en lecture
  seule ;
- système de fichiers qui ignore les droits ;
- parcours récursif : profondeur, liens, dossier qui se ferme ou s'ouvre, échecs ;
- types par nom et par contenu, règle d'exécution de GIO ; comparés à `gio info` sur
  plus de 15 000 fichiers du système et du projet, sans aucun écart.

**Écarts restant avec Nautilus.**
- Pas de droits pour une sélection de plusieurs éléments.
- Le propriétaire ne se change pas : il faut être root, et Oxyde ne demande pas
  d'élévation.

**Fichiers communs touchés** :
- `models/permissions.rs` (niveaux, changement, résultats) ;
- `FileProperties` (`permissions`, `mime_type`) ;
- `Platform` (`unix_permissions`, `set_unix_permissions`, `mime_type`) ;
- la fenêtre Propriétés : onglets en énumération, module `tabs`.

Windows garde ses onglets et ne reçoit ni droits ni type MIME. La fonctionnalité
`process` de rustix est activée, sans changement de `Cargo.lock`.

**Leçons.**
- ntfs-3g accepte `chmod` et `chgrp` sans rien garder : relire l'objet après coup est
  le seul moyen de le savoir.
- La base MIME classe tout ELF comme programme (`application/x-executable`) ; les
  bibliothèques se distinguent par leur nom (`*.so*`). Un tri par `e_type` aurait
  masqué la case d'exécution des programmes PIE, c'est-à-dire de presque tous.
- Une liste de dossiers à parcourir, plutôt que la récursion, permet de modifier
  chaque dossier après son contenu sans craindre les arborescences profondes.
