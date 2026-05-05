use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;
use voxel_light::{propagate, remove, LightEngine, LightUpdate, VoxelAccess, VoxelInfo};

struct BenchWorld {
    blocks: HashMap<(i32, i32, i32), VoxelInfo>,
}

impl BenchWorld {
    fn air_cube(radius: i32) -> Self {
        let mut blocks = HashMap::with_capacity((radius * 2 + 1).pow(3) as usize);
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    blocks.insert((x, y, z), VoxelInfo {
                        transparent: true,
                        block_light: 0,
                        emission: 0,
                    });
                }
            }
        }
        Self { blocks }
    }

    fn with_walls(radius: i32) -> Self {
        let mut world = Self::air_cube(radius);
        // Add walls every 8 blocks to simulate rooms
        for x in (-radius..=radius).step_by(8) {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    world.blocks.insert((x, y, z), VoxelInfo {
                        transparent: false,
                        block_light: 0,
                        emission: 0,
                    });
                }
            }
        }
        world
    }

    fn apply_updates(&mut self, updates: &[LightUpdate]) {
        for u in updates {
            if let Some(voxel) = self.blocks.get_mut(&(u.x, u.y, u.z)) {
                voxel.block_light = u.level;
            }
        }
    }
}

impl VoxelAccess for BenchWorld {
    fn get_voxel(&self, x: i32, y: i32, z: i32) -> Option<VoxelInfo> {
        self.blocks.get(&(x, y, z)).copied()
    }
}

fn bench_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("propagation");

    for level in [7, 10, 14] {
        group.bench_with_input(
            BenchmarkId::new("open_air", level),
            &level,
            |b, &level| {
                let world = BenchWorld::air_cube(16);
                b.iter(|| propagate(&world, [0, 0, 0], level));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_walls", level),
            &level,
            |b, &level| {
                let world = BenchWorld::with_walls(16);
                b.iter(|| propagate(&world, [1, 0, 0], level));
            },
        );
    }

    group.finish();
}

fn bench_removal(c: &mut Criterion) {
    let mut group = c.benchmark_group("removal");

    for level in [7, 10, 14] {
        group.bench_with_input(
            BenchmarkId::new("single_source", level),
            &level,
            |b, &level| {
                b.iter_batched(
                    || {
                        let mut world = BenchWorld::air_cube(16);
                        let updates = propagate(&world, [0, 0, 0], level);
                        world.apply_updates(&updates);
                        world.blocks.get_mut(&(0, 0, 0)).unwrap().emission = level;
                        world
                    },
                    |world| remove(&world, [0, 0, 0]),
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_neighbor_source", level),
            &level,
            |b, &level| {
                b.iter_batched(
                    || {
                        let mut world = BenchWorld::air_cube(20);
                        let u1 = propagate(&world, [0, 0, 0], level);
                        world.apply_updates(&u1);
                        let u2 = propagate(&world, [10, 0, 0], level);
                        world.apply_updates(&u2);
                        world.blocks.get_mut(&(0, 0, 0)).unwrap().emission = level;
                        world.blocks.get_mut(&(10, 0, 0)).unwrap().emission = level;
                        world
                    },
                    |world| remove(&world, [0, 0, 0]),
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_engine_reuse(c: &mut Criterion) {
    c.bench_function("engine/place_remove_cycle", |b| {
        let mut world = BenchWorld::air_cube(16);
        let mut engine = LightEngine::new();
        b.iter(|| {
            let updates = engine.place_light(&world, [0, 0, 0], 14);
            world.apply_updates(&updates);
            world.blocks.get_mut(&(0, 0, 0)).unwrap().emission = 14;
            let removal = engine.remove_light(&world, [0, 0, 0]);
            world.apply_updates(&removal);
            world.blocks.get_mut(&(0, 0, 0)).unwrap().emission = 0;
        });
    });
}

criterion_group!(benches, bench_propagation, bench_removal, bench_engine_reuse);
criterion_main!(benches);
