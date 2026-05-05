# voxel-light

Generic BFS flood-fill light propagation with two-phase removal for voxel engines. Trait-generic over any storage that implements `get_voxel(x, y, z) -> Option<VoxelInfo>`. Returns `Vec<LightUpdate>` — never mutates caller data.

Published on crates.io. GitHub: sevenevesai/voxel-light

## Architecture

Seven files, ~950 LOC library + 670 LOC tests:

- `src/lib.rs` (82 LOC) -- `LightEngine` struct with buffer reuse, `recalculate_area`, `ClearedOverlay` helper. Re-exports public API.
- `src/traits.rs` (24 LOC) -- `VoxelAccess` trait and `VoxelInfo` struct. Single method: `get_voxel(x, y, z) -> Option<VoxelInfo>`.
- `src/types.rs` (48 LOC) -- `LightUpdate`, internal `LightNode`/`RemovalNode`, `DIRECTIONS` constant.
- `src/propagate.rs` (112 LOC) -- BFS propagation with visited map. `propagate()` free function and `propagate_reuse()` for buffer sharing.
- `src/remove.rs` (232 LOC) -- Two-phase removal. Phase 1: zero dependents, collect boundaries. Phase 2: `OverlayAccess` presents zeroed state, BFS re-propagates from boundaries.
- `src/sky.rs` (179 LOC) -- Feature-gated. Column propagation (no decay downward), then horizontal BFS spread (decays by 1).
- `src/colored.rs` (273 LOC) -- Feature-gated. RGB per-channel propagation and removal in a single BFS pass.

Key types:
- `VoxelInfo` -- `{ transparent: bool, block_light: u8, emission: u8 }`. Everything the algorithm needs per position.
- `LightUpdate` -- `{ x: i32, y: i32, z: i32, level: u8 }`. One pending write.
- `LightEngine` -- Owns `VecDeque` buffers for reuse across calls. Stateless between calls.
- `OverlayAccess` (internal) -- Wraps a `VoxelAccess` and overlays zeroed values from removal phase 1 so phase 2 sees correct state.

## Core algorithm

**Propagation**: Standard BFS. Queue seeded with source at emission level. Each step decays by 1. Skip if: already visited at >= level, opaque, `None`, or current stored light already brighter.

**Removal** (the hard part):
1. BFS from source. Neighbor light < old_level → dependent (zero it, enqueue). Neighbor light >= old_level → boundary source (save for re-propagation).
2. Build overlay of zeroed positions. BFS from all boundary sources against the overlay. This correctly fills gaps without requiring the caller to apply zeroes first.

The overlay trick is the key insight: re-propagation must see the "world after removal" but before the caller has written anything.

## Feature gates

- `std` (default): enables `ahash::AHashMap` for the visited map. ~2x faster than `BTreeMap`.
- `sky`: `SkyAccess` trait, `propagate_sky_column`, `propagate_sky_area`.
- `colored`: `ColoredVoxelAccess` trait, `propagate_colored`, `remove_colored`.

## no_std support

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`. Without `std`, visited maps use `alloc::collections::BTreeMap`. Queue uses `alloc::collections::VecDeque`.

## Integration pattern (Mosaic)

Mosaic wraps this crate with a thin adapter in `src/world/lighting.rs`:
- `ChunkMapAccess` implements `VoxelAccess` over `AHashMap<ChunkPos, Arc<Chunk>>`
- `convert_updates()` groups flat `LightUpdate` list into Mosaic's per-chunk format
- `apply_light_updates()` does copy-on-write chunk replacement (`Arc<Chunk>` → clone → mutate → reinsert)

## Known gaps

- Visited map allocated per call (not pooled). Fine for typical torch operations (~11k entries for level 14).
- Colored removal over-zeroes when channels have different ranges from overlapping sources.
- Sky light removal not implemented (caller must recalculate columns when roof placed).
- `recalculate_area` is O(radius^3) scan — use sparingly, prefer targeted `block_placed` for single opaque blocks.

## Platform

- Windows (S: drive), should work on any platform
- Rust edition 2021, stable toolchain
- Only dependency: `ahash 0.8` (optional, default)
