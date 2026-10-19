# Aperçus, miniatures et icônes

Les aperçus de survol restent en mémoire ; les miniatures de grille ont un cache disque distinct. Le chargement, la validation des fichiers et le décodage restent sur les workers.

## Survol et présentation

`components/tooltip/hover.rs` partage une session par fenêtre, panneau et onglet/dossier. Délai initial : 750 ms ; tolérance entre éléments : 300 ms. Sortie, clic, défilement, navigation ou menu ferment la session. Timers annulables et révisions écartent les résultats obsolètes.

Au premier survol, la vue et son aperçu sont préparés pendant les 750 ms, sans affichage anticipé. La tâche annulable possède cette vue jusqu’à publication ; quitter l’élément la libère. Les 750 ms sont un minimum : si l’aperçu est encore en cours, la publication attend son contenu (première frame comprise pour une vidéo) ou son erreur définitive. Texte et aperçu apparaissent ensemble, sans étape « Chargement de l’aperçu… ». Ce délai supplémentaire ne concerne pas les remplacements dans une session déjà visible.

Un remplacement avec nouvelle génération affiche immédiatement les nouvelles métadonnées et l’ancien aperçu en niveaux de gris à 15 % d’opacité, lorsqu’une image précédente est disponible. L’opacité et les niveaux de gris sont appliqués d’un coup au rendu GPU, sans animation, conversion CPU ni copie des pixels. Les dimensions de cet aperçu temporaire sont conservées pendant l’attente. L’aperçu définitif le remplace après 150 ms de survol stable ; un résultat prêt dans le cache est publié immédiatement après validation de la source. Si des pixels sont déjà présents en RAM, la bulle précédente reste nette pendant cette validation asynchrone afin d’éviter un flash d’atténuation. Cette indication de présence ne lit pas le fichier, ne bloque pas sur le verrou du cache et ne permet jamais de publier des pixels non validés. Si aucune image précédente n’est disponible, la bulle précédente reste visible jusqu’au remplacement. Les vidéos et infobulles sans aperçu changent immédiatement. Une transition atténuée attend le contenu ou l’erreur définitive, sans message de chargement superposé. Hors transition atténuée, « Chargement de l’aperçu… » n’apparaît qu’après une seconde.

Les dimensions logiques suivent le zoom : hauteur maximale 12,5 rem pour images/vidéos, 18,75 rem pour PDF, largeur maximale 50 rem. Conserver les proportions et ne pas agrandir les petites sources. L’aperçu prêt fixe la largeur ; le nom y revient à la ligne.

Le repli intégré ancre le bloc nom/métadonnées au curseur (0,75 rem de décalage, marge de bord 3,125 rem ajustable). L’aperçu apparaît au-dessus sans déplacer ce repère ni réserver de place vide. `tooltip/position.rs` et `placement.rs` portent ces calculs. Les infobulles fixes conservent leur ancre ; transport natif : [fenêtres](windows.md).

## Viewer

Le bouton œil de la barre de titre ouvre ou ferme l’unique onglet `viewer://`,
seul dans son panneau ; son infobulle suit cet état. Les dépôts sur les barres d’onglets refusent toute fusion dans les deux
sens ; les zones de division déplacent le Viewer avec son propre panneau.
La session restaure cet onglet sans lancer de listing ni de surveillance sur
son emplacement interne.

`app/viewer` suit la sélection du dernier onglet explorateur actif, dans l’ordre
de sa liste, et conserve le fichier courant s’il reste sélectionné. Précédent /
suivant (ou les flèches) parcourent une sélection multiple. Sans sélection,
l’aperçu et le lecteur sont libérés. La préparation des grandes listes se fait
en arrière-plan ; une révision de source écarte les résultats obsolètes.
Le Viewer ouvre le fichier complet (`app/viewer/content`) : image en pleine
résolution par une session LPAC propre au Viewer (`image_session`, type 2 :
job de 1 Gio, 30 s, côté jusqu’à 16384 px et 256 Mio de pixels, hors budget
des infobulles ; démarrée à la création du Viewer, réutilisée environ 35 ms par
image, libérée dès que le Viewer affiche autre chose ou se ferme), affichée sans agrandissement ; toutes les pages PDF (`viewer/pdf` : une session `pdf-session` du
self-worker garde le document ouvert, job de 1 Gio et 15 s par page ; seules
les pages proches de la vue sont rendues, les demandes sorties de la vue sont
abandonnées, six pages décodées au plus) ; texte entier jusqu’à 512 Mio, chargé
en mémoire (`viewer/text` : retour à la ligne aux espaces, calculé en arrière-plan
pour la largeur en colonnes de JetBrains Mono, caractères larges comptés double,
tabulation de quatre colonnes ; les continuations reprennent l’indentation
initiale, plafonnée à la moitié de la largeur disponible dans les panneaux étroits.
Chaque rangée garde sa plage d’octets et sa ligne
logique ; numéros à gauche, sans répétition sur les lignes repliées. Syntect
détecte le langage par nom ou extension, puis par première ligne (shebang ou
mode line) seulement si aucun type n’a été trouvé ; pas de détection générale
sur le corps du texte. Les `.txt` restent du texte brut. L’analyse est hors UI ;
ses catégories utilisent les couleurs sémantiques du thème. Coloration limitée
aux syntaxes réellement embarquées ; XAML utilise la grammaire XML (coloration
lexicale, sans interprétation WPF). `ui/file_types.rs` centralise les extensions,
suffixes composés et noms de fichiers Code pour les familles, icônes et aperçus.
Son inventaire inclut les formats Oxyde existants et ceux du pack Syntect 5.3,
sans charger les grammaires sur le chemin de rendu des listes. Les familles
spécialisées conservent leur priorité, notamment SVG en Images et MD en Documents.
Un fichier classé Code n’a donc pas nécessairement une grammaire embarquée.
SCSS et PowerShell ont des grammaires supplémentaires épinglées dans
`vendor/syntaxes/README.md`, avec Sass comme dépendance de SCSS. Les notices MIT
sont conservées et distribuées dans `licenses/syntaxes/`. Syntect `yaml-load`
lit uniquement les grammaires embarquées, une fois hors UI. PowerShell couvre
`.ps1`, `.psm1` et `.psd1`, dans la famille Scripts avec Shell/Batch. Les variables
utilisent la couleur sémantique accent, y compris dans les blocs Markdown.
Coloration limitée
aux fichiers de 2 Mio, 200 000 plages ; arrêt avant une ligne de plus de 16 Kio
ou entre deux lignes après 200 ms d’analyse, suite en texte brut. Ce délai
est coopératif et ne borne pas une analyse de ligne en cours ni l’initialisation
des syntaxes) ; documents Word/ODF
Office Open XML et OpenDocument complets (`office-document` dans le worker
Office : Word/Writer, chaque feuille Excel/Calc, chaque diapositive
PowerPoint/Impress ; lignes `balise<TAB>texte`). Les valeurs de tableur sont
affichées telles que stockées, sans formats ni formules. Les longs contenus sont
virtualisés par `list` avec l’ascenseur partagé ; documents et cadres de
pages PDF sont mesurés d’emblée (seules les pages proches de la vue sont
décodées), donc l’ascenseur garde sa taille. Les formats binaires anciens
(doc, xls, ppt) gardent l’aperçu existant. Le texte utilise le fond `sidebar`
comme ses aperçus ; les autres contenus reprennent celui des panneaux actifs.
Ces parcours sont propres au Viewer : les aperçus des infobulles gardent
leurs modes et limites.
Les images utilisent tout le cadre sans marge. Les listes texte/code, PDF et
Office/ODF ont 1 rem de marge intérieure en haut et en bas, intégrée au
défilement ; les espacements internes entre pages et blocs sont conservés.
Markdown (`md`, `markdown`, `mdown`, `mkd`, `mkdn`) utilise `pulldown-cmark`
avec rendu GPUI natif : titres, styles inline, listes, citations, tableaux et
blocs de code colorés par Syntect. Le rendu est activé par défaut. La toolbar
propose une bascule Rendu / Code pour Markdown et un bouton « Coloration
syntaxique » activable/désactivable pour les autres fichiers texte ayant
effectivement des plages colorées, avec les boutons
standards à icône et libellé ; ces deux choix
indépendants sont enregistrés dans `AppConfig`, avec défauts compatibles avec
les anciennes configurations. Le Markdown source reste sans coloration ; les
blocs de code du Markdown rendu sont toujours colorés selon leur langage,
indépendamment du choix des autres fichiers texte. Les images relatives au document sont décodées à la demande
par le parcours d’aperçu isolé et son budget mémoire, avec 32 demandes maximum
par document. Images absentes ou non locales : texte alternatif. Aucune URL
distante ou chemin absolu/UNC d’image n’est chargé. Les liens sont présentés
comme du texte coloré ; le HTML brut est affiché littéralement.
L’analyse est hors UI et le rendu virtualisé ; limites : 2 Mio de source,
100 000 événements, 20 000 blocs et 16 Kio par bloc de texte, avec avertissement
de troncature. La source complète reste accessible en mode Code selon les
limites du lecteur texte. Changer de mode invalide les tâches précédentes et
libère leurs textures ; les images décodées après un changement ne sont pas publiées.
La barre d’outils en haut du Viewer, à la hauteur de la barre de navigation
de l’explorateur, contient le nom du fichier, la navigation dans la sélection
et le bouton de détachement. Le Viewer n’a pas de barre de statut.
Le panneau intégré affiche le nom seul ; la fenêtre secondaire affiche le chemin
complet, avec une infobulle permettant de le lire s’il est tronqué. Le chargement
du Viewer utilise « Chargement du fichier… », distinct de celui des aperçus.
Le contenu occupe l’espace restant et l’ascenseur touche le bord droit. Le panneau du Viewer
ne devient jamais le panneau actif : ses commandes
internes agissent sur lui et le panneau explorateur garde sélection, clavier
et barre d’outils. Seule la fenêtre détachée reçoit le focus clavier.
Les dossiers, entrées d’archives virtuelles et types non pris en charge restent
sans aperçu.

Le bouton supérieur droit ouvre une fenêtre secondaire redimensionnable,
possédée par la fenêtre principale ; sa position, sa taille et son état
maximisé sont mémorisés. L’onglet et son panneau conservent leur place ;
un bouton dans le panneau permet de retrouver la fenêtre secondaire. Seule cette
fenêtre rend l’aperçu et le lecteur pendant le détachement. Le suivi de sélection
reste actif. Fermer la fenêtre secondaire libère sa session et restaure l’aperçu
dans le panneau conservé. Fermer la fenêtre principale ferme également les
Viewers détachés.

Sous Windows, audio et vidéo utilisent les codecs système via IMFMediaEngine (mode HWND) dans le
mode `viewer-media` du self-worker. Un même lecteur natif synchronise image et
son et reçoit play/pause (ou Espace). La toolbar propose « Lecture automatique »,
activée par défaut et persistée dans `AppConfig`. Désactivée, les nouveaux médias
s’ouvrent en pause ; une lecture en cours et une reprise après détachement gardent
leur état. Le worker
renvoie des rapports de 25 octets (état, position, durée en ms et dimensions vidéo) ; la barre de
lecture permet de cliquer ou glisser pour se déplacer (`seek_id`/`seek_ms`).
La vidéo et sa barre de lecture forment un bloc centré : la barre suit immédiatement
le bas de l’image, dans le panneau comme dans la fenêtre secondaire. Les dimensions
natives remontent du worker ; le bloc adapte ses proportions à l’espace disponible
dans la frame courante, sans attendre un second rendu. Les changements de géométrie
réveillent le superviseur puis le lecteur natif ; les attentes périodiques restent
présentes pour les rapports, la visibilité et les déplacements de la fenêtre.
Le détachement ou le retour au panneau reprend à la même position et dans le
même état ; listes et pages gardent leur défilement. La fin de lecture
reste arrêtée jusqu’à une action Lire. Le worker possède une surface vidéo
native passive, possédée par la fenêtre du Viewer et sans statut topmost global.
Elle suit en pixels physiques le rectangle mesuré et borné du contenu, y compris
lors des déplacements de sa fenêtre. Ce transport évite que la composition GPUI,
placée au-dessus des HWND enfants, masque la vidéo. L’audio
seul n’affiche pas cette surface. Une lecture ne passe pas dans le parent en cas
d’échec. Ce parcours est distinct des vidéos muettes de survol et n’occupe pas
leurs admissions : deux sessions Viewer au plus, puis attente annulable.

Le job natif reste hors LPAC, avec un plafond de 1 Gio et l’admission agrégée
existante. Le démarrage et l’absence de heartbeat IPC sont bornés à 15 secondes ;
ce heartbeat ne prouve pas la progression d’un codec. Les commandes JSON sont
bornées à 512 octets et les réponses à des états d’un octet. Changement de fichier,
fermeture et dernier propriétaire annulent le lecteur, retirent sa surface et
récoltent son processus. Les anciens aperçus ne peuvent publier dans un nouveau
Viewer. Linux conserve un message d’aperçu indisponible pour audio/vidéo ; ce
lecteur natif n’y est pas implémenté.

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
