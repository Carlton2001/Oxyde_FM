# Comparaison des moteurs de copie

Cet outil expérimental est indépendant des opérations normales d'Oxyde et de leur
historique. Il teste les cinq modes sur un fichier ou un dossier existant :

| CSV `mode` | Transfert | BLAKE3 |
|---|---|---|
| `native_blake3` | API Windows actuelle via `Platform` | Relecture de la destination |
| `native_plain` | Même API Windows | Aucun |
| `stream_blake3` | Boucle Rust, blocs de 1 Mio | Calcul pendant le transfert |
| `stream_plain` | Même boucle Rust | Aucun |
| `native_source_blake3` | API Windows, avec un lecteur source en parallèle | Empreinte de la source |

Le moteur Rust reprend le principe de l'ancien backend Tauri, sans ses pauses
artificielles, réglages de priorité ni parallélisme. Les cinq modes traitent les
fichiers séquentiellement avec la même priorité. Il s'agit d'un benchmark des
données ordinaires : le moteur Rust ne reproduit pas les dates, attributs, flux
alternatifs et toutes les garanties du moteur de production. Les liens et points
de réanalyse sont refusés. Ce moteur n'est pas un remplacement prêt à déployer.

## Utilisation sous Windows

Fermer Oxyde avant de compiler si ses exécutables sont utilisés.

```powershell
cargo build --offline --release --bin copy_benchmark
.\target\release\copy_benchmark.exe --help
.\target\release\copy_benchmark.exe "C:\donnees\gros-fichier.bin" "E:\essais" 5 > copie-gros.csv
.\target\release\copy_benchmark.exe "C:\donnees\petits-fichiers" "E:\essais" 5 > copie-petits.csv
```

La destination doit être un dossier existant, extérieur à la source. Chaque essai
crée un dossier temporaire distinct dans la destination et le supprime après la
mesure. Les fichiers déjà présents ne sont pas écrasés. Prévoir assez d'espace pour
une copie complète du jeu de données. Une interruption brutale du processus peut
laisser un dossier `oxyde-copy-benchmark-*` à nettoyer manuellement.

Effectuer d'abord un essai de correction, sur un petit échantillon :

```powershell
.\target\release\copy_benchmark.exe "C:\donnees\echantillon" "E:\essais" 1 --validate
```

`--validate` calcule une référence sur la source avant les essais et compare après
chaque passe le contenu de la source, de la destination et l'empreinte éventuelle
du moteur. Ces lectures sont hors chronométrage mais réchauffent le cache. Ne pas
modifier la source pendant les essais. Sans cette option, seules les tailles sont
contrôlées ; les modes `plain` ne calculent aucune empreinte.

## Lecture des résultats

- `total_s` : temps réel de la passe, création des sous-dossiers, préparation,
  transfert, hachage, vidage, publication et nettoyage du staging inclus. La
  validation indépendante et la suppression finale des copies sont exclues.
- `transfer_s` : durée cumulée du transfert ; contient le vidage natif Windows
  dans les modes `native`, et le calcul BLAKE3 en ligne dans `stream_blake3`.
- `stream_flush_s` : vidage explicite `sync_all` du moteur Rust. Zéro pour le
  moteur natif ne signifie pas absence de vidage : celui-ci est dans `transfer_s`.
- `post_hash_s` : relecture BLAKE3 après copie, uniquement pour `native_blake3`.
- `publish_s` : renommage des fichiers temporaires vers leur nom final.
- `mib_s` : volume des données ordinaires divisé par le temps total.
- `source_hash_s` : durée du calcul sur la source, uniquement pour
  `native_source_blake3`. Cette durée chevauche la copie native : ne pas
  l'additionner à `transfer_s`.
- `hash_wait_s` : attente restante du lecteur source après la copie native,
  avant la publication. Inclut le coût de jonction du thread ; peut être presque
  nul si l'empreinte est déjà prête. Le temps total attend toujours les deux tâches.

Le mode natif réutilise le callback de production qui vide chaque flux terminé.
L'empreinte native et l'empreinte en ligne ne réalisent pas le même contrôle : la
première relit la destination, la seconde représente les blocs transférés.
Le cinquième mode utilise un lecteur indépendant sur la source et conserve le
verrou de lecture Windows pendant le transfert, le hachage et la publication.
Les identités des deux handles source sont comparées avant le lancement. Une
erreur de copie ou de hachage empêche la publication ; le thread de hachage est
rejoint avant le nettoyage, y compris en cas d'erreur de copie.

Ce mode effectue deux lectures de la source : il peut concurrencer le transfert
ou bénéficier du cache. Il ne relit pas la destination pour établir son empreinte.
Utiliser `--validate` sur un échantillon pour contrôler indépendamment le résultat,
puis comparer les durées sans cette option sur D: vers E: et D: vers D:.

L'ordre des cinq modes tourne à chaque répétition (cinq répétitions équilibrent
les positions). Le cache Windows n'est pas purgé. Comparer plusieurs passages,
examiner leur dispersion et tester séparément gros fichiers, petits fichiers et
chaque combinaison de disques. Fermer les autres tâches d'I/O. Les chiffres ne
mesurent pas l'interface, les conflits, l'historique ou une copie concurrente.
