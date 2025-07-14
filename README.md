```
   ____                    _____               __       __   __
  / __/_ _____  ___ ____  / ___/__ ___ _  ___ / /____ _/ /  / /__
 _\ \/ // / _ \/ -_) __/ / (_ / _ `/  ' \/ -_) __/ _ `/ _ \/ / -_)
/___/\_,_/ .__/\__/_/    \___/\_,_/_/_/_/\__/\__/\_,_/_.__/_/\__/
        /_/
```

A service to run [libmahjong](https://github.com/realliance/libmahjong) matches at scale.

This service is designed to work alongside [parlor-room](https://github.com/realliance/parlor-room,
accepting incoming match requests, publishing game state updates, and game completions all against a message queue backbone.

# Getting Started

We use [just](https://github.com/casey/just) for simple command running. This isn't required, but is handy.

We do however require a [Nix](https://nixos.org/learn/) environment to build the docker images. While this isn't
ideal, building libmahjong within nix and extending that to container available is very advantagous.

```sh

# Build and deploy your testing environment
just up

# Run a match. Available controllers:
# https://github.com/realliance/libmahjong/tree/next/src/controllers
just queue-match AngryDiscardoBot AngryDiscardoBot AngryDiscardoBot AngryDiscardoBot

# Check out state updates on the service
docker compose logs super-gametable

```

# Design

Libmahjong's nature as a C++ library interfaced with the [libmahjong-rs](https://github.com/realliance/libmahjong-rs) FFI layer. libmahjong-rs required synchronous locking (which is ideal for FFI anyways), so super-gametable is designed with a sync-async boundary to handle queue interaction and game pool execution.
