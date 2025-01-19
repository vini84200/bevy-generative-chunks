use crate::generative_chunks::bounds::{Bounds, ChunkIdx, Point};
use crate::generative_chunks::layer::{Chunk, Dependency, IntoLayerConfig, Layer, LayerConfig};
use crate::generative_chunks::layer_client::{IntoLayerClient, LayerClient};
use crate::generative_chunks::layer_id::LayerId;
use bevy::math::Vec2;
use bimap::BiMap;
use daggy::petgraph::dot::{Config, Dot};
use daggy::petgraph::visit::Topo;
use daggy::{Dag, NodeIndex};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct LayersManagerBuilder {
    layers: Vec<LayerConfig>,
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
        let Vec2 {
            x: width,
            y: height,
        } = layer.get_chunk_size();
        let chunk_idx = ChunkIdx::from_point(pos, width, height);
        let wrapped_chunk = layer.get_storage().get(&chunk_idx)?;
        let data = wrapped_chunk.get_chunk::<L::Chunk>();
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
            let chunk = layer.get_storage().get(&chunk_idx);
            if let Some(chunk_wrapper) = chunk {
                let data = chunk_wrapper.get_chunk::<L::Chunk>();
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
        for (chunk_idx, chunk_wrapper) in layer.get_storage().iter() {
            let data = chunk_wrapper.get_chunk::<L::Chunk>();
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
    fn get_chunk_from_idx<L: Layer + 'static>(
        &self,
        layer_id: LayerId,
        chunk_idx: ChunkIdx,
    ) -> Option<L::Chunk>
    where
        L::Chunk: Clone,
    {
        let layer = self.layers.get(&layer_id).unwrap().borrow();
        let chunk = layer.get_storage().get(&chunk_idx)?;
        let data = chunk.get_chunk::<L::Chunk>();
        data.and_then(|c| Some(c.clone()))
    }

    fn get_chunk<L: Layer + 'static>(&self, layer_id: LayerId, pos: Point) -> Option<L::Chunk>
    where
        L::Chunk: Clone,
    {
        // Get the chunk index
        let Vec2 {
            x: width,
            y: height,
        } = L::Chunk::get_size();
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
    pub(crate) fn print_dot(&self) {
        println!(
            "{:?}",
            Dot::with_config(
                &self.dag.graph(),
                &[
                    Config::EdgeNoLabel,
                    // Config::NodeIndexLabel
                ]
            )
        );
    }

    pub fn regenerate(&mut self) {
        // Check what the layer clients need to be regenerated
        for layer_client in self.layer_client.iter_mut() {
            if !layer_client.is_active() {
                continue;
            }
            for dep in layer_client.get_dependencies().iter() {
                let mut layer = self
                    .layers
                    .get_mut(&dep.get_layer_id())
                    .unwrap()
                    .borrow_mut();
                layer.ensure_generated(
                    &Bounds::from_point(layer_client.get_center()).add_padding(dep.get_padding()),
                );
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
        LayersManagerBuilder { layers: Vec::new() }
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
            dag_index.insert(layer.get_layer_id(), dag.add_node(layer.get_layer_id()));
        }
        for layer in self.layers.iter() {
            let idx = dag_index.get_by_left(&layer.get_layer_id()).unwrap();
            dag.add_edges(layer.get_dependencies().iter().map(|id| {
                (
                    idx.clone(),
                    dag_index.get_by_left(&id.get_layer_id()).unwrap().clone(),
                    (),
                )
            }))
            .expect("Adding edges to DAG created a cycle");
        }
        for layer in self.layers {
            layers.insert(layer.get_layer_id(), RefCell::new(layer));
        }

        LayersManager {
            layers,
            dag,
            dag_index,
            layer_client: vec![],
        }
    }
}
