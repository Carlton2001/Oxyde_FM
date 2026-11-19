# Audit UI-COMP-06

Fichiers lus en entier :

- [x] src/ui/components/tooltip.rs
- [x] src/ui/components/tooltip/assets.rs
- [x] src/ui/components/tooltip/hover.rs
- [x] src/ui/components/tooltip/hover/anchored.rs
- [x] src/ui/components/tooltip/hover/handover.rs
- [x] src/ui/components/tooltip/hover/native_window.rs
- [x] src/ui/components/tooltip/hover/scope.rs
- [x] src/ui/components/tooltip/image_cache.rs
- [x] src/ui/components/tooltip/native.rs
- [x] src/ui/components/tooltip/pdf_cache.rs
- [x] src/ui/components/tooltip/pixels.rs
- [x] src/ui/components/tooltip/placement.rs
- [x] src/ui/components/tooltip/position.rs
- [x] src/ui/components/tooltip/presentation.rs
- [x] src/ui/components/tooltip/preview.rs
- [x] src/ui/components/tooltip/video.rs
- [x] src/ui/components/tooltip/viewer.rs
- [x] src/ui/components/top_bar.rs
- [x] src/ui/components/top_bar/breadcrumb_fades.rs
- [x] src/ui/components/top_bar/breadcrumb_glide.rs
- [x] src/ui/components/top_bar/breadcrumb_menu.rs
- [x] src/ui/components/window_controls.rs
- [x] src/ui/components/window_controls/tests.rs

Résumé : 0 critique, 0 élevé, 3 moyens, 7 faibles, 5 optimisations. Les caches image et PDF, la comptabilité des propriétaires d'assets, la gestion de la révision des sessions et l'annulation vidéo n'ont pas révélé de défaut.

## Moyen

- `src/ui/components/tooltip/preview.rs:136-145, 184-397` (déclenché par `hover.rs:189`), moyen, confirmé. Les aperçus texte, audio et Office démarrent leur chargement dès la construction de l'infobulle, dans une tâche détachée et sans lien avec la vie de l'entité. Le seul emplacement hover de `preview_jobs` (FIFO, sans annulation) les exécute tous, même si l'infobulle est détruite entre-temps. Scénario : balayer rapidement le pointeur sur 30 fichiers Office ou audio empile 30 travaux périmés. L'aperçu de l'élément finalement survolé attend que tous soient terminés, et les miniatures Office, coûteuses, sont toutes calculées. Les images et PDF sont épargnés, car leur future est abandonné avec l'asset.
- `src/ui/components/tooltip/preview.rs:370-379`, `assets.rs:16-22`, `pdf_cache.rs:142`, moyen, plausible. Le slot unique de la file hover n'a ni délai maximal ni annulation d'un travail démarré. Une lecture bloquante (fichier texte sur un partage réseau injoignable, tube nommé nommé en .txt sous Linux, PDF ou Office pathologique) occupe le slot. Plus aucun aperçu hover n'est produit jusqu'au retour de l'appel. Les boucles d'attente de `hover.rs:197-237` et `handover.rs:62-109` interrogent alors toutes les 16 ms sans échéance tant que le pointeur reste sur l'élément.
- `src/ui/components/top_bar.rs:341-343, 324-348`, moyen, plausible. Pour un chemin de type URI (`smb://hôte/partage`, utilisé pour les serveurs consultables dans `browsable_remotes`), le premier segment `smb:` produit la cible `smb:/` avec une seule barre, et les suivants `smb:/hôte`. Ces cibles ne sont plus des URI distantes. Scénario : parcourir un serveur SMB ou SFTP en liste, puis cliquer un segment parent du fil d'Ariane ou l'ouvrir dans un onglet. La navigation échoue ou traite la cible comme un chemin local.

## Faible

- `src/ui/components/top_bar.rs:341`, faible, plausible. Sous Linux, un dossier de premier niveau dont le nom se termine par deux-points (`/projet:`) est pris pour une lettre de lecteur. La cible du segment devient `projet:/`, un chemin relatif, au lieu de `/projet:`. Cliquer ce segment navigue vers un chemin faux.
- `src/ui/components/top_bar.rs:509-510`, faible, confirmé. Sous Windows, le résultat de l'écriture dans le presse-papiers est ignoré. Si un autre processus détient le presse-papiers, l'ouverture échoue sans nouvelle tentative (`set_text` n'en fait aucune). Le menu se ferme et l'utilisateur croit avoir copié le chemin.
- `src/ui/components/top_bar/breadcrumb_menu.rs:57`, faible, plausible. Tous les menus de chevron partagent un unique handle de défilement de l'application, et rien ne le remet à zéro à l'ouverture (`toggle_breadcrumb_menu`). Scénario : faire défiler un long menu, le fermer, puis ouvrir celui d'un autre chevron. Il peut s'ouvrir avec le décalage du précédent.
- `src/ui/components/tooltip/presentation.rs:285-309, 32-39`, faible, plausible. Un nom de fichier contenant un saut de ligne (valide sous Linux) est passé tel quel à `shape_line`. Le GPUI vendoré le refuse par une assertion de débogage (`vendor/gpui/src/text_system.rs:636`). En build de débogage, survoler ce fichier fait paniquer l'interface. Les chemins d'emplacement des résultats de recherche sont concernés de la même façon.
- `src/ui/components/tooltip/hover.rs:176-186` avec `tooltip.rs:131-163`, faible, plausible. L'infobulle est construite une seule fois à l'entrée sur l'élément, avec la taille, la date et le contenu d'alors. La balise de portée ne contient que l'onglet et son chemin. Scénario : survoler un fichier en cours de copie ou de modification, la liste se rafraîchit. L'infobulle affichée garde la taille et l'aperçu anciens jusqu'à la sortie du pointeur.
- `src/ui/components/tooltip.rs:28-32`, `assets.rs:94-106`, faible, plausible. `invalidate_media_preview` vide les caches RAM, mais pas les assets GPUI des infobulles, qui ne sont retirés qu'à la libération du dernier propriétaire. Scénario : le visualiseur (`new_viewer`, même asset et même source) reste ouvert sur une image modifiée ensuite sur disque. Une infobulle sur ce fichier réutilise les anciens pixels, sans contrôle de signature.
- `src/ui/components/tooltip/hover/native_window.rs:534-538`, faible, plausible. Un échec transitoire de création ou de configuration de la surface native marque la fenêtre parente comme définitivement en échec (`failed`) jusqu'à sa fermeture. Les infobulles image et PDF retombent alors dans la fenêtre, sans nouvelle tentative.

## Optimisation

- `src/ui/components/tooltip/hover/native_window.rs:516-528`, optimisation, confirmé. Sous Windows, la surface cachée subsiste pour chaque fenêtre parente (`hide` ne la ferme pas), et sa tâche de surveillance se réveille toutes les 100 ms et met à jour la fenêtre parente pour toute la durée de vie de celle-ci. Cela fait environ 10 réveils par seconde en inactivité après la première infobulle native, y compris quand rien n'est affiché.
- `src/ui/components/tooltip.rs:78-88` (appelé à chaque tour de 16 ms de `hover.rs:217` et `handover.rs:62-105`), optimisation, confirmé. Chaque contrôle d'état clone l'intégralité de `PreviewTask`, y compris le texte d'aperçu (jusqu'à plusieurs milliers de caractères) et les `Arc` d'images. Une lecture par référence suffirait.
- `src/ui/components/tooltip/presentation.rs:45-79, 251-351`, optimisation, plausible. Le rendu de l'infobulle est refait à chaque image, donc à chaque mouvement du pointeur : mise en forme, formatage des dates et libellés, puis découpage en lignes. Ce découpage appelle `x_for_index` pour chaque graphème, ce qui est quadratique pour un long chemin d'emplacement. Un résultat de recherche profondément imbriqué le fait sentir.
- `src/ui/components/tooltip/placement.rs:61-72`, optimisation, plausible. La taille maximale de l'aperçu dépend de la position verticale du pointeur. Dans la partie haute de la fenêtre (hors surface native), chaque mouvement modifie la limite et déclenche une notification de l'aperçu pendant le calcul de la mise en page, donc une image supplémentaire.
- `src/ui/components/top_bar.rs:62-68, 20-30`, optimisation, plausible. À chaque rendu de la barre, l'icône racine parcourt tous les ancêtres du chemin pour chaque lecteur, avec conversions en chaîne et allocations (profondeur × nombre de lecteurs). `drive_display` met en majuscules tous les chemins de lecteur pour chaque segment.

<!-- LOT TERMINÉ -->
