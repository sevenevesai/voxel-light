use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::traits::VoxelAccess;
use crate::types::{LightNode, LightUpdate, DIRECTIONS};

#[cfg(feature = "std")]
type VisitedMap = ahash::AHashMap<(i32, i32, i32), u8>;

#[cfg(not(feature = "std"))]
type VisitedMap = alloc::collections::BTreeMap<(i32, i32, i32), u8>;

/// Propagate light from a source position using BFS flood fill.
///
/// Returns a list of updates representing the new light levels. The source
/// position itself is included in the results. Light decays by 1 per block
/// traveled and stops at opaque voxels or world boundaries.
pub fn propagate(access: &impl VoxelAccess, source: [i32; 3], level: u8) -> Vec<LightUpdate> {
    let mut queue = VecDeque::with_capacity(512);
    propagate_reuse(access, source, level, &mut queue)
}

pub(crate) fn propagate_reuse(
    access: &impl VoxelAccess,
    source: [i32; 3],
    level: u8,
    queue: &mut VecDeque<LightNode>,
) -> Vec<LightUpdate> {
    if level == 0 {
        return Vec::new();
    }

    let mut visited = VisitedMap::default();
    let mut updates = Vec::new();

    queue.clear();
    queue.push_back(LightNode {
        x: source[0],
        y: source[1],
        z: source[2],
        level,
    });
    visited.insert((source[0], source[1], source[2]), level);
    updates.push(LightUpdate::new(source, level));

    while let Some(node) = queue.pop_front() {
        let next_level = node.level.saturating_sub(1);
        if next_level == 0 {
            continue;
        }

        for dir in DIRECTIONS {
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

            if next_level > voxel.block_light {
                visited.insert((nx, ny, nz), next_level);
                updates.push(LightUpdate::new([nx, ny, nz], next_level));
                queue.push_back(LightNode {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: next_level,
                });
            }
        }
    }

    updates
}

/// Handle a transparent block being revealed (broken/removed).
/// Checks neighbors for light sources and propagates inward.
pub(crate) fn block_removed(
    access: &impl VoxelAccess,
    pos: [i32; 3],
    queue: &mut VecDeque<LightNode>,
) -> Vec<LightUpdate> {
    let mut max_neighbor_light: u8 = 0;

    for dir in DIRECTIONS {
        let nx = pos[0] + dir[0];
        let ny = pos[1] + dir[1];
        let nz = pos[2] + dir[2];

        if let Some(voxel) = access.get_voxel(nx, ny, nz) {
            max_neighbor_light = max_neighbor_light.max(voxel.block_light);
        }
    }

    if max_neighbor_light <= 1 {
        return Vec::new();
    }

    let fill_level = max_neighbor_light - 1;
    propagate_reuse(access, pos, fill_level, queue)
}
