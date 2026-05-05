//! Generic 3D BFS light propagation with two-phase removal for voxel engines.
//!
//! Implement [`VoxelAccess`] for your storage, call [`propagate`] / [`remove`],
//! and apply the returned [`LightUpdate`]s. The library never mutates your data.
//!
//! For repeated operations, use [`LightEngine`] which reuses internal buffers.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod propagate;
mod remove;
#[cfg(feature = "sky")]
pub mod sky;
#[cfg(feature = "colored")]
pub mod colored;
mod traits;
mod types;

pub use propagate::propagate;
pub use remove::remove;
pub use traits::{VoxelAccess, VoxelInfo};
pub use types::LightUpdate;

pub struct LightEngine {
    propagation_queue: alloc::collections::VecDeque<types::LightNode>,
    removal_queue: alloc::collections::VecDeque<types::RemovalNode>,
}

impl LightEngine {
    pub fn new() -> Self {
        Self {
            propagation_queue: alloc::collections::VecDeque::with_capacity(512),
            removal_queue: alloc::collections::VecDeque::with_capacity(512),
        }
    }

    pub fn place_light(
        &mut self,
        access: &impl VoxelAccess,
        pos: [i32; 3],
        level: u8,
    ) -> alloc::vec::Vec<LightUpdate> {
        propagate::propagate_reuse(access, pos, level, &mut self.propagation_queue)
    }

    pub fn remove_light(
        &mut self,
        access: &impl VoxelAccess,
        pos: [i32; 3],
    ) -> alloc::vec::Vec<LightUpdate> {
        remove::remove_reuse(
            access,
            pos,
            &mut self.removal_queue,
            &mut self.propagation_queue,
        )
    }

    pub fn block_placed(
        &mut self,
        access: &impl VoxelAccess,
        pos: [i32; 3],
    ) -> alloc::vec::Vec<LightUpdate> {
        remove::block_placed(access, pos, &mut self.removal_queue, &mut self.propagation_queue)
    }

    pub fn block_removed(
        &mut self,
        access: &impl VoxelAccess,
        pos: [i32; 3],
    ) -> alloc::vec::Vec<LightUpdate> {
        propagate::block_removed(access, pos, &mut self.propagation_queue)
    }

    /// Clear all light in a cubic area and re-propagate from any emitters found.
    ///
    /// Use when an opaque block is placed and you need to recalculate the area
    /// rather than tracing individual affected paths.
    pub fn recalculate_area(
        &mut self,
        access: &impl VoxelAccess,
        center: [i32; 3],
        radius: i32,
    ) -> alloc::vec::Vec<LightUpdate> {
        use alloc::vec::Vec;

        let mut updates = Vec::new();
        let mut emitters: Vec<([i32; 3], u8)> = Vec::new();

        // Phase 1: clear all light, collect emitters
        for z in (center[2] - radius)..=(center[2] + radius) {
            for y in (center[1] - radius)..=(center[1] + radius) {
                for x in (center[0] - radius)..=(center[0] + radius) {
                    if let Some(voxel) = access.get_voxel(x, y, z) {
                        if voxel.emission > 0 {
                            emitters.push(([x, y, z], voxel.emission));
                        }
                        if voxel.block_light > 0 {
                            updates.push(LightUpdate::new([x, y, z], 0));
                        }
                    }
                }
            }
        }

        // Phase 2: re-propagate from each emitter against cleared state
        let mut reprop_updates = Vec::new();
        {
            let cleared = ClearedOverlay { inner: access, clears: &updates };
            for (pos, emission) in emitters {
                let prop = propagate::propagate_reuse(
                    &cleared,
                    pos,
                    emission,
                    &mut self.propagation_queue,
                );
                reprop_updates.extend(prop);
            }
        }

        updates.extend(reprop_updates);
        updates
    }
}

impl Default for LightEngine {
    fn default() -> Self {
        Self::new()
    }
}

struct ClearedOverlay<'a, A: VoxelAccess> {
    inner: &'a A,
    clears: &'a [LightUpdate],
}

impl<A: VoxelAccess> VoxelAccess for ClearedOverlay<'_, A> {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        let mut voxel = self.inner.get_voxel(x, y, z)?;
        if self.clears.iter().any(|u| u.x == x && u.y == y && u.z == z) {
            voxel.block_light = 0;
        }
        Some(voxel)
    }
}
