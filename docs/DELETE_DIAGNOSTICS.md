# Suppressions de lots sous Windows

Les suppressions définitives n'ajoutent plus une attente artificielle de 5 ms par
élément en mode normal. La mise à la corbeille n'ajoute plus celle de 25 ms. Le mode
normal conserve la priorité d'arrière-plan ; la suppression définitive conserve
également sa limite d'un worker actif en mode normal.

Les nouvelles suppressions, copies et opérations d'archive reprennent le réglage
turbo des paramètres, pour l'affichage comme pour le flag lu par les workers.
Auparavant, la création des suppressions et des opérations d'archive omettait ce
réglage : elles démarraient en mode normal même si le turbo par défaut était activé.

## Corbeille

Les fichiers ordinaires dont l'identité a été capturée sont regroupés par lots
d'au plus 256 fichiers par défaut dans une opération Shell Windows, plutôt que de créer un
thread STA et une opération COM pour chaque fichier. Chaque élément conserve son
propre callback, son contrôle d'identité avant suppression et son reçu de résultat.
Les résultats de tous les éléments du lot sont enregistrés avant de remonter une
erreur ou de terminer sur annulation. L'affichage du nombre traité se met à jour
au retour du lot.

Les dossiers et les éléments spéciaux conservent le parcours individuel qui suit
les effets partiels sur les descendants. Les lots sont séparés lorsque la politique
Windows de corbeille du volume diffère : le réglage de suppression immédiate reste
respecté. Une suppression de dossier n'est pas comptée comme réussie uniquement
parce que certains de ses enfants ont été supprimés.

La pause et l'annulation sont vérifiées avant les lots et dans les callbacks Shell
avant suppression et de progression. Une annulation libère aussi une attente de
pause. La réponse reste dépendante du retour de Windows dans ces callbacks : un
appel système déjà bloqué n'est pas interrompu de force. Le changement de priorité
des lots suit le réglage turbo au lancement de chaque lot.

## Mesures et validation

Dans `oxyde.log`, la cible `oxyde_lib::delete_timing` écrit :

- `delete_operation_start` et `delete_operation_end` : type, statut, durée totale,
  compteurs et nombre d'entrées d'historique ;
- `trash_selection` : nombre de racines retenues et temps de préparation.
- `operation_cancel_requested` et `operation_cancel_accepted` : demande reçue
  par le gestionnaire et délai d'obtention des verrous avant activation du flag.
  La différence avec `delete_operation_end` permet ensuite d'estimer le délai de
  fin d'opération. Ces événements ne mesurent pas une attente préalable dans la
  boucle d'interface avant que le gestionnaire reçoive la demande.
- `trash_native_batch` : taille du lot natif, premier chemin du lot, résultats et
  durées `setup_s` (initialisation COM et mise en file), `native_s` (exécution
  Windows), `proof_s` (contrôles d'identité dans les callbacks) et `remaining_s`
  (finition native et construction des résultats). **`proof_s` est inclus dans
  `native_s` : ne pas les additionner.** La durée `wall_s` commence dans le thread
  natif ; elle exclut sa création, le choix de politique du volume et l'inscription
  des résultats dans l'historique par l'appelant. Elle inclut les pauses éventuelles
  dans les callbacks. Ces mesures servent à distinguer le coût de préparation des
  lots du travail effectué pendant leur exécution, avant d'augmenter leur taille.

Une ligne `trash_batches` dans la cible habituelle des opérations de fichiers
indique le nombre de lots et le temps de traitement en cas de retour normal, y
compris une annulation observée entre lots. La durée totale inclut les pauses.

Les tests natifs ajoutés suppriment uniquement des fichiers de fixtures privés,
avec suppression définitive ; ils ne déposent rien dans la corbeille de l'utilisateur.
Ils vérifient les reçus réels de lots, le refus d'une identité incorrecte,
l'annulation avant lancement et la conservation d'un reçu antérieur à l'annulation.
Les tests existants des effets partiels des dossiers restent applicables.

Le gain sur 17 000 fichiers et le parcours réel de corbeille restent à mesurer dans
Oxyde. Fermer l'application avant de reconstruire le code partagé et les workers :

```powershell
cargo build --offline --bins
cargo run --offline
```

## Comparer 64 et 256 fichiers par lot

Le défaut à 256 est retenu après les essais locaux : environ 73,4 s pour 17 140
fichiers, contre 93,8 à 96 s à 64. Un essai d'annulation sur 1 005 fichiers, après
correction du travail périodique de l'historique, s'est terminé environ 53 ms après
la demande, avec 793 reçus conservés. Ces mesures décrivent les essais disponibles,
pas une garantie de débit ou de délai sur d'autres supports.

Le défaut est 256. Un réglage de diagnostic limité à ces deux valeurs permet
la comparaison, sans modifier les paramètres persistants. La valeur est lue une
fois au début du traitement de chaque opération de corbeille. Une valeur invalide
est signalée dans le journal et utilise 256. Les dossiers gardent leur parcours
individuel. Les contrôles d'identité et callbacks d'annulation restent actifs.

Fermer Oxyde entre les deux lancements, activer le turbo dans les paramètres et
recréer le même lot de fichiers à supprimer avant chaque essai :

```powershell
$env:OXYDE_TRASH_BATCH_SIZE = "64"
cargo run --offline
# Après fermeture d'Oxyde et préparation d'une nouvelle copie du lot :
$env:OXYDE_TRASH_BATCH_SIZE = "256"
cargo run --offline
```

`trash_batch_plan` et `trash_batches` indiquent la taille choisie. Pour 17 140
fichiers ordinaires, une seule politique de volume et aucune erreur, on attend
268 lots à 64 ou 67 lots à 256. Le nombre traité affiché se met à jour au retour
d'un lot : avec 256 il avance par pas plus grands. La variable ne change que la
session du terminal courant et les processus lancés depuis celui-ci.

Revenir au défaut pour les prochains lancements :

```powershell
Remove-Item Env:OXYDE_TRASH_BATCH_SIZE -ErrorAction SilentlyContinue
```

## Réactivité après annulation

Le journal a confirmé un arrêt partiel à 2 991 fichiers sur 17 140, mais ne
contenait pas encore l'heure de demande pour mesurer le délai d'annulation.
Un travail périodique inutile a été corrigé : la persistance de l'historique
reconstruisait et sérialisait toutes les sélections finales toutes les 100 ms,
même sans modification. Elle contrôle désormais une clé légère des dix dernières
opérations finales et ne reconstruit le JSON qu'en cas de changement, ou à l'arrêt.
La sérialisation se déroule sans les verrous du gestionnaire et des opérations.
La lecture du nom courant ne construit plus un instantané complet des fichiers.

Cela supprime un coût identifié, sans prouver à lui seul que tous les problèmes
de fluidité sont résolus. Pour les prochains essais de réactivité, utiliser environ
500 petits fichiers, avec un volume limité, plutôt que recopier les 17 140 fichiers
ou les vidéos. Les tests de persistance génèrent des chemins en mémoire et de petits
fichiers JSON ; ils ne recopient pas le jeu de données de plusieurs gigaoctets.
Commencer par relancer Oxyde et naviguer avec l'historique existant, sans recopier
ni supprimer de nouveaux fichiers : le coût périodique corrigé existait aussi au
repos après restauration d'un historique volumineux.

## Historique et rendu de la fenêtre principale

Une seconde source de lenteur a été identifiée : le bouton du gestionnaire et les
notifications de la fenêtre principale construisaient des instantanés complets
de toutes les opérations à chaque rendu, puis filtraient les opérations terminées.
Les sources et bilans par fichier étaient donc copiés même avec la fenêtre du
gestionnaire fermée. Cela rendait le coût d'un simple survol dépendant de la taille
des sélections présentes dans l'historique.

Ces lectures utilisent désormais `operation_counts()` et `list_active_progress()`.
Le suivi périodique de l'opération utilise `operation_state()` plutôt qu'un
instantané complet. Ces API ne copient pas les listes de sources ou les bilans
par fichier, y compris pour les opérations actives. Les vues du gestionnaire et
des fichiers gardent les API détaillées ; les données ne sont ni supprimées ni
tronquées pour accélérer le rendu principal. Un simple comptage de lignes ou un
test d'existence de l'historique ne doit pas appeler `list_operations()`.

Pour vérifier la fluidité, conserver l'historique existant et survoler les fichiers
avec le gestionnaire fermé : aucune nouvelle copie massive n'est nécessaire.

## Restauration et vidage : erreurs indépendantes (2026-10-16)

La restauration depuis la vue Corbeille utilise `restore_items_report` : index des
identifiants et chemins d'origine, dédoublonnage, préparation et déplacement d'un
seul élément à la fois. Un élément indisponible, ambigu ou en conflit est signalé
sans bloquer les suivants. Aucun écrasement ni création automatique du dossier
d'origine. Le bilan traduit affiche les réussites, les échecs et le premier chemin
en échec. Les appels historiques `restore_items` conservent leur contrat Result,
mais traitent également les éléments indépendants avant de retourner une erreur.
La restauration du journal d'annulation garde son propre protocole de preuves.

Logs `oxyde_lib::trash_restore` : début, énumération, résolution des noms,
progression toutes les 256 tentatives, bilan et durées. Chaque échec contient
l'identifiant, la destination et la cause. Les erreurs natives précisent l'étape
(hold_source_parents, open_metadata, read_metadata, open_payload,
hold_destination_parents, rename_payload) et le Debug de l'erreur IO, incluant
le code système. Les réussites individuelles sont au niveau Debug ; les échecs
et les bilans restent disponibles avec la configuration de log normale.

Logs `oxyde_lib::trash_purge` : distinguent `empty_native` (paramètres) et
`selected` (vue Corbeille), énumération, préparation et appel Shell, erreurs et
durée totale. Le moteur de vidage n'est pas remplacé dans ce changement ; ces
mesures doivent permettre de situer sa lenteur. Aucun essai automatique ne vide
la corbeille réelle de l'utilisateur.
