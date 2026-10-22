# Mémoire des aperçus d'infobulles — 22 octobre 2026

## Conservation des sources

Le cache ne garde pas une image source de 65 Mio après fabrication de son
aperçu. `image_preview_decode::decode` produit des frames réduites ;
`image_preview::PreviewImage` contient seulement un `Arc<RenderImage>`, une
information de limite et les dimensions intrinsèques éventuelles du SVG.
Les clones de cet Arc partagent les pixels sans les recopier.

Sous Windows, `image_handle_worker/session` lit et décode la source, puis
transmet uniquement les pixels BGRA réduits via `preview_protocol`. Les octets
encodés et le raster source sortent de portée avant la requête suivante.
La session images ne possède pas de cache de sources. Elle peut conserver des
allocations de décodeur/allocateur jusqu'à son arrêt, après 15 s d'inactivité,
32 requêtes ou 60 s de durée maximale. Le job est limité à 512 Mio ; ce plafond
n'est pas une consommation habituelle ni une limite du processus parent.

| Contenu | Pixels conservés pour le survol |
| --- | --- |
| Image fixe ou SVG | 1024 px de côté au plus, soit 4 Mio maximum |
| GIF/WebP animé | 32 Mio cumulés ; dépassement : première frame seule |
| PDF | Première page à 1024 px au plus ; document libéré après rendu |
| Pochette audio | 512 px au plus, soit 1 Mio ; worker : dernière pochette réduite seulement |
| Office | Miniature à 256 px ; repli texte limité aux premières lignes |
| Vidéo | Frames réduites à 640 px, file de quatre frames et annulation au dernier propriétaire |
| Texte | Lecture de 64 Kio au plus ; 15 lignes / 4096 caractères affichés |

Les images ont un LRU de 32 entrées / 64 Mio ; les PDF un LRU indépendant de
32 entrées / 64 Mio ; les pochettes de survol ont 32 entrées / 32 Mio maximum.
Le budget commun des pixels d'aperçu est de 256 Mio / 256 images, comptés par
références faibles jusqu'à la libération du dernier Arc. Les caches et ce budget
se recouvrent : leurs plafonds ne s'additionnent pas. Le budget exclut le GPU,
les octets de transport, les buffers de décodage et le Viewer pleine résolution.

## Rétentions corrigées

1. **Textures sans retrait explicite.** `App::remove_asset` retire l'asset GPUI,
   mais ne retire pas les textures de l'atlas. Les aperçus précédents et les
   frames vidéo peintes pouvaient y rester après leur remplacement. Le module
   `tooltip/pixels` associe chaque entité à son image affichée, retire les
   textures au dernier affichage et protège le partage entre aperçus et
   transitions. L'action est différée pour inclure la fenêtre courante,
   temporairement absente de `App.windows` pendant son callback.
2. **Anciennes frames d'une fenêtre masquée.** Retirer `NativeTooltip.view`
   puis masquer la HWND ne provoquait pas de nouveau rendu. Ses deux frames
   GPUI pouvaient conserver les anciens éléments et entités, donc leurs pixels
   et le propriétaire d'annulation vidéo. Les parcours `hide` et `sync`
   remplacent maintenant ces deux frames par du contenu vide. Une nouvelle
   infobulle publiée avant ce nettoyage n'est pas effacée.
3. **Session inactive encore propriétaire.** Le contrôle natif périodique
   masquait la fenêtre sans fermer `Sessions.tooltip`. En l'absence d'événement
   de sortie du pointeur (bord de fenêtre, désactivation), cet autre propriétaire
   pouvait garder l'aperçu et sa vidéo. Le contrôle ferme désormais la session.
4. **Copie RGBA superflue.** `fitted_bgra` utilisait `to_rgba8`, qui clone un
   buffer déjà RGBA. `into_rgba8` transfère ce buffer avant l'échange rouge/bleu.
   Les dimensions, filtres, couleurs, alpha et délais d'animation sont conservés.

Les pixels réduits du LRU restent volontairement disponibles après fermeture
de la bulle pour éviter un nouveau décodage lors d'un retour sur le fichier.
Ce cache CPU borné est indépendant du retrait des textures.

## Vérification et limites

Les tests couvrent la réduction d'une source de 65 Mio de pixels vers moins de
4 Mio retenus dans le LRU, sa libération après invalidation, le partage des
images entre deux affichages, 2000 remplacements de frames, la réutilisation
du buffer RGBA et les contrats existants de décodage et de budget.

Un test natif isolé `native_hidden_tooltip_releases_preview_pixels_and_video_owner`
dessine une véritable infobulle GPUI, avec une fenêtre propriétaire et une
session de survol, puis appelle le contrôle de session invalide utilisé par le
timer natif. Il vérifie la disparition des pixels et l'annulation du propriétaire
vidéo, sans événement de sortie du pointeur ni nouveau rendu côté test après
fermeture. Les logs ont confirmé `gpu-atlas-remove` avec zéro clé restante,
puis `gpu-atlas-destroy` pour la texture Polychrome concernée.

Vérifications locales :

| Parcours | Résultat |
| --- | --- |
| Infobulles, caches, transitions et vidéo | 45 tests réussis |
| Décodage UI et budget partagé | 16 tests réussis |
| Réutilisation/réduction des buffers du décodeur partagé | 2 tests réussis |
| Sessions Windows réelles : changement de fichier, résolutions, SVG puis raster | 2 tests réussis, hors bac à sable externe pour permettre le lancement LPAC |
| Fermeture native GPUI, exécutée explicitement en isolation | 1 test réussi |

Soit 66 tests ciblés réussis. Le test natif reste ignoré par défaut dans la
suite ordinaire ; il a bien été lancé explicitement.
`cargo check --offline --locked --all-targets` est passé sans avertissement du
projet, et `cargo build --offline --locked --bins` a reconstruit les exécutables.
Les workers doivent être
reconstruits après le changement de leur décodeur partagé : fermer Oxyde,
`cargo build --offline --bins`, puis `cargo run --offline`.

Les compteurs de pixels et les références faibles ne mesurent pas le RSS,
les Private Bytes ni la mémoire du pilote. Une baisse immédiate des compteurs
du processus n'est donc pas garantie. Le décodeur doit encore ouvrir et souvent
décoder la source à pleine résolution avant réduction ; cet audit ne met pas
en place de décodage JPEG à résolution native réduite. Linux n'est pas validé
par le test de fenêtre Windows.
