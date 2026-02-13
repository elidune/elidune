## Guide frontend — Endpoint `GET /stats/catalog`

### Description

Cet endpoint retourne des statistiques sur les exemplaires du catalogue (actifs, entrés, archivés) et les emprunts sur une période. Il accepte des flags booléens optionnels qui contrôlent le niveau de détail de la réponse.

### Paramètres de requête

| Paramètre | Type | Description |
|---|---|---|
| `start_date` | string (ISO 8601) | Début de période pour entrées/archivages/emprunts |
| `end_date` | string (ISO 8601) | Fin de période |
| `by_source` | bool | Ventiler par source (fournisseur/dépôt) |
| `by_media_type` | bool | Ventiler par type de média (livre, CD, DVD…) |
| `by_public_type` | bool | Ventiler par public visé (adulte/jeunesse) |

### Structure de la réponse

La réponse contient toujours un objet `totals` avec les compteurs globaux :

```json
{
  "totals": {
    "active_specimens": 12000,
    "entered_specimens": 350,
    "archived_specimens": 120,
    "loans": 2800
  }
}
```

Chaque entrée à chaque niveau porte **4 métriques** :
- `active_specimens` : exemplaires actifs (non archivés)
- `entered_specimens` : exemplaires entrés dans la période (0 si pas de période)
- `archived_specimens` : exemplaires archivés dans la période (0 si pas de période)
- `loans` : emprunts dans la période (0 si pas de période)

Les champs optionnels `by_source`, `by_media_type`, `by_public_type` apparaissent **uniquement quand ils sont le niveau racine de la ventilation demandée**. Quand plusieurs flags sont actifs, seule la version imbriquée est retournée. Les champs absents sont omis du JSON (pas de `null`).

### Principe d'imbrication

La hiérarchie est : **source → media_type → public_type**.

Quand plusieurs flags sont combinés, chaque niveau inférieur est imbriqué dans le niveau supérieur. On ne retourne jamais un niveau plat s'il est déjà imbriqué ailleurs.

### Comportement selon les flags demandés

**1. Aucun flag** → seul `totals` est retourné.

**2. Un seul flag** → le champ correspondant est une liste plate au niveau racine.

- `by_source=true` → `by_source[]`
- `by_media_type=true` → `by_media_type[]`
- `by_public_type=true` → `by_public_type[]`

```json
{ "totals": { "..." : "..." }, "by_source": [
    { "source_id": 1, "source_name": "Médiathèque", "active_specimens": 8000, "entered_specimens": 200, "archived_specimens": 80, "loans": 1900 }
]}
```

**3. `by_source` + `by_media_type`** → `by_source[].by_media_type[]`

```json
{ "totals": { "..." : "..." }, "by_source": [
    { "source_id": 1, "source_name": "Médiathèque",
      "active_specimens": 8000, "entered_specimens": 200, "archived_specimens": 80, "loans": 1900,
      "by_media_type": [
        { "label": "b", "active_specimens": 5000, "entered_specimens": 120, "archived_specimens": 40, "loans": 1200 },
        { "label": "vd", "active_specimens": 1500, "entered_specimens": 30, "archived_specimens": 15, "loans": 400 }
      ]
    }
]}
```

**4. `by_source` + `by_public_type`** (sans `by_media_type`) → `by_source[].by_public_type[]`

```json
{ "totals": { "..." : "..." }, "by_source": [
    { "source_id": 1, "source_name": "Médiathèque",
      "active_specimens": 8000, "entered_specimens": 200, "archived_specimens": 80, "loans": 1900,
      "by_public_type": [
        { "label": "adult", "active_specimens": 5200, "entered_specimens": 130, "archived_specimens": 50, "loans": 1200 },
        { "label": "children", "active_specimens": 2800, "entered_specimens": 70, "archived_specimens": 30, "loans": 700 }
      ]
    }
]}
```

**5. `by_media_type` + `by_public_type`** (sans `by_source`) → `by_media_type[].by_public_type[]`

```json
{ "totals": { "..." : "..." }, "by_media_type": [
    { "label": "b", "active_specimens": 7000, "entered_specimens": 180, "archived_specimens": 60, "loans": 1800,
      "by_public_type": [
        { "label": "adult", "active_specimens": 4500, "entered_specimens": 110, "archived_specimens": 35, "loans": 1100 },
        { "label": "children", "active_specimens": 2500, "entered_specimens": 70, "archived_specimens": 25, "loans": 700 }
      ]
    }
]}
```

**6. `by_source` + `by_media_type` + `by_public_type`** → `by_source[].by_media_type[].by_public_type[]`

```json
{ "totals": { "..." : "..." }, "by_source": [
    { "source_id": 1, "source_name": "Médiathèque",
      "active_specimens": 8000, "entered_specimens": 200, "archived_specimens": 80, "loans": 1900,
      "by_media_type": [
        { "label": "b", "active_specimens": 5000, "entered_specimens": 120, "archived_specimens": 40, "loans": 1200,
          "by_public_type": [
            { "label": "adult", "active_specimens": 3200, "entered_specimens": 75, "archived_specimens": 25, "loans": 750 },
            { "label": "children", "active_specimens": 1800, "entered_specimens": 45, "archived_specimens": 15, "loans": 450 }
          ]
        }
      ]
    }
]}
```

### Tableau récapitulatif

| Flags activés | Champs racine retournés | Structure |
|---|---|---|
| *(aucun)* | `totals` | — |
| `by_source` | `by_source[]` | plat |
| `by_media_type` | `by_media_type[]` | plat |
| `by_public_type` | `by_public_type[]` | plat |
| `by_source` + `by_media_type` | `by_source[]` | `source.by_media_type[]` |
| `by_source` + `by_public_type` | `by_source[]` | `source.by_public_type[]` |
| `by_media_type` + `by_public_type` | `by_media_type[]` | `media.by_public_type[]` |
| les 3 | `by_source[]` | `source.by_media_type[].by_public_type[]` |

### Règles pour le rendu

1. **Détection du niveau d'imbrication** : vérifier la présence des champs optionnels (`by_media_type` / `by_public_type` dans un objet source ou media). S'ils sont absents, affichage plat.
2. **Les totaux du parent = somme des enfants** : `source.active_specimens` == Σ `source.by_media_type[].active_specimens`, etc. Même chose pour `loans`.
3. **Tri** : toutes les listes sont pré-triées par `active_specimens` décroissant.
4. **Quatre métriques systématiques** : chaque entrée porte `active_specimens`, `entered_specimens`, `archived_specimens`, `loans`. Les 3 dernières valent 0 si aucune période n'est spécifiée.
5. **Affichage recommandé** : accordéon/arbre pour les niveaux imbriqués, tableau simple pour les listes plates, pie chart pour un seul niveau de ventilation.

### Note sur les emprunts

Les emprunts sont comptés depuis les deux tables (`loans` + `loans_archives`) via `UNION ALL`, toujours rattachés à l'exemplaire physique (`specimens`) pour obtenir la source, le type de média et le public visé. Tous les niveaux de ventilation (source, media, public) incluent donc l'intégralité des emprunts de la période.
