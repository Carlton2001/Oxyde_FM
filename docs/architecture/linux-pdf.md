# PDF sous Linux (poppler)

**Statut : implémenté, vérification visuelle en attente** (première partie de T09 de
l'[audit de portabilité](../reports/2026-10-26-linux-portability-audit.md)). Windows ne
change pas.

## Principe : comme Nautilus et Papers

Papers (la visionneuse de GNOME) et son miniaturiseur `papers-thumbnailer`, que
Nautilus lance pour les PDF, rendent les pages avec poppler. Nautilus exécute ses
miniaturiseurs hors de son processus : un document piégé ne peut faire tomber que
le miniaturiseur. Oxyde fait de même avec un worker, c'est-à-dire son propre binaire
lancé en sous-processus (`--oxyde-preview-worker-v1`). Le décodage reste ainsi hors
du processus de l'interface.

## Couches

- `platform/linux_library.rs` : macro `library!` commune à GIO et poppler, qui charge
  une bibliothèque système à la première utilisation par `libloading`. Sans elle, seule
  sa fonction manque ; aucun paquet `-devel` n'est nécessaire à la compilation.
- `platform/linux_worker.rs` : lancement du worker avec entrée et sortie en tubes,
  octet de démarrage `0xA7` comme sous Windows, lecture bornée dans le temps par
  `poll`, arrêt forcé (`SIGKILL`) et récolte à la libération. La mémoire privée du
  worker est plafonnée par `RLIMIT_DATA`, qui joue le rôle de la limite de mémoire
  engagée du job Windows. L'espace d'adressage (`RLIMIT_AS`) compterait le code mappé :
  le binaire de debug pèse 1 Gio. `MALLOC_ARENA_MAX=2` évite que glibc réserve 64 Mio
  par fil.
  Le worker surveille son parent toutes les 500 ms et se tue s'il a changé.
  `PR_SET_PDEATHSIG` ne convient pas : le signal suit le fil qui a lancé le worker,
  et un pool peut retirer ce fil.
- `platform/linux_pdf.rs` (côté worker) : `poppler_document_new_from_fd`, puis rendu
  cairo sur fond blanc en ARGB32 prémultiplié, c'est-à-dire BGRA en mémoire, le format
  que l'interface envoie au GPU. La taille de page de poppler tient compte de la
  rotation. Les côtés sont les mêmes que sous Windows : 128 px pour la grille, 1024 px
  pour le survol, 2048 px pour le Viewer. Au plus 100 000 pages listées.
- `platform/linux_pdf/session.rs` (côté parent) : un worker par miniature (`pdf-grid`,
  `pdf`). Pour le Viewer, un worker par document (`pdf-session`) garde le document
  ouvert et rend une page par requête. Le protocole est celui de Windows : tailles des
  pages, puis une trame par page, et `u32::MAX` pour fermer, avec un accusé de
  fermeture.

## Garanties et limites

- Plafonds : 1 Gio de mémoire privée et 15 s par réponse ; la fermeture attend 2 s.
  Délai, plantage ou flux illisible retirent le worker. Une erreur de page garde la
  session ouverte.
- Le worker n'est **pas confiné** : il a les droits de l'utilisateur (M09/R5). Il
  isole les plantages, pas les fuites de données.
- Une miniature coûte un lancement de processus, environ 50 à 90 ms en debug. Une
  session réutilisable comme `pdf-previews` sous Windows n'est pas implémentée.
- Les documents protégés par mot de passe affichent l'erreur de poppler.

## Tests

`cargo test --offline --lib linux_pdf` : rendu, rotation, pages hors document,
fichiers endommagés. Les tests de session lancent le vrai worker et exigent d'abord
`cargo build --offline --bins`.
