use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::traits::VoxelAccess;
use crate::types::{LightNode, LightUpdate, RemovalNode, DIRECTIONS};

#[cfg(feature = "std")]
type HashMap<K, V> = ahash::AHashMap<K, V>;

#[cfg(not(feature = "std"))]
type HashMap<K, V> = alloc::collections::BTreeMap<K, V>;

/// Remove light from a source position using two-phase BFS.
///
/// Phase 1: BFS from the source, zeroing all voxels whose light depended on
/// it (neighbor_light < node.old_level). Voxels with light >= old_level are
/// collected as "boundary sources" — they have independent light.
///
/// Phase 2: Re-propagate from all boundary sources and any emissive voxels
/// found during removal. This fills gaps correctly.
///
/// Returns a flat list of updates. Apply them in order — zeroing first, then
/// re-propagation values will overwrite where appropriate.
pub fn remove(access: &impl VoxelAccess, source: [i32; 3]) -> Vec<LightUpdate> {
    let mut removal_queue = VecDeque::with_capacity(512);
    let mut propagation_queue = VecDeque::with_capacity(512);
    remove_reuse(access, source, &mut removal_queue, &mut propagation_queue)
}

pub(crate) fn remove_reuse(
    access: &impl VoxelAccess,
    source: [i32; 3],
    removal_queue: &mut VecDeque<RemovalNode>,
    propagation_queue: &mut VecDeque<LightNode>,
) -> Vec<LightUpdate> {
    let Some(source_voxel) = access.get_voxel(source[0], source[1], source[2]) else {
        return Vec::new();
    };

    let old_level = source_voxel.block_light;
    if old_level == 0 {
        return Vec::new();
    }

    let mut visited: HashMap<(i32, i32, i32), bool> = HashMap::default();
    let mut updates: Vec<LightUpdate> = Vec::new();
    let mut relight_sources: Vec<LightNode> = Vec::new();

    // Seed removal with the source
    removal_queue.clear();
    removal_queue.push_back(RemovalNode {
        x: source[0],
        y: source[1],
        z: source[2],
        old_level,
    });
    visited.insert((source[0], source[1], source[2]), true);
    updates.push(LightUpdate::new(source, 0));

    // Phase 1: BFS removal — zero dependents, collect boundary sources
    while let Some(node) = removal_queue.pop_front() {
        for dir in DIRECTIONS {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if visited.contains_key(&(nx, ny, nz)) {
                continue;
            }

            let Some(voxel) = access.get_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            let neighbor_light = voxel.block_light;

            if neighbor_light != 0 && neighbor_light < node.old_level {
                // Dependent on the removed source — zero it
                visited.insert((nx, ny, nz), true);
                updates.push(LightUpdate::new([nx, ny, nz], 0));

                removal_queue.push_back(RemovalNode {
                    x: nx,
                    y: ny,
                    z: nz,
                    old_level: neighbor_light,
                });
            } else if neighbor_light >= node.old_level {
                // Independent source — boundary for re-propagation
                visited.insert((nx, ny, nz), true);
                relight_sources.push(LightNode {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: neighbor_light,
                });
            }
        }
    }

    // Also check any emissive voxels that were zeroed — they should re-propagate
    // (handles the case where an emitter is adjacent to the removed source)
    for update in &updates {
        if update.level == 0 {
            if let Some(voxel) = access.get_voxel(update.x, update.y, update.z) {
                if voxel.emission > 0 && (update.x != source[0] || update.y != source[1] || update.z != source[2]) {
                    relight_sources.push(LightNode {
                        x: update.x,
                        y: update.y,
                        z: update.z,
                        level: voxel.emission,
                    });
                }
            }
        }
    }

    // Phase 2: Re-propagate from all boundary sources
    // We build an overlay of the zeroed state so re-propagation sees correct values
    let zeroed: HashMap<(i32, i32, i32), u8> = updates
        .iter()
        .map(|u| ((u.x, u.y, u.z), 0u8))
        .collect();

    let overlay = OverlayAccess {
        inner: access,
        overlay: &zeroed,
    };

    propagation_queue.clear();

    // Seed all boundary sources into the propagation queue
    let mut prop_visited: HashMap<(i32, i32, i32), u8> = HashMap::default();

    for src in &relight_sources {
        if let Some(&existing) = prop_visited.get(&(src.x, src.y, src.z)) {
            if existing >= src.level {
                continue;
            }
        }
        prop_visited.insert((src.x, src.y, src.z), src.level);
        propagation_queue.push_back(LightNode {
            x: src.x,
            y: src.y,
            z: src.z,
            level: src.level,
        });
    }

    // BFS re-propagation using the overlay (sees zeroed values)
    while let Some(node) = propagation_queue.pop_front() {
        let next_level = node.level.saturating_sub(1);
        if next_level == 0 {
            continue;
        }

        for dir in DIRECTIONS {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if let Some(&existing) = prop_visited.get(&(nx, ny, nz)) {
                if existing >= next_level {
                    continue;
                }
            }

            let Some(voxel) = overlay.get_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            if next_level > voxel.block_light {
                prop_visited.insert((nx, ny, nz), next_level);
                updates.push(LightUpdate::new([nx, ny, nz], next_level));
                propagation_queue.push_back(LightNode {
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

/// Handle an opaque block being placed, potentially cutting off light paths.
///
/// Removes light at the placed position and re-propagates from boundaries.
pub(crate) fn block_placed(
    access: &impl VoxelAccess,
    pos: [i32; 3],
    removal_queue: &mut VecDeque<RemovalNode>,
    propagation_queue: &mut VecDeque<LightNode>,
) -> Vec<LightUpdate> {
    let Some(voxel) = access.get_voxel(pos[0], pos[1], pos[2]) else {
        return Vec::new();
    };

    if voxel.block_light == 0 {
        return Vec::new();
    }

    // Treat it as removing the light at this position
    remove_reuse(access, pos, removal_queue, propagation_queue)
}

/// An overlay that presents zeroed values for positions in the removal set,
/// while delegating everything else to the underlying access.
struct OverlayAccess<'a, A: VoxelAccess> {
    inner: &'a A,
    overlay: &'a HashMap<(i32, i32, i32), u8>,
}

impl<A: VoxelAccess> VoxelAccess for OverlayAccess<'_, A> {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<crate::traits::VoxelInfo> {
        let mut voxel = self.inner.get_voxel(x, y, z)?;
        if let Some(&level) = self.overlay.get(&(x, y, z)) {
            voxel.block_light = level;
        }
        Some(voxel)
    }
}
