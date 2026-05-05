use std::collections::HashMap;
use voxel_light::{propagate, remove, LightEngine, LightUpdate, VoxelAccess, VoxelInfo};

/// A mock world for testing. Stores blocks and light levels.
struct MockWorld {
    blocks: HashMap<(i32, i32, i32), VoxelInfo>,
}

impl MockWorld {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    fn air_world(radius: i32) -> Self {
        let mut world = Self::new();
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    world.set(x, y, z, VoxelInfo {
                        transparent: true,
                        block_light: 0,
                        emission: 0,
                    });
                }
            }
        }
        world
    }

    fn set(&mut self, x: i32, y: i32, z: i32, info: VoxelInfo) {
        self.blocks.insert((x, y, z), info);
    }

    fn set_solid(&mut self, x: i32, y: i32, z: i32) {
        self.set(x, y, z, VoxelInfo {
            transparent: false,
            block_light: 0,
            emission: 0,
        });
    }

    fn apply_updates(&mut self, updates: &[LightUpdate]) {
        for u in updates {
            if let Some(voxel) = self.blocks.get_mut(&(u.x, u.y, u.z)) {
                voxel.block_light = u.level;
            }
        }
    }

    fn get_light(&self, x: i32, y: i32, z: i32) -> u8 {
        self.blocks
            .get(&(x, y, z))
            .map(|v| v.block_light)
            .unwrap_or(0)
    }
}

impl VoxelAccess for MockWorld {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        self.blocks.get(&(x, y, z)).copied()
    }
}

// ============== Propagation Tests ==============

#[test]
fn propagate_in_empty_world() {
    let world = MockWorld::air_world(16);
    let updates = propagate(&world, [0, 0, 0], 14);

    // Source gets level 14
    let source_update = updates.iter().find(|u| u.pos() == [0, 0, 0]).unwrap();
    assert_eq!(source_update.level, 14);

    // Direct neighbor gets level 13
    let neighbor = updates.iter().find(|u| u.pos() == [1, 0, 0]).unwrap();
    assert_eq!(neighbor.level, 13);

    // 13 blocks away gets level 1
    let far = updates.iter().find(|u| u.pos() == [13, 0, 0]).unwrap();
    assert_eq!(far.level, 1);

    // 14 blocks away gets nothing (level would be 0)
    assert!(updates.iter().find(|u| u.pos() == [14, 0, 0]).is_none());
}

#[test]
fn propagate_blocked_by_opaque() {
    let mut world = MockWorld::air_world(16);
    // Place a wall at x=2
    for y in -15..=15 {
        for z in -15..=15 {
            world.set_solid(2, y, z);
        }
    }

    let updates = propagate(&world, [0, 0, 0], 14);

    // x=1 should get light (level 13)
    let before_wall = updates.iter().find(|u| u.pos() == [1, 0, 0]).unwrap();
    assert_eq!(before_wall.level, 13);

    // x=2 (opaque) should not be in updates
    assert!(updates.iter().find(|u| u.x == 2 && u.y == 0 && u.z == 0).is_none());

    // x=3 should not get light from this direction (wall blocks it)
    // But it might get light going around if the world is large enough
    // In a 16-radius world with a full wall, x=3 should not be reachable
    assert!(updates.iter().find(|u| u.pos() == [3, 0, 0]).is_none());
}

#[test]
fn propagate_stops_at_world_boundary() {
    // Small world: only 3 blocks
    let mut world = MockWorld::new();
    world.set(0, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 14 });
    world.set(1, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 0 });
    world.set(2, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 0 });

    let updates = propagate(&world, [0, 0, 0], 14);

    // Source and its one reachable neighbor
    assert!(updates.iter().any(|u| u.pos() == [0, 0, 0] && u.level == 14));
    assert!(updates.iter().any(|u| u.pos() == [1, 0, 0] && u.level == 13));
    assert!(updates.iter().any(|u| u.pos() == [2, 0, 0] && u.level == 12));
    // Beyond the known world — not reached
    assert!(updates.iter().all(|u| u.pos() != [3, 0, 0]));
}

#[test]
fn propagate_does_not_overwrite_brighter() {
    let mut world = MockWorld::air_world(16);
    // Position (3, 0, 0) already has light level 14
    world.set(3, 0, 0, VoxelInfo { transparent: true, block_light: 14, emission: 0 });

    let updates = propagate(&world, [0, 0, 0], 14);

    // (3, 0, 0) should not be in updates because it already has light 14 > 11 (14-3)
    let at_3 = updates.iter().find(|u| u.pos() == [3, 0, 0]);
    assert!(at_3.is_none());
}

#[test]
fn propagate_level_zero_returns_empty() {
    let world = MockWorld::air_world(5);
    let updates = propagate(&world, [0, 0, 0], 0);
    assert!(updates.is_empty());
}

#[test]
fn propagate_diamond_shape() {
    let world = MockWorld::air_world(5);
    let updates = propagate(&world, [0, 0, 0], 3);

    // Level 3 only reaches manhattan distance 2 (levels: 3, 2, 1)
    // At manhattan distance 3, level would be 0 — not included
    for u in &updates {
        let dist = u.x.unsigned_abs() + u.y.unsigned_abs() + u.z.unsigned_abs();
        assert!(dist <= 2, "update at distance {dist} with level {}", u.level);
        assert_eq!(u.level, 3 - dist as u8);
    }
}

// ============== Removal Tests ==============

#[test]
fn remove_single_source() {
    let mut world = MockWorld::air_world(16);

    // Place a torch and propagate
    let updates = propagate(&world, [0, 0, 0], 14);
    world.apply_updates(&updates);

    // Now remove it
    let removal_updates = remove(&world, [0, 0, 0]);

    // All positions should be zeroed (no other light sources exist)
    assert!(!removal_updates.is_empty());

    // Apply removals and check everything is dark
    world.apply_updates(&removal_updates);
    assert_eq!(world.get_light(0, 0, 0), 0);
    assert_eq!(world.get_light(1, 0, 0), 0);
    assert_eq!(world.get_light(5, 0, 0), 0);
    assert_eq!(world.get_light(13, 0, 0), 0);
}

#[test]
fn remove_with_surviving_source() {
    let mut world = MockWorld::air_world(20);

    // Place two torches: one at (0,0,0) and one at (10,0,0), both level 14
    let updates1 = propagate(&world, [0, 0, 0], 14);
    world.apply_updates(&updates1);
    let updates2 = propagate(&world, [10, 0, 0], 14);
    world.apply_updates(&updates2);

    // Mark them as emitters for the removal algorithm
    world.set(0, 0, 0, VoxelInfo { transparent: true, block_light: 14, emission: 14 });
    world.set(10, 0, 0, VoxelInfo { transparent: true, block_light: 14, emission: 14 });

    // Remove the torch at (0,0,0)
    let removal_updates = remove(&world, [0, 0, 0]);
    world.apply_updates(&removal_updates);

    // The torch at (10,0,0) should still be 14
    assert_eq!(world.get_light(10, 0, 0), 14);

    // Its neighbors should still be lit
    assert_eq!(world.get_light(9, 0, 0), 13);
    assert_eq!(world.get_light(11, 0, 0), 13);

    // The removed position should be 0 (it was the source, not transparent to the second torch)
    // Actually it IS transparent, so it should get light from the second torch
    // Distance from (10,0,0) to (0,0,0) is 10, so level = 14 - 10 = 4
    assert_eq!(world.get_light(0, 0, 0), 4);
}

#[test]
fn remove_adjacent_to_another_emitter() {
    let mut world = MockWorld::air_world(16);

    // Two torches next to each other
    world.set(0, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 14 });
    world.set(1, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 14 });

    // Propagate both
    let u1 = propagate(&world, [0, 0, 0], 14);
    world.apply_updates(&u1);
    let u2 = propagate(&world, [1, 0, 0], 14);
    world.apply_updates(&u2);

    // Remove the one at (0,0,0)
    let removal_updates = remove(&world, [0, 0, 0]);
    world.apply_updates(&removal_updates);

    // (1,0,0) should still be 14 (it's an emitter)
    assert_eq!(world.get_light(1, 0, 0), 14);

    // (0,0,0) should get 13 from the remaining torch at (1,0,0)
    assert_eq!(world.get_light(0, 0, 0), 13);

    // (-1,0,0) should get 12
    assert_eq!(world.get_light(-1, 0, 0), 12);
}

// ============== Engine Tests ==============

#[test]
fn engine_place_and_remove() {
    let mut world = MockWorld::air_world(16);
    let mut engine = LightEngine::new();

    let updates = engine.place_light(&world, [0, 0, 0], 14);
    world.apply_updates(&updates);
    assert_eq!(world.get_light(0, 0, 0), 14);
    assert_eq!(world.get_light(5, 0, 0), 9);

    world.set(0, 0, 0, VoxelInfo { transparent: true, block_light: 14, emission: 14 });
    let removal = engine.remove_light(&world, [0, 0, 0]);
    world.apply_updates(&removal);
    assert_eq!(world.get_light(0, 0, 0), 0);
    assert_eq!(world.get_light(5, 0, 0), 0);
}

#[test]
fn engine_block_removed_fills_gap() {
    let mut world = MockWorld::air_world(16);
    let mut engine = LightEngine::new();

    // Light at origin
    let updates = engine.place_light(&world, [0, 0, 0], 14);
    world.apply_updates(&updates);

    // Place opaque block at (3, 0, 0) — but first, simulate it being there
    // by reducing light behind it to 0
    world.set_solid(3, 0, 0);

    // Remove the solid block — light should flow in from neighbors
    world.set(3, 0, 0, VoxelInfo { transparent: true, block_light: 0, emission: 0 });
    let fill_updates = engine.block_removed(&world, [3, 0, 0]);
    world.apply_updates(&fill_updates);

    // (3, 0, 0) should now have light = max_neighbor - 1
    // Neighbors at (2,0,0) have 12, so (3,0,0) should get 11
    assert_eq!(world.get_light(3, 0, 0), 11);
}

// ============== Cross-boundary Tests ==============

/// Simulates a chunked world where chunks are 16x16x16 and boundaries are handled
/// by the VoxelAccess implementation mapping world coords to chunk+local coords.
struct ChunkedWorld {
    /// Store voxels by chunk position and local position
    chunks: HashMap<(i32, i32, i32), HashMap<(usize, usize, usize), VoxelInfo>>,
}

impl ChunkedWorld {
    fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    fn fill_chunk(&mut self, cx: i32, cy: i32, cz: i32) {
        let mut local = HashMap::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    local.insert((x, y, z), VoxelInfo {
                        transparent: true,
                        block_light: 0,
                        emission: 0,
                    });
                }
            }
        }
        self.chunks.insert((cx, cy, cz), local);
    }

    fn set_world(&mut self, wx: i32, wy: i32, wz: i32, info: VoxelInfo) {
        let cx = wx.div_euclid(16);
        let cy = wy.div_euclid(16);
        let cz = wz.div_euclid(16);
        let lx = wx.rem_euclid(16) as usize;
        let ly = wy.rem_euclid(16) as usize;
        let lz = wz.rem_euclid(16) as usize;
        if let Some(chunk) = self.chunks.get_mut(&(cx, cy, cz)) {
            chunk.insert((lx, ly, lz), info);
        }
    }

    fn apply_updates(&mut self, updates: &[LightUpdate]) {
        for u in updates {
            let cx = u.x.div_euclid(16);
            let cy = u.y.div_euclid(16);
            let cz = u.z.div_euclid(16);
            let lx = u.x.rem_euclid(16) as usize;
            let ly = u.y.rem_euclid(16) as usize;
            let lz = u.z.rem_euclid(16) as usize;
            if let Some(chunk) = self.chunks.get_mut(&(cx, cy, cz)) {
                if let Some(voxel) = chunk.get_mut(&(lx, ly, lz)) {
                    voxel.block_light = u.level;
                }
            }
        }
    }

    fn get_light(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        let cx = wx.div_euclid(16);
        let cy = wy.div_euclid(16);
        let cz = wz.div_euclid(16);
        let lx = wx.rem_euclid(16) as usize;
        let ly = wy.rem_euclid(16) as usize;
        let lz = wz.rem_euclid(16) as usize;
        self.chunks
            .get(&(cx, cy, cz))
            .and_then(|c| c.get(&(lx, ly, lz)))
            .map(|v| v.block_light)
            .unwrap_or(0)
    }
}

impl VoxelAccess for ChunkedWorld {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        let cx = x.div_euclid(16);
        let cy = y.div_euclid(16);
        let cz = z.div_euclid(16);
        let lx = x.rem_euclid(16) as usize;
        let ly = y.rem_euclid(16) as usize;
        let lz = z.rem_euclid(16) as usize;
        self.chunks.get(&(cx, cy, cz))?.get(&(lx, ly, lz)).copied()
    }
}

#[test]
fn cross_chunk_propagation() {
    let mut world = ChunkedWorld::new();
    world.fill_chunk(0, 0, 0);
    world.fill_chunk(1, 0, 0); // +X neighbor chunk

    // Place light near chunk boundary: world x=15 is the last block in chunk (0,0,0)
    let updates = propagate(&world, [15, 8, 8], 14);
    world.apply_updates(&updates);

    // Light should cross into chunk (1,0,0): world x=16 is chunk(1,0,0) local x=0
    assert_eq!(world.get_light(16, 8, 8), 13);
    assert_eq!(world.get_light(17, 8, 8), 12);
}

#[test]
fn cross_chunk_negative_boundary() {
    let mut world = ChunkedWorld::new();
    world.fill_chunk(0, 0, 0);
    world.fill_chunk(-1, 0, 0); // -X neighbor chunk

    // Light at world x=0 (first block in chunk 0)
    let updates = propagate(&world, [0, 8, 8], 14);
    world.apply_updates(&updates);

    // Should cross into chunk (-1,0,0): world x=-1 is chunk(-1,0,0) local x=15
    assert_eq!(world.get_light(-1, 8, 8), 13);
    assert_eq!(world.get_light(-2, 8, 8), 12);
}

#[test]
fn missing_neighbor_chunk_stops_propagation() {
    let mut world = ChunkedWorld::new();
    world.fill_chunk(0, 0, 0);
    // No chunk at (1, 0, 0)

    let updates = propagate(&world, [15, 8, 8], 14);
    world.apply_updates(&updates);

    // x=15 is lit (inside chunk 0)
    assert_eq!(world.get_light(15, 8, 8), 14);

    // x=16 is in a non-existent chunk — should not be in updates
    assert!(updates.iter().all(|u| u.x < 16));
}

#[test]
fn cross_chunk_removal() {
    let mut world = ChunkedWorld::new();
    world.fill_chunk(0, 0, 0);
    world.fill_chunk(1, 0, 0);

    // Place and propagate
    let updates = propagate(&world, [15, 8, 8], 14);
    world.apply_updates(&updates);
    world.set_world(15, 8, 8, VoxelInfo { transparent: true, block_light: 14, emission: 14 });

    // Remove the light
    let removal = remove(&world, [15, 8, 8]);
    world.apply_updates(&removal);

    // Everything should be dark
    assert_eq!(world.get_light(15, 8, 8), 0);
    assert_eq!(world.get_light(16, 8, 8), 0);
    assert_eq!(world.get_light(14, 8, 8), 0);
}
