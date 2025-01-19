use crate::generative_chunks::bounds::{Bounds, ChunkIdx, Point};
use crate::generative_chunks::usage::UsageStrategy;
use bevy::prelude::Vec2;
use daggy::Walker;
use downcast_rs::{impl_downcast, Downcast};
use layer_client::LayerClient;
use layer_manager::LayerLookupChunk;
use std::fmt::Debug;

pub mod bounds;
pub mod layer;
pub mod layer_client;
pub mod layer_id;
pub mod layer_manager;
pub mod usage;

// Tests
#[cfg(test)]
mod test {
    use super::*;

    mod test_layer {
        use super::*;
        use crate::generative_chunks::layer::{Chunk, Layer};
        use crate::generative_chunks::layer_manager::LayersManagerBuilder;
        struct TestLayer;

        #[derive(Debug)]
        struct TestChunk;

        impl Chunk for TestChunk {
            fn get_size() -> Vec2 {
                Vec2::new(1., 1.)
            }
        }

        impl Layer for TestLayer {
            type Chunk = TestChunk;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                TestChunk
            }
        }

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new().add_layer(TestLayer).build();
            layers_manager.print_dot();
            layers_manager.regenerate();
        }
    }

    mod test_layer_with_dependencies {
        use super::*;
        use crate::generative_chunks::layer::{Chunk, Dependency, Layer};
        use crate::generative_chunks::layer_manager::LayersManagerBuilder;

        #[derive(Debug)]
        struct ChunkA;

        struct TestLayerA;

        impl Chunk for ChunkA {
            fn get_size() -> Vec2 {
                Vec2::new(1., 1.)
            }
        }

        impl Layer for TestLayerA {
            type Chunk = ChunkA;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                ChunkA
            }

            fn get_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::new::<TestLayerB>(Vec2::new(1.0, 1.0))]
            }
        }

        struct TestLayerB;

        impl Layer for TestLayerB {
            type Chunk = ChunkA;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                ChunkA
            }
        }

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayerA)
                .add_layer(TestLayerB)
                .build();
            layers_manager.print_dot();
            layers_manager.regenerate();
        }
    }

    mod test_simple_generation {
        use super::*;
        use crate::generative_chunks::layer::{Chunk, Dependency, Layer};
        use crate::generative_chunks::layer_manager::LayersManagerBuilder;

        #[derive(Debug, Clone)]
        struct ChunkA {
            x: i32,
            y: i32,
        }

        struct TestLayerA;

        impl Chunk for ChunkA {
            fn get_size() -> Vec2 {
                Vec2::new(1., 1.)
            }
        }

        impl Layer for TestLayerA {
            type Chunk = ChunkA;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                println!("Generating chunk {:?}", chunk_idx);
                ChunkA {
                    x: chunk_idx.x,
                    y: chunk_idx.y,
                }
            }
        }

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new().add_layer(TestLayerA).build();
            layers_manager.add_layer_client(LayerClient::new(
                Vec2::new(0.0, 0.0),
                vec![Dependency::new::<TestLayerA>(Vec2::new(2.0, 2.0))],
                UsageStrategy::Fast,
            ));
            layers_manager.regenerate();
            // Check if the chunk is generated correctly
            assert!(layers_manager
                .get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0))
                .is_some());
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0))
                    .unwrap()
                    .x,
                0
            );
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0))
                    .unwrap()
                    .y,
                0
            );
            assert!(layers_manager
                .get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0))
                .is_some());
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0))
                    .unwrap()
                    .x,
                2
            );
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0))
                    .unwrap()
                    .y,
                2
            );
            assert!(layers_manager
                .get_chunk::<TestLayerA>(Vec2::new(3.0, 3.0))
                .is_none());
            assert!(layers_manager
                .get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0))
                .is_some());
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0))
                    .unwrap()
                    .x,
                -1
            );
            assert_eq!(
                layers_manager
                    .get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0))
                    .unwrap()
                    .y,
                -1
            );
        }
    }

    mod test_simple_generation_with_deps {
        use super::*;
        use crate::generative_chunks::layer::{Dependency, Layer};
        use crate::generative_chunks::layer_manager::LayersManagerBuilder;
        use layer::Chunk;
        use rand::{Rng, SeedableRng};

        /// For this test we will have a layer that depends on another layer
        /// The points layer will have a chunk size of 5x5 and will generate a single random point
        /// and a random color for that point
        /// The voronoi layer will have a chunk size of 1x1 and will generate the color of the closest point
        /// in the points layer, the dependency will have a padding of 10x10 to ensure that the voronoi layer
        /// has enough information to generate the color of the closest point

        #[derive(Debug, Clone)]
        struct PointChunk {
            /// The point is in real coordinates
            point: Point,
            color: (u8, u8, u8),
        }

        struct PointsLayer;
        impl Chunk for PointChunk {
            fn get_size() -> Vec2 {
                Vec2::new(5., 5.)
            }
        }

        impl Layer for PointsLayer {
            type Chunk = PointChunk;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                // Get thread random with the chunk_idx as seed
                let seed = chunk_idx.x + chunk_idx.y * 23;
                let mut random = rand::prelude::SmallRng::seed_from_u64(seed as u64);

                PointChunk {
                    point: Vec2::new(
                        random.gen_range(0.0..5.0) + chunk_idx.x as f32,
                        random.gen_range(0.0..5.0) + chunk_idx.y as f32,
                    ),
                    color: (
                        random.gen_range(0..255),
                        random.gen_range(0..255),
                        random.gen_range(0..255),
                    ),
                }
            }
        }

        #[derive(Debug, Clone)]
        struct VoronoiChunk {
            /// The color of the closest point
            color: (u8, u8, u8),
        }

        impl Chunk for VoronoiChunk {
            fn get_size() -> Vec2 {
                return Vec2::new(1., 1.);
            }
        }

        struct VoronoiLayer;

        impl Layer for VoronoiLayer {
            type Chunk = VoronoiChunk;

            fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
                // Get the closest point from the points layer
                let bounds = Bounds::from_point(chunk_idx.to_point(Self::Chunk::get_size()))
                    .expand(10.0, 10.0);
                let points = lookup.get_chunks_in::<PointsLayer>(bounds);
                let closest_point = points
                    .iter()
                    .min_by(|a, b| {
                        let a_dist = a.point.distance(chunk_idx.center(Self::Chunk::get_size()));
                        let b_dist = b.point.distance(chunk_idx.center(Self::Chunk::get_size()));
                        a_dist.partial_cmp(&b_dist).unwrap()
                    })
                    .unwrap();
                VoronoiChunk {
                    color: closest_point.color,
                }
            }

            fn get_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::new::<PointsLayer>(Vec2::new(10.0, 10.0))]
            }
        }

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(PointsLayer)
                .add_layer(VoronoiLayer)
                .build();
            layers_manager.add_layer_client(LayerClient::new(
                Vec2::new(0.0, 0.0),
                vec![Dependency::new::<VoronoiLayer>(Vec2::new(8.0, 9.0))],
                UsageStrategy::Fast,
            ));
            layers_manager.regenerate();
            // Check if the chunk is generated correctly
            assert!(layers_manager
                .get_chunk::<VoronoiLayer>(Vec2::new(0.0, 0.0))
                .is_some());
            assert_eq!(
                layers_manager
                    .get_chunk::<VoronoiLayer>(Vec2::new(0.0, 0.0))
                    .unwrap()
                    .color,
                (193, 180, 73)
            );
        }
    }
}
