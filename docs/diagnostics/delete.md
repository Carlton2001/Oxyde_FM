# Diagnostiquer suppressions et Corbeille

Sous Windows, les fichiers ordinaires à identité capturée passent en lots Shell de 256 au maximum ; dossiers et objets spéciaux gardent leur suivi individuel. Les lots sont séparés selon la politique de corbeille du volume. Chaque élément conserve contrôles et reçu, même si le lot finit en erreur ou sur annulation.

La progression est publiée au retour d’un lot. Pause et annulation sont vérifiées avant les lots et dans les callbacks ; un appel Windows déjà bloqué reste dépendant du système. Les priorités suivent le turbo. Aucune attente artificielle par élément n’est ajoutée.

L’exemple `delete_tree_benchmark [PARENT]` chronomètre la suppression définitive d’un dossier généré (plat, imbriqué, gros fichiers), sans et avec Turbo.

## Où chercher dans `oxyde.log`

| Cible / événement | Usage |
| --- | --- |
| `oxyde_lib::delete_timing` | Début/fin, sélection, plan et exécution des lots |
| `operation_cancel_requested` / `operation_cancel_accepted` | Délai d’obtention des verrous ; comparer ensuite à la fin de l’opération |
| `trash_native_batch` | `setup_s`, `native_s`, `proof_s`, `remaining_s`, `wall_s` et résultats |
| `oxyde_lib::trash_restore` | Énumération, noms, progression, erreurs et bilan de restauration |
| `oxyde_lib::trash_purge` | Distingue vidage `empty_native` et suppression `selected`, puis préparation et appel Shell |

`proof_s` est inclus dans `native_s` : ne pas les additionner. `wall_s` commence dans le thread natif et exclut sa création et l’enregistrement ultérieur des reçus. Ces durées ne mesurent pas l’attente préalable dans la boucle UI.

## Comparer les tailles de lots

Sur des copies de fixtures, fermer Oxyde entre les lancements et recréer le même jeu avant chaque suppression :

```powershell
$env:OXYDE_TRASH_BATCH_SIZE = "64"
cargo run --offline
# Fermer Oxyde et recréer les fixtures avant le second essai.
$env:OXYDE_TRASH_BATCH_SIZE = "256"
cargo run --offline
Remove-Item Env:OXYDE_TRASH_BATCH_SIZE -ErrorAction SilentlyContinue
```

Construire auparavant les [binaires et workers](../guides/development.md). Seules 64 et 256 sont acceptées ; une valeur invalide est journalisée et utilise 256. La valeur est lue au début de chaque opération, sans modifier les préférences persistantes. Les anciens chronométrages locaux restent dans l’[historique](../reports/README.md), sans garantie de débit sur d’autres supports.

## Fluidité et effets partiels

Commencer avec environ 500 petits fichiers. Pour un ralentissement au repos, conserver l’historique existant et survoler avec le gestionnaire fermé : pas besoin de recréer un gros transfert. Les résumés UI et la persistance doivent éviter de recopier périodiquement toutes les sélections ; voir [opérations](../architecture/operations.md).

Une restauration directe traite les éléments indépendamment, sans écraser ni créer le dossier parent absent. Une erreur n’arrête pas les suivants ; le bilan contient réussites, échecs et premier chemin fautif. Le journal d’Undo garde son propre protocole de preuves. Les tests ne doivent pas vider la Corbeille réelle de l’utilisateur.
