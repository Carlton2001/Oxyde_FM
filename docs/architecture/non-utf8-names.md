# Noms non UTF-8 sous Linux

**Statut : domaine terminé** (neuvième domaine de l'option B, après l'[identité de
bureau](desktop-identity.md)) ; bilan ci-dessous. Ce document est le contrat du domaine
(point M05 de l'audit).

## Périmètre

Un nom Unix est une suite d'octets : une clé formatée sous un autre encodage ou une
archive ancienne peuvent laisser des noms qui ne sont pas du texte UTF-8, comme
`photo-\xe9t\xe9.jpg` en Latin-1. Oxyde garde ses chemins en `String` dans l'interface,
les sélections et le journal. Windows n'est pas concerné : ses noms UTF-16 gardent leur
conversion actuelle.

## Constat Linux

- Le listage convertissait nom et chemin par `to_string_lossy()` : chaque octet invalide
  devenait `U+FFFD`. `photo-\xe9t\xe9.jpg` et un fichier réellement nommé `photo-�t�.jpg`
  recevaient le même chemin. Copier, supprimer ou renommer l'un pouvait viser l'autre ;
  seul, le premier était introuvable.
- La recherche et la corbeille faisaient la même conversion.
- Nautilus (GIO) garde les octets, affiche « nom (encodage non valide) » et permet toutes
  les actions. Le renommage sert de remède : il propose le nom avec `U+FFFD`.

## Règles

- **Texte réversible** (`utils::path_text`) : un chemin valide garde son texte. Dans un
  chemin non UTF-8, chaque octet invalide s'écrit NUL suivi de deux chiffres
  hexadécimaux. Aucun chemin Unix ne contient NUL : le texte reste distinct de tout autre
  fichier, et un code qui l'ouvre sans `decode` échoue (`InvalidInput`) au lieu
  d'atteindre son sosie. Le listage, la recherche et la corbeille l'emploient.
- **Affichage comme GLib** : `name` montre `U+FFFD`, et l'interface ajoute
  « (encodage non valide) » dans la liste, la grille et l'infobulle. L'onglet, le fil
  d'Ariane et la barre d'adresse montrent `U+FFFD`.
- **Parcours qui gardent les octets** : ouvrir un tel dossier (listage et surveillance),
  renommer un tel élément, et restaurer ou purger la corbeille, qui sélectionne les
  éléments par leur texte réversible.
- **Refus explicite ailleurs** : copier, déplacer, supprimer, mettre à la corbeille,
  compresser et extraire reconstruisent des chemins et journalisent du texte. Ils
  refusent un tel élément avant tout effet (`UnrepresentablePathFailure`, vérifiée à la
  préparation et dans la file des opérations). Le message traduit invite à renommer
  l'élément. Les autres actions (ouvrir, aperçu, Propriétés) échouent sans effet par
  construction.

## Étapes

1. **Identité et affichage** : texte réversible au listage, marque à l'affichage, refus
   des opérations.
2. **Remède** : renommage avec les octets d'origine, ouverture des dossiers.
3. **Bilan.**

## Bilan

**Résultat.** Un nom non UTF-8 est listé à part de son sosie, marqué comme dans Nautilus,
et ne peut plus faire agir sur un autre fichier. Il se corrige par un renommage, qui part
du nom avec `U+FFFD`. Copier, déplacer, supprimer ou compresser un tel élément est refusé
sans rien modifier, avec un message qui invite à le renommer. Vérifié par des tests sur
Btrfs et tmpfs : listage, ouverture de dossier, renommage, refus de chaque type
d'opération, sélection dans la corbeille. NTFS (ntfs-3g) refuse lui-même ces noms
(`EILSEQ`). Le dossier `~/oxyde-test/noms-invalides` sert à l'essai visuel.

**Écart restant avec Nautilus.** Copier, déplacer, supprimer et compresser ces éléments
restent refusés. Les accepter demanderait de garder les octets dans tout le moteur
d'opérations et dans le journal d'Annuler/Rétablir.

**Fichiers communs touchés** :
- `models/file_entry.rs` (chemin réversible, `has_invalid_name`) ;
- `models/error.rs` (`UnrepresentablePathFailure`) ;
- `file_ops/scheduler.rs` et `ops/conflicts.rs` (refus) ;
- `commands/io.rs` (listage, renommage) et `commands/search.rs` ;
- l'affichage : liste, grille, infobulle, onglet, fil d'Ariane et renommage.

Sous Windows, `encode` et `decode` gardent la conversion d'origine.

**Leçons.**
- NUL est le seul octet absent de tout chemin Unix. Comme échappement, il rend un oubli
  de `decode` inoffensif au lieu de dangereux, ce qui a évité de reprendre les quelque
  370 conversions `String`/`Path` du code.
- ntfs-3g ne crée pas de nom non UTF-8 : les essais se font sur Btrfs, ext4 ou tmpfs.
