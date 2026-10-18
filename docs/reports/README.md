# Bilans et historique

Les références actives décrivent les contrats ; les bilans ci-dessous décrivent ce qui a réellement été vérifié à une date donnée. Aucun ancien test n’a été relancé pour la refonte documentaire du 17 octobre 2026.

| Bilan | Lecture utile |
| --- | --- |
| [Sécurité, 13 octobre](2026-10-13-security.md) | Décisions, commits, bornes de tests et limites |
| [Fenêtres et aperçus, 15 octobre](2026-10-15-windows.md) | Migration, corrections après essais et validation manquante |
| [Code, 17 octobre](2026-10-17-code-review.md) | Corrections A01–A10 et optimisations O01–O04 intégrées |
| [Transport d’archives, 17 octobre](2026-10-17-archive-transport.md) | Correction des attentes IPC, mesures avant/après et validation native |
| [Turbo des archives, 17 octobre](2026-10-17-archive-turbo.md) | Parallélisme, mesures ZIP/7z/TAR.ZST/TAR et répartition de la mémoire |
| [Couverture de Turbo, 17 octobre](2026-10-17-turbo-coverage.md) | Effets par opération, lacunes et pistes de correction priorisées |
| [Corrections UI, 18 octobre](2026-10-18-ui-audit-fixes.md) | Douze corrections, commits, vérifications et parcours visuels précis |

## Retrouver une preuve complète

Les longs rapports, plans achevés et anciennes conventions ont été retirés du checkout après synthèse. Ils restent dans Git à la révision **`3befdc51a689e907d99b7c0603bbf8d666ae8976`**. Exemple, depuis la racine :

```powershell
git show 3befdc51a689e907d99b7c0603bbf8d666ae8976:docs/CODE_AUDIT_2026-10-16.md
```

Pour retrouver l’évolution d’un sujet, utiliser `git log --all -- <ancien-chemin>`. Les journaux et dumps sous `target/` ne sont pas versionnés et peuvent avoir disparu ; la présence d’une référence n’atteste pas leur disponibilité.

## Correspondance des anciens documents

Tous les chemins de la première colonne sont relatifs à la racine et récupérables par la commande ci-dessus.

| Ancien chemin | Référence de remplacement |
| --- | --- |
| `docs/UI_ARCHITECTURE.md` | [Interface](../architecture/interface.md), [fenêtres](../architecture/windows.md), [aperçus](../architecture/previews.md), [archives](../architecture/archives.md), [opérations](../architecture/operations.md) |
| `docs/POPUP_WINDOWS_PLAN.md` | Plan terminé ; [contrat des fenêtres](../architecture/windows.md) |
| `docs/NATIVE_HOVER_PROTOTYPE.md` | Prototype dépassé ; [contrat des fenêtres](../architecture/windows.md) |
| `docs/POPUP_WINDOWS_AUDIT.md` | [Bilan fenêtres](2026-10-15-windows.md) |
| `docs/INPUTS_AUDIT.md` | [Interface](../architecture/interface.md), bilan fenêtres |
| `docs/IMAGE_PREVIEW_SESSIONS.md` | [Aperçus](../architecture/previews.md), bilan fenêtres |
| `docs/SHARED_PREVIEW_CACHE.md` | [Aperçus](../architecture/previews.md), bilan du code |
| `docs/SHUTDOWN_DIAGNOSTICS.md` | [Fermeture](../diagnostics/shutdown.md) |
| `docs/COPY_DIAGNOSTICS.md` | [Mesures de copie](../diagnostics/copy.md) |
| `docs/DELETE_DIAGNOSTICS.md` | [Suppressions](../diagnostics/delete.md) |
| `docs/COPY_BENCHMARK.md` | [Benchmark](../guides/copy-benchmark.md) |
| `docs/CODE_AUDIT_2026-10-16.md` | [Bilan du code](2026-10-17-code-review.md) |
| `security/AUDIT_SECURITE_RELIQUAT.md` | [Limites R1–R9](../security/status.md) |
| `security/AUDIT_GROUPE_1_RESULTATS.md` | [Bilan sécurité](2026-10-13-security.md) |
| `security/AUDIT_R123_RESULTATS.md` | Bilan sécurité |
| `security/AUDIT_R46789_RESULTATS.md` | Bilan sécurité |
| `security/ISOLATION_ARCHIVES.md` | [Archives](../architecture/archives.md), bilan sécurité |
| `security/VERIFICATION_VISUELLE.md` | [Guide visuel](../guides/visual-checks.md), bilan sécurité |
| `security/DEPENDENCY_MAINTENANCE.md` | [Décision de dépendances](../security/dependencies.md) |

Le README racine est conservé pour la version publiée. `installer/README.md` reste un point d’entrée vers la procédure unique de distribution. Les données RustSec restent dans `security/` ; elles ne sont pas une analyse actuelle.
