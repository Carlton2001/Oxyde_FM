# Mesure des copies réelles

Les copies normales d'Oxyde écrivent désormais leurs mesures dans `oxyde.log`, à
côté de `config.json` (habituellement `%APPDATA%\oxyde\oxyde.log` sous Windows ;
`data\oxyde.log` à côté de l'exécutable en mode portable). Le chemin exact figure
dans la ligne `session start`. Aucun réglage ni variable d'environnement n'est
nécessaire avec la configuration de journalisation habituelle.

## Recompiler et mesurer

Fermer Oxyde, puis reconstruire les binaires, car le code partagé de la copie et
de la couche Platform est modifié :

```powershell
cargo build --offline --bins
cargo run --offline
```

Effectuer une copie depuis l'interface, attendre son achèvement, puis consulter :

```powershell
Get-Content "$env:APPDATA\oxyde\oxyde.log" | Select-String 'copy_timing'
```

Le benchmark séparé ne fournit pas ces diagnostics de l'application : il continue
de produire son propre CSV. Pour comparer les performances de l'application,
utiliser les mêmes fichiers et la même destination, sans pause, et faire plusieurs
essais en alternant normal et turbo. Indiquer les supports utilisés et conserver
les journaux des essais. Le cache Windows reste actif.

## Lignes du journal

Toutes les lignes utilisent la cible `oxyde_lib::copy_timing` et des durées en
secondes, avec un point décimal.

- `copy_operation_start` : identifiant, copie/déplacement, turbo initial.
- `copy_source_selection` : nombre de racines sélectionnées et durée de leur
  capture et du contrôle des chevauchements. L'index des chemins évite désormais
  de comparer chaque racine à toutes les précédentes. Cette optimisation concerne
  surtout une sélection de milliers de fichiers ; sélectionner un seul dossier
  contenant ces fichiers ne présente pas ce coût quadratique.
- `copy_plan_traversal` : durée du parcours de ces racines et de la construction
  du plan avec les conflits, avant les transferts. Ces deux durées incluent les
  attentes éventuelles de pause pendant la préparation ; les premières opérations
  de préparation de la destination et certaines étapes intermédiaires restent
  hors de ces deux mesures.
- `copy_plan` : nombre de fichiers nécessitant un transfert, volume prévu,
  workers disponibles et turbo au lancement. Le mode normal active un worker ;
  turbo peut activer les autres lorsque le plan en a créé plusieurs. Ce nombre
  est une capacité, pas une mesure du nombre de workers occupés à un instant donné.
- `operation_turbo` : changement demandé pendant une opération.
- `copy_file` : résultat de chaque fichier, chemins source/destination,
  identifiant de l'opération et index du worker. Les copies hors worker principal
  (certaines opérations d'historique, par exemple) portent `op=unattributed`.
- `copy_prepare` : préparation préalable du worker, avec les mêmes identifiant,
  index et chemin source que `copy_file`. `destination_parent_s` mesure l'obtention
  des parents destination, la création des dossiers manquants et les contrôles
  associés ; `source_open_s` mesure l'ouverture et la préparation de la source.
  Ces deux durées précèdent `wall_s` du fichier ; elles incluent les éventuelles
  attentes de pause à ce stade. Une erreur pendant cette préparation peut empêcher
  l'émission de cette ligne ; le statut final de l'opération reste journalisé.
- `copy_operation_end` : statut final, compteurs, turbo final et durée réelle
  depuis la prise en charge de l'opération jusqu'à l'enregistrement de l'historique.

### Mesures par fichier

| Champ | Signification |
|---|---|
| `published` | Le renommage vers la destination finale a réussi |
| `bytes` | Taille de la copie terminée avant hachage ; peut rester zéro en cas d'échec antérieur |
| `wall_s` | Temps écoulé dans le parcours de copie du fichier, pauses incluses |
| `pause_s` | Temps d'attente de pause observé dans les callbacks de ce fichier |
| `active_s` | `wall_s` moins `pause_s` |
| `native_s` | Appel de copie Platform, vidage inclus, pauses observées exclues |
| `flush_s` | Temps cumulé des appels explicites de vidage, y compris le dernier appel en échec |
| `flush_calls` | Nombre d'appels explicites de vidage |
| `hash_s` | Relecture et calcul BLAKE3, pauses observées exclues |
| `publish_s` | Renommage final, y compris une tentative en échec |
| `parents_s` | Ouverture et résolution des parents source dans le parcours de copie |
| `staging_create_s` | Création du dossier temporaire privé et de son fichier de verrouillage |
| `writer_release_s` | Capture de l'identité finale et fermeture du handle d'écriture conservé |
| `output_open_s` | Réouverture du fichier copié et vérification de son identité |
| `checks_s` | Contrôles avant copie et publication, dont résolution du remplacement |
| `cleanup_s` | Nettoyage du staging dans son destructeur, sur succès comme sur erreur |
| `remaining_s` | Reste de `active_s` après soustraction des phases détaillées, sans compter `flush_s` une seconde fois |

**`flush_s` est inclus dans `native_s` : ne pas les additionner.** Sur Windows,
il mesure `FlushFileBuffers` dans le callback de copie. Sous Linux, il mesure le
`sync_all` explicite. `native_s - flush_s` reste une durée de copie hors ces appels,
pas une mesure pure du débit matériel : le système peut attendre des écritures à
d'autres endroits. `hash_s` mesure une lecture de la destination potentiellement
servie par le cache, pas une preuve de relecture physique du support.

Les nouveaux champs détaillent les étapes de préparation, contrôle et nettoyage.
Le chronomètre par fichier commence dans `file_copy`, après certaines
préparations du worker, et finit lors de la sortie de ce parcours. Il ne comprend
pas tout l'enregistrement ultérieur des effets du fichier dans l'historique.
`remaining_s` comprend notamment les métadonnées hors phase et la destruction des
handles hors des blocs instrumentés. `cleanup_s` mesure le corps du destructeur du
staging ; la fermeture automatique de ses champs après ce corps reste dans le
reste non attribué. Les ouvertures internes à l'API de copie restent dans `native_s`.

Les nouveaux champs sont ajoutés aux lignes existantes : les anciens journaux ne
les contiennent pas. Pour un premier diagnostic, refaire un seul lot de petits
fichiers et un seul lot de vidéos depuis Oxyde ; inutile de relancer les 75 essais
du benchmark. Cette instrumentation concerne les copies réelles de l'application,
pas les colonnes du CSV du benchmark.

Le total d'opération inclut les pauses, la planification et l'attente des workers ;
les pauses entre fichiers ne figurent pas dans les pauses par fichier. Avec plusieurs
workers, leurs durées se chevauchent : ne pas additionner les temps des fichiers
pour reconstruire la durée réelle de l'opération. Un déplacement sur le même volume
peut ne produire aucune ligne `copy_file`, puisqu'il ne transfère pas les données.

Les chronomètres s'arrêtent aussi sur les retours d'erreur ; `published=false`
signale que le fichier n'a pas été publié. En cas d'arrêt brutal du processus,
les récapitulatifs correspondants peuvent manquer. Les journaux contiennent les
chemins des fichiers : les retirer avant un partage public si nécessaire.
