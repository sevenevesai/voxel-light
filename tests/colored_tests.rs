#![cfg(feature = "colored")]

use std::collections::HashMap;
use voxel_light::colored::{
    propagate_colored, remove_colored, ColoredLightUpdate, ColoredVoxelAccess, ColoredVoxelInfo,
};

struct MockColorWorld {
    blocks: HashMap<(i32, i32, i32), ColoredVoxelInfo>,
}

impl MockColorWorld {
    fn air_world(radius: i32) -> Self {
        let mut blocks = HashMap::new();
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    blocks.insert((x, y, z), ColoredVoxelInfo {
                        transparent: true,
                        light: [0, 0, 0],
                        emission: [0, 0, 0],
                    });
                }
            }
        }
        Self { blocks }
    }

    fn apply_updates(&mut self, updates: &[ColoredLightUpdate]) {
        for u in updates {
            if let Some(voxel) = self.blocks.get_mut(&(u.x, u.y, u.z)) {
                voxel.light = u.rgb;
            }
        }
    }

    fn get_light(&self, x: i32, y: i32, z: i32) -> [u8; 3] {
        self.blocks.get(&(x, y, z)).map(|v| v.light).unwrap_or([0, 0, 0])
    }
}

impl ColoredVoxelAccess for MockColorWorld {
    fn get_colored_voxel(&self, x: i32, y: i32, z: i32) -> Option<ColoredVoxelInfo> {
        self.blocks.get(&(x, y, z)).copied()
    }
}

#[test]
fn red_torch_only_emits_red() {
    let world = MockColorWorld::air_world(16);
    let updates = propagate_colored(&world, [0, 0, 0], [14, 0, 0]);

    // Neighbor should have red=13, green=0, blue=0
    let neighbor = updates.iter().find(|u| u.x == 1 && u.y == 0 && u.z == 0).unwrap();
    assert_eq!(neighbor.rgb, [13, 0, 0]);
}

#[test]
fn mixed_emission() {
    let world = MockColorWorld::air_world(16);
    let updates = propagate_colored(&world, [0, 0, 0], [14, 7, 3]);

    let at_1 = updates.iter().find(|u| u.x == 1 && u.y == 0 && u.z == 0).unwrap();
    assert_eq!(at_1.rgb, [13, 6, 2]);

    let at_2 = updates.iter().find(|u| u.x == 2 && u.y == 0 && u.z == 0).unwrap();
    assert_eq!(at_2.rgb, [12, 5, 1]);

    // At distance 3, blue channel would be 0 so the update should only have [11, 4, 0]
    let at_3 = updates.iter().find(|u| u.x == 3 && u.y == 0 && u.z == 0).unwrap();
    assert_eq!(at_3.rgb, [11, 4, 0]);
}

#[test]
fn overlapping_colored_sources() {
    let mut world = MockColorWorld::air_world(16);

    // Red torch at (-5, 0, 0)
    let u1 = propagate_colored(&world, [-5, 0, 0], [14, 0, 0]);
    world.apply_updates(&u1);

    // Blue torch at (5, 0, 0)
    let u2 = propagate_colored(&world, [5, 0, 0], [0, 0, 14]);
    world.apply_updates(&u2);

    // At origin (equidistant from both): red=9, blue=9
    let center = world.get_light(0, 0, 0);
    assert_eq!(center, [9, 0, 9]);
}

#[test]
fn colored_removal() {
    let mut world = MockColorWorld::air_world(16);

    // Place and propagate a green torch
    let updates = propagate_colored(&world, [0, 0, 0], [0, 14, 0]);
    world.apply_updates(&updates);
    world.blocks.get_mut(&(0, 0, 0)).unwrap().emission = [0, 14, 0];

    // Remove it
    let removal = remove_colored(&world, [0, 0, 0]);
    world.apply_updates(&removal);

    // Everything should be dark
    assert_eq!(world.get_light(0, 0, 0), [0, 0, 0]);
    assert_eq!(world.get_light(1, 0, 0), [0, 0, 0]);
    assert_eq!(world.get_light(5, 0, 0), [0, 0, 0]);
}
