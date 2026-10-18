# Aperçus, miniatures et icônes

Les aperçus de survol restent en mémoire ; les miniatures de grille ont un cache disque distinct. Le chargement, la validation des fichiers et le décodage restent sur les workers.

## Survol et présentation

`components/tooltip/hover.rs` partage une session par fenêtre, panneau et onglet/dossier. Délai initial : 750 ms ; tolérance entre éléments : 300 ms. Sortie, clic, défilement, navigation ou menu ferment la session. Timers annulables et révisions écartent les résultats obsolètes.

Au premier survol, la vue et son aperçu sont préparés pendant les 750 ms, sans affichage anticipé. La tâche annulable possède cette vue jusqu’à publication ; quitter l’élément la libère. Les 750 ms sont un minimum : si l’aperçu est encore en cours, la publication attend son contenu (première frame comprise pour une vidéo) ou son erreur définitive. Texte et aperçu apparaissent ensemble, sans étape « Chargement de l’aperçu… ». Ce délai supplémentaire ne concerne pas les remplacements dans une session déjà visible.

Un remplacement avec nouvelle génération affiche immédiatement les nouvelles métadonnées et l’ancien aperçu en niveaux de gris à 15 % d’opacité, lorsqu’une image précédente est disponible. L’opacité et les niveaux de gris sont appliqués d’un coup au rendu GPU, sans animation, conversion CPU ni copie des pixels. Les dimensions de cet aperçu temporaire sont conservées pendant l’attente. L’aperçu définitif le remplace après 150 ms de survol stable ; un résultat prêt dans le cache est publié immédiatement après validation de la source. Si des pixels sont déjà présents en RAM, la bulle précédente reste nette pendant cette validation asynchrone afin d’éviter un flash d’atténuation. Cette indication de présence ne lit pas le fichier, ne bloque pas sur le verrou du cache et ne permet jamais de publier des pixels non validés. Si aucune image précédente n’est disponible, la bulle précédente reste visible jusqu’au remplacement. Les vidéos et infobulles sans aperçu changent immédiatement. Une transition atténuée attend le contenu ou l’erreur définitive, sans message de chargement superposé. Hors transition atténuée, « Chargement de l’aperçu… » n’apparaît qu’après une seconde.

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
- Les icônes disposent de deux places de chargement distinctes des quatre places des aperçus fixes. La session Shell reste séquentielle ; les accès au cache ne dépendent plus de la file des aperçus. Le parent et le worker conservent chacun un cache de pixels borné à 32 Mio / 1000 entrées. Le PNG sert uniquement à la persistance : une lecture disque décode une seule fois, une extraction native transmet directement ses pixels après validation des dimensions.
- Le worker d'icônes ne persiste plus le PNG. Le parent met les pixels validés en RAM et les propose à `windows_icon_persistence` : un seul thread, au plus 32 demandes / 4 Mio de pixels en attente, plus l'image active (256 Kio maximum). L'admission n'attend aucune écriture ; saturation ou échec de publication laisse l'icône disponible sans garantie de persistance. L'encodage PNG, les handles contrôlés, le quota, la synchronisation et le renommage restent hors du chemin de disponibilité des pixels. Les accès en cache RAM/disque ne sont pas republiés.
- Effacer le cache suspend les admissions, invalide les révisions et retire les demandes en attente. Le nettoyage en arrière-plan attend la publication déjà engagée puis garde la suspension pendant la suppression des fichiers. Une réponse commencée avant le vidage ne peut remplir à nouveau le cache RAM ni programmer une ancienne écriture. À la sortie, les demandes en attente sont abandonnées ; la publication active dispose d'au plus deux secondes avant la sortie native finale. Un arrêt immédiat peut donc perdre les dernières icônes persistées, qui sont régénérables.
- Les icônes ordinaires partagent leur source par extension/taille ; dossiers génériques également. Exécutables, raccourcis, lecteurs et chemins spéciaux gardent leurs exceptions. Les personnalisations arbitraires `desktop.ini` ne sont pas détectées par cette heuristique.
- PDF, vidéo et texte Office gardent leurs parcours distincts. La vidéo utilise une file bornée, l’annulation du dernier propriétaire et l’horloge PTS ; seuls les flux terminés normalement après une image bouclent.

La [fermeture](../diagnostics/shutdown.md) arrête les superviseurs image puis icônes avant la sortie native finale. Les [limites de sécurité](../security/status.md) restent applicables. Les tests de propriété des pixels ne mesurent pas une session GPUI réelle ; Linux et le DPI exact du décodage ne sont pas validés par ces contrats.
