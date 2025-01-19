mod generative_chunks;

use crate::generative_chunks::bounds::Point;
use crate::generative_chunks::usage::UsageStrategy;
use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use bevy::utils::HashMap;
use generative_chunks::bounds::{Bounds, ChunkIdx};
use generative_chunks::{Chunk, Dependency, Layer, LayerClient, LayerLookupChunk, LayersManager, LayersManagerBuilder};
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

const POINT_CHUNK_SIZE: Vec2 = Vec2::new(5., 5.);
struct PointsLayer;
impl Chunk for PointChunk {
    fn get_size() -> Vec2 {
        POINT_CHUNK_SIZE
    }
}

impl Layer for PointsLayer {
    type Chunk = PointChunk;

    fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
        // Get thread random with the chunk_idx as seed
        let seed = chunk_idx.x + chunk_idx.y * 512;
        let mut random = rand::prelude::SmallRng::seed_from_u64(seed as u64);
        info!("Generating points chunk with idx: {:?}", chunk_idx);

        PointChunk {
            point: Vec2::new(random.gen_range(0.0..POINT_CHUNK_SIZE.x) + chunk_idx.x as f32 * POINT_CHUNK_SIZE.x,
                             random.gen_range(0.0..POINT_CHUNK_SIZE.y) + chunk_idx.y as f32 * POINT_CHUNK_SIZE.y),
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
            .expand(POINT_CHUNK_SIZE.x * 4.0, POINT_CHUNK_SIZE.y * 4.0);
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
            Dependency::new::<PointsLayer>(Vec2::new(20.0, 20.0))
        ]
    }
}

// #[derive(Resource)]
// struct LayerManagerResource {
//     manager: LayersManager,
// }

fn main() {
    App::new().add_plugins(DefaultPlugins)
        .insert_resource(ChunkIndex { index: HashMap::new() })
        .add_systems(Startup, (setup, setup_layers_manager))
        .add_systems(Update, (regenerate, draw).chain())
        .run();
}

pub fn setup_layers_manager(world: &mut World) {
    let mut manager = LayersManagerBuilder::new()
        .add_layer(PointsLayer)
        .add_layer(VoronoiLayer)
        .build();
    world.insert_non_send_resource(manager);
}

#[derive(Resource)]
pub struct RectShape(Handle<Mesh>);

pub fn setup(mut commands: Commands,
             mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn(Camera2d::default());
    let rect = meshes.add(Rectangle::new(10.0, 10.0));
    commands.insert_resource(RectShape(rect));
}

pub fn regenerate(
    mut commands: Commands,
    mut layer_manager: NonSendMut<LayersManager>,
    mut query: Query<&Transform, With<Camera2d>>, )
{
    let camera_transform = query.single();
    let camera_position = camera_transform.translation;

    // let bounds = Bounds::new(
    //     Vec2::new(camera_position.x - 100.0, camera_position.y - 100.0),
    //     Vec2::new(camera_position.x + 100.0, camera_position.y + 100.0),
    // );

    layer_manager.clear_layer_clients();
    layer_manager.add_layer_client(LayerClient::new(camera_position.xy(), vec![Dependency::new::<VoronoiLayer>(Vec2::new(50.0, 50.0))], UsageStrategy::Fast));

    layer_manager.regenerate();
}

#[derive(Component)]
struct VornoiChunkVisual(
    // The index of the chunk
    ChunkIdx,
);

#[derive(Resource)]
struct ChunkIndex {
    index: HashMap<ChunkIdx, Entity>,
}


fn draw(
    mut commands: Commands,
    layer_manager: NonSend<LayersManager>,
    rect_shape: Res<RectShape>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut chunk_index: ResMut<ChunkIndex>,
) {
    for (idx, chunk) in layer_manager.get_all_chunks_in::<VoronoiLayer>() {
        let color = Color::srgb(chunk.color.0 as f32 / 255.0, chunk.color.1 as f32 / 255.0, chunk.color.2 as f32 / 255.0);

        // TODO: Check if the chunk needs to be recreated
        if chunk_index.index.contains_key(&idx) {
            continue;
        } else {
            let entity = commands.spawn((
                Transform::from_translation(
                    Vec3::new(
                        idx.center(VoronoiChunk::get_size()).x * 10.0,
                        idx.center(VoronoiChunk::get_size()).y * 10.0,
                        0.0, )),
                Mesh2d(rect_shape.0.clone()),
                MeshMaterial2d(materials.add(color)),
                VornoiChunkVisual(idx),
            ));
            chunk_index.index.insert(idx, entity.id());
            println!("Drawing chunk at {:?}", idx);
        }
    }
}
