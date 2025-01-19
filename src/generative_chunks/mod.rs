use crate::generative_chunks::bounds::{Bounds, ChunkIdx, Point};
use crate::generative_chunks::usage::{UsageCounter, UsageStrategy};
use bevy::prelude::Vec2;
use bimap::BiMap;
use daggy::petgraph::visit::Topo;
use daggy::{petgraph::dot::{Config, Dot}, Dag, NodeIndex, Walker};
use downcast_rs::{impl_downcast, Downcast};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;

pub mod usage;
pub mod bounds;


pub struct LayersManagerBuilder {
    layers: Vec<LayerConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LayerId(&'static str);

impl LayerId {
    fn from_type<T: Layer + 'static>() -> LayerId {
        LayerId(std::any::type_name::<T>())
    }
}

#[derive(Debug)]
pub struct LayerClient {
    active: bool,
    center: Point,
    dependencies: Vec<Dependency>,
    strategy: UsageStrategy,
}

impl IntoLayerClient for LayerClient {
    fn into_layer_client(self) -> LayerClient {
        self
    }
}

impl LayerClient {
    pub(crate) fn new(center: Point, dependencies: Vec<Dependency>, strength: UsageStrategy) -> Self {
        LayerClient {
            active: true,
            center,
            dependencies,
            strategy: strength,
        }
    }

    fn activate(&mut self) {
        self.active = true;
    }
    fn deactivate(&mut self) {
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

trait IntoLayerClient {
    fn into_layer_client(self) -> LayerClient;
}


// #[derive(Debug)]
pub struct LayersManager {
    layers: HashMap<LayerId, RefCell<LayerConfig>>,
    dag: Dag<LayerId, ()>,
    dag_index: BiMap<LayerId, NodeIndex>,
    layer_client: Vec<LayerClient>,
}

impl LayersManager {
    pub fn get_chunk<L: Layer + 'static>(&self, pos: Point) -> Option<L::Chunk>
    where
        L::Chunk: Clone,
    {
        let layer_id = LayerId::from_type::<L>();
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let Vec2 { x: width, y: height } = layer.chunk_size;
        let chunk_idx = ChunkIdx::from_point(pos, width, height);
        let chunk = layer.storage.get(&chunk_idx)?;
        let data = chunk.chunk.as_ref().and_then(|c| c.downcast_ref::<L::Chunk>());
        data.and_then(|c| Some(c.clone()))
    }

    pub fn get_chunks_in<L: Layer + 'static>(&self, bounds: Bounds) -> Vec<(ChunkIdx, L::Chunk)>
    where
        L::Chunk: Clone,
    {
        let layer_id = LayerId::from_type::<L>();
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let mut chunks = Vec::new();
        for chunk_idx in bounds.chunks(L::Chunk::get_size()) {
            let chunk = layer.storage.get(&chunk_idx);
            if let Some(chunk) = chunk {
                let data = chunk.chunk.as_ref().and_then(|c| c.downcast_ref::<L::Chunk>());
                if let Some(data) = data {
                    chunks.push((chunk_idx.clone(), data.clone()));
                }
            }
        }
        chunks
    }

    pub fn get_all_chunks_in<L: Layer + 'static>(&self) -> Vec<(ChunkIdx, L::Chunk)>
    where
        L::Chunk: Clone,
    {
        let layer_id = LayerId::from_type::<L>();
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let mut chunks = Vec::new();
        for (chunk_idx, chunk) in layer.storage.iter() {
            let data = chunk.chunk.as_ref().and_then(|c| c.downcast_ref::<L::Chunk>());
            if let Some(data) = data {
                chunks.push((chunk_idx.clone(), data.clone()));
            }
        }
        chunks
    }

    pub fn add_layer_client(&mut self, layer_client: impl IntoLayerClient) {
        self.layer_client.push(layer_client.into_layer_client());
    }

    pub fn clear_layer_clients(&mut self) {
        self.layer_client.clear();
    }
}

pub(crate) struct LayerLookupChunk<'a> {
    layers: &'a HashMap<LayerId, RefCell<LayerConfig>>,
}

impl LayerLookupChunk<'_> {
    fn get_chunk_from_idx<L: Layer + 'static>(&self, layer_id: LayerId, chunk_idx: ChunkIdx) -> Option<L::Chunk>
    where
        L::Chunk: Clone,
    {
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let chunk = layer.storage.get(&chunk_idx)?;
        let data = chunk.chunk.as_ref().and_then(|c| c.downcast_ref::<L::Chunk>());
        data.and_then(|c| Some(c.clone()))
    }

    fn get_chunk<L: Layer + 'static>(&self, layer_id: LayerId, pos: Point) -> Option<L::Chunk>
    where
        L::Chunk: Clone,
    {
        // Get the chunk index
        let Vec2 { x: width, y: height } = L::Chunk::get_size();
        let chunk_idx = ChunkIdx::from_point(pos, width, height);
        self.get_chunk_from_idx::<L>(layer_id, chunk_idx)
    }

    pub(crate) fn get_chunks_in<L: Layer + 'static>(&self, bounds: Bounds) -> Vec<L::Chunk>
    where
        L::Chunk: Clone,
    {
        let layer_id = LayerId::from_type::<L>();
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let mut chunks = Vec::new();
        for chunk_idx in bounds.chunks(L::Chunk::get_size()) {
            let chunk = self.get_chunk_from_idx::<L>(layer_id, chunk_idx);
            if let Some(chunk) = chunk {
                chunks.push(chunk);
            }
        }
        chunks
    }
}

impl LayersManager {
    fn print_dot(&self) {
        println!("{:?}", Dot::with_config(&self.dag.graph(), &[
            Config::EdgeNoLabel,
            // Config::NodeIndexLabel
        ]
        ));
    }

    pub fn regenerate(&mut self) {
        // Check what the layer clients need to be regenerated
        for layer_client in self.layer_client.iter_mut() {
            if !layer_client.is_active() {
                continue;
            }
            for Dependency {
                padding,
                layer_id
            } in layer_client.dependencies.iter() {
                let mut layer = self.layers.get_mut(layer_id).unwrap().borrow_mut();
                layer.ensure_generated(&Bounds::from_point(layer_client.center).add_padding(*padding));
            }
        }

        // Transverse the DAG in topological order
        let mut topo = Topo::new(&self.dag);
        // Stack so we may generate the chunks in reverse topological order later
        let mut stack = Vec::new();

        while let Some(node) = topo.next(&self.dag) {
            // Check if the layer has any requirements to pass to its dependencies
            let layer_id = self.dag[node];
            let layer = self.layers.get(&layer_id).unwrap().borrow();
            let requirements = layer.requires();
            for (dependency_id, bounds) in requirements {
                let mut dependency = self.layers.get(&dependency_id).unwrap().borrow_mut();
                dependency.ensure_generated(&bounds);
            }
            stack.push(node);
        }

        // Now we can generate the chunks, by transversing the DAG in topological order in reverse
        stack.iter().rev().for_each(|node| {
            let layer_id = self.dag[*node];
            let mut layer = self.layers.get(&layer_id).unwrap().borrow_mut();
            // Generate the chunks
            let layer_lookup = LayerLookupChunk {
                layers: &self.layers,
            };
            layer.generate(&layer_lookup);
        });
    }
}


impl LayersManagerBuilder {
    pub fn new() -> Self {
        LayersManagerBuilder {
            layers: Vec::new(),
        }
    }

    pub fn add_layer(mut self, layer: impl IntoLayerConfig) -> Self {
        self.layers.push(layer.into_layer_config());
        self
    }

    pub fn build(self) -> LayersManager {
        let mut layers: HashMap<LayerId, RefCell<LayerConfig>> = HashMap::new();
        let mut dag = Dag::new();
        let mut dag_index = BiMap::new();

        for layer in self.layers.iter() {
            dag_index.insert(layer.layer_id, dag.add_node(layer.layer_id));
        }
        for layer in self.layers.iter() {
            let idx = dag_index.get_by_left(&layer.layer_id).unwrap();
            dag.add_edges(
                layer.depends_on.iter().map(|id| (idx.clone(), dag_index.get_by_left(&id.layer_id).unwrap().clone(), ()))
            ).expect("Adding edges to DAG created a cycle");
        }
        for layer in self.layers {
            layers.insert(layer.layer_id, RefCell::new(layer));
        }

        LayersManager {
            layers,
            dag,
            dag_index,
            layer_client: vec![],
        }
    }
}

type ChunkGenerator = Box<dyn Fn(&LayerLookupChunk, &ChunkIdx) -> Box<dyn Chunk>>;


// #[derive(Debug)]
struct LayerConfig {
    /// This layer id
    layer_id: LayerId,
    /// The layer depends on these layers to be generated
    depends_on: Vec<Dependency>,
    /// The margins are in real coordinates
    margins: Point,
    /// Chunk size of the layer
    chunk_size: Point,
    /// Chunk storage
    storage: HashMap<ChunkIdx, ChunkWrapper>,
    /// Generate chunk function
    generate: ChunkGenerator,
}

impl LayerConfig {
    pub(crate) fn requires(&self) -> Vec<(LayerId, Bounds)> {
        self.storage.iter().map(|(idx, chunk)| {
            let Vec2 { x: width, y: height } = self.chunk_size;
            let bounds = idx.to_bounds(width, height);
            self.depends_on.iter().map(move |dep| {
                let padding = dep.padding;
                (dep.layer_id, bounds.add_padding(padding))
            })
        }).flatten().collect()
        // TODO: Merge the bounds, if they overlap
    }
}

impl LayerConfig {
    pub(crate) fn ensure_generated(&mut self, bounds: &Bounds) {
        // Check if the bounds are already generated
        for chunk_idx in bounds.chunks(self.chunk_size) {
            if !self.storage.contains_key(&chunk_idx) {
                // Generate the chunk
                let chunk = ChunkWrapper::new();
                self.storage.insert(chunk_idx, chunk);
            }
            let chunk_wrapper = self.storage.get_mut(&chunk_idx).unwrap();
            chunk_wrapper.usage_counter.increment(UsageStrategy::Fast); // TODO: Implement the correct usage strategy

        }
    }

    pub(crate) fn generate(&mut self, lookup: &LayerLookupChunk) {
        for (chunk_idx, mut chunk) in self.storage.iter_mut() {
            if chunk.chunk.is_none() {
                let gen_chunk = (self.generate)(lookup, chunk_idx);
                chunk.chunk = Some(gen_chunk);
            }
        }
    }
}

trait IntoLayerConfig {
    fn into_layer_config(self) -> LayerConfig;
}

pub(crate) trait Chunk: Send + Sync + Downcast + Debug + 'static {
    fn get_size() -> Vec2
    where
        Self: Sized;
}
impl_downcast!(Chunk);

#[derive(Debug)]
struct ChunkWrapper {
    chunk: Option<Box<dyn Chunk>>,
    usage_counter: UsageCounter,
}

impl ChunkWrapper {
    fn new() -> Self {
        ChunkWrapper {
            chunk: None,
            usage_counter: UsageCounter::new(),
        }
    }
}

pub(crate) trait Layer {
    // Required
    type Chunk: Chunk;

    fn generate(&self, lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx) -> Self::Chunk;

    // Optional
    fn get_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn get_margin(&self) -> Point {
        Vec2::new(0.0, 0.0)
    }

    // Given

    fn get_layer_id(&self) -> LayerId
    where
        Self: Sized + 'static,
    {
        LayerId::from_type::<Self>()
    }
}

/// The dependency of a layer
/// The padding is in real coordinates
#[derive(Debug)]
pub(crate) struct Dependency {
    layer_id: LayerId,
    padding: Point,
}

impl Dependency {
    pub(crate) fn new<T: Layer + Sized + 'static>(padding: Point) -> Self {
        Dependency {
            layer_id: LayerId::from_type::<T>(),
            padding,
        }
    }
}

impl<T> IntoLayerConfig for T
where
    T: Layer + 'static,
    T::Chunk: Chunk,
{
    fn into_layer_config(self) -> LayerConfig {
        LayerConfig {
            layer_id: LayerId::from_type::<T>(),
            depends_on: self.get_dependencies(),
            margins: self.get_margin(),
            chunk_size: T::Chunk::get_size(),
            storage: HashMap::new(),
            generate: Box::new(move |lookup: &LayerLookupChunk, chunk_idx: &ChunkIdx| {
                Box::new(self.generate(lookup, chunk_idx))
            }),
        }
    }
}

// Tests
#[cfg(test)]
mod test {
    use super::*;

    mod test_layer {
        use super::*;
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
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayer)
                .build();
            layers_manager.print_dot();
            layers_manager.regenerate();
        }
    }

    mod test_layer_with_dependencies {
        use super::*;

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
                vec![
                    Dependency::new::<TestLayerB>(Vec2::new(1.0, 1.0))
                ]
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
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayerA)
                .build();
            layers_manager.add_layer_client(LayerClient::new(Vec2::new(0.0, 0.0), vec![
                Dependency::new::<TestLayerA>(Vec2::new(2.0, 2.0))
            ], UsageStrategy::Fast));
            layers_manager.regenerate();
            // Check if the chunk is generated correctly
            assert!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0)).is_some());
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0)).unwrap().x, 0);
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(0.0, 0.0)).unwrap().y, 0);
            assert!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0)).is_some());
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0)).unwrap().x, 2);
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(2.0, 2.0)).unwrap().y, 2);
            assert!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(3.0, 3.0)).is_none());
            assert!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0)).is_some());
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0)).unwrap().x, -1);
            assert_eq!(layers_manager.get_chunk::<TestLayerA>(Vec2::new(-1.0, -1.0)).unwrap().y, -1);
        }
    }

    mod test_simple_generation_with_deps {
        use super::*;
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
                let closest_point = points.iter().min_by(|a, b| {
                    let a_dist = a.point.distance(chunk_idx.center(Self::Chunk::get_size()));
                    let b_dist = b.point.distance(chunk_idx.center(Self::Chunk::get_size()));
                    a_dist.partial_cmp(&b_dist).unwrap()
                }).unwrap();
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

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(PointsLayer)
                .add_layer(VoronoiLayer)
                .build();
            layers_manager.add_layer_client(LayerClient::new(Vec2::new(0.0, 0.0), vec![
                Dependency::new::<VoronoiLayer>(Vec2::new(8.0, 9.0))
            ], UsageStrategy::Fast));
            layers_manager.regenerate();
            // Check if the chunk is generated correctly
            assert!(layers_manager.get_chunk::<VoronoiLayer>(Vec2::new(0.0, 0.0)).is_some());
            assert_eq!(layers_manager.get_chunk::<VoronoiLayer>(Vec2::new(0.0, 0.0)).unwrap().color, (193, 180, 73));
        }
    }
}

