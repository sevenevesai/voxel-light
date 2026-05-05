use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::traits::{VoxelAccess, VoxelInfo};
use crate::types::{LightNode, DIRECTIONS};

#[cfg(feature = "std")]
type VisitedMap = ahash::AHashMap<(i32, i32, i32), u8>;

#[cfg(not(feature = "std"))]
type VisitedMap = alloc::collections::BTreeMap<(i32, i32, i32), u8>;

/// Extended trait for sky light propagation.
pub trait SkyAccess: VoxelAccess {
    /// The Y coordinate where sky starts (topmost world boundary).
    fn sky_height(&self) -> i32;

    /// The Y coordinate of the lowest world boundary.
    fn ground_height(&self) -> i32;
}

/// Sky light update with the sky light channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkyLightUpdate {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub level: u8,
}

/// Propagate sky light downward through a column, then spread horizontally.
///
/// Sky light starts at level 15 at `sky_height` and travels downward without
/// decay until hitting an opaque block. Horizontal spread from sky-lit columns
/// decays by 1 per block (same as block light BFS).
pub fn propagate_sky_column(access: &impl SkyAccess, x: i32, z: i32) -> Vec<SkyLightUpdate> {
    let sky_y = access.sky_height();
    let ground_y = access.ground_height();
    let mut updates = Vec::new();
    let mut horizontal_seeds: Vec<LightNode> = Vec::new();

    // Phase 1: Downward propagation (no decay while unobstructed)
    let mut y = sky_y;
    while y >= ground_y {
        match access.get_voxel(x, y, z) {
            Some(VoxelInfo { transparent: true, .. }) => {
                updates.push(SkyLightUpdate { x, y, z, level: 15 });
                horizontal_seeds.push(LightNode { x, y, z, level: 15 });
            }
            Some(VoxelInfo { transparent: false, .. }) => {
                break;
            }
            None => {
                break;
            }
        }
        y -= 1;
    }

    // Phase 2: Horizontal spread from sky-lit positions (with decay)
    let mut queue = VecDeque::with_capacity(256);
    let mut visited = VisitedMap::default();

    for seed in &horizontal_seeds {
        visited.insert((seed.x, seed.y, seed.z), 15);
    }

    for seed in horizontal_seeds {
        queue.push_back(seed);
    }

    while let Some(node) = queue.pop_front() {
        let next_level = node.level.saturating_sub(1);
        if next_level == 0 {
            continue;
        }

        // Only horizontal spread (skip Y directions)
        const HORIZONTAL: [[i32; 3]; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

        for dir in HORIZONTAL {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if let Some(&existing) = visited.get(&(nx, ny, nz)) {
                if existing >= next_level {
                    continue;
                }
            }

            let Some(voxel) = access.get_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            visited.insert((nx, ny, nz), next_level);
            updates.push(SkyLightUpdate { x: nx, y: ny, z: nz, level: next_level });
            queue.push_back(LightNode { x: nx, y: ny, z: nz, level: next_level });
        }
    }

    updates
}

/// Propagate sky light for an area (multiple columns).
pub fn propagate_sky_area(
    access: &impl SkyAccess,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
) -> Vec<SkyLightUpdate> {
    let sky_y = access.sky_height();
    let ground_y = access.ground_height();
    let mut updates = Vec::new();
    let mut queue = VecDeque::with_capacity(1024);
    let mut visited = VisitedMap::default();

    // Phase 1: Downward propagation for all columns
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            let mut y = sky_y;
            while y >= ground_y {
                match access.get_voxel(x, y, z) {
                    Some(VoxelInfo { transparent: true, .. }) => {
                        updates.push(SkyLightUpdate { x, y, z, level: 15 });
                        visited.insert((x, y, z), 15);
                        queue.push_back(LightNode { x, y, z, level: 15 });
                    }
                    _ => break,
                }
                y -= 1;
            }
        }
    }

    // Phase 2: Horizontal BFS spread from all sky-lit positions
    while let Some(node) = queue.pop_front() {
        let next_level = node.level.saturating_sub(1);
        if next_level == 0 {
            continue;
        }

        for dir in DIRECTIONS {
            // Skip downward — sky light only goes down in columns, not via BFS
            if dir == [0, -1, 0] {
                continue;
            }

            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if let Some(&existing) = visited.get(&(nx, ny, nz)) {
                if existing >= next_level {
                    continue;
                }
            }

            let Some(voxel) = access.get_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            visited.insert((nx, ny, nz), next_level);
            updates.push(SkyLightUpdate { x: nx, y: ny, z: nz, level: next_level });
            queue.push_back(LightNode { x: nx, y: ny, z: nz, level: next_level });
        }
    }

    updates
}
