# Blocage à la fermeture après des aperçus PDF sous Windows

Référence : `SHUTDOWN-PDF-INTEL-2026-10-04`.
Résolution validée manuellement par l’utilisateur le 4 octobre 2026, après de nombreuses ouvertures d’aperçus PDF suivies d’une fermeture correcte.

## Symptôme et preuves

La fenêtre disparaissait, mais `oxyde.exe` restait dans le gestionnaire des tâches avec environ 3 % de CPU. Le journal atteignait pourtant `main returning` : le nettoyage de l’historique, les sauvegardes des préférences, la boucle GPUI et l’arrêt de Tokio avaient tous terminé.

Lors de la reproduction, le processus PID 13532 ne conservait qu’un thread, ID 26944. `Get-Process` ne le trouvait plus, tandis que WMI (`Win32_Process`, `Win32_Thread`) et `tasklist` le voyaient encore. L’absence du PID dans `Get-Process` ne prouve donc pas que le processus est terminé.

Un minidump a été capturé avant que l’utilisateur tue le processus. Le registre RIP du thread restant pointait dans le pilote graphique Intel `igd10um64xe.DLL`, à l’offset `0x71398`. La mémoire de pile contenait des adresses de ce pilote, de `d3d11.dll`, de `Windows.Data.Pdf.dll`, du runtime C et de `ntdll.dll`.

Ces indices situent le blocage dans le nettoyage natif final du processus, impliquant le moteur PDF Windows et le pilote Intel. Les adresses relevées sur la pile ne constituent pas une pile entièrement déroulée avec symboles ; elles ne permettent pas d’identifier précisément le défaut interne du pilote.

Le dump local est conservé dans `target/shutdown-diagnostics/oxyde-13532.dmp`. Ce dossier est ignoré par Git : le dump peut disparaître après un nettoyage de `target`. Cette note conserve les constats essentiels.

## Contournement appliqué

- `src/main.rs` termine d’abord la boucle GPUI, quitte le contexte Tokio, appelle `shutdown_runtime`, puis vide les logs avant la sortie finale.
- `Platform::finish_process_shutdown` centralise cette dernière étape.
- Sous Windows, `src/platform/windows_impl.rs` appelle `TerminateProcess(GetCurrentProcess(), 0)` pour éviter les callbacks de détachement des DLL qui pouvaient boucler.
- Sous Linux, `src/platform/linux_impl.rs` retourne normalement.

Ce contournement doit rester à la toute fin, après le nettoyage applicatif. Il ne remplace ni les sauvegardes, ni le nettoyage des fichiers temporaires, ni les hooks de fermeture. `std::process::exit` ne constitue pas un remplacement équivalent sous Windows : la sortie normale passe encore par le détachement des DLL.

Documentation : [TerminateProcess — Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess).

La correction antérieure `Runtime::shutdown_timeout(2 secondes)` évite l’attente indéfinie des workers Tokio bloquants, mais n’avait pas résolu ce blocage ultérieur dans les DLL.

## Journal et procédure en cas de récidive

Le journal persistant est `%APPDATA%\oxyde\oxyde.log`, avec rotation au prochain démarrage au-delà de 2 Mio vers `oxyde.previous.log`. Chaque session indique son PID. Les traces couvrent la demande de fermeture, le verrou et le nettoyage de l’historique, les générations de sauvegarde, le retour de GPUI et l’arrêt de Tokio. Un thread indépendant signale toutes les trois secondes la dernière étape tant que le nettoyage applicatif n’est pas terminé.

1. Lire la fin du journal et vérifier le PID de la session concernée.
2. Vérifier si `application cleanup complete; final process exit` a été atteint. Si la fermeture bloque avant, utiliser la dernière étape pour enquêter ; le contournement final ne traite pas ce cas.
3. Vérifier le processus avec WMI ou `tasklist`, même si `Get-Process` ne le trouve pas.
4. Si possible, capturer un dump avant de tuer le processus et comparer le module du thread actif avec les constats ci-dessus.
5. Vérifier que l’exécutable testé contient bien `finish_process_shutdown`, puis refaire des aperçus PDF et plusieurs fermetures.

## Validation réalisée

`cargo check`, `cargo build` et le test `shutdown_returns_while_a_blocking_worker_is_still_running` passent. Ce test vérifie l’arrêt borné de Tokio ; la validation du contournement PDF/Intel repose sur la reproduction puis sur les essais manuels de l’utilisateur.

## Aperçus séparés — 9 octobre 2026

Le mode interne `--oxyde-preview-worker-v1` produit un seul aperçu dans un processus sans interface. Il est traité avant logging, préférences et boucle GPUI. Après sérialisation et flush des pixels/du texte, il appelle la même sortie native finale ; ce worker n’a pas d’état applicatif à sauvegarder. Le parent conserve un délai et un Job Object pour terminer/récupérer un worker bloqué. Les étapes et hooks de fermeture de l’application principale restent inchangés. Le test d’intégration `tests/preview_worker.rs` exerce un véritable rendu PDF d’une page et vérifie la libération de son fichier après la réponse. Cela ne remplace pas la validation visuelle des aperçus et de la fermeture de la fenêtre principale.


## Géométrie persistante — 10 octobre 2026

Le hook GPUI de la fenêtre principale force maintenant la sauvegarde de la dernière géométrie lorsqu’un debounce de déplacement/redimensionnement est en attente, avant de prendre et attendre la queue des préférences. Le snapshot en mémoire est déjà à jour ; la minimisation est ignorée. Aucune écriture bloquante sur le thread UI et aucun changement du contournement final TerminateProcess. Le retour utilisateur du 10 octobre 2026 (« tout fonctionne ») confirme globalement le lot 58, dont la persistance de géométrie après fermeture/réouverture. Ce retour ne constitue pas une nouvelle reproduction dédiée du blocage PDF/Intel.

## Session image réutilisable — 14 octobre 2026

Avant le `TerminateProcess` final existant, `finish_process_shutdown` demande
l’arrêt du superviseur de session image s’il a été démarré. Le signal est contrôlé
dans les lectures et checkpoints de lancement ; le nettoyage attend au maximum
deux secondes. Les arrêts des workers conservent leur Job Object et leur attente
bornée. Les étapes GPUI/Tokio, les sauvegardes et le contournement PDF/Intel restent
inchangés. Voir [IMAGE_PREVIEW_SESSIONS.md](IMAGE_PREVIEW_SESSIONS.md) pour les limites
de cycle de vie, de protocole et de sécurité.

## Session des icônes Windows — 14 octobre 2026

`finish_process_shutdown` arrête aussi le superviseur de session Shell des icônes, après la session image et avant le TerminateProcess final. Le signal est contrôlé dans les checkpoints et lectures de la session ; le nettoyage attend au maximum deux secondes. Le processus reste attaché au Job Object existant, avec destruction et attente bornée. La session ne traite ni les PDF ni les aperçus documentaires.
