# Cache des fichiers ouverts depuis une archive, sous Linux

**Statut : domaine terminé** (treizième domaine de l'option B, T07 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md), après l'[accès sans
lecture](linux-access.md)) ; bilan ci-dessous. Ce document est le contrat du domaine.

## Périmètre

Ouvrir un fichier contenu dans une archive l'extrait dans une entrée du cache, puis
le confie à l'application associée. Le cache est borné (1 Gio, 512 entrées, 256 Mio par
fichier) ; une entrée fermée et intacte part après 7 jours, ou dès 60 secondes quand la
place manque. Une copie modifiée, encore ouverte ou sans preuve de propriété reste.
Windows garde son cache v2 et sa récupération authentifiée, sans changement.

## Constat Linux

- `open_cache_cleanup` refusait toute éviction : un verrou `flock` ne prouve pas qu'un
  lecteur externe a fermé le fichier. Rien ne partait jamais, ni après 7 jours, ni
  sous pression, ni après un crash : après 512 ouvertures ou 1 Gio, chaque ouverture
  depuis une archive échouait (« cache plein »).
- Le cache vivait dans `/tmp/oxyde-archive-open`, partagé par tous les comptes : un
  autre utilisateur pouvait créer ce dossier avant Oxyde et bloquer le cache.
- `remove_open_file` supprimait par chemin, sans vérifier l'identité, et ne savait
  pas supprimer un dossier.

## Règles

- **Emplacement** : `$XDG_CACHE_HOME/Oxyde` (`~/.cache/Oxyde`), propre à l'utilisateur ;
  `data/cache` en mode portable. Sous Windows, `%TEMP%` est déjà propre à l'utilisateur.
- **Preuve de fermeture** : un bail d'écriture (`F_SETLEASE`, `F_WRLCK`) sur chaque
  fichier de l'entrée, marqueur compris. Le noyau ne l'accorde que si aucune autre
  description de fichier ni projection mémoire n'existe, quel que soit le processus,
  et tant qu'il est tenu, un nouvel ouvreur attend. C'est l'équivalent du partage
  exclusif de Windows ; `flock` n'apporte pas cette preuve.
  - fichier ouvert ailleurs : l'entrée entière reste, rien n'est supprimé ;
  - juste avant de supprimer, chaque bail est revérifié : un ouvreur arrivé entre-temps
    garde l'entrée ;
  - la rupture d'un bail signale son détenteur, par `SIGIO` qui terminerait Oxyde :
    signal ignoré par défaut (`SIGURG`) et aucun destinataire ;
  - système sans bail (FUSE, NFS) : rien n'est évincé, comme avant.
- **Suppression** : par l'objet tenu ; un remplaçant publié au même chemin est gardé.
- Le reste du contrat est commun : liens refusés, empreinte BLAKE3 du contenu, entrée
  à un seul fichier, préparations en cours protégées par leur marqueur ouvert.

## Étapes

1. **Bail et suppression par identité** derrière `Platform`.
2. **Emplacement par utilisateur.**
3. **Tests communs activés sous Linux**, plus lecteurs d'un autre processus, liens et
   saturation.
4. **Bilan.**

## Bilan

**Résultat.** Le cache Linux se nettoie comme celui de Windows : une copie fermée et
intacte part après 7 jours, ou après 60 secondes quand la place manque, la plus
ancienne d'abord ; une copie ouverte, même dans un autre processus ou projetée en
mémoire, une copie modifiée et une entrée contenant un fichier étranger restent. Une
préparation interrompue par un crash est reprise 60 secondes plus tard, dès que son
marqueur n'est plus ouvert. Le cache est dans le dossier de l'utilisateur.

Vérifié par des tests sur Btrfs :
- bail : fichier fermé, lecteur du même processus ou d'un autre, ouvreur pendant le
  nettoyage (noté, il attend, aucun signal), lien, FIFO, dossier ;
- les tests du cache, jusqu'ici réservés à Windows, tournent sous Linux, sauf ceux qui
  emploient le partage et les jonctions Windows, remplacés par un lecteur ouvert et un
  lien symbolique ;
- saturation : au seuil d'entrées, les copies récentes ou ouvertes donnent « cache
  plein » sans rien supprimer, puis la plus ancienne part dès sa fermeture ;
- avec l'ancien refus, cinq de ces tests échouent : aucune éviction.

Une sonde a aussi confirmé qu'une projection mémoire, même après fermeture de son
descripteur, empêche le bail, et qu'un descripteur `O_PATH` ne l'empêche pas.

**Écarts restant.**
- Une application qui lit puis ferme le fichier (la plupart des éditeurs) n'est pas
  visible : sa copie peut partir sous pression après 60 secondes, comme le Bloc-notes
  sous Windows. Une copie déjà modifiée reste protégée par son empreinte, et un éditeur
  qui pose un fichier de verrou à côté (LibreOffice) protège l'entrée.
- Les copies déjà présentes dans `/tmp/oxyde-archive-open` ne sont pas reprises ;
  `/tmp` est vidé au redémarrage.

**Fichiers communs touchés** : `remove_open_file` vérifie l'identité sous Linux ;
`Platform::confirm_cache_cleanup` (sans effet sous Windows) ; les tests du cache sont
communs. Le chemin Windows du cache ne change pas.

**Leçons.**
- Un processus en cours de lancement partage tous les descripteurs jusqu'à son
  `exec` : un bail ou un `flock` peut être refusé quelques millisecondes. L'éviction
  réessaie deux fois, l'admission attend déjà le verrou.
- Ouvrir avec `O_NONBLOCK` un fichier sous bail renvoie `EWOULDBLOCK` et commence quand
  même la rupture du bail.
