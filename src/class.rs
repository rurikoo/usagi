use crate::object::Object;

pub struct Class<'vm> {
    object: Object<'vm>,
}

impl<'vm> Class<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Self {
        Self { object }
    }

    /// the underlying object handle
    pub fn as_object(&self) -> &Object<'vm> {
        &self.object
    }
}
