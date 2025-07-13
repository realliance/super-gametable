# Build the docker image for super-gametable using nix
build-docker:
    nix build .#dockerImage

# Load the docker image built by nix into docker daemon
load-docker: build-docker
    ./result | docker load
    rm -f ./result

# Start all services
up: load-docker
    docker compose up -d

# Stop all services
down:
    docker compose down -v

# Follow logs of the super-gametable service
logs:
    docker compose logs -f super-gametable

# Queue a match with the given players
queue-match *players:
    docker compose run --rm super-gametable /bin/super-gametable tools queue-match {{players}}

# Restart services
restart: down up

# A command to run everything
run: up
    just logs 