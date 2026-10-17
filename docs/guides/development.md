# Développer et vérifier Oxyde

Sous Windows, utiliser Rust MSVC x64 et les Build Tools Visual C++. Les dépendances doivent déjà être disponibles pour les commandes `--offline` ; retirer cette option pour un premier téléchargement.

## Construire et lancer

Fermer Oxyde avant de reconstruire ses exécutables :

```powershell
cargo build --offline --bins
cargo run --offline
```

`cargo run` seul ne reconstruit pas les workers. Garder les trois exécutables auxiliaires à côté du binaire principal. Pour livrer : [distribution](distribution.md).

## Vérifier un changement

```powershell
cargo check --offline --locked --all-targets
```

Exécuter les tests pertinents ; annoncer les scénarios réellement découverts, les échecs et les cibles non vérifiées. Un filtre sans test ne valide rien. Ne pas confondre compilation, tests natifs et [essai visuel](visual-checks.md).

La suite Windows complète utilise un TEMP/TMP privé, compile toutes les cibles, construit explicitement les binaires puis exécute les tests en série :

```powershell
./scripts/test-windows.ps1 -Offline
```

Le workflow `.github/workflows/tests.yml` utilise le même script. Linux nécessite son propre environnement ; les tentatives croisées historiques sur Windows étaient bloquées par Fontconfig/sysroot.

## Découper sans changer le comportement

Une façade déclare ses sous-modules privés et assemble leurs résultats. Nommer les enfants par responsabilité (`details`, `validation`, `thumbnails`), sans modules fourre-tout ni dépendances circulaires. Répartir les `impl` dans les enfants quand cela conserve l’état privé. Garder les helpers locaux et utiliser des réexportations ciblées.

Lors d’une extraction, préserver types d’actions, captures, identifiants GPUI, focus, propagation des événements, ordre des overlays, géométrie, virtualisation et propriétaires des tâches/abonnements. Procéder par étapes vérifiables, dans le périmètre du changement.

## Ranger les tests unitaires

Placer les nouveaux tests dans un sous-répertoire `tests/` du module concerné. Le dossier racine `tests/` est réservé aux intégrations d’interfaces publiques. Garder fixtures et helpers près de leurs scénarios.

Le déplacement physique conserve le parent et le nom du module Rust :

```rust
#[cfg(test)]
#[path = "search/tests/content_tests.rs"]
mod content_tests;
```

Le fichier extrait contient directement le corps de l’ancien module, sans ajouter `mod tests`. Conserver les conditions de cible, attributs (`ignore`, Tokio, `should_panic`), assertions, synchronisation et nettoyage. Ne pas élargir les visibilités ; les helpers conditionnels peuvent rester dans le parent.

Vérifier les chemins de `include_str!`, `include_bytes!`, `file!` et `#[path]`. Comparer la découverte avant/après puis compiler et exécuter les mêmes configurations disponibles. Ne pas migrer tous les petits blocs existants à l’occasion d’une tâche locale.
