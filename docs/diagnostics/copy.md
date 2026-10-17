# Mesurer une copie réelle

Les opérations normales écrivent dans `oxyde.log`, à côté de `config.json`. La ligne `session start` indique le chemin exact ; en mode portable, utiliser `data/oxyde.log`. Le [benchmark séparé](../guides/copy-benchmark.md) produit un CSV et ne mesure pas l’application.

Après [construction des binaires](../guides/development.md), copier un même échantillon depuis l’interface, attendre la fin et relever les lignes `copy_timing` :

```powershell
Get-Content "$env:APPDATA\oxyde\oxyde.log" | Select-String 'copy_timing'
```

Comparer plusieurs passages normal/turbo en alternant leur ordre, avec mêmes fichiers, destination et supports. Le cache Windows reste actif. Un petit lot et un lot de gros fichiers suffisent pour un premier diagnostic ; ne pas lancer systématiquement une campagne volumineuse.

## Lire les phases

Cible : `oxyde_lib::copy_timing`, durées en secondes avec point décimal.

| Événement | Ce qu’il mesure |
| --- | --- |
| `copy_operation_start` / `copy_operation_end` | Opération, mode turbo, durée et bilan |
| `copy_source_selection` | Capture des racines et contrôle des chevauchements |
| `copy_plan_traversal` | Parcours et préparation du plan/conflits, pauses comprises |
| `copy_file` | Parcours d’un fichier, de la copie à sa publication/nettoyage |

Pour l’inventaire exact des événements et champs, consulter `src/systems/copy_diagnostics.rs` et rechercher `copy_timing` dans `src/`.

| Champs par fichier | Interprétation |
| --- | --- |
| `published`, `bytes` | Publication réussie et taille terminée avant hachage ; zéro possible si échec précoce |
| `wall_s`, `pause_s`, `active_s` | Durée réelle, pauses observées et différence |
| `native_s` | Copie Platform, vidage inclus, pauses observées exclues |
| `flush_s`, `flush_calls` | Appels explicites de vidage ; **temps déjà inclus dans `native_s`** |
| `hash_s` | Relecture BLAKE3 de destination, pauses observées exclues |
| `publish_s` | Renommage final, y compris tentative en échec |
| `parents_s`, `checks_s` | Résolution des parents et contrôles avant copie/publication |
| `staging_create_s`, `writer_release_s`, `output_open_s` | Préparation privée, fermeture du writer et réouverture vérifiée |
| `cleanup_s`, `remaining_s` | Destructeur du staging et reste non attribué, sans compter le vidage deux fois |

## Éviter les mauvaises conclusions

Le temps d’opération inclut planification, attente des workers et pauses entre fichiers. Les durées des workers se chevauchent : leur somme n’est pas le temps réel. Le parcours par fichier exclut certaines préparations et l’enregistrement ultérieur dans l’historique.

`native_s - flush_s` n’est pas un débit matériel pur. `hash_s` peut lire le cache système. Un déplacement sur le même volume peut ne produire aucune ligne de copie. Les récapitulatifs peuvent manquer après un arrêt brutal ; `published=false` indique une absence de publication. Retirer les chemins personnels des logs avant partage.
