# Frequently asked questions

## What are the differences between rustic and restic?
- Written in Rust instead of golang
- Optimized for small resource usage (in particular memory usage, but also overall CPU usage)
- Philosophy of development (release new features early)
- New features (e.g. lock-free pruning)

## Why is rustic written in Rust
Rust is a powerful language designed to build reliable and efficient software.
This is a very good fit for a backup software.

## How does the lock-free prune work?
Like the prune within restic, rustic decides for each pack whether to keep it, remove it or repack it.
Instead of removing packs, it however only marks the packs to remove in a separate index structure.
Packs which are marked for removal are checked if they are really not needed and have been marked
long enough ago. Depending on these checks they are either finally removed, recovered or kept in the
state of being marked for removal.

## You said "rustic uses less resources than restic" but I'm observing the opposite...
In general rustic uses less resources, but there may be some exceptions. For instance the crypto libraries
of Rust and golang both have optimizations for some CPUs. But it might be that your CPU benefits from a
golang optimization which is not present in the Rust implementation.
If you observe some unexpected resource usage, please don't hesitate to submit an issue. 
