# Build stage
FROM rust:1.70-slim-bullseye as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    postgresql-client \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty project
WORKDIR /usr/src/r8s

# Copy over manifests and source code
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    postgresql-client \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user to run the application
RUN groupadd -r r8s && useradd -r -g r8s r8s

# Create necessary directories
RUN mkdir -p /opt/r8s/plugins /opt/r8s/data /opt/r8s/config /var/log/r8s \
    && chown -R r8s:r8s /opt/r8s /var/log/r8s

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/r8s/target/release/r8s-cli /usr/local/bin/r8s
COPY --from=builder /usr/src/r8s/crates/r8s-persistence/src/migrations /opt/r8s/migrations

# Switch to the non-root user
USER r8s
WORKDIR /opt/r8s

# Create a default configuration file
RUN echo '[database]\n\
    url = "postgres://postgres:postgres@postgres:5432/r8s"\n\
    max_connections = 5\n\
    run_migrations = true\n\
    \n\
    [api]\n\
    host = "0.0.0.0"\n\
    port = 3000\n\
    jwt_secret = "change_me_in_production"\n\
    enable_openapi = true\n\
    \n\
    [worker]\n\
    threads = 4\n\
    max_concurrent_executions = 10\n\
    \n\
    [plugin]\n\
    load_on_startup = true\n\
    plugin_dirs = ["/opt/r8s/plugins"]\n\
    ' > /opt/r8s/config/r8s.toml

# Set environment variables
ENV R8S_CONFIG_FILE="/opt/r8s/config/r8s.toml"
ENV RUST_LOG="info"

# Expose the API port
EXPOSE 3000

# Define a volume for plugins, config, and data
VOLUME ["/opt/r8s/plugins", "/opt/r8s/config", "/opt/r8s/data"]

# Set up a healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Create an entrypoint script
USER root
RUN echo '#!/bin/bash\n\
    set -e\n\
    \n\
    # Wait for PostgreSQL to be available\n\
    until pg_isready -h ${R8S_DATABASE__HOST:-postgres} -p ${R8S_DATABASE__PORT:-5432}; do\n\
    echo "Waiting for PostgreSQL to become available..."\n\
    sleep 2\n\
    done\n\
    \n\
    # Check if this is the first run\n\
    if [ ! -f /opt/r8s/data/.initialized ]; then\n\
    echo "First run detected, initializing r8s..."\n\
    r8s init --non-interactive \\\n\
    --config-file=${R8S_CONFIG_FILE} \\\n\
    --admin-username=${R8S_ADMIN_USERNAME:-admin} \\\n\
    --admin-email=${R8S_ADMIN_EMAIL:-admin@example.com} \\\n\
    --admin-password=${R8S_ADMIN_PASSWORD:-changeme}\n\
    touch /opt/r8s/data/.initialized\n\
    fi\n\
    \n\
    # Start the server\n\
    exec r8s serve\n\
    ' > /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/docker-entrypoint.sh && \
    chown r8s:r8s /opt/r8s/data

USER r8s

# Command to run
ENTRYPOINT ["docker-entrypoint.sh"]