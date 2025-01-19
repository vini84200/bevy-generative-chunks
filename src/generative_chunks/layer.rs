use std::collections::HashMap;
use bevy::math::Vec2;
use downcast_rs::{impl_downcast, Downcast};
use std::fmt::Debug;
use crate::generative_chunks::bounds::{Bounds, ChunkIdx, Point};
use crate::generative_chunks::layer_id::LayerId;
use crate::generative_chunks::layer_manager::LayerLookupChunk;
use crate::generative_chunks::usage::{UsageCounter, UsageStrategy};

type ChunkGenerator = Box<dyn Fn(&LayerLookupChunk, &ChunkIdx) -> Box<dyn Chunk>>;

// #[derive(Debug)]
pub struct LayerConfig {
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
    pub fn requires(&self) -> Vec<(LayerId, Bounds)> {
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
    pub fn ensure_generated(&mut self, bounds: &Bounds) {
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

    pub fn get_chunk_size(&self) -> Point {
        self.chunk_size
    }

    pub fn get_margin(&self) -> Point {
        self.margins
    }

    pub fn get_layer_id(&self) -> LayerId {
        self.layer_id
    }

    pub fn get_storage(&self) -> &HashMap<ChunkIdx, ChunkWrapper> {
        &self.storage
    }

    pub fn get_storage_mut(&mut self) -> &mut HashMap<ChunkIdx, ChunkWrapper> {
        &mut self.storage
    }

    pub fn get_dependencies(&self) -> &Vec<Dependency> {
        &self.depends_on
    }
}

pub trait IntoLayerConfig {
    fn into_layer_config(self) -> LayerConfig;
}

pub(crate) trait Chunk: Send + Sync + Downcast + Debug + 'static {
    fn get_size() -> Vec2
    where
        Self: Sized;
}
impl_downcast!(Chunk);

#[derive(Debug)]
pub struct ChunkWrapper {
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

    pub fn get_chunk<T: Chunk>(&self) -> Option<&T> {
        self.chunk.as_ref().and_then(|c| c.downcast_ref::<T>())
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

    pub(crate) fn get_layer_id(&self) -> LayerId {
        self.layer_id
    }

    pub(crate) fn get_padding(&self) -> Point {
        self.padding
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