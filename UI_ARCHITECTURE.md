# Architecture et conventions de l'interface

Ce document décrit les conventions à suivre pour faire évoluer l'interface GPUI d'Oxyde. Les consignes générales des agents sont dans [AGENTS.md](AGENTS.md).

## Organisation

- `src/ui/app/` porte l'état de l'application et les parcours fonctionnels.
- `src/ui/components/` regroupe les composants d'interface composés et les vues réutilisables.
- `src/ui/elements/` regroupe les éléments simples de bas niveau, tels que les boutons.
- `src/ui/theme.rs` contient les palettes et les jetons de couleur sémantiques.
- `src/ui/i18n.rs` expose les messages typés et le contexte de traduction.
- `locales/fr.ftl` et `locales/en.ftl` contiennent les catalogues embarqués.
- `src/ui/format.rs` centralise les formats de nombres, dates et tailles.

## Thèmes

Le thème actif est accessible avec `cx.theme()`. Les thèmes pris en charge sont `ThemeId::Dark` et `ThemeId::Light`, définis dans `src/models/preferences.rs`.

- Utiliser les jetons de `OxydeTheme` pour les surfaces, textes, bordures, actions, sélection, survol, erreurs, menus, onglets, icônes et barres de défilement.
- Si un rôle visuel partagé manque, ajouter un jeton sémantique et lui donner une valeur dans chaque palette. Ne pas introduire une couleur littérale dans une vue pour contourner le thème.
- Garder une valeur locale uniquement si elle exprime un effet propre au composant, comme un fond entièrement transparent. Le rôle doit rester évident dans le code.
- Vérifier le contraste et les états normal, survol, focus, sélection, désactivé et erreur dans les deux palettes. Le texte sur une action remplie doit utiliser le jeton de texte correspondant à cette action.

## Typographie

- Le poids par défaut est Regular (400), défini par `DEFAULT_FONT_WEIGHT` dans `src/ui/style.rs` et appliqué à la racine de l'application. Les descendants héritent de ce poids sauf lorsqu'un ancêtre ou le composant définit explicitement un autre poids.
- Ne préciser un poids local (Light, Medium, Semibold, Bold, etc.) que pour une variation intentionnelle. Les surfaces rendues séparément, comme les infobulles, utilisent le même poids par défaut à leur racine.

## Dimensions et zoom

- Exprimer les dimensions de conception, tailles de texte, espacements, icônes et bordures en `rems()`. La référence initiale est de 16 pixels logiques par rem ; `ui_rem_size` la conserve dans la configuration (10 à 32, par pas de 1). Le réglage « Échelle de l'interface » et Ctrl + molette appliquent le changement immédiatement. Cliquer sur la valeur du réglage rétablit 16.
- `Window::set_rem_size` et les conversions de `src/ui/scale.rs` partagent la même référence, synchronisée sur le thread UI avant le rendu et lors du zoom. Utiliser `rem_pixels` lorsqu'une API impose des `Pixels` (canvas, texte mesuré, listes virtuelles, défilement) ; utiliser `logical` pour convertir les anciennes réservations numériques définies à l'échelle 16. Ces conversions appartiennent au thread UI, jamais aux tâches de fond.
- Les positions de souris, bounds et offsets fournis par GPUI sont déjà en pixels : ne pas les convertir à nouveau. Les ratios, compteurs et durées ne sont pas des dimensions. Les bounds de la fenêtre native restent également en pixels pour que le zoom change le contenu, pas la taille de la fenêtre.
- Les réservations de colonnes et la largeur sauvegardée de la sidebar restent à l'échelle de référence 16 pour préserver les espaces de travail existants. Les convertir au rendu et reconvertir les déplacements de redimensionnement avant sauvegarde. Rendu et sélection de la grille partagent `GridGeometry` pour garder leurs zones alignées.

## Internationalisation

Le contexte `cx.i18n()` fournit la langue active et résout les clés de `Message` vers des `SharedString`.

- Pour ajouter ou modifier un texte destiné à l'utilisateur, ajouter ou réutiliser une variante `Message`, son identifiant dans `Message::key()`, et sa traduction dans **les deux** catalogues. Compléter le test qui vérifie les messages et paramètres des catalogues.
- Conserver les phrases entières dans Fluent. Utiliser `text(Message)` pour une phrase simple et `text_with_args(Message, &FluentArgs)` pour les paramètres et pluriels. Fournir une valeur numérique aux sélecteurs de pluriel ; formater séparément le nombre présenté si nécessaire.
- Garder les noms de fichiers, chemins, extensions, libellés reçus du système et autres données utilisateur comme des données ; ne pas les transformer en clés de traduction. Fluent isole les paramètres pour le texte bidirectionnel. Retirer ces marqueurs uniquement si le résultat devient une donnée de système de fichiers, comme un nom de dossier généré.
- Garder les erreurs d'interface sous forme de clé `Message` dans l'état quand elles peuvent rester visibles après un changement de langue. Les détails techniques peuvent être fournis en paramètre ; les journaux backend ne sont pas des textes UI.
- Les catalogues sont construits au démarrage ou au changement de langue, pas à chaque rendu. Ne pas ajouter de lecture de fichier ou de construction de catalogue dans un chemin de rendu fréquent.
- Pour ajouter une langue, étendre `Language`, son identifiant stable, les formats dépendants de la locale, le catalogue, le sélecteur de préférences et les tests.

Les clés manquantes ou impossibles à formater retombent sur le français ; si le repli échoue, l'interface affiche un marqueur et journalise un avertissement. Les langues actives sont le français et l'anglais.

## Composants partagés et modales

Factoriser lorsqu'au moins deux usages partagent une structure ou un comportement stable. Une ressemblance visuelle seule ne suffit pas : préserver les différences utiles de taille, d'interaction, de focus et d'accessibilité.

`src/ui/components/modal.rs` fournit `Modal`, un composant de présentation sans dépendance à `OxydeApp` :

Les nouvelles fenêtres popup internes adoptent la présentation compacte de Tauri : barre de titre de 2,75 rem (texte de 0,875 rem, poids Medium), séparée du corps par une bordure ; corps avec marges horizontales de 1 rem et verticales de 1,25 rem ; pied avec bordure supérieure et marges de 0,625 rem × 1 rem. `Modal` applique ce cadre par défaut. Les formulaires utilisent `modal::form_row` pour aligner les libellés à gauche (5,5 rem) et les champs à droite, avec un intervalle de 0,75 rem ; le texte des formulaires et boutons mesure 0,8125 rem. Les champs compacts mesurent 2 rem de haut, les boutons ont une marge verticale de 0,375 rem. Tous les boutons de modales passent par `modal::action_button` ; le corps utilise un texte de 0,8125 rem et un intervalle de 0,375 rem par défaut. Les modales utilisent un interligne de 1,25 fois la taille du texte ; les formulaires réseau espacent leurs lignes de 0,375 rem. Les titres personnalisés conservent ces dimensions. Garder les séparateurs entre groupes de champs et les couleurs sémantiques dans les deux thèmes.

- l'appelant fournit le contenu, les boutons et les callbacks ;
- l'état métier et les opérations restent dans la vue appelante ;
- le clic extérieur ne ferme la modale que si l'appelant configure `on_backdrop_dismiss` ;
- la gestion du clavier, du focus initial et de l'ordre de rendu reste explicite dans l'appelant ;
- chaque barre de titre comporte une icône Lucide officielle de 1 rem, avec la couleur sémantique du texte secondaire et un intervalle de 0,5 rem avant le titre ; utiliser `Modal::title_icon` ou conserver cette présentation dans un titre personnalisé ;
- toutes les modales sont déplaçables au clic gauche par leur barre de titre ; le composant partagé garde leur cadre entier dans les limites de la fenêtre, en fonction de leur taille réelle, et recalcule les limites lors d'un redimensionnement ou d'un changement de contenu ;
- aucun fond assombri n'est affiché derrière les modales ; la couche transparente bloque toujours les interactions et le défilement sur le contenu recouvert.

Appliquer le même principe aux futurs composants partagés : API de présentation claire, callbacks et état appartenant à l'appelant, et aucun couplage au modèle métier sans nécessité.

Les listes déroulantes des formulaires s’ouvrent en menus flottants ancrés au champ, au-dessus du contenu et hors du découpage de la modale, sans changer sa hauteur. Elles restent dans les limites de la fenêtre. L’expansion dans le flux du contenu est réservée aux sections repliables avec chevron.

## Préférences et persistance

Les préférences UI sont stockées dans `AppConfig` et lues au démarrage. Elles incluent le thème, la langue, les options d'affichage des fichiers cachés et système et les aperçus médias.

Le regroupement par date est propre à chaque onglet (`TabState.group_by_date`) et persisté dans `SavedTab`, avec une valeur par défaut désactivée pour les anciens espaces de travail. Le bouton calendrier et le réglage des paramètres modifient l’onglet concerné uniquement. Les sections repliées restent un état temporaire de cet onglet. Les périodes suivent les jours du calendrier local et les semaines commençant le lundi, comme dans Tauri ; le tri courant est conservé dans chaque période. La grille groupée virtualise ses lignes de hauteurs différentes ; la géométrie de sélection utilise les mêmes lignes que le rendu.

- Appliquer les changements de thème et de langue immédiatement via les globals GPUI, sans redémarrer ni recharger le dossier.
- Sauvegarder les changements en arrière-plan dans l'ordre. Préserver les autres options et champs de configuration inconnus lors de l'écriture.
- Garder les anciennes configurations lisibles avec des valeurs par défaut pour les champs absents ou inconnus.
- Si la configuration est illisible, ne pas l'écraser automatiquement. Présenter l'état à l'utilisateur et ne pas prétendre que les changements sont persistés.
- Les formats passent par `src/ui/format.rs`. Les nombres et tailles suivent la langue ; les dates suivent la préférence persistée `date_format` (US, European, ISO). Sans choix enregistré, la locale régionale système en-US sélectionne US, les autres European. `show_network` contrôle la visibilité du voisinage réseau dans la sidebar.

Le panneau des paramètres reprend deux colonnes : Apparence et Fichiers à gauche ; Région & langue, Performances, Données de l’application et À propos à droite. Le panneau compact est ancré sous le bouton Oxyde GPUI, sans fond assombri. Les sélecteurs de thème, langue et format de date ouvrent des sous-menus latéraux défilants, sans agrandir le panneau principal. Les sous-menus s’ouvrent à gauche si nécessaire et restent dans les limites de la fenêtre. Une sélection applique le choix sans fermer le sous-menu ; Échap ou un clic extérieur le ferme. Les options reprennent la taille de texte des menus contextuels (0,8125 rem). La rubrique Performances propose un seul menu Compression : son sous-menu affiche un tableau à trois lignes (ZIP, 7z, Zst) et quatre colonnes (Format, Rapide, Équilibrée, Maximale). Une coche indique le choix de chaque format ; le clic gauche sur une cellule applique et persiste ce choix sans fermer le tableau. Les qualités restent indépendantes et sont appliquées aux nouvelles opérations. TAR reste sans compression. Les autres options de performances et les données de l’application gardent le libellé « À venir » tant que leurs actions ne sont pas implémentées.

## Réactivité et validation

L’édition du chemin dans la breadcrumb propose jusqu’à douze dossiers correspondant au préfixe saisi, dans un menu flottant sous le champ. La recherche attend 150 ms après la dernière modification et utilise la lecture de dossiers asynchrone ; les réponses obsolètes sont ignorées. Les flèches haut/bas sélectionnent une suggestion, Tab ou le clic gauche complète le chemin en gardant l’édition active, et Entrée ou le bouton interne valide la navigation. Échap ferme d’abord les suggestions, puis l’édition. Les chemins virtuels ne proposent pas de complétion.

### Lecteurs réseau

Sous Windows, les actions « Connecter un lecteur réseau » et « Déconnecter un lecteur réseau » sont disponibles dans le menu contextuel de l’item Réseau du panneau gauche. Ces actions ne figurent pas dans la barre de navigation. Un partage UNC propose la connexion avec son chemin prérempli dans son menu contextuel ; un lecteur distant propose sa déconnexion, également depuis les dossiers qu’il contient. Les serveurs seuls et les périphériques Shell ne proposent pas de montage.

La navigation vers un serveur UNC utilise `Platform::list_network_shares`, qui énumère ses partages disque en arrière-plan. Les partages continuent ensuite dans le parcours normal du système de fichiers. Les périphériques Shell sont présentés comme des éléments à ouvrir avec Windows. Le double-clic en liste ou grille et l’action « Ouvrir » passent par `open_external_item`, qui appelle `Platform::open_item` sur un worker ; ne jamais appeler le Shell directement dans un callback GPUI, car ses traitements de messages peuvent réentrer dans l’interface. Le worker initialise COM pour les périphériques Shell, comme les box dont l’action par défaut ouvre la page de gestion. Les erreurs d’énumération et d’ouverture sont remontées et les compteurs du voisinage reflètent les éléments reçus.

Les deux dialogues GPUI reprennent les fonctionnalités de Tauri et utilisent `Modal`, les thèmes et les catalogues de l’application. Le dialogue de connexion propose les lettres de Z à A, désactive les lettres occupées avec leur libellé, préremplit le partage si disponible, et fournit des champs dossier, utilisateur facultatif et mot de passe masqué ainsi que la reconnexion à l’ouverture de session (activée par défaut). Le dialogue de déconnexion charge les lecteurs distants et permet d’en sélectionner plusieurs ; le lecteur d’origine est présélectionné lorsqu’il est ouvert depuis sa propre entrée. Comme dans le dialogue Tauri, la validation effectue une déconnexion forcée des lecteurs sélectionnés.

Le titre permet de déplacer chaque dialogue. Tab/Maj+Tab déplacent le focus, Entrée valide, Échap ferme (ou ferme d’abord le sélecteur de lettre), et Espace/flèches permettent de manipuler les sélections au clavier. Le formulaire bloque les raccourcis de fichiers en arrière-plan. Pendant une opération, la validation et la fermeture sont désactivées. Les erreurs restent dans le dialogue, qui conserve la saisie pour une nouvelle tentative ; le succès ferme le dialogue et affiche une notification temporaire. Les identifiants restent en mémoire pour la durée du dialogue et ne sont pas persistés.

Les appels passent par `Platform::map_network_drive` et `Platform::disconnect_network_drive` sur un worker, avec une seule opération de lecteur à la fois. La déconnexion libère les observateurs de dossiers au préalable ; après l’opération, même partiellement réussie, les lecteurs sont actualisés et les onglets des lecteurs retirés sont redirigés. Une annulation avant validation ne lance aucune opération. Aucun lecteur occupé n’est remplacé silencieusement par le backend. Les callbacks de saisie synchronisent les valeurs dans l’état du dialogue : ne pas relire l’entité `TextInput` depuis son propre callback Entrée, car elle est alors déjà empruntée par GPUI.

- Garder les opérations de fichiers, les calculs de taille et les autres travaux coûteux hors du rendu synchrone. Utiliser les mécanismes asynchrones et de fond déjà présents.
- Avant de terminer une modification UI, lancer `cargo check --all-targets --offline` et résoudre les avertissements du projet.
- Exécuter les tests pertinents pour les catalogues, formats, préférences et comportements modifiés.
- Vérifier l'interface en français et en anglais, dans chaque thème. Inclure les libellés longs, les menus, les modales, les états vides et les erreurs, ainsi que les interactions clavier et souris.
- Une compilation réussie ne valide pas à elle seule le contraste, la disposition ou le comportement visuel.

## Références

- [GPUI](https://gpui.rs/)
- [API Rust Fluent](https://docs.rs/fluent-bundle/0.16.0/fluent_bundle/)
- [Sélecteurs et pluriels Fluent](https://projectfluent.org/fluent/guide/selectors.html)

### Archives natives

Le menu Compresser ouvre un formulaire de nommage avant de créer une archive ZIP, 7z, TAR ou TAR.ZST. Les archives s’ouvrent au double-clic ou avec Ouvrir comme des dossiers virtuels. Les chemins des entrées utilisent les séparateurs natifs sans perdre les niveaux de dossiers ni les racines UNC.

Les opérations de copie, déplacement, suppression, renommage et création de dossier dans les quatre formats créés passent par un espace de travail temporaire. La nouvelle archive est produite dans le dossier de l’original, puis publiée avec sauvegarde et restauration en cas d’échec ; une modification externe de l’original interrompt le remplacement. Un déplacement ne retire que les fichiers effectivement copiés, en conservant les conflits ignorés. Les liens contenus dans une archive ne sont pas suivis. Les autres formats navigables restent en lecture/extraction.

Les qualités persistées sont appliquées aussi aux reconstructions : ZIP 1/6/9, 7z LZMA2 1/6/9, Zstandard 1/3/19 (rapide/équilibrée/maximale). TAR est non compressé. Les phases de lecture et de reconstruction respectent pause et annulation. Les mutations d’archives ne produisent pas d’entrées Annuler/Rétablir de système de fichiers. L’ouverture externe d’un fichier extrait utilise une copie temporaire, sans réintégration automatique des éditions. Les onglets d’archive observent le dossier contenant l’archive et sont actualisés après les opérations.

### Images disque

Les ISO et IMG au format ISO9660 s’ouvrent comme des dossiers virtuels en lecture seule, avec copie vers l’extérieur et ouverture externe des fichiers extraits. Leur extraction respecte pause et annulation. Sous Windows, une image que le lecteur ISO9660 ne reconnaît pas est ouverte via son lecteur monté ; les VHD/VHDX passent directement par ce parcours. Ouvrir dans un nouvel onglet et le clic du milieu suivent le même comportement.

Le menu contextuel d’une image propose Monter ou Démonter selon l’état détecté. Un lecteur appartenant à une image montée propose également Démonter dans la barre latérale. Les appels Windows passent par `Platform` sur un worker, avec chemins transmis comme données à PowerShell et console masquée. La détection consulte les volumes et disques, car l’énumération de MSFT_DiskImage peut omettre des images montées. Avant le démontage, les observateurs de dossiers sont libérés ; les opérations de fichiers encore actives sur les volumes concernés empêchent ce démontage. Le succès actualise les lecteurs et redirige les onglets dont le lecteur a disparu. Linux conserve la navigation ISO9660 ; le montage Windows n’y est pas proposé.
Lors d’un montage effectué par Oxyde, un gestionnaire natif `QueryCancelAutoPlay` installé sur sa fenêtre répond TRUE tant que l’opération est active et pendant les 1,5 seconde qui suivent. Windows interroge la fenêtre au premier plan : cette inhibition dépend donc d’Oxyde restant actif. Le gestionnaire ne touche pas au registre ni aux préférences de lecture automatique et retrouve son comportement normal après ce court délai, y compris si le montage échoue. Il est retiré lors de la destruction de la fenêtre. La réponse native est testée ; l’absence d’ouverture de l’Explorateur reste à vérifier dans la session interactive.
