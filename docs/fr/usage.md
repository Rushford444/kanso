# Utilisation

Depuis la racine du projet, lancez `kanso check .`. Les commandes `check`, `score` et `fix` acceptent un ou plusieurs chemins ; sans chemin, elles utilisent le répertoire courant. La découverte respecte les règles gitignore.

```sh
kanso check .
kanso check . --json
kanso check . --format sarif
kanso score .
kanso score . --badge
kanso explain KS007
kanso init
kanso fix .
kanso fix . --write
```

`init` crée un fichier `kanso.toml` commenté sans écraser un fichier existant. `fix` donne un aperçu par défaut ; `--dry-run` demande explicitement cet aperçu et `--write` applique les corrections prudentes. Relisez le diff avant le commit. Les corrections ignorent les fichiers HTML bruts ; celles des espaces sont limitées aux lignes de liste simples.

## Sorties et codes

`--json` produit du JSON au schéma `kanso/v1` ; `--format sarif` produit du SARIF 2.1.0 pour `check`. `--quiet` supprime la sortie destinée aux humains ; `--config PATH` choisit le fichier de configuration. `--yes` est accepté globalement. Les commandes ne demandent aucune interaction.

| Code | Signification |
| --- | --- |
| 0 | Succès. `score` renvoie aussi 0 en présence de constats. |
| 1 | `check` a trouvé au moins un diagnostic autre qu'un alias, même informatif. |
| 2 | Utilisation ou configuration invalide, règle inconnue ou chemin d'entrée absent. |
| 3 | Erreur d'exécution. |

KS000 signale les erreurs d'entrée-sortie pendant l'analyse ; un chemin absent produit plutôt le code 2. Le score part de 100 et retire 15 par erreur, 5 par avertissement et 1 par information, avec un plafond de 30 points par règle et un minimum de 0. Les alias ne sont pas comptés. Le score global utilise tous les diagnostics et non la moyenne des scores par fichier.

## Limites

L'analyse fonctionne hors ligne et n'exécute pas les commandes des documents. Elle détecte des motifs, pas tous les secrets ou contradictions possibles. Les valeurs secrètes des diagnostics sont masquées, mais pas les chemins : ne placez pas de secrets dans les noms de fichiers. Voir [sécurité](../../SECURITY.md) et [configuration](configuration.md).
