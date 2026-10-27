# Effets par dossiers tenus sous Linux

**Statut : domaine terminé** (quatorzième domaine de l'option B, M03 et M04 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md), après le [cache
Linux](linux-open-cache.md)). Bilan ci-dessous. Ce document est le contrat du domaine.

## Périmètre

Le code commun prépare chaque opération de fichiers en tenant ses dossiers parents et
les objets vérifiés (`HeldParents`, sélection, mise en attente), puis demande à
`Platform` d'agir relativement à eux. Windows agit par ces handles ; Linux les recevait
et agissait par chemin. Copier, déplacer, publier, nettoyer et annuler sont concernés.
Windows ne change pas.

## Constat Linux

- Créer, ouvrir et suivre un dossier ou un fichier, créer la sortie privée d'une copie,
  sa mise en attente et un dossier de journal : par chemin, le dossier tenu étant
  ignoré. Si un dossier est renommé ou remplacé entre la vérification et l'effet,
  l'effet a lieu dans le remplaçant.
- Renommer un fichier vérifié (déplacement, publication d'une copie) : par chemin
  (`rename`, ou `renameat2` depuis le dossier courant), sans contrôler que le nom
  désigne encore l'objet vérifié.
- Supprimer un fichier vérifié (source d'un déplacement entre disques après contrôle
  de son empreinte, sortie privée, sauvegarde) : par chemin. Un fichier substitué entre
  le contrôle et la suppression serait supprimé à sa place.
- Deux tests d'Annuler échouent pour cette raison : un dossier remplacé pendant
  l'annulation reçoit les fichiers remis en place, au lieu du dossier d'origine.

## Règles

- **Relatif aux dossiers tenus** : tout effet nommé passe par le dossier tenu et le
  dernier composant (`openat`, `mkdirat`, `renameat2`, `unlinkat`), sans suivre de lien
  au dernier composant.
- **Objet tenu, où qu'il soit** : renommer ou supprimer un objet vérifié agit sur lui à
  son emplacement actuel, lu par le noyau (`/proc/self/fd`), puis contrôle son
  identité (`dev`, `ino`) dans ce dossier.
- **Suppression sans substitution** : Linux ne sait pas supprimer « ce nom s'il désigne
  cet objet ». L'entrée est d'abord renommée à part dans son dossier, son identité
  contrôlée, puis supprimée ; un autre objet est remis à sa place, intact.
- **Renommage contrôlé** : identité contrôlée avant, puis à l'arrivée ; un objet arrivé
  à la place de l'original pendant ce court intervalle est remis à son nom d'origine
  et l'opération échoue.
- **Publication exclusive** : `RENAME_NOREPLACE` relatif ; sur FUSE (ntfs-3g), qui le
  refuse, lien physique puis suppression contrôlée de l'ancien nom ; sans lien
  physique, vérification que le nom est libre puis renommage, la seule fenêtre restante.
- Aucune de ces règles n'est une transaction : elles garantissent qu'Oxyde n'agit que
  sur les objets qu'il a vérifiés, pas que les noms restent figés.

## Étapes

1. **Primitives relatives** (`linux_held.rs`) derrière les méthodes `Platform`
   existantes, sans changer leurs signatures.
2. **Renommage et suppression par objet tenu.**
3. **Tests** : dossier renommé ou remplacé avant l'effet, fichier substitué, liens,
   puis tmpfs, Btrfs, vfat, exFAT et NTFS ; les deux tests d'Annuler.
4. **Bilan.**

## Bilan

**Résultat.** Sous Linux, les opérations agissent dans les dossiers tenus et sur les
objets vérifiés, où qu'ils soient : un dossier renommé ou remplacé entre la
vérification et l'effet ne reçoit rien, un fichier substitué n'est ni renommé ni
supprimé, et la publication d'une copie ne remplace jamais un nom pris, y compris sur
ntfs-3g. Les deux tests d'Annuler qui échouaient passent.

Vérifié par la suite complète sur Btrfs et tmpfs (lib 550, bins 456, sans échec), et sur
des images exFAT et NTFS : les échecs nouveaux par rapport à avant passent tous isolés
(tests sensibles à la charge), sauf le refus de lien sur exFAT, qui n'a pas de liens
symboliques. La publication exclusive est testée sur vfat, exFAT, NTFS, tmpfs et ext4
`casefold` (`OXYDE_TEST_PUBLISH`).

**Écarts restant.**
- Sur exFAT, une suppression contrôle l'entrée puis la supprime directement : le noyau
  donne une nouvelle date de création à un dossier dès qu'une entrée y est renommée, ce
  qui invaliderait sa preuve d'historique. Une substitution reste possible dans cet
  intervalle très court.
- Sans `RENAME_NOREPLACE` ni lien physique, la publication vérifie le nom puis renomme.

**Leçons.**
- ntfs-3g ne décrit plus un descripteur `O_PATH` dont le nom a disparu (`ENOENT`) :
  l'identité est alors lue dans `/proc/self/fdinfo` (inode) et `mountinfo` (périphérique).
- Copie : quand le seul worker actif échouait en mode normal, les workers inactifs
  attendaient indéfiniment (code commun). Tout échec de worker arrête maintenant le lot.
