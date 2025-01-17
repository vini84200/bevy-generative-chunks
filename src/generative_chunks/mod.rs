use daggy::{Dag, NodeIndex, petgraph::dot::{Dot, Config}};
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::Debug;
use daggy::petgraph::visit::NodeRef;

mod usage;
mod bounds;


struct LayersManagerBuilder {
    layers: Vec<LayerConfig>,
}

type LayerId = TypeId;

#[derive(Debug)]
struct LayersManager {
    layers: Vec<LayerConfig>,
    dag: Dag<LayerId, ()>,
    dag_index: HashMap<LayerId, NodeIndex>,
}

impl LayersManager {
    pub fn print_dot(&self) {
        println!("{:?}", Dot::with_config(&self.dag.graph(), &[
            Config::EdgeNoLabel,
            Config::NodeIndexLabel
        ]
        ));
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
        let mut dag = Dag::new();
        let mut dag_index = HashMap::new();

        for layer in self.layers.iter() {
            dag_index.insert(layer.layer_id, dag.add_node(layer.layer_id));
        }
        for layer in self.layers.iter() {
            let idx = dag_index.get(&layer.layer_id).unwrap();
            dag.add_edges(
                layer.depends_on.iter().map(|id| (dag_index.get(id).unwrap().clone(), idx.clone(), ()))
            ).expect("Adding edges to DAG created a cycle");
        }
        LayersManager {
            layers: self.layers,
            dag,
            dag_index,
        }
    }
}

#[derive(Debug)]
struct LayerConfig {
    layer_id: LayerId,
    depends_on: Vec<LayerId>,
}

trait IntoLayerConfig {
    fn into_layer_config(self) -> LayerConfig;
}

trait Chunk: Send + Sync + 'static {}

trait Layer {
    type Chunk: Chunk;

    fn get_dependencies(&self) -> Vec<LayerId> {
        vec![]
    }
}

impl<T> IntoLayerConfig for T
where
    T: Layer + 'static,
    T::Chunk: Chunk,
{
    fn into_layer_config(self) -> LayerConfig {
        LayerConfig {
            layer_id: TypeId::of::<T>(),
            depends_on: self.get_dependencies(),
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

        struct TestChunk;

        impl Chunk for TestChunk {}

        impl Layer for TestLayer {
            type Chunk = TestChunk;
        }


        #[test]
        fn test_layers_manager() {
            let layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayer)
                .build();
            println!("{:?}", layers_manager);
            layers_manager.print_dot();
        }
    }

    mod test_layer_with_dependencies {
        use super::*;

        struct ChunkA;

        struct TestLayerA;

        impl Chunk for ChunkA {}

        impl Layer for TestLayerA {
            type Chunk = ChunkA;

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<TestLayerB>()]
            }
        }

        struct TestLayerB;

        impl Layer for TestLayerB {
            type Chunk = ChunkA;
        }

        #[test]
        fn test_layers_manager() {
            let layers_manager = LayersManagerBuilder::new()
                .add_layer(TestLayerA)
                .add_layer(TestLayerB)
                .build();
            println!("{:?}", layers_manager);
            layers_manager.print_dot();
        }
    }
}

