use crate::generative_chunks::layer::Layer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(&'static str);

impl LayerId {
    pub fn from_type<T: Layer + 'static>() -> LayerId {
        LayerId(std::any::type_name::<T>())
    }
}
