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
- tmpfs monté dans l'arbre (espace de noms utilisateur) : non visité, son contenu reste ;
- suppression réelle de 40 000 fichiers : la pause fige l'arbre, l'annulation en garde
  le reste (avant : tout le dossier partait malgré la pause) ;
- droits donnés par ACL à un autre utilisateur que le propriétaire (`u:…:--x` sur
  l'ancêtre, `u:…:-wx` sur la destination, fichier illisible) : déplacement et copie
  réussissent ; sans ces entrées, `mv` est refusé.

Le test du montage se lance avec `OXYDE_TEST_MOUNTED=<dossier>:<montage dedans>`, par
exemple sous `unshare --map-root-user --mount` après `mount -t tmpfs none <montage>`.
Le test des ACL se lance avec `OXYDE_TEST_ACL=<dossier préparé>` (disposition dans le
test), sous un autre utilisateur : `unshare --map-auto --map-root-user --mount`, le
dossier lié sur `/mnt` pour ne pas traverser le dossier personnel, puis `setpriv
--reuid=1000`.

**Parallélisme, mesuré** (NVMe, cache chaud). Un dossier sélectionné seul est parcouru
par un seul worker sous Linux ; Turbo supprime déjà jusqu'à 4 éléments sélectionnés à
la fois. Par la suppression d'Oxyde, Normal puis Turbo :

| Sélection | Btrfs | ext4 (image) |
| --- | --- | --- |
| 60 dossiers de 2 000 fichiers | 1,1–1,2 s → 1,25–1,3 s | 0,50 s → 0,20–0,22 s |
| 8 dossiers de 10 000 fichiers | 0,72–0,74 s → 0,78–0,80 s | 0,32–0,34 s → 0,12–0,14 s |
| 2 000 dossiers de 20 fichiers | 0,40–0,50 s → 0,46 s | 0,20 s → 0,06–0,08 s |

Sur Btrfs, supprimer en parallèle ralentit d'environ 10 % ; sur ext4, Turbo est 2,5 à
3 fois plus rapide.

**Choix assumé.** Un disque monté dans l'arbre arrête la suppression, comme
`rm -r --one-file-system` : supprimer un dossier n'efface jamais le contenu d'un autre
disque. Il faut le démonter d'abord.

**Fichiers communs touchés** : aucun hors `src/platform/` ; Windows ne change pas.

**Leçons.**
- Déplacer un dossier demande d'écrire dedans (son entrée `..` change) : `mv` échoue
  aussi sur un dossier `000`.
- `O_PATH` suffit aux appels relatifs (`openat`, `renameat`, `unlinkat`, `fstat`) ;
  seuls lister et changer les attributs exigent un descripteur lisible.
