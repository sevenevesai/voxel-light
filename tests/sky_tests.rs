#![cfg(feature = "sky")]

use std::collections::HashMap;
use voxel_light::sky::{propagate_sky_area, propagate_sky_column, SkyAccess, SkyLightUpdate};
use voxel_light::{VoxelAccess, VoxelInfo};

struct MockSkyWorld {
    blocks: HashMap<(i32, i32, i32), VoxelInfo>,
    sky_height: i32,
    ground_height: i32,
}

impl MockSkyWorld {
    fn new(sky_height: i32, ground_height: i32) -> Self {
        let mut blocks = HashMap::new();
        // Fill with air
        for x in -16..=16 {
            for z in -16..=16 {
                for y in ground_height..=sky_height {
                    blocks.insert((x, y, z), VoxelInfo {
                        transparent: true,
                        block_light: 0,
                        emission: 0,
                    });
                }
            }
        }
        Self { blocks, sky_height, ground_height }
    }

    fn set_solid(&mut self, x: i32, y: i32, z: i32) {
        self.blocks.insert((x, y, z), VoxelInfo {
            transparent: false,
            block_light: 0,
            emission: 0,
        });
    }

    fn find_update(updates: &[SkyLightUpdate], x: i32, y: i32, z: i32) -> Option<u8> {
        updates.iter()
            .filter(|u| u.x == x && u.y == y && u.z == z)
            .last()
            .map(|u| u.level)
    }
}

impl VoxelAccess for MockSkyWorld {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        self.blocks.get(&(x, y, z)).copied()
    }
}

impl SkyAccess for MockSkyWorld {
    fn sky_height(&self) -> i32 { self.sky_height }
    fn ground_height(&self) -> i32 { self.ground_height }
}

#[test]
fn sky_column_open() {
    let world = MockSkyWorld::new(10, 0);
    let updates = propagate_sky_column(&world, 0, 0);

    // All blocks in the column from sky to ground should get level 15
    for y in 0..=10 {
        let level = MockSkyWorld::find_update(&updates, 0, y, 0);
        assert_eq!(level, Some(15), "y={y} should be sky-lit");
    }
}

#[test]
fn sky_column_blocked_by_roof() {
    let mut world = MockSkyWorld::new(10, 0);
    world.set_solid(0, 5, 0);

    let updates = propagate_sky_column(&world, 0, 0);

    // Above roof: sky lit
    assert_eq!(MockSkyWorld::find_update(&updates, 0, 10, 0), Some(15));
    assert_eq!(MockSkyWorld::find_update(&updates, 0, 6, 0), Some(15));

    // At roof: no update (opaque)
    assert!(MockSkyWorld::find_update(&updates, 0, 5, 0).is_none());

    // Below roof: no sky light from this column
    assert!(MockSkyWorld::find_update(&updates, 0, 4, 0).is_none());
}

#[test]
fn sky_horizontal_spread() {
    let mut world = MockSkyWorld::new(10, 0);
    // Create a partial roof: solid at y=5 only at x=0
    world.set_solid(0, 5, 0);

    // Propagate column at x=1 (open) — it should spread horizontally to x=0 below the roof
    let updates = propagate_sky_column(&world, 1, 0);

    // x=1 column is fully lit
    assert_eq!(MockSkyWorld::find_update(&updates, 1, 4, 0), Some(15));

    // Horizontal spread to x=0 at y=4 (below the roof) should give 14
    assert_eq!(MockSkyWorld::find_update(&updates, 0, 4, 0), Some(14));
}

#[test]
fn sky_area_propagation() {
    let world = MockSkyWorld::new(5, 0);
    let updates = propagate_sky_area(&world, -1, 1, -1, 1);

    // All columns in the 3x3 area should be fully sky-lit
    for x in -1..=1 {
        for z in -1..=1 {
            for y in 0..=5 {
                let level = MockSkyWorld::find_update(&updates, x, y, z);
                assert_eq!(level, Some(15), "({x},{y},{z}) should be 15");
            }
        }
    }
}
