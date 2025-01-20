use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Mutex;
use crate::generative_chunks::layer::Layer;
use lazy_static::lazy_static;

lazy_static! {
    static ref LAYER_ID_MAP: Mutex<HashMap<TypeId, &'static str>> = Mutex::new(HashMap::new());
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(TypeId);

impl LayerId {
    pub fn from_type<T: Layer + 'static>() -> LayerId {
        let id = TypeId::of::<T>();
        {
            let mut map = LAYER_ID_MAP.lock().unwrap();
            map.insert(id, std::any::type_name::<T>());
        }
        LayerId(id)
    }
}

impl Debug for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = {
            let map = LAYER_ID_MAP.lock().unwrap();
            <&str>::clone(map.get(&self.0).unwrap())
        };
        write!(f, "{}", name)
    }
}