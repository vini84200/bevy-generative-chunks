mod generative_chunks;

use generative_chunks::bounds::{Bounds, ChunkIdx};
use generative_chunks::{Chunk, LayersManager, LayersManagerBuilder, Dependency, Layer, LayerLookupChunk, LayerClient};
use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use crate::generative_chunks::bounds::Point;
use crate::generative_chunks::usage::UsageStrategy;

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
        info!("Generating points chunk with idx: {:?}", chunk_idx);

        PointChunk {
            point: Vec2::new(random.gen_range(0.0..5.0) + chunk_idx.x as f32, random.gen_range(0.0..5.0) + chunk_idx.y as f32),
            color: (random.gen_range(0..255), random.gen_range(0..255), random.gen_range(0..255)),
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
        Vec2::new(1., 1.)
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
        let closest_point = points.iter().min_by(|a, b| {
            let a_dist = a.point.distance(chunk_idx.center(Self::Chunk::get_size()));
            let b_dist = b.point.distance(chunk_idx.center(Self::Chunk::get_size()));
            a_dist.partial_cmp(&b_dist).unwrap()
        }).unwrap();
        info!("Generating voronoi chunk with closest point: {:?}", closest_point);
        VoronoiChunk {
            color: closest_point.color,
        }
    }

    fn get_dependencies(&self) -> Vec<Dependency> {
        vec![
            Dependency::new::<PointsLayer>(Vec2::new(10.0, 10.0))
        ]
    }
}

// #[derive(Resource)]
// struct LayerManagerResource {
//     manager: LayersManager,
// }

fn main() {
    App::new().add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup, setup_layers_manager))
        .add_systems(Update,(regenerate, ).chain())
        .run();
}

pub fn setup_layers_manager(world: &mut World) {
    let mut manager = LayersManagerBuilder::new()
        .add_layer(PointsLayer)
        .add_layer(VoronoiLayer)
        .build();
    world.insert_non_send_resource(manager);
}

pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

pub fn regenerate(
    mut commands: Commands,
    mut layer_manager: NonSendMut<LayersManager>,
    mut query: Query<&Transform, With<Camera2d>>,)
{
    let camera_transform = query.single();
    let camera_position = camera_transform.translation;

    // let bounds = Bounds::new(
    //     Vec2::new(camera_position.x - 100.0, camera_position.y - 100.0),
    //     Vec2::new(camera_position.x + 100.0, camera_position.y + 100.0),
    // );

    layer_manager.clear_layer_clients();
    layer_manager.add_layer_client(LayerClient::new(camera_position.xy(), vec![Dependency::new::<VoronoiLayer>(Vec2::new(10.0, 10.0))], UsageStrategy::Fast));

    layer_manager.regenerate();
}
