# Audit CORE-02

## Fichiers lus en entier

- [x] `src/commands/search/content/vm.rs`
- [x] `src/commands/search/criteria.rs`
- [x] `src/commands/sidebar.rs`
- [x] `src/commands/system.rs`
- [x] `src/lib.rs`
- [x] `src/main.rs`
- [x] `src/models/application.rs`
- [x] `src/models/config.rs`
- [x] `src/models/custom_theme.rs`
- [x] `src/models/error.rs`
- [x] `src/models/file_entry.rs`
- [x] `src/models/history.rs`
- [x] `src/models/mod.rs`
- [x] `src/models/permissions.rs`
- [x] `src/models/preferences.rs`
- [x] `src/models/transaction.rs`
- [x] `src/utils/audio_metadata.rs`
- [x] `src/utils/audio_metadata/tests.rs`

Sans constat : `vm.rs` (simulation Thompson cohérente : contexte de 12 octets suffisant, annulation vérifiée à chaque octet, compteurs bornés), `application.rs`, `lib.rs`, `mod.rs`, `permissions.rs` (précédences de `from_umask` et `shared` correctes), `audio_metadata/tests.rs`.

## Constats

### Moyen

1. `src/models/file_entry.rs:273` (aussi `file_entry.rs:285`, `sidebar.rs:31`, `sidebar.rs:105`) — moyen, confirmé. Sous Windows, `Metadata::is_dir()` renvoie faux pour une jonction ou un lien symbolique de dossier (vérifié par un essai avec `mklink /J` : `is_dir=false`, `is_symlink=true`). L'entrée est donc listée avec `is_dir` faux, `is_junction` ne peut jamais être vrai (condition `!is_symlink`), et la barre latérale ignore ces dossiers. Scénario : un dossier jonction ou lien symbolique dans un répertoire listé s'affiche comme un lien-fichier, un double clic passe par `open_item` au lieu de naviguer dans Oxyde, et il n'apparaît pas dans l'arbre.
2. `src/commands/sidebar.rs:68-72` — moyen, confirmé. `get_subtree_nodes` valide le chemin puis appelle `is_dir()` (appel système) dans le corps asynchrone, avant `spawn_blocking`. L'appelant (`ui/app/navigation.rs:136-139`) l'attend dans une tâche du thread GPUI, donc le stat s'exécute sur le thread UI. Scénario : « Tout développer » sur un lecteur réseau déconnecté ou un disque en veille gèle l'interface jusqu'au délai système.
3. `src/commands/sidebar.rs:81-148` (avec `platform/linux_impl.rs:529`, hors lot) — moyen, confirmé pour l'absence de garde-fou, plausible pour le déclenchement. Le parcours « tout développer » est récursif, sans plafond de nœuds ni de profondeur, sans annulation (la tâche bloquante ne peut pas être interrompue), et la liste d'exclusions Linux ne contient que deux noms Windows. Scénario : « Tout développer » sur `/` sous Linux parcourt `/proc`, `/sys`, `/run/user/*/gvfs` (FUSE réseau) et les montages ; la tâche tourne pendant des minutes et renvoie une table géante insérée d'un bloc dans l'état de la barre latérale.
4. `src/models/config.rs:286-294` (et `ui/app/preferences.rs:127,231,404`, hors lot) — moyen, confirmé. Un `themes.json` illisible fait échouer toute la lecture de la configuration, alors que `config.json` est valide : `main.rs` retombe sur la configuration par défaut avec `load_failed`, ce qui désactive toute sauvegarde de préférences et d'espace de travail pour la session. Scénario : l'utilisateur modifie à la main `themes.json` (documenté comme lisible), laisse une virgule en trop ; tous ses réglages et son espace de travail sont ignorés et plus rien n'est enregistré jusqu'à correction du fichier.

### Faible

5. `src/commands/sidebar.rs:35,50,90,126,142` — faible, confirmé. Les chemins des nœuds sont produits avec `to_string_lossy`, alors que le reste du code (`file_entry.rs:296`) utilise `path_text::encode`, réversible. Scénario (Linux) : un dossier dont le nom n'est pas UTF-8 reçoit un chemin avec U+FFFD ; cliquer le nœud échoue, et le parcours « tout développer » ne peut pas y entrer (`read_dir` échoue, ignoré silencieusement).
6. `src/commands/sidebar.rs:25-31` — faible, plausible. Sous Unix, `DirEntry::metadata` ne suit pas les liens : un lien symbolique vers un dossier est écarté de l'arbre, et `has_subdirs` (`file_type`, lignes 45 et 120) ignore de même les sous-liens. Scénario : `~/Documents` pointant vers un autre disque, ou un dossier ne contenant que des liens vers des dossiers, apparaît sans nœud ni flèche, alors que la liste de fichiers (`file_entry.rs:270`) suit ces liens.
7. `src/commands/sidebar.rs:63-79` (appliqué par `ui/app/navigation.rs:146-157`, hors lot) — faible, plausible. Le résultat du parcours est fusionné avec `extend` sans vérifier qu'il correspond toujours à l'état : `show_hidden` et `show_system` sont capturés au lancement et aucune génération ne distingue deux demandes. Scénario : l'utilisateur lance « tout développer », active l'affichage des fichiers cachés ou supprime un dossier pendant le parcours ; le résultat périmé écrase les nœuds plus récents fournis par les observateurs.
8. `src/commands/system.rs:9-19` — faible, plausible. `open_item` est `async` mais appelle `validate_path` et `platform().open_item` directement, sans `spawn_blocking` (sous Linux l'implémentation attend jusqu'à 3 s en dormant par pas de 20 ms). Aucun appelant actuel dans `src`, donc défaut latent : le premier appel depuis une tâche GPUI bloquerait l'interface. Les préfixes `::{` et `?` contournent aussi `validate_path`.
9. `src/commands/system.rs:33-53` — faible, plausible. La purge supprime `icons` puis `thumbnails` et s'arrête à la première erreur. Sous Linux il n'y a pas de pause d'écriture du cache (seul Windows a `pause_cache_writes`). Scénario : un travailleur de vignettes écrit pendant `remove_dir_all` (dossier non vide), la purge échoue sur `icons` et `thumbnails` reste intact alors que l'interface a déjà invalidé ses vignettes.
10. `src/models/config.rs:275-277` — faible, plausible. La configuration est lue par `read_to_string` puis `from_str` strict : un BOM UTF-8 (anciens Bloc-notes), un fichier UTF-16 (redirection PowerShell 5) ou un seul champ mal typé (rétrogradation de version) invalident tout le fichier, sans tolérance par champ, sans sauvegarde ni mise en quarantaine du fichier fautif. Scénario : l'édition manuelle avec BOM remet toutes les préférences à zéro pour la session et bloque l'enregistrement.
11. `src/models/config.rs:327-329` — faible, plausible. `persist` n'est ni retenté ni suivi d'un `fsync` du dossier parent, et les fichiers temporaires restent après un arrêt brutal. Scénario (Windows) : un antivirus ou un outil de sauvegarde ouvre `config.json` sans partage de suppression au moment de l'écriture ; l'enregistrement échoue (message d'erreur affiché), sans nouvelle tentative avant la prochaine modification.
12. `src/models/custom_theme.rs:47-50` — faible, plausible. `last + 1` déborde si `themes.json` contient un identifiant `theme-4294967295` : panique en debug, retour à `theme-0` en release (collision possible). Scénario : fichier édité à la main ou corrompu.
13. `src/models/error.rs:131-143,168-170,410-412,431-436,451,465-469,510-513` — faible, plausible. Les causes structurées sont retrouvées en cherchant des sous-chaînes dans des messages qui contiennent des noms de fichiers non fiables ; `split_once` prend la première occurrence. Scénario : un fichier ou une entrée d'archive nommé comme un marqueur (par exemple celui de mot de passe requis, ou « Archive recovery [archive-recovery]: … ») fausse la classification (invite de mot de passe, refus d'accès) ou, pour la récupération d'archive, fait échouer l'analyse JSON et perd les emplacements de récupération affichés à l'utilisateur.
14. `src/models/history.rs:142-148,170-183` (appel UI : `ui/app/rename.rs:471`, `file_chooser/create.rs:54`, hors lot) — faible, confirmé pour le chemin. `push` purge les entrées expirées (pile de rétablissement invalidée ou plus de 100 entrées) en appelant `cleanup_metadata_backups`, qui ouvre et hache chaque sauvegarde de métadonnées de corbeille, de façon synchrone sur le thread appelant. Scénario : créer un dossier depuis le thread UI alors que la pile de rétablissement contient des suppressions vers la corbeille déclenche ces E/S sur le thread UI.
15. `src/models/transaction.rs:146,152` — faible, plausible. `snapshot` et `refresh_from_file` utilisent `lock().unwrap()`, contrairement au reste de la gestion de l'historique qui tolère l'empoisonnement. Appelé aussi depuis le `Drop` de `HistoryManager` (`history.rs:150-163`) : si le verrou est empoisonné, la panique en cours d'arrêt devient un abandon du processus.
16. `src/models/preferences.rs:2-6` — faible, plausible. `MAX_SEARCH_LIMIT` vaut environ 4,29 milliards ; `search_limit` est transmis tel quel comme limite de résultats (`ui/app/search.rs:858,867`). Scénario : une valeur éditée à la main dans `config.json` supprime en pratique le plafond et la recherche accumule des millions d'entrées en mémoire sur un gros disque.
17. `src/main.rs:63-85` — faible, plausible. L'initialisation du journal, avec rotation au-delà de 2 Mo, s'exécute avant `claim_instance` : un lancement transmis à l'instance en cours renomme `oxyde.log` sous elle. Sous Linux, l'instance active continue d'écrire dans le fichier renommé ; sous Windows, `remove_file` du précédent journal réussit puis `rename` échoue silencieusement, l'ancien journal est perdu et le fichier dépasse 2 Mo. La limite de 32 Mo (`main.rs:37`) est aussi par processus.
18. `src/utils/path_security.rs:20-29` (hors lot, vu en suivant `validate_path` appelé par `sidebar.rs` et `system.rs`) — faible, plausible. `validate_path` retire les points et espaces finaux du dernier composant sous Windows, y compris pour un chemin `\\?\` où `foo.` et `foo` sont deux fichiers distincts. Scénario : une opération ciblant `\\?\C:\d\foo.` agit sur `foo`.

### Optimisation

19. `src/commands/sidebar.rs:42-46,117-121` — optimisation, confirmé. Pour chaque sous-dossier, `has_subdirs` ouvre et énumère le dossier jusqu'au premier sous-dossier : un dossier de 100 000 fichiers est lu en entier pour rien, et en mode « tout développer » chaque dossier est lu deux fois (contrôle du parent, puis dépilement). Scénario : développer un dossier dont les enfants sont de gros dossiers de photos.
20. `src/commands/sidebar.rs:58,133` — optimisation, confirmé. `sort_by_key` avec `to_lowercase` alloue une chaîne à chaque comparaison (n log n allocations) ; `sort_by_cached_key` en ferait n.
21. `src/commands/search/criteria.rs:14-20,37-44` — optimisation, confirmé. Pour chaque nom testé, la requête littérale est de nouveau normalisée, passée en minuscules, et le nom est converti en `String` jusqu'à deux fois. Scénario : une recherche sur des millions de fichiers refait plusieurs allocations par entrée pour un résultat identique ; la requête pourrait être préparée une fois dans le constructeur.
22. `src/models/config.rs:299-303,313-331` — optimisation, confirmé. Chaque sauvegarde de préférences réécrit `themes.json` (copie des thèmes comprise) puis `config.json`, avec deux `sync_all`, même quand les thèmes n'ont pas changé. Scénario : chaque changement d'onglet ou de largeur de volet (état d'espace de travail) coûte deux écritures synchronisées.
23. `src/models/file_entry.rs:270,293` — optimisation, confirmé. Sous Unix, chaque lien symbolique listé subit deux `fs::metadata` identiques (cible, puis `FileStamp::for_entry`) ; les deux résultats peuvent aussi diverger si la cible change entre les appels. Scénario : un dossier rempli de liens (par exemple un dépôt de paquets) double les appels système de la liste.
24. `src/utils/audio_metadata.rs:38-44` (réponse lue dans `platform/mod.rs:896`, hors lot) — optimisation, plausible. Les pixels de la pochette (jusqu'à 1 Mio) traversent le worker en JSON sous forme de tableau de nombres, soit environ 3 à 4 Mo de texte à analyser par fichier audio. Scénario : parcourir des fichiers avec pochette 512 x 512 ; un encodage binaire éviterait cette analyse.

<!-- LOT TERMINÉ -->
