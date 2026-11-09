# Configuration

`kanso init` crée un fichier `kanso.toml` commenté dans le répertoire courant sans écraser un fichier existant. Par défaut, Kanso cherche `kanso.toml` à la racine du projet déduite des chemins analysés ; `--config PATH` choisit explicitement un fichier. Les clés inconnues sont refusées.

```toml
# Seuils de tokens par fichier
 token_warn = 2000
 token_error = 6000
# Seuils du contexte racine
 total_token_warn = 10000
 total_token_error = 20000
# Similarité entre 0.0 et 1.0
 similarity_threshold = 0.85
# Taille maximale lue pour un fichier d'instructions ou une ressource liée
 max_file_bytes = 2097152
# Identifiants KS001 à KS009 uniquement
 disabled_rules = []
# Exclusions de découverte au format gitignore
 ignore = []
```

Les valeurs indiquées sont celles par défaut. Chaque seuil d'avertissement doit être inférieur ou égal au seuil d'erreur correspondant. La similarité doit être finie et comprise entre 0 et 1 inclus ; `max_file_bytes` doit être positif. KS000 est un diagnostic d'entrée-sortie, pas une règle configurable. Les tokens sont comptés avec o200k.

Le contexte racine comprend les fichiers d'instructions pris en charge à la racine, `.cursor/rules/*.mdc` à la racine et `.github/copilot-instructions.md`. Il exclut `SKILL.md` et les contextes imbriqués. Les contrôles entre fichiers portent uniquement sur ce contexte ; les contrôles par fichier s'appliquent ailleurs. Les règles gitignore s'appliquent aussi. Les liens symboliques vers des fichiers réguliers internes sont traités comme des alias ; leur cible peut être analysée même hors du sous-arbre sélectionné ou si elle correspond à une règle d'exclusion.
