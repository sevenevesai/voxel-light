/// Minimal information the algorithm needs about a single voxel position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoxelInfo {
    /// Whether light can pass through this voxel.
    pub transparent: bool,
    /// Current stored block light level (0-15).
    pub block_light: u8,
    /// Light emission of this voxel (0 = not a source, 1-15 = emitter).
    pub emission: u8,
}

/// Trait for querying voxel data. Implementors map world-space coordinates
/// to their internal storage (chunks, octrees, flat arrays, etc.).
///
/// The library never mutates storage — it returns a list of updates for the
/// caller to apply. This keeps the library decoupled from any particular
/// concurrency model or storage layout.
pub trait VoxelAccess {
    /// Query the voxel at world-space coordinates `(x, y, z)`.
    ///
    /// Returns `None` if the position is outside the known world (e.g.,
    /// the chunk is not loaded). Light propagation stops at `None` boundaries.
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo>;
}
