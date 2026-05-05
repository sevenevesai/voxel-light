use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::types::DIRECTIONS;

#[cfg(feature = "std")]
type VisitedMap = ahash::AHashMap<(i32, i32, i32), [u8; 3]>;

#[cfg(not(feature = "std"))]
type VisitedMap = alloc::collections::BTreeMap<(i32, i32, i32), [u8; 3]>;

#[cfg(feature = "std")]
type VisitedBool = ahash::AHashMap<(i32, i32, i32), bool>;

#[cfg(not(feature = "std"))]
type VisitedBool = alloc::collections::BTreeMap<(i32, i32, i32), bool>;

/// Minimal voxel info for colored light propagation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredVoxelInfo {
    pub transparent: bool,
    /// Current RGB light levels (0-15 per channel).
    pub light: [u8; 3],
    /// RGB emission (0-15 per channel).
    pub emission: [u8; 3],
}

/// Trait for querying voxels with colored light data.
pub trait ColoredVoxelAccess {
    fn get_colored_voxel(&self, x: i32, y: i32, z: i32) -> Option<ColoredVoxelInfo>;
}

/// A pending colored light update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredLightUpdate {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub rgb: [u8; 3],
}

#[derive(Clone, Copy)]
struct ColorNode {
    x: i32,
    y: i32,
    z: i32,
    rgb: [u8; 3],
}

/// Propagate colored light from a source. Each RGB channel decays independently.
pub fn propagate_colored(
    access: &impl ColoredVoxelAccess,
    source: [i32; 3],
    emission: [u8; 3],
) -> Vec<ColoredLightUpdate> {
    if emission == [0, 0, 0] {
        return Vec::new();
    }

    let mut visited = VisitedMap::default();
    let mut updates = Vec::new();
    let mut queue = VecDeque::with_capacity(512);

    queue.push_back(ColorNode {
        x: source[0],
        y: source[1],
        z: source[2],
        rgb: emission,
    });
    visited.insert((source[0], source[1], source[2]), emission);
    updates.push(ColoredLightUpdate { x: source[0], y: source[1], z: source[2], rgb: emission });

    while let Some(node) = queue.pop_front() {
        let next = [
            node.rgb[0].saturating_sub(1),
            node.rgb[1].saturating_sub(1),
            node.rgb[2].saturating_sub(1),
        ];

        if next == [0, 0, 0] {
            continue;
        }

        for dir in DIRECTIONS {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if let Some(&existing) = visited.get(&(nx, ny, nz)) {
                if existing[0] >= next[0] && existing[1] >= next[1] && existing[2] >= next[2] {
                    continue;
                }
            }

            let Some(voxel) = access.get_colored_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            // Current effective light = max of stored and any visited value
            let current = if let Some(&existing) = visited.get(&(nx, ny, nz)) {
                [
                    existing[0].max(voxel.light[0]),
                    existing[1].max(voxel.light[1]),
                    existing[2].max(voxel.light[2]),
                ]
            } else {
                voxel.light
            };

            // Check if we'd improve any channel
            let improves = next[0] > current[0]
                || next[1] > current[1]
                || next[2] > current[2];

            if !improves {
                continue;
            }

            // Merge: take max of current effective and next
            let merged = [
                current[0].max(next[0]),
                current[1].max(next[1]),
                current[2].max(next[2]),
            ];

            visited.insert((nx, ny, nz), merged);
            updates.push(ColoredLightUpdate { x: nx, y: ny, z: nz, rgb: merged });
            queue.push_back(ColorNode { x: nx, y: ny, z: nz, rgb: merged });
        }
    }

    updates
}

/// Remove colored light from a source with two-phase BFS.
pub fn remove_colored(
    access: &impl ColoredVoxelAccess,
    source: [i32; 3],
) -> Vec<ColoredLightUpdate> {
    let Some(source_voxel) = access.get_colored_voxel(source[0], source[1], source[2]) else {
        return Vec::new();
    };

    let old_rgb = source_voxel.light;
    if old_rgb == [0, 0, 0] {
        return Vec::new();
    }

    let mut visited = VisitedBool::default();
    let mut updates: Vec<ColoredLightUpdate> = Vec::new();
    let mut relight_sources: Vec<ColorNode> = Vec::new();

    struct RemovalColorNode {
        x: i32,
        y: i32,
        z: i32,
        old_rgb: [u8; 3],
    }

    let mut removal_queue = VecDeque::with_capacity(512);
    removal_queue.push_back(RemovalColorNode {
        x: source[0],
        y: source[1],
        z: source[2],
        old_rgb,
    });
    visited.insert((source[0], source[1], source[2]), true);
    updates.push(ColoredLightUpdate { x: source[0], y: source[1], z: source[2], rgb: [0, 0, 0] });

    // Phase 1: Removal BFS
    while let Some(node) = removal_queue.pop_front() {
        for dir in DIRECTIONS {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if visited.contains_key(&(nx, ny, nz)) {
                continue;
            }

            let Some(voxel) = access.get_colored_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            // A channel is dependent if it's nonzero and less than the node's old value
            let dependent = (voxel.light[0] != 0 && voxel.light[0] < node.old_rgb[0])
                || (voxel.light[1] != 0 && voxel.light[1] < node.old_rgb[1])
                || (voxel.light[2] != 0 && voxel.light[2] < node.old_rgb[2]);

            let boundary = voxel.light[0] >= node.old_rgb[0]
                || voxel.light[1] >= node.old_rgb[1]
                || voxel.light[2] >= node.old_rgb[2];

            if dependent {
                visited.insert((nx, ny, nz), true);
                updates.push(ColoredLightUpdate { x: nx, y: ny, z: nz, rgb: [0, 0, 0] });
                removal_queue.push_back(RemovalColorNode {
                    x: nx,
                    y: ny,
                    z: nz,
                    old_rgb: voxel.light,
                });
            } else if boundary {
                visited.insert((nx, ny, nz), true);
                relight_sources.push(ColorNode { x: nx, y: ny, z: nz, rgb: voxel.light });
            }
        }
    }

    // Phase 2: Re-propagate from boundary sources
    let mut prop_visited = VisitedMap::default();
    let mut queue = VecDeque::with_capacity(256);

    for src in &relight_sources {
        prop_visited.insert((src.x, src.y, src.z), src.rgb);
        queue.push_back(*src);
    }

    while let Some(node) = queue.pop_front() {
        let next = [
            node.rgb[0].saturating_sub(1),
            node.rgb[1].saturating_sub(1),
            node.rgb[2].saturating_sub(1),
        ];
        if next == [0, 0, 0] {
            continue;
        }

        for dir in DIRECTIONS {
            let nx = node.x + dir[0];
            let ny = node.y + dir[1];
            let nz = node.z + dir[2];

            if let Some(&existing) = prop_visited.get(&(nx, ny, nz)) {
                if existing[0] >= next[0] && existing[1] >= next[1] && existing[2] >= next[2] {
                    continue;
                }
            }

            let Some(voxel) = access.get_colored_voxel(nx, ny, nz) else {
                continue;
            };

            if !voxel.transparent {
                continue;
            }

            let merged = if let Some(&existing) = prop_visited.get(&(nx, ny, nz)) {
                [
                    existing[0].max(next[0]),
                    existing[1].max(next[1]),
                    existing[2].max(next[2]),
                ]
            } else {
                next
            };

            prop_visited.insert((nx, ny, nz), merged);
            updates.push(ColoredLightUpdate { x: nx, y: ny, z: nz, rgb: merged });
            queue.push_back(ColorNode { x: nx, y: ny, z: nz, rgb: merged });
        }
    }

    updates
}
