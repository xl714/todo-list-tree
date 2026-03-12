# TODO LIST TREE

Il s'agit d'une application de gestion de tâches organisée en arbre, permettant de créer des tâches et des sous-tâches, avec une interface utilisateur intuitive. L'application est conçue pour être légère et rapide, en utilisant les technologies modernes du web.
La V2 est en React (via CDN) + HTML + CSS, tandis que la V3 est en cours de migration vers une PWA 100% Rust / WebAssembly.

## Launch container

```bash
# Démarrez votre environnement avec :
docker compose up -d --build

# Entrez dedans : 
docker exec -it openclaw-sandbox bash

# Lancer l'application :
openclaw

# Collez-lui le prompt.
```

## Prompt initial pour OpenClaw

Rôle : Tu es un Développeur Senior expert en Rust, WebAssembly (Wasm) et architectures Front-end modernes (React vers Rust).

Contexte de sécurité : Tu travailles dans /app/project.

v2 contient mon application actuelle "Todo List Tree". Elle est écrite en React (via CDN) + HTML + CSS. Ce dossier est en lecture seule.

v3 est ton espace de travail. Tu peux y écrire.

Le dossier .git et les fichiers de configuration à la racine sont verrouillés.

Objectif Phase 1 : Migrer l'application React de la v2 vers une PWA 100% Rust / WebAssembly dans le dossier v3, de manière strictement iso-fonctionnelle pour l'interface, mais en modernisant le stockage.

Choix techniques imposés pour la v3 :

Framework Front-end : Utilise le framework Rust Leptos (ou Yew), car leur logique de composants se mariera parfaitement avec mon architecture React actuelle.

Stockage local : Remplace l'usage actuel du localStorage par IndexedDB (via des crates Rust adaptées) pour préparer le terrain aux futures évolutions volumineuses.

Instructions pour démarrer :

Analyse les composants React de la v2 pour comprendre l'arbre d'UI, le format JSON des données actuel, et la gestion de l'état (View mode vs Edit mode).

Initialise le projet Rust dans v3 configuré pour compiler en WebAssembly.

Traduis les structures de données JSON actuelles en struct Rust typées avec Serde.

Reproduis les composants React en composants Rust.

Important : N'ajoute AUCUNE NOUVELLE FONCTIONNALITÉ métier pour le moment. On doit d'abord avoir une base Rust solide, identique visuellement et fonctionnellement à la v2.

Règle absolue : Demande toujours confirmation avant d'exécuter des commandes de suppression (rm) ou si tu as un doute sur une traduction de composant React vers Rust.