# Utilisation de la version "slim" pour économiser de l'espace disque
FROM node:22-bookworm-slim

# 1. Installation des dépendances, suivie d'un nettoyage immédiat du cache apt
RUN apt-get update && \
    apt-get install -y git build-essential curl ca-certificates && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# 2. Installation de Rust (indispensable pour la v3)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 3. Installation d'OpenClaw et nettoyage du cache npm pour gagner de la place
RUN npm install -g openclaw@latest && \
    npm cache clean --force

# 4. Configuration du dossier de travail
WORKDIR /app/project

# 5. Maintien en vie du conteneur
CMD ["tail", "-f", "/dev/null"]