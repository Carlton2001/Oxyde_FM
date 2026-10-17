# Aperçus, miniatures et icônes

Les aperçus de survol restent en mémoire ; les miniatures de grille ont un cache disque distinct. Le chargement, la validation des fichiers et le décodage restent sur les workers.

## Survol et présentation

`components/tooltip/hover.rs` partage une session par fenêtre, panneau et onglet/dossier. Délai initial : 750 ms ; tolérance entre éléments : 300 ms. Sortie, clic, défilement, navigation ou menu ferment la session. Timers annulables et révisions écartent les résultats obsolètes.

Un remplacement avec nouvelle génération attend 150 ms de survol stable et conserve la bulle précédente jusqu’à publication complète du nom, des métadonnées et de l’aperçu. Un résultat prêt dans le cache est publié immédiatement après validation de la source. Les vidéos et infobulles sans aperçu changent immédiatement. « Chargement de l’aperçu… » n’apparaît qu’après une seconde ; erreurs et résultats prêts ne sont pas retardés.

Les dimensions logiques suivent le zoom : hauteur maximale 12,5 rem pour images/vidéos, 18,75 rem pour PDF, largeur maximale 50 rem. Conserver les proportions et ne pas agrandir les petites sources. L’aperçu prêt fixe la largeur ; le nom y revient à la ligne.

Le repli intégré ancre le bloc nom/métadonnées au curseur (0,75 rem de décalage, marge de bord 3,125 rem ajustable). L’aperçu apparaît au-dessus sans déplacer ce repère ni réserver de place vide. `tooltip/position.rs` et `placement.rs` portent ces calculs. Les infobulles fixes conservent leur ancre ; transport natif : [fenêtres](windows.md).

## Caches et durée de vie

| Usage | Limite et politique |
| --- | --- |
| Pixels images | LRU RAM : 32 entrées / 64 Mio ; toutes les frames comptent |
| Pixels PDF | LRU RAM distinct : 32 entrées / 64 Mio |
| Décodage de survol | Enveloppe fixe 1024 px ; Gaussian pour grandes images, sans JPEG intermédiaire ; PDF natif 1024 px |
| Miniature de grille | Une frame, 128 px, Triangle ; réutilisation opportuniste des pixels RAM, sans charger un aperçu en cas d’absence |
| Miniature persistée | JPEG qualité 75 si opaque, PNG si alpha ; 24 Kio maximum par fichier |
| Stockage Windows | `oxyde/thumbnails` sous le cache utilisateur, ou cache portable ; quota 256 Mio / 4096 entrées |

Le survol ne lit ni n’écrit le cache disque. Une miniature ne remplit pas le cache des aperçus. Les sorties trop lourdes restent affichables sans persistance ; les petites images ne sont pas agrandies. Les anciens caches d’aperçus sont ignorés jusqu’à l’action Effacer le cache.

Les clés utilisent chemin, type, taille et date ; les workers vérifient la source avant réutilisation et autour du décodage. Pas de cache des erreurs. Effacer le cache invalide les générations en cours. La signature taille/date n’est pas une preuve d’identité ou de contenu.

`ui/preview_disk` et les assets de grille lient chaque miniature à la révision listée (`FileStamp`). Les cartes partagent une lease ; sortie du viewport, changement de carte/vue ou fermeture libèrent celle-ci. Le dernier propriétaire retire l’asset et ses pixels de l’atlas GPUI par `App::drop_image`. Les références de frames peuvent survivre temporairement : ce contrat ne borne pas le RSS/GPU global. Aucun appel de métadonnées dans le rendu.

## Sessions Windows

- Images/SVG : `windows_preview_worker/image_session.rs`, worker LPAC séquentiel à la demande. Arrêt après 15 s d’inactivité, recyclage après 32 demandes ou 60 s ; délai actif 15 s, mémoire 512 Mio et admission commune conservés.
- Le parent retient la source et duplique seulement son handle en lecture. Commandes et réponses sont bornées, identifiées et validées ; erreur/crash/délai retirent la session, sans repli local ni nouvelle tentative automatique du fichier fautif.
- La réutilisation conserve potentiellement l’état d’un décodeur compromis entre images d’une session. Le recyclage réduit cette fenêtre sans rétablir l’isolation par document.
- Icônes Shell : `icon_session.rs`, session native distincte hors LPAC, mémoire 256 Mio, arrêt après 15 s d’inactivité, 64 demandes ou budget cumulé de requêtes épuisé. Réponse bornée à 1 Mio et 256 × 256 ; chemins trop longs via worker ponctuel.
- Les icônes ordinaires partagent leur source par extension/taille ; dossiers génériques également. Exécutables, raccourcis, lecteurs et chemins spéciaux gardent leurs exceptions. Les personnalisations arbitraires `desktop.ini` ne sont pas détectées par cette heuristique.
- PDF, vidéo et texte Office gardent leurs parcours distincts. La vidéo utilise une file bornée, l’annulation du dernier propriétaire et l’horloge PTS ; seuls les flux terminés normalement après une image bouclent.

La [fermeture](../diagnostics/shutdown.md) arrête les superviseurs image puis icônes avant la sortie native finale. Les [limites de sécurité](../security/status.md) restent applicables. Les tests de propriété des pixels ne mesurent pas une session GPUI réelle ; Linux et le DPI exact du décodage ne sont pas validés par ces contrats.
