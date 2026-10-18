# Fermeture bloquée après un aperçu PDF

Référence : `SHUTDOWN-PDF-INTEL-2026-10-04`. Le contournement Windows a été validé manuellement par l’utilisateur le 4 octobre 2026. Une ancienne mention du blocage ne prouve pas une régression actuelle.

## Contrat à préserver

1. Fermer les fenêtres et terminer le nettoyage applicatif : historique, temporaires et dernières préférences, y compris le debounce de géométrie.
2. Sortir de GPUI puis arrêter Tokio de façon bornée et vider les logs.
3. Appeler `Platform::finish_process_shutdown` à la toute fin.
4. Sous Windows, arrêter les superviseurs image puis icônes, puis la file de persistance des icônes (attentes bornées, deux secondes chacun), puis `TerminateProcess(GetCurrentProcess(), 0)`. La file abandonne les demandes en attente ; l'écriture active peut terminer pendant ce délai. Linux retourne normalement.

`TerminateProcess` évite les callbacks finaux de détachement des DLL ; `std::process::exit` n’est pas un remplacement équivalent. Ce contournement ne remplace aucune sauvegarde ni hook de fermeture. Les workers sans état applicatif sérialisent et vident leur réponse avant leur sortie finale.

Points d’entrée : `src/main.rs`, `platform/windows_impl.rs`, `platform/linux_impl.rs`, `platform/windows_preview_worker/`. Voir les [sessions d’aperçus](../architecture/previews.md).

## Si le problème revient

1. Lire la fin d’`oxyde.log` à côté de `config.json` (`%APPDATA%\oxyde` habituellement, `data` en portable) et relever le PID de session.
2. Chercher `application cleanup complete; final process exit`. Si cette étape manque, enquêter à la dernière étape atteinte : le contournement final ne traite pas un blocage antérieur.
3. Contrôler aussi WMI et `tasklist`. L’absence dans `Get-Process` ne suffit pas à conclure à la fin du processus.
4. Capturer si possible un dump avant de tuer le processus, puis vérifier le binaire et reproduire avec plusieurs aperçus PDF et fermetures.

## Preuve historique et limites

La fenêtre avait disparu et le journal atteignait `main returning`, mais un thread continuait dans le pilote Intel `igd10um64xe.DLL` (offset `0x71398`). La pile contenait aussi D3D11 et `Windows.Data.Pdf.dll`. Cela localise le blocage dans la sortie native finale ; sans pile symbolisée complète, le défaut précis du pilote n’est pas établi.

Le dump `target/shutdown-diagnostics/oxyde-13532.dmp` n’est pas versionné et peut avoir disparu. Le test `shutdown_returns_while_a_blocking_worker_is_still_running` vérifie l’arrêt borné de Tokio, pas le défaut PDF/Intel. La preuve du contournement repose sur la reproduction et le retour manuel utilisateur.
