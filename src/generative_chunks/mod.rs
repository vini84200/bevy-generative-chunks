use daggy::{Dag, NodeIndex, petgraph::dot::{Dot, Config}, Walker};
use std::collections::HashMap;
use std::fmt::Debug;
use bimap::BiMap;
use daggy::petgraph::visit::{NodeRef, Topo, Visitable};
use downcast_rs::{impl_downcast, Downcast};
use crate::generative_chunks::bounds::{Bounds, ChunkIdx, Point};
use crate::generative_chunks::usage::{UsageStrategy, UsageCounter};

mod usage;
mod bounds;


struct LayersManagerBuilder {
    layers: Vec<LayerConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct  LayerId (&'static str);

impl LayerId {
    fn from_type<T: Layer + 'static>() -> LayerId {
        LayerId(std::any::type_name::<T>())
    }
}

#[derive(Debug)]
struct LayerClient {
    active: bool,
    center: (f32, f32),
    dependencies: Vec<Dependency>,
    strategy: UsageStrategy,
}

impl LayerClient {
    fn new(center: (f32, f32), dependencies: Vec<Dependency>, strength: UsageStrategy) -> Self {
        LayerClient {
            active: false,
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




#[derive(Debug)]
struct LayersManager {
    layers: HashMap<LayerId, LayerConfig>,
    dag: Dag<LayerId, ()>,
    dag_index: BiMap<LayerId, NodeIndex>,
    layer_client: Vec<LayerClient>,
}

trait LayerLookupChunk {

    fn get_chunk<C: Chunk
    >(&self, layer_id: LayerId, pos: Point) -> Option<&dyn Chunk>;
}

struct LayerLookupChunkImpl<'a> {
    layers: &'a HashMap<LayerId, LayerConfig>,
}

impl LayerLookupChunk for LayerLookupChunkImpl<'_> {
    fn get_chunk<C: Chunk>(&self, layer_id: LayerId, pos: Point) -> Option<&dyn Chunk> {
        let layer = self.layers.get(&layer_id)?;
        let (width, height) = layer.chunk_size;
        let chunk_idx = ChunkIdx::from_point(pos, width, height);
        let chunk = layer.storage.get(&chunk_idx)?;
        chunk.chunk.as_ref().and_then(|c| c.downcast_ref::<C>())
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

    fn add_layer_client(&mut self, layer_client: impl IntoLayerClient) {
        self.layer_client.push(layer_client.into_layer_client());
    }

    fn clear_layer_clients(&mut self) {
        self.layer_client.clear();
    }

    fn regenerate(&mut self) {
        // Check what the layer clients need to be regenerated
        for layer_client in self.layer_client.iter_mut() {
            if !layer_client.is_active() {
                continue;
            }
            for Dependency {
                padding,
                layer_id
            } in layer_client.dependencies.iter() {
                let layer = self.layers.get_mut(layer_id).unwrap();
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
            let layer = self.layers.get(&layer_id).unwrap();
            let requirements = layer.requires();
            for (dependency_id, bounds) in requirements {
                let dependency = self.layers.get_mut(&dependency_id).unwrap();
                dependency.ensure_generated(&bounds);
            }
            stack.push(node);
        }

        let layer_lookup = LayerLookupChunkImpl {
            layers: &self.layers,
        };
        // Now we can generate the chunks, by transversing the DAG in topological order in reverse
        stack.iter().rev().for_each(|node| {
            let layer_id = self.dag[*node];
            let layer = self.layers.get_mut(&layer_id).unwrap();
            // Generate the chunks
            layer.generate(layer_lookup);
        });

    }

}




impl LayersManagerBuilder {
    fn new() -> Self {
        LayersManagerBuilder {
            layers: Vec::new(),
        }
    }

    fn add_layer(mut self, layer: impl IntoLayerConfig) -> Self {
        self.layers.push(layer.into_layer_config());
        self
    }

    fn build(self) -> LayersManager {
        let mut layers:HashMap<LayerId, LayerConfig> = HashMap::new();
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
            layers.insert(layer.layer_id, layer);
        }

        LayersManager {
            layers,
            dag,
            dag_index,
            layer_client: vec![]
        }
    }
}


#[derive(Debug)]
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
}

impl LayerConfig {
    pub(crate) fn requires(&self) -> Vec<(LayerId, Bounds)> {
        self.storage.iter().map(|(idx, chunk)| {
            let (width , height) = self.chunk_size;
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

    pub(crate) fn generate(&mut self, lookup: impl LayerLookupChunk) {

    }
}

trait IntoLayerConfig {
    fn into_layer_config(self) -> LayerConfig;
}

trait Chunk: Send + Sync + Downcast + Debug + 'static {
    fn get_size() -> (f32, f32) where Self: Sized;
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

trait Layer {
    type Chunk: Chunk;

    fn get_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn get_margin(&self) -> Point {
        (0.0, 0.0)
    }

    fn get_layer_id(&self) -> LayerId where Self: Sized + 'static {
        LayerId::from_type::<Self>()
    }
}

/// The dependency of a layer
/// The padding is in real coordinates
#[derive(Debug)]
struct Dependency {
    layer_id: LayerId,
    padding: Point
}

impl Dependency {
    fn new<T: Layer + Sized + 'static>(padding: Point) -> Self {
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
            fn get_size() -> (f32, f32) {
                (1., 1.)
            }
        }


        impl Layer for TestLayer {
            type Chunk = TestChunk;
        }


        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayer)
                .build();
            println!("{:?}", layers_manager);
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
            fn get_size() -> (f32, f32) {
                (1., 1.)
            }
        }

        impl Layer for TestLayerA {
            type Chunk = ChunkA;

            fn get_dependencies(&self) -> Vec<Dependency> {
                vec![
                    Dependency::new::<TestLayerB>((1.0, 1.0))
                    ]
            }
        }

        struct TestLayerB;

        impl Layer for TestLayerB {
            type Chunk = ChunkA;
        }

        #[test]
        fn test_layers_manager() {
            let mut layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayerA)
                .add_layer(TestLayerB)
                .build();
            println!("{:?}", layers_manager);
            layers_manager.print_dot();
            layers_manager.regenerate();
        }
    }
}

