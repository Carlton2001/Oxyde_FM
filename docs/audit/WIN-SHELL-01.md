# WIN-SHELL-01

Fichiers lus en entier :

- [x] src/platform/windows_com.rs
- [x] src/platform/windows_executables.rs
- [x] src/platform/windows_explorer_browse.rs
- [x] src/platform/windows_file_dialog.rs
- [x] src/platform/windows_file_dialog/registration.rs
- [x] src/platform/windows_file_dialog/relay.rs
- [x] src/platform/windows_fixed_dialog.rs
- [x] src/platform/windows_folder_handler.rs
- [x] src/platform/windows_folder_handler/direct_launch.rs
- [x] src/platform/windows_folder_handler/menu.rs
- [x] src/platform/windows_folder_launcher.rs
- [x] src/platform/windows_hover_window.rs
- [x] src/platform/windows_hover_window/keyboard.rs
- [x] src/platform/windows_hover_window/pointer.rs
- [x] src/platform/windows_instance.rs
- [x] src/platform/windows_jump_list.rs
- [x] src/platform/windows_peek.rs
- [x] src/platform/windows_pointer.rs
- [x] src/platform/windows_process_launch.rs

Résumé : 1 élevé, 2 moyens, 11 faibles, 1 optimisation. Aucun appel de ce lot n'ouvre de fichier avec des drapeaux de reparse : la classe de bug 4395 n'apparaît pas ici.

## Constats

### src/platform/windows_file_dialog/registration.rs:23 (élevé, plausible)
L'inscription écrit la DLL x64 sous `HKCU\Software\Classes\CLSID` sans tenir compte de l'architecture. Or l'installeur accepte `x64compatible` (`packaging/windows/oxyde.iss:19`), ce qui inclut Windows 11 ARM64.
Scénario : sur un PC ARM64, l'utilisateur active l'option. Les applications ARM64 natives (Bloc-notes, Paint, Office ARM64) lisent la même vue 64 bits du registre et tentent de charger en processus une DLL x64. CoCreateInstance échoue et ces applications n'ont plus aucune boîte Ouvrir ni Enregistrer, pas même celle du système.

### src/platform/windows_explorer_browse.rs:46 (moyen, plausible)
Le lanceur fait des appels COM inter-processus sans délai maximal vers chaque fenêtre de l'Explorateur. Cela concerne Count et Item, puis QueryActiveShellView et GetItemObject (ligne 86), et BindToHandler sur les raccourcis sélectionnés (ligne 78).
Scénario : un onglet de l'Explorateur énumère un partage réseau hors ligne, ou la sélection contient un .lnk vers un tel partage. Le thread de cette fenêtre est occupé et le lanceur bloque sans limite. Le dossier double-cliqué ne s'ouvre alors ni dans l'Explorateur ni dans Oxyde.

### src/platform/windows_instance.rs:189 (moyen, plausible)
Le thread unique qui sert le pipe d'instance fait des I/O disque de façon synchrone avant de reprendre l'écoute. Il appelle `metadata` via `windows_startup::folders`/`items`, ainsi que `is_dir` sur le dossier, les places et le dossier par défaut d'une requête de dialogue (`windows_file_dialog.rs:259`, `:265-266`).
Scénario : un chemin est sur un partage SMB injoignable, ce qui bloque le thread plusieurs dizaines de secondes. Pendant ce temps, les lancements suivants reçoivent ERROR_PIPE_BUSY. Après 5 s de patience, le lanceur démarre un nouvel Oxyde, qui échoue à son tour à transmettre puis finit en `Alone` : une seconde instance complète s'ouvre. Pour les dialogues, chaque tentative peut durer 5 s par pipe et dépasser la fenêtre de 15 s du relais.

### src/platform/windows_instance.rs:161 (faible, plausible)
`receive` traite comme complet un message tronqué : soit la limite de 256 Kio est dépassée, soit l'émetteur s'interrompt.
Scénario : une application montre des milliers de fichiers dans un dossier (SELECT au-delà de 256 Kio). Oxyde sélectionne une liste partielle, dont le dernier chemin est coupé. L'écriture de l'émetteur échoue, donc `select` dans le lanceur renvoie le message toutes les 200 ms pendant 20 s, et chaque envoi produit une nouvelle sélection partielle.

### src/platform/windows_instance.rs:193 (faible, plausible)
Le drapeau `RELEASED` n'est relu qu'en tête de boucle. Un message déjà en cours de lecture au moment de `release` est donc transmis à une application qui se ferme.
Scénario : un dossier est ouvert depuis l'Explorateur pendant les dernières sauvegardes de la fermeture. L'écriture du lanceur réussit, il se termine, et le dossier n'est affiché nulle part.

### src/platform/windows_file_dialog.rs:212 (faible, plausible)
Si `OpenProcess` échoue sur le lanceur, ou si son PID est inconnu (ligne 244), la requête n'est jamais retirée.
Scénario : le thread du pipe est resté bloqué plus de 15 s (constat précédent). Le lanceur est sorti avec le code d'indisponibilité et l'application a montré le dialogue système. La fenêtre de choix d'Oxyde reste pourtant ouverte pour une application qui n'attend plus rien. Un PID réutilisé peut aussi faire attendre le mauvais processus.

### src/platform/windows_file_dialog.rs:143 (faible, confirmé)
La réponse omet en silence les chemins non convertibles en UTF-8 (surrogates UTF-16 isolés, permis par NTFS) et ceux qui contiennent CR ou LF.
Scénario : en sélection multiple, l'un des fichiers a un nom avec un surrogate isolé. L'application reçoit « ok » avec une liste incomplète, ou un échec si c'était le seul fichier, sans que l'utilisateur en soit averti.

### src/platform/windows_explorer_browse.rs:137 (faible, plausible)
Le dossier est passé à explorer.exe avec les guillemets de la bibliothèque standard Rust, qui ne cite que les arguments contenant une espace ou une tabulation. Explorer, lui, découpe sa ligne de commande sur les virgules.
Scénario : depuis une sélection multiple, l'utilisateur ouvre `D:\Photos,2024`. La nouvelle fenêtre de l'Explorateur ouvre un autre emplacement (`D:\Photos` ou le dossier par défaut) au lieu du dossier demandé.

### src/platform/windows_explorer_browse.rs:136 (faible, confirmé)
Le chemin d'explorer.exe vient de la variable d'environnement `SystemRoot`, ce qui contourne `windows_executables`, dont la raison d'être est justement d'ignorer les racines fournies par l'environnement.
Scénario : un processus parent lance le lanceur avec un `SystemRoot` modifié. Le lanceur exécute alors l'`explorer.exe` de ce dossier, ou échoue si la variable est absente.

### src/platform/windows_jump_list.rs:98 (faible, plausible)
Chaque changement lance `set` dans un `spawn_blocking` détaché (`src/ui/app/system_menus.rs:26`), sans sérialisation ni contrôle de révision. Deux écritures BeginList/CommitList, ainsi que les écritures de registre de `refresh_menu`, peuvent donc se croiser.
Scénario : l'utilisateur épingle puis désépingle deux dossiers rapidement. La tâche la plus ancienne valide en dernier, et le menu de la barre des tâches affiche l'état périmé jusqu'au changement suivant.

### src/platform/windows_jump_list.rs:83 (faible, plausible)
Les arguments des raccourcis retirés sont lus dans un tampon de 1024 unités, donc tronqués au-delà. Par ailleurs, tout échec d'AppendCategory est interprété comme « éléments récents désactivés ».
Scénario : l'utilisateur retire du menu un dossier épinglé dont le chemin long dépasse environ 1020 caractères. Il n'est plus reconnu comme retiré et il est ré-ajouté. Windows refuse alors toute la catégorie, et tous les dossiers épinglés passent en tâches.

### src/platform/windows_jump_list.rs:139 (faible, plausible)
ShellExecuteW est appelé sur le thread principal sans initialisation COM préalable (`src/main.rs:149`), alors que la documentation l'exige pour ShellExecute. Le code d'erreur est ensuite lu par GetLastError plutôt que tiré de la valeur de retour SE_ERR.
Scénario : avec certaines associations ou extensions du Shell, la tâche « Exécuter en tant qu'administrateur » échoue, ou le journal affiche une erreur sans rapport avec la cause.

### src/platform/windows_hover_window/pointer.rs:27 (faible, plausible)
Le relâchement mémorisé après un appui qui a seulement fermé un menu n'est effacé que lorsqu'un relâchement arrive sur la même fenêtre.
Scénario : l'utilisateur clique hors d'un menu interactif pour le fermer, puis glisse hors de la fenêtre avant de relâcher. Sans capture, le WM_LBUTTONUP part ailleurs. Au clic normal suivant sur le formulaire, le relâchement est avalé (`windows_hover_window.rs:141`) : GPUI voit l'appui sans le relâchement et l'action déclenchée au relâchement est perdue.

### src/platform/windows_hover_window.rs:211 (faible, plausible)
GetDpiForWindow renvoie 0 pour une fenêtre détruite, ce qui donne une échelle nulle et une division par zéro. `work_area_size` renvoie alors des dimensions infinies.
Scénario : une mise en page de menu est demandée juste après la destruction de sa fenêtre native. GPUI reçoit une zone de travail infinie, et la hauteur des menus n'est plus bornée.

### src/platform/windows_peek.rs:22 (optimisation, confirmé)
`peek_status` refait à chaque appel jusqu'à neuf sondages `is_file` et relit deux fichiers JSON. La boucle de `src/ui/app/shell_features.rs:202` l'appelle toutes les 5 s, et `open_peek` refait le même travail et appelle `executable` une seconde fois.
Scénario : en usage normal, ces I/O disque redondantes se répètent toutes les 5 s pendant toute la session. Un contrôle par date de modification, ou un cache invalidé par la surveillance du dossier de réglages, les supprimerait.

<!-- LOT TERMINÉ -->
