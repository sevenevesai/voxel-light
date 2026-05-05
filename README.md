# Generic 3D light propagation for voxel engines

[<img alt="github" src="https://img.shields.io/badge/github-sevenevesai/voxel--light-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/sevenevesai/voxel-light)
[<img alt="crates.io" src="https://img.shields.io/crates/v/voxel-light.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/voxel-light)

Every voxel engine implements the same BFS flood-fill light propagation from
scratch. The hard part — two-phase removal with re-propagation from surviving
sources — is routinely left as a TODO or implemented incorrectly. This crate
extracts the algorithm into a single trait boundary so you don't write it again.

<br>

## Install

```
cargo add voxel-light
```

<br>

## Usage

Implement the one-method trait for your storage:

```rust
use voxel_light::{VoxelAccess, VoxelInfo};

impl VoxelAccess for MyWorld {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        let chunk = self.get_chunk(x.div_euclid(16), y.div_euclid(16), z.div_euclid(16))?;
        let block = chunk.get_block(x.rem_euclid(16) as usize, /* ... */);
        Some(VoxelInfo {
            transparent: block.is_transparent(),
            block_light: chunk.get_light(x, y, z),
            emission: block.emission(),
        })
    }
}
```

Then call the algorithm and apply the results:

```rust
use voxel_light::LightEngine;

let mut engine = LightEngine::new();

// Place a torch (level 14)
let updates = engine.place_light(&world, [10, 64, 10], 14);
for u in &updates {
    world.set_light(u.x, u.y, u.z, u.level);
}

// Remove it — re-propagation from surviving sources is handled automatically
let updates = engine.remove_light(&world, [10, 64, 10]);
for u in &updates {
    world.set_light(u.x, u.y, u.z, u.level);
}
```

The library never mutates your storage. It returns a `Vec<LightUpdate>` and you
decide how to apply it.

<br>

## API

| Method | Use case |
|--------|----------|
| `place_light(access, pos, level)` | Torch placed |
| `remove_light(access, pos)` | Torch broken — zeroes dependents, re-propagates from boundaries |
| `block_placed(access, pos)` | Opaque block placed — removes light it blocks |
| `block_removed(access, pos)` | Solid block broken — light flows in from neighbors |
| `recalculate_area(access, center, radius)` | Nuclear option — clears and re-propagates from all emitters in radius |

All methods are also available as free functions (`propagate`, `remove`) if you
don't need buffer reuse.

<br>

## Features

| Feature | What it adds |
|---------|-------------|
| `std` (default) | Uses `ahash` for the visited map (~2x faster than `BTreeMap`) |
| `sky` | Column-first sky light propagation with horizontal BFS spread |
| `colored` | Per-channel RGB light (3 channels, single BFS pass) |

The core is `no_std` compatible (requires `alloc`). Without `std`, the visited
map falls back to `BTreeMap`.

<br>

## Performance

Measured on AMD Ryzen 9 7900, 64 GB RAM, Windows 11. Open-air world (no
obstructions, worst case for BFS expansion):

| Operation | Level 7 | Level 10 | Level 14 |
|-----------|---------|----------|----------|
| Propagation | 17 µs | 60 µs | 174 µs |
| Removal (single source) | 105 µs | — | 432 µs |
| Removal (neighboring source) | — | — | 463 µs |
| Full place + remove cycle | — | — | 585 µs |

A level-14 torch touches ~11,500 voxels. The full place-and-remove cycle
(propagate, then remove with re-propagation) completes in under 600 µs.

Benchmarks use criterion. Run `cargo bench` to reproduce.

<br>

## Why this exists

The BFS flood-fill algorithm for Minecraft-style lighting is well-documented.
The propagation half is straightforward. The removal half is not — you need a
two-phase BFS that correctly identifies dependent voxels, collects boundary
sources, then re-propagates against a virtual "zeroed" state before the caller
has applied any updates.

Projects that implement this by hand:

- [Luanti/Minetest](https://github.com/luanti-org/luanti/blob/master/src/voxelalgorithms.cpp)
  (~500 LOC in `voxelalgorithms.cpp`, has spawned multiple issues about rigidity)
- [Seed of Andromeda](https://github.com/RegrowthStudios/SoACode-Public)
  (three separate BFS passes per RGB channel, their tutorial is the de-facto
  reference)
- [PocketMine-MP](https://github.com/dktapps/lighting-algorithm-spec)
  (a standalone specification document just for the algorithm)
- Every Rust voxel engine on GitHub (Rezcraft, Aern-do/voxel, TanTanDev,
  Technici4n/voxel-rs) writes it from scratch with no shared code

No crate on crates.io provides trait-generic 3D BFS light propagation with
two-phase removal. `block-mesh` handles meshing. `building-blocks` handles
storage. `tapestry` is 2D-only. `pathfinding` solves a different problem. This
crate fills the gap.

<br>

## How it works

1. **Propagation**: BFS from source, decaying by 1 per block. Stops at opaque
   voxels (`transparent: false`) and world boundaries (`None` from
   `get_voxel`). Tracks visited positions to avoid re-processing.

2. **Removal phase 1**: BFS from removed source. For each neighbor: if its
   light is less than the current node's old level, it depended on the removed
   source — zero it and continue. If its light is >= the old level, it has
   independent light — save it as a boundary source.

3. **Removal phase 2**: Build an overlay that presents zeroed values for all
   removed positions (without requiring the caller to apply them first). BFS
   re-propagate from all boundary sources against this overlay.

4. **Output**: Flat `Vec<LightUpdate>` with zeroing updates first, then
   re-propagation values. Apply in order — later entries overwrite earlier ones
   at the same position.

<br>

## Known limitations

- **Not a spatial data structure.** The crate does not store light values. It
  queries your storage via the trait and returns updates. You own the data.

- **`AHashMap` allocation per call.** The visited map is allocated fresh each
  call (or reused via `LightEngine`'s internal buffers for the queue, but the
  hashmap is per-call). For extremely hot paths this could be pooled.

- **Colored removal is per-channel approximate.** A voxel is considered
  "dependent" if any channel is less than the corresponding old level. This can
  over-zero in edge cases with overlapping colored sources of different ranges.

- **No sky light removal.** The `sky` feature provides propagation only. Sky
  light removal (e.g., placing a roof) requires column recalculation, which is
  left to the caller.

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
