# Trait-generic 3D BFS light propagation for voxel engines

[<img alt="github" src="https://img.shields.io/badge/github-sevenevesai/voxel--light-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/sevenevesai/voxel-light)
[<img alt="crates.io" src="https://img.shields.io/crates/v/voxel-light.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/voxel-light)

BFS flood-fill light propagation with two-phase removal and re-propagation
from surviving sources. Operates on any storage that implements a single trait
method. Returns updates as a `Vec` — never mutates caller data.

<br>

## Install

```
cargo add voxel-light
```

<br>

## Usage

Implement the trait for your storage:

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

Call the algorithm and apply results:

```rust
use voxel_light::LightEngine;

let mut engine = LightEngine::new();

// Place a torch (level 14)
let updates = engine.place_light(&world, [10, 64, 10], 14);
for u in &updates {
    world.set_light(u.x, u.y, u.z, u.level);
}

// Remove it — two-phase removal zeroes dependents, then re-propagates
// from any neighboring sources that still exist
let updates = engine.remove_light(&world, [10, 64, 10]);
for u in &updates {
    world.set_light(u.x, u.y, u.z, u.level);
}
```

<br>

## API

| Method | When to call |
|--------|-------------|
| `place_light(access, pos, level)` | Light-emitting block placed |
| `remove_light(access, pos)` | Light-emitting block broken |
| `block_placed(access, pos)` | Opaque block placed (may cut light paths) |
| `block_removed(access, pos)` | Opaque block broken (light may flow in) |
| `recalculate_area(access, center, radius)` | Clear and re-propagate all emitters in a cubic radius |

Free functions (`propagate`, `remove`) are available if you don't need
`LightEngine`'s buffer reuse.

<br>

## Features

| Feature | Effect |
|---------|--------|
| `std` (default) | Uses `ahash` for the visited map (~2x faster than `BTreeMap`) |
| `sky` | Column-first sky light propagation with horizontal BFS spread |
| `colored` | Per-channel RGB light (3 channels, single BFS pass) |

Core is `no_std` compatible (requires `alloc`). Without `std`, the visited map
uses `BTreeMap`.

<br>

## Performance

Open-air world (no obstructions, worst-case BFS expansion). AMD Ryzen 9 7900,
64 GB RAM, Windows 11:

| Operation | Level 7 | Level 10 | Level 14 |
|-----------|---------|----------|----------|
| Propagation | 17 µs | 60 µs | 174 µs |
| Removal (single source) | 105 µs | — | 432 µs |
| Removal (neighboring source) | — | — | 463 µs |
| Full place + remove cycle | — | — | 585 µs |

A level-14 torch touches ~11,500 voxels. Benchmarks use criterion: `cargo bench`.

<br>

## Algorithm

**Propagation.** BFS from source, decaying by 1 per block. Stops at opaque
voxels (`transparent: false`) and world boundaries (`None` from `get_voxel`).
Visited map prevents re-processing.

**Removal phase 1.** BFS from removed source. For each neighbor: if light <
node's old level, it was dependent — zero it and enqueue. If light >= old level,
it has independent light — save as boundary source.

**Removal phase 2.** Build an overlay that returns zeroed values for all
positions cleared in phase 1 (without requiring the caller to apply them first).
BFS re-propagate from all boundary sources against this overlay.

**Output.** Flat `Vec<LightUpdate>`. Zeroing updates appear first, then
re-propagation values. Apply in order.

<br>

## Prior art

The algorithm is standard (Seed of Andromeda documented it in 2015). These
projects implement it by hand:

- [Luanti/Minetest](https://github.com/luanti-org/luanti/blob/master/src/voxelalgorithms.cpp)
  — `voxelalgorithms.cpp`, ~500 LOC, C++
- [Seed of Andromeda](https://github.com/RegrowthStudios/SoACode-Public)
  — three BFS passes per RGB channel, C++
- [PocketMine-MP](https://github.com/dktapps/lighting-algorithm-spec)
  — standalone specification document for the algorithm
- Rust voxel engines (Rezcraft, Aern-do/voxel, Technici4n/voxel-rs)
  — each contains a bespoke implementation

No existing crate on crates.io provides this. `block-mesh` handles meshing.
`building-blocks` handles storage. `tapestry` is 2D. `pathfinding` solves
graph traversal without light-decay or removal semantics.

<br>

## Known limitations

- **Not a spatial data structure.** Queries your storage via the trait and
  returns updates. Does not store light values.

- **Visited map allocated per call.** The `AHashMap` is not pooled across
  calls. Typical cost: ~11k entries for level-14 propagation.

- **Colored removal over-zeroes.** A voxel is considered dependent if any
  channel is less than the corresponding old level. Edge cases with overlapping
  colored sources of different ranges may zero more than necessary.

- **No sky light removal.** The `sky` feature provides propagation only.
  Placing a roof requires the caller to recalculate affected columns.

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
