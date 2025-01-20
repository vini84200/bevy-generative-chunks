use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Mutex;
use crate::generative_chunks::layer::Layer;
use lazy_static::lazy_static;

lazy_static! {
    // Static but mutable hashmap
    static ref LAYER_ID_MAP: Mutex<HashMap<TypeId, &'static str>> = Mutex::new(HashMap::new());
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(TypeId);

impl LayerId {
    pub fn from_type<T: Layer + 'static>() -> LayerId {
        let mut map = LAYER_ID_MAP.lock().unwrap(); 
        let id = TypeId::of::<T>();
        map.insert(id, std::any::type_name::<T>());
        LayerId(id)
    }
}

impl Debug for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let map = LAYER_ID_MAP.lock().unwrap();
        let name = map.get(&self.0).unwrap();
        write!(f, "{}", name)
    }
}