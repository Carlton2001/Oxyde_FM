# Opérations de fichiers et historique

`FileOperationManager` exécute en arrière-plan ; `OxydeApp` possède l’état d’interface. `components/operations/` sépare notifications, fenêtres, cartes, indicateurs et graphe.

## Présentation et persistance

Chaque opération active occupe une notification compacte de progression. Sa fin retire cette ligne et affiche le résultat habituel. Un clic ouvre ou réactive le gestionnaire unique et redimensionnable ; il ne s’ouvre pas automatiquement. Fermer ou masquer le gestionnaire conserve opérations et historique ; la fermeture principale retire ses fenêtres.

Les opérations actives restent dépliées ; les états finaux sont repliés par défaut. Les détails montrent au plus cinq racines sélectionnées. « Éléments concernés » ouvre une liste virtualisée par opération, avec filtre du chemin complet préparé en arrière-plan et rejet des résultats obsolètes. Les icônes réutilisent `render_file_icon`.

Le graphe s’affiche uniquement pendant les opérations actives ayant des mesures de débit. Il disparaît dans les états finaux ; la durée et le débit moyen restent visibles dans les détails. Les résumés de la fenêtre principale passent par `operation_counts()`, `list_active_progress()` et `operation_state()` ; ne pas cloner toutes les sources et tous les bilans pour un compteur ou un simple survol.

La liste des opérations reste uniquement en mémoire pendant la session. Aucun historique d’opérations n’est écrit ni restauré au démarrage ; les anciens fichiers `operation_history.json` sont ignorés.

## Bilans individuels

| État | Signification |
| --- | --- |
| Traité | Résultat conservé confirmé |
| Non traité / ignoré | Aucun traitement, ou décision d’ignorer |
| Interrompu | Commencé, sans effet conservé confirmé |
| Partiellement traité | Effets conservés confirmés : fichier publié ou dossier créé |
| Inconnu | Preuves insuffisantes, ancien bilan ou parcours virtuel non couvert |

Les reçus réels priment sur la progression. Les transferts suivent les descendants et la suppression de la racine déplacée ; les suppressions suivent la confirmation effective. Les anciens bilans Partiel non discriminants sont chargés comme inconnus, avec versionnement JSON. Une erreur finale ne supprime pas les reçus confirmés.

## Conflits et intégrité

Le plan réserve toutes les destinations avant le premier effet, y compris entre racines distinctes. L’autorisation de remplacer une destination existante ne permet pas d’écraser une autre source du même lot. Un déplacement sur soi est ignoré ; une copie sur soi choisit un nom libre traduit et le backend garde la publication exclusive.

Le dialogue conserve les métadonnées source/cible et les décisions par chemin. Le mode avancé hérite des décisions des dossiers, avec priorité aux exceptions explicites. Au premier choix individuel, les indécis reçoivent le choix contraire ; ce défaut automatique est identifiable et ne se réapplique pas ensuite. Aucun transfert avec conflit ne démarre avant validation complète. Échap annule l’opération en attente.

Copier/couper depuis la Corbeille conserve les chemins physiques comme identité et résout les noms via `Platform`. Restaurer reste un parcours distinct. La restauration directe traite les éléments indépendants sans écrasement ni création automatique du parent absent ; ses échecs n’empêchent pas les suivants. La suppression Shell garde ses limites de course par chemin : [sécurité R2](../security/status.md).

Pour mesurer les opérations : [copie](../diagnostics/copy.md) et [suppression](../diagnostics/delete.md). Pour les transferts virtuels : [archives](archives.md).
