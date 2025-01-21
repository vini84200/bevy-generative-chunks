use bevy::math::NormedVectorSpace;
use bevy_generative_chunks::generative_chunks::bounds::Point;
use bevy_generative_chunks::generative_chunks::usage::UsageStrategy;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy_generative_chunks::generative_chunks::bounds::{Bounds, ChunkIdx};
use bevy_generative_chunks::generative_chunks::layer::{Chunk, Dependency, Layer};
use bevy_generative_chunks::generative_chunks::layer_client::LayerClient;
use bevy_generative_chunks::generative_chunks::layer_manager::{LayerLookupChunk, LayersManager, LayersManagerBuilder};
use rand::{Rng, SeedableRng};
use bevy_inspector_egui::quick::WorldInspectorPlugin;

#[derive(Debug, Clone)]
struct PointChunk {
    /// The point is in real coordinates
    point: Point,
    color: (u8, u8, u8),
    strength: f32
}

const POINT_CHUNK_SIZE: Vec2 = Vec2::new(25., 25.);
struct PointsLayer;
impl Chunk for PointChunk {
    fn get_size() -> Vec2 {
        POINT_CHUNK_SIZE
    }
}

impl Layer for PointsLayer {
    type Chunk = PointChunk;

    fn generate(&self, _: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
        // Get thread random with the chunk_idx as seed
        let seed = chunk_idx.x + chunk_idx.y * 512;
        let mut random = rand::prelude::SmallRng::seed_from_u64(seed as u64);
        // info!("Generating points chunk with idx: {:?}", chunk_idx);

        PointChunk {
            point: Vec2::new(
                random.gen_range(0.0..POINT_CHUNK_SIZE.x) + chunk_idx.x as f32 * POINT_CHUNK_SIZE.x,
                random.gen_range(0.0..POINT_CHUNK_SIZE.y) + chunk_idx.y as f32 * POINT_CHUNK_SIZE.y,
            ),
            color: (
                random.gen_range(0..255),
                random.gen_range(0..255),
                random.gen_range(0..255),
            ),
            strength: random.gen_range(0.8..1.4),
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

fn manhattan_distance(a: Vec2, b: Vec2) -> f32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn euclidean_distance(a: Vec2, b: Vec2) -> f32 {
    a.distance(b)
}

struct VoronoiLayer;

impl Layer for VoronoiLayer {
    type Chunk = VoronoiChunk;

    fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk {
        // Get the closest point from the points layer
        let bounds = Bounds::from_point(chunk_idx.to_point(Self::Chunk::get_size()))
            .expand(POINT_CHUNK_SIZE.x * 5.0, POINT_CHUNK_SIZE.y * 5.0);
        let points = lookup.get_chunks_in::<PointsLayer>(bounds);
        let closest_point = points
            .iter()
            .min_by(|a, b| {
                let a_dist = manhattan_distance(a.point, chunk_idx.center(Self::Chunk::get_size())) / a.strength;
                let b_dist = manhattan_distance(b.point, chunk_idx.center(Self::Chunk::get_size())) / b.strength;
                a_dist.partial_cmp(&b_dist).unwrap()
            })
            .unwrap();
        // info!(
        //     "Generating voronoi chunk with closest point: {:?}",
        //     closest_point
        // );
        VoronoiChunk {
            color: closest_point.color,
        }
    }

    fn get_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new::<PointsLayer>(Vec2::new(POINT_CHUNK_SIZE.x * 5.0, POINT_CHUNK_SIZE.y * 5.0))]
    }
}


#[derive(Resource, Deref, DerefMut )]
struct LayerManagerRes(LayersManager);

fn main() {
    App::new()
        .add_plugins((DefaultPlugins,PanCamPlugin))
        .add_plugins(WorldInspectorPlugin::new())
        .insert_resource(ChunkIndex {
            index: HashMap::new(),
        })
        .add_systems(Startup, (setup, setup_layers_manager))
        .add_systems(Update, (regenerate, draw).chain())
        .run();
}

pub fn setup_layers_manager(mut commands: Commands) {
    let manager = LayersManagerBuilder::new()
        .add_layer(PointsLayer)
        .add_layer(VoronoiLayer)
        .build();
    manager.print_dot();
    commands.insert_resource(LayerManagerRes(manager));
}

#[derive(Resource)]
pub struct RectShape(Handle<Mesh>);

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.spawn((Camera2d, PanCam::default()));
    let rect = meshes.add(Rectangle::new(10.0, 10.0));
    commands.insert_resource(RectShape(rect));

}


fn regenerate(
    // mut commands: Commands,
    mut layer_manager: ResMut<LayerManagerRes>,
    query: Query<&Transform, With<Camera2d>>,
) {
    let camera_transform = query.single();
    let camera_position = camera_transform.translation.xy();

    // let bounds = Bounds::new(
    //     Vec2::new(camera_position.x - 100.0, camera_position.y - 100.0),
    //     Vec2::new(camera_position.x + 100.0, camera_position.y + 100.0),
    // );
    layer_manager.clear_layer_clients();
    layer_manager.add_layer_client(LayerClient::new(
        camera_position/10.,
        vec![Dependency::new::<VoronoiLayer>(Vec2::new(40.0, 40.0))],
        UsageStrategy::Fast,
    ));

    layer_manager.regenerate();
}

// #[derive(Component)]
// struct VornoiChunkVisual(
//     // The index of the chunk
//     ChunkIdx,
// );

#[derive(Resource)]
struct ChunkIndex {
    index: HashMap<ChunkIdx, Entity>,
}

fn draw(
    mut commands: Commands,
    layer_manager: Res<LayerManagerRes>,
    rect_shape: Res<RectShape>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut chunk_index: ResMut<ChunkIndex>,
) {
    for (idx, chunk) in layer_manager.get_all_chunks_in::<VoronoiLayer>() {
        let color = Color::srgb(
            chunk.color.0 as f32 / 255.0,
            chunk.color.1 as f32 / 255.0,
            chunk.color.2 as f32 / 255.0,
        );

        // TODO: Check if the chunk needs to be recreated
        if chunk_index.index.contains_key(&idx) {
            continue;
        } else {
            let entity = commands.spawn((
                Transform::from_translation(Vec3::new(
                    idx.center(VoronoiChunk::get_size()).x * 10.0,
                    idx.center(VoronoiChunk::get_size()).y * 10.0,
                    0.0,
                )),
                Mesh2d(rect_shape.0.clone()),
                MeshMaterial2d(materials.add(color)),
                // VornoiChunkVisual(idx),
            ));
            chunk_index.index.insert(idx, entity.id());
            // println!("Drawing chunk at {:?}", idx);
        }
    }
    for idx in layer_manager.get_deleted_chunks::<VoronoiLayer>() {
        if let Some(entity) = chunk_index.index.remove(idx) {
            commands.entity(entity).despawn();
            // println!("Despawning chunk at {:?}", idx);
        } 
    }
}
