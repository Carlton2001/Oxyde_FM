# Accès sans lecture et suppression Linux

**Statut : domaine terminé** (douzième domaine de l'option B, T06 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md), après les [noms et
chemins](linux-names.md)) ; bilan ci-dessous. Ce document est le contrat du domaine.

## Périmètre

Les droits qu'une opération demande vraiment sous Linux. Déplacer ou renommer un
élément change une entrée de son dossier : il faut écrire dans le dossier, pas lire
l'élément. Traverser un dossier demande `x`, pas `r`. Et une suppression récursive doit
pouvoir être mise en pause ou annulée entre deux éléments.

## Constat Linux

Mesuré par les opérations d'Oxyde sur Btrfs, chaque cas réussissant avec `mv` ou `cp` :

| Cas | Oxyde | Cause |
| --- | --- | --- |
| Déplacer un fichier `000` dans le même disque | refus | ouvert en lecture |
| Déplacer un dossier `300` (`-wx`) dans le même disque | refus | ouvert pour être listé |
| Déplacer un fichier sous un dossier traversable `311` | refus | chaque ancêtre ouvert en lecture |
| Copier vers un dossier `300` | refus | destination ouverte en lecture |

La suppression définitive vérifie pause et annulation une seule fois, puis
`remove_dir_all` vide tout le dossier d'un bloc.

## Règles

- **Dossiers tenus** (ancêtres, parent, destination, source) : ouverts en lecture si
  c'est permis, sinon en `O_PATH`. Le descripteur suffit pour l'identité, les appels
  relatifs (`openat`, `renameat`, `unlinkat`) et le montage ; seuls lister ou changer
  les attributs demandent la lecture, et échouent alors comme `cp`.
- **Fichier déplacé ou renommé** : `O_PATH | O_NOFOLLOW`, sans lecture. Le contenu
  n'est lu que si le déplacement devient une copie, entre deux disques.
- **Suppression récursive** : parcours explicite par descripteurs, sans suivre de lien.
  - pause et annulation vérifiées avant chaque élément ;
  - chaque dossier est supprimé après son contenu, par son parent tenu ;
  - un autre système de fichiers monté dedans n'est pas visité : sa suppression
    échoue, rien n'est effacé chez lui ;
  - le premier échec arrête l'élément sélectionné et est signalé, comme avant ;
  - un sous-dossier illisible n'est pas ouvert de force : échec, comme `rm -r` et
    Nautilus.
- Windows ne change pas.

## Étapes

1. **Ouvertures adaptées** : dossiers tenus et fichiers déplacés.
2. **Suppression récursive** avec points de contrôle.
3. **Bilan.**

## Bilan

**Résultat.** Oxyde demande sous Linux les droits que demandent `mv`, `cp` et Nautilus :
- un fichier illisible se déplace et se renomme dans son disque ;
- un dossier `-wx` se déplace, mais ne se copie pas (il faudrait le lister) ;
- un dossier seulement traversable (`--x`) laisse atteindre ce qu'il contient ;
- copier vers un dossier `-wx` fonctionne.

La suppression définitive parcourt l'arbre par descripteurs (`linux_file_tree.rs`) :
pause et annulation répondent entre deux éléments, au lieu d'attendre la fin du dossier
sélectionné, et ce qui n'est pas encore atteint reste intact. Les liens sont supprimés
sans être suivis. Un dossier remplacé pendant le parcours, ou un autre système de
fichiers monté dedans, arrête la suppression sans rien y effacer, avec le chemin dans
le message.

Vérifié par des tests :
- modes `000`, `300` et `311` sur Btrfs, sur le dossier, ses ancêtres et la
  destination ;
- arbre profond avec liens vers l'extérieur et vers un ancêtre : la cible survit ;
- annulation au quatrième point de contrôle : arbre partiellement supprimé, reste
  intact ;
- sous-dossier illisible : refus `PermissionDenied` nommant le dossier, contenu gardé ;
- tmpfs monté dans l'arbre (espace de noms utilisateur) : non visité, son contenu reste.

Le test du montage se lance avec `OXYDE_TEST_MOUNTED=<dossier>:<montage dedans>`, par
exemple sous `unshare --map-root-user --mount` après `mount -t tmpfs none <montage>`.

**Écarts restant.**
- La suppression Linux reste séquentielle, comme `remove_dir_all` avant : Turbo n'y
  ajoute pas de parallélisme.
- Un dossier monté dans l'arbre est refusé plutôt que démonté ou contourné, comme
  `rm -r --one-file-system`.
- Les ACL ne sont pas testées à part : le noyau les applique aux mêmes appels.

**Fichiers communs touchés** : aucun hors `src/platform/` ; Windows ne change pas.

**Leçons.**
- Déplacer un dossier demande d'écrire dedans (son entrée `..` change) : `mv` échoue
  aussi sur un dossier `000`.
- `O_PATH` suffit aux appels relatifs (`openat`, `renameat`, `unlinkat`, `fstat`) ;
  seuls lister et changer les attributs exigent un descripteur lisible.
