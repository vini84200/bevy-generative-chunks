mod usage;
mod bounds;


struct LayersManager {
    layers: Vec<LayerConfig>,
}

impl LayersManager {
    fn new() -> Self {
        LayersManager {
            layers: Vec::new(),
        }
    }

    fn add_layer(&mut self, layer: impl IntoLayerConfig) {
        self.layers.push(layer.into_layer_config());
    }
}

struct LayerConfig {
}

trait IntoLayerConfig {
    fn into_layer_config(self) -> LayerConfig;
}

trait Chunk {

}

trait Layer {
    type Chunk: Chunk;

    fn into_layer_config(self) -> LayerConfig where Self: Sized {
        LayerConfig {}
    }
}

impl<T> IntoLayerConfig for T
where
    T: Layer,
    T::Chunk: Chunk
{
    fn into_layer_config(self) -> LayerConfig {
        LayerConfig {}
    }
}

// Tests
#[cfg(test)]
mod test {
    use super::*;

    struct TestLayer;

    struct TestChunk;

    impl Chunk for TestChunk {

    }

    impl Layer for TestLayer {
        type Chunk = TestChunk;
    }

    #[test]
    fn test_layers_manager() {
        let mut layers_manager = LayersManager::new();
        layers_manager.add_layer(TestLayer);
    }
}

