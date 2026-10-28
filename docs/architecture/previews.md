# Aperçus, miniatures et icônes

Les aperçus de survol restent en mémoire ; les miniatures de grille ont un cache disque distinct. Le chargement, la validation des fichiers et le décodage restent sur les workers.

La [recherche Office](search.md#recherche-de-contenu-office) utilise son propre
mode LPAC d’`oxyde-image-worker`, une admission distincte et une réponse texte
bornée à 8 Mio. Elle ne réutilise pas les sorties tronquées des aperçus de survol.

Le démarrage d’un ghost de fichiers ferme immédiatement l’aperçu de survol,
y compris sa préparation en attente. Les survols restent désactivés pendant
le glisser-déposer, même lorsqu’aucune cible de dépôt n’est acceptée.

## Survol et présentation

`components/tooltip/hover.rs` partage une session par fenêtre, panneau et onglet/dossier. Délai initial : 750 ms ; tolérance entre éléments : 300 ms. Sortie, clic, défilement, navigation ou menu ferment la session. Timers annulables et révisions écartent les résultats obsolètes.

Sans élément survolé, la bulle se masque immédiatement et libère son aperçu.
La tolérance de 300 ms conserve seulement la session : un nouvel élément de la
même région évite le délai initial. Un passage direct entre deux éléments garde
la bulle affichée, sans fermeture intermédiaire.

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

`app/viewer` suit la sélection du dernier onglet explorateur actif et affiche un
seul fichier : celui du curseur clavier (ou de l’ancre) s’il est sélectionné,
sinon le premier de la sélection dans l’ordre de la liste. Sur une image ou une
vidéo, les flèches latérales (ou ← / →) passent au média précédent ou suivant
du même type dans la liste du panneau source, qui le sélectionne seul et le
fait défiler en vue ; le Viewer suit alors cette sélection. Sans sélection,
l’aperçu et le lecteur sont libérés. La préparation des grandes listes se fait
en arrière-plan ; une révision de source écarte les résultats obsolètes.

Avant une suppression, `viewer/deletion` bloque la réouverture des chemins
concernés jusqu’à la fin de l’opération. Le Viewer libère son contenu et attend
les chargements et rendus déjà engagés avant de confier la suppression au moteur
d’opérations. La session PDF ferme explicitement son document en arrière-plan,
même si des références de rendu subsistent. Le worker libère le document avant
d’acquitter la fermeture, puis quitte ; un délai borné conserve l’arrêt forcé
comme repli. Les rendus encore actifs gardent leur acquittement jusqu’à la
libération de leur référence native.
Une erreur ou une annulation autorise de nouveau l’affichage du fichier conservé.

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
La plage de l’ascenseur inclut ces deux marges lorsque le contenu défile ;
le maximum exposé par GPUI pour les blocs seuls les exclut.
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
La source est lue hors UI, jusqu’à 512 Mio comme le lecteur texte, avec
avertissement au-delà. Après cette lecture, une seule analyse Markdown continue
publie le premier bloc immédiatement, puis des lots de 32 blocs au plus via
une file bornée. Le Viewer ajoute ces blocs sans réinitialiser le défilement ;
chaque lot est mesuré intégralement avant son affichage, y compris hors écran,
pour donner à l’ascenseur toute la hauteur disponible sans parcourir les blocs
intermédiaires. L’arrivée des lots étend encore la plage pendant l’analyse.
Le rendu reste virtualisé ; la mesure ne déclenche pas de décodage d’image,
réservé aux blocs peints. Les images arrivées invalident la mesure de leur bloc
en conservant l’ancre de défilement. Les limites de 100 000 événements et 20 000 blocs
sont supprimées. Les longs blocs sont découpés en portions de 16 Kio maximum,
aux frontières UTF-8, sans perte de texte ni des styles inline. Les clôtures de
code, listes et références gardent leur contexte entre les lots ; la coloration
du code conserve son budget existant. La source et les blocs préparés restent
en mémoire : le plafond de source ne constitue pas une borne du RSS global.
Changer de fichier ou de mode interrompt la lecture entre morceaux de 64 Kio,
puis l’analyse entre événements et portions ; un appel du parseur ou de Syntect
en cours n’est pas préempté. Changer de mode invalide les tâches précédentes et
libère leurs textures ; les images décodées après un changement ne sont pas publiées.
La barre d’outils en haut du Viewer, à la hauteur de la barre de navigation
de l’explorateur et avec ses métriques (boutons de 1,5 rem, icônes de 1 rem,
espacement de 0,25 rem, marges 0,5 / 0,25 rem), contient le nom du fichier,
les options d’affichage et le bouton de détachement. Le Viewer n’a pas de barre de statut.
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

Le Viewer audio affiche les tags textuels reconnus par Lofty (titre, artiste,
album, date, genre, piste/disque, commentaires, paroles et autres champs
reconnus) et la pochette intégrée, avec priorité à la couverture avant. Sans
pochette décodable, l’icône musicale reste visible ; une erreur de métadonnées
n’empêche pas la lecture. Une carte centrée regroupe la pochette et le titre,
puis trois colonnes Album / Piste / Année. La barre de lecture et
de progression suit immédiatement la carte, à la même largeur. Seuls le titre,
la piste, l’album et l’année sont affichés ; l’ensemble défile si la hauteur
disponible est insuffisante. Les valeurs sont limitées à 4096
caractères, 256 champs et 256 Kio de texte.
Sous Windows, l’extraction et le décodage passent par `oxyde-image-worker`
en LPAC avec le seul handle source hérité, 512 Mio et 15 secondes. La réponse
JSON est bornée à 8 Mio ; le parent valide dimensions et longueur des pixels
BGRA (512 × 512 maximum), sans redécoder l’image. Les images intégrées de plus
de 16 Mio et les images dépassant 64 Mio de pixels source sont ignorées.
Les réponses obsolètes sont écartées par génération et les textures libérées
au changement de contenu. Aucun fichier ni URL de pochette externe n’est chargé.
Linux lit les métadonnées en arrière-plan sans confinement ; la lecture audio
native y reste indisponible.

Sous Windows, audio et vidéo utilisent les codecs système via IMFMediaEngine (mode HWND) dans le
mode `viewer-media` du self-worker. Un même lecteur natif synchronise image et
son et reçoit play/pause (ou Espace). La toolbar propose « Lecture automatique »,
activée par défaut et persistée dans `AppConfig`. Désactivée, les nouveaux médias
s’ouvrent en pause ; une lecture en cours et une reprise après détachement gardent
leur état. Ouvrir le fichier à l’extérieur d’Oxyde met son lecteur en pause,
sans changer sa position, y compris dans le Viewer détaché ou pendant son
chargement. Les commandes natives « Ouvrir avec » et de lecture suivent la même
règle lorsqu’elles sont identifiées par leur verbe Shell. Le sous-menu « Ouvrir
avec » sans verbe Shell est reconnu par son libellé système dans les cinq langues
prises en charge ; ses applications héritent de cette action. La reprise dans Oxyde
reste manuelle. Le worker
renvoie des rapports de 25 octets (état, position, durée en ms et dimensions vidéo) ; la barre de
lecture permet de cliquer ou glisser pour se déplacer (`seek_id`/`seek_ms`).
L’ouverture des paramètres masque seulement la surface vidéo native ; la lecture
audio/vidéo conserve son état, y compris une pause demandée par l’utilisateur.
Un cadre noir aux dimensions calculées de la vidéo indique que l’affichage est
en pause pendant ce masquage. Les fichiers non affichables ont un message propre
à la visionneuse et un bouton Ouvrir, via l’action Ouvrir existante dans le panneau
source : navigation interne pour les dossiers et archives, ouverture associée
pour les autres fichiers.
La vidéo et sa barre de lecture forment un bloc centré : la barre suit immédiatement
le bas de l’image, dans le panneau comme dans la fenêtre secondaire. Les dimensions
natives remontent du worker ; le bloc adapte ses proportions à l’espace disponible
dans la frame courante, sans attendre un second rendu.
En plein écran, la vidéo utilise le plus grand rectangle qui conserve ses
proportions dans l’écran, sans réserver de hauteur aux commandes. Une barre
flottante est superposée en bas, dans une fenêtre GPUI transparente possédée et
sans activation, au-dessus de la surface native du worker, sans topmost global.
Les mouvements de souris sur la vidéo sont transmis au propriétaire ; la barre
apparaît puis se masque après deux secondes d’inactivité, sauf pendant son survol
ou une recherche dans la progression. Le fondu et le glissement de six pixels
utilisent les ressorts courts communs (entrée environ 110 ms, sortie 85 ms).
La sortie du plein écran, le changement de fichier et la fermeture retirent la
fenêtre de commandes et son timer ; le focus clavier reste dans le Viewer.
Les changements de géométrie
réveillent le superviseur puis le lecteur natif ; les attentes périodiques restent
présentes pour les rapports, la visibilité et les déplacements de la fenêtre.
Le détachement ou le retour au panneau reprend à la même position et dans le
même état ; listes et pages gardent leur défilement. La fin de lecture
reste arrêtée jusqu’à une action Lire. Le worker possède une surface vidéo
native passive, possédée par la fenêtre du Viewer et sans statut topmost global.
Elle reste activée pour recevoir la souris, sans prendre l’activation ni le
focus (`WS_EX_NOACTIVATE`, `MA_NOACTIVATE`). Les pressions et relâchements du
bouton gauche sont postés à son propriétaire GPUI après conversion en coordonnées
client physiques ; GPUI compte les clics pour le plein écran. Les clics droits
et le menu contextuel sont absorbés par la surface, sans action native.
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

Le double clic dans un aperçu image ou vidéo bascule le plein écran ; Échap
le quitte. Au survol du média, un bouton `maximize-2` apparaît dans son coin
supérieur droit, remplacé par `minimize-2` (sortie) en plein écran ; le fond du
plein écran est toujours noir ; près des bords gauche et droit de la
zone de contenu, les flèches précédent / suivant apparaissent s’il existe un
média voisin du même type, dans le panneau, la fenêtre secondaire et le plein
écran. Ils utilisent le fondu du bouton de retour en haut (`motion::presence`).
`app/viewer/media_overlay` les dessine dans le Viewer pour les images ; pour la
vidéo, placée au-dessus de la composition GPUI, ce sont des popups GPUI
transparentes possédées et sans activation, comme la barre plein écran, créées
au premier survol, conservées d’un fichier à l’autre (un pointeur immobile sur
une flèche permet d’enchaîner les sauts) et retirées au changement de fenêtre.
Comme la barre plein écran, elles sont possédées par la surface vidéo visible
(retrouvée par sa classe `OxydeMediaSurface`) : Windows les garde au-dessus
d’elle quel que soit le réordonnancement. GPUI crée ses PopUp en topmost ; ce
statut leur est retiré, si bien qu’elles restent visibles même quand Oxyde n’est
pas au premier plan, sans recouvrir les autres applications. Une HWND masquée ne
reçoit pas de `WM_PAINT` et c’est leur rendu qui les place : une commande à
afficher dont la popup est masquée (fin de fondu, changement de surface) est
donc redessinée de force, comme la barre plein écran. Avant la destruction de la surface
(fin de session, fermeture de fenêtre ou sortie du plein écran temporaire), elles
reviennent à la fenêtre GPUI. La classe de la surface déclare le curseur flèche,
qui remplace celui d’un séparateur survolé juste avant.
En plein écran, le bouton de sortie apparaît au moindre mouvement et s’efface
après deux secondes d’immobilité, sauf sous le pointeur. Un clic n’importe où
dans la bande latérale d’une flèche saute au média voisin, comme dans les
visionneuses de photothèques ; ces clics ne basculent jamais le plein écran. Le survol transmis par la surface ne signale pas la sortie
de la fenêtre : la position réelle du pointeur est relue toutes les 100 ms, et
les commandes restent visibles tant qu’il demeure dans leur zone. Le survol d’une
popup ne sert qu’à relancer ce suivi : son événement de sortie peut manquer, et
chaque commande se trouve dans sa propre zone.
Les infobulles et aperçus de survol réutilisés passent au sommet de leur
famille de fenêtres possédées à chaque apparition, au-dessus de cette surface. Depuis le panneau intégré, une fenêtre temporaire accueille le
plein écran et restitue le contenu au panneau à sa fermeture. Une fenêtre
secondaire existante conserve son rôle à la sortie du plein écran.
La molette sur les pixels d’une image zoome autour du pointeur (de 1 à 32 fois
la taille ajustée) ; revenir à 1 réinitialise le déplacement. Après agrandissement,
maintenir le bouton gauche déplace l’image. Le changement de fichier réinitialise
ce cadrage. Sous Windows, les curseurs natifs utilisent les SVG Lucide officiels `search.svg`
et `hand-grab.svg` : loupe brièvement pendant le zoom, main pendant le glissement. Ctrl+molette conserve le zoom de l’interface.

## Caches et durée de vie

Les grammaires de coloration sont chargées à la première demande d'analyse,
hors UI ; ouvrir le Viewer sur des images ne les précharge pas. Les mesures
opt-in des pixels et des caches sont décrites dans le [diagnostic mémoire](../diagnostics/memory.md).

Les caches de survol gardent uniquement les pixels réduits, jamais les octets
du fichier ni le raster source : 4 Mio au plus pour une image fixe à 1024 × 1024,
32 Mio de frames pour une animation. Le décodage peut temporairement charger la
source entière dans le worker (64 Mio encodés, 128 Mio de pixels source pour
les images). Ce pic et les allocations du décodeur sont distincts du cache.
Les buffers RGBA sont déplacés vers la conversion BGRA sans copie supplémentaire.

`tooltip/pixels` suit l'image affichée par chaque entité d'aperçu. Un remplacement
(y compris une frame vidéo), une erreur ou la libération du dernier affichage
retire les anciennes textures de l'atlas par une action différée ; les pixels
partagés avec un autre aperçu visible et les caches CPU restent disponibles.
Masquer la fenêtre native vide aussi ses deux frames GPUI, sans attendre le
prochain survol : une HWND masquée ne reçoit pas de `WM_PAINT` et pourrait
sinon garder ses anciennes entités, pixels et propriétaire d'annulation vidéo.
Le contrôle périodique ferme aussi la session de survol devenue invalide,
même si aucun événement de sortie du pointeur n'a été reçu.
Voir l'[audit mémoire des infobulles](../reports/2026-10-22-tooltip-memory.md).

Le remplacement ou la fermeture du contenu du Viewer retire ses images de
l'atlas GPUI par une action différée. Pendant une interaction, GPUI retire
temporairement la fenêtre courante de sa liste : un `drop_image(image, None)`
immédiat peut donc manquer cette fenêtre. L'action différée attend son retour
dans la liste et couvre aussi les fenêtres détachées. Ce nettoyage ne garantit
pas une baisse immédiate des Private Bytes : les frames, l'allocateur et le
pilote graphique peuvent conserver des ressources.

Le même retrait différé s'applique aux pages PDF évincées pendant le défilement,
sans attendre la fermeture du document. Les demandes en attente d'une page
évincée ou d'un PDF fermé sont annulées avant leur décodage ; un rendu natif
déjà commencé peut terminer. Chaque réponse doit toujours posséder sa demande :
une ancienne requête annulée ne retire pas la nouvelle requête du même numéro
de page. Les pages PDF et images Markdown sont suivies par le diagnostic dès
le décodage, puis à leur publication et à leur retrait.

| Usage | Limite et politique |
| --- | --- |
| Pixels images | LRU RAM : 32 entrées / 64 Mio ; toutes les frames comptent |
| Pixels PDF | LRU RAM distinct : 32 entrées / 64 Mio |
| Décodage de survol | Enveloppe fixe 1024 px ; Gaussian pour grandes images, sans JPEG intermédiaire ; PDF natif 1024 px |
| Miniature de grille | Une frame, 128 px, Triangle ; réutilisation opportuniste des pixels RAM, sans charger un aperçu en cas d’absence |
| Miniature persistée | JPEG qualité 75 si opaque, PNG si alpha ; 24 Kio maximum par fichier |
| Stockage Windows | `oxyde/thumbnails` sous le cache utilisateur, ou cache portable ; quota 256 Mio / 4096 entrées |

Le survol ne lit ni n’écrit le cache disque. Une miniature ne remplit pas le cache des aperçus. Les sorties trop lourdes restent affichables sans persistance ; les petites images ne sont pas agrandies. Les anciens caches d’aperçus sont ignorés jusqu’à l’action Effacer le cache.

Les fichiers audio de la grille utilisent leur pochette intégrée (priorité à
la couverture avant), extraite par le même parcours `Platform::audio_metadata`
que le Viewer. Les pixels validés sont réduits à 128 px avec Triangle puis
conservés dans le cache de miniatures, sous une clé audio distincte. Sans
pochette ou en cas de format non pris en charge, la carte garde son icône.
La reconnaissance utilise l’extension ou le type audio natif. Les fichiers
audio reconnus par extension affichent aussi leur pochette au survol, sans
lecture ni écriture du cache disque. Chargement, contrôles de révision et persistance
restent en arrière-plan, avec les mêmes leases que les autres cartes.

Les pochettes disposent de deux quotas RAM indépendants : 256 miniatures de grille
à 128 px (16 Mio au plus) et 32 aperçus de survol à 512 px (32 Mio au plus).
Les lectures du cache disque alimentent aussi ce cache RAM ; revenir sur une
carte évite de relire et redécoder la miniature. Les révisions des fichiers sont
vérifiées avant réutilisation ; changement de fichier et effacement du cache
invalident les entrées concernées.
Le survol audio utilise la même transition que les images : un indice RAM sans
I/O évite d’atténuer l’ancienne pochette pendant la validation ; seul un succès
de cache validé par le worker permet de publier sans le délai de remplacement.
Une empreinte BLAKE3 des dimensions et pixels
permet de partager les images identiques entre morceaux, et les miniatures
identiques entre cartes. Ce partage ne repose pas sur le nom d’album. Sur
disque, une référence de 40 octets par révision de morceau pointe vers une
miniature commune nommée par son empreinte, vérifiée à la lecture. Références
et miniatures restent dans le quota existant ; la suppression d’une miniature
commune entraîne un cache manquant, régénéré au prochain chargement. Les
fichiers sources conservent leurs contrôles de révision indépendants.

Les pochettes de survol et de grille utilisent une session LPAC audio dédiée
et réutilisable de `oxyde-image-worker`, distincte des sessions images et de
l’extraction complète du Viewer. Les requêtes transmettent seulement le handle
source et la résolution attendue ; les réponses contiennent les pixels BGRA
binaires, sans tags ni JSON. La grille est réduite directement à 128 px dans
le worker, le survol à 512 px. Le job audio est plafonné à 256 Mio, avec 15 s
par requête, 15 s d’inactivité et recyclage après 32 requêtes ou 60 s. Un fichier
valide sans pochette ne provoque pas de redémarrage ; erreur de parse/décodage,
crash ou délai retirent la session, sans repli local. Le worker garde uniquement
la dernière pochette décodée, par empreinte des octets intégrés et résolution
(1 Mio au plus). Une grande source audio peut être parcourue par seek ; elle
n’est pas chargée intégralement pour produire une pochette.
Le survol audio et les tags du Viewer ont chacun une admission dédiée d’une
requête, afin de ne pas attendre toute la file des miniatures. Ces admissions
s’appliquent désormais à l’ensemble du survol et aux chargements initiaux du
Viewer (images, texte, Markdown, Office et ouverture PDF). Les images relatives
du Markdown conservent leur admission d’arrière-plan. Les tags du
Viewer conservent leur worker ponctuel et leur réponse complète, mais ne
prennent plus le verrou global des aperçus fixes. Les budgets agrégés restent
communs : cette séparation ne garantit pas une disponibilité immédiate sous
pression mémoire ou lorsque toutes les places de processus sont occupées.

Sous Linux, poppler rend ces pages dans un worker d'Oxyde, aux mêmes tailles ; voir
le [PDF sous Linux](linux-pdf.md). La suite décrit Windows.

Les miniatures PDF de la grille et les aperçus PDF partagent une session native
dédiée de `oxyde.exe` (`pdf-previews`), distincte de celle du Viewer et des
icônes Shell. Un document est ouvert, rendu puis libéré à chaque requête ; le
processus est réutilisé. Le rendu natif est directement limité à 128 px pour
la grille et 1024 px pour le survol, sans rendu intermédiaire à 1024 px pour
les miniatures. Le transport partage le protocole borné des sessions natives
à chemins : une commande, une longueur et un chemin UTF-8 ; réponses identifiées
et bornées, un seul cadre BGRA à la résolution attendue. Le parent retient la
source en lecture pendant la requête. Plafonds : 512 Mio, 15 s par requête,
15 s d’inactivité, 32 requêtes, 60 s de durée de session et 4095 octets cumulés
de commandes. Les chemins dépassant ce transport gardent un worker ponctuel,
avec la même résolution de sortie. Erreur, crash ou délai retirent le processus,
sans décodage PDF dans le parent. Cette session reste hors LPAC et peut conserver
de l’état du moteur PDF entre documents ; elle ne garantit pas l’isolation par
document. Les sessions images, audio et PDF n’attendent plus le verrou des
traitements ponctuels Office. Les requêtes déjà engagées dans une même session
restent séquentielles ; elles ne sont pas préemptées par une demande de survol.

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

La [fermeture](../diagnostics/shutdown.md) arrête les superviseurs image, icônes et aperçus PDF avant la sortie native finale. Les [limites de sécurité](../security/status.md) restent applicables. Les tests de propriété des pixels ne mesurent pas une session GPUI réelle ; Linux et le DPI exact du décodage ne sont pas validés par ces contrats.
