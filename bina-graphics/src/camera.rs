use bina_ecs::component::{NumberField, Component, NumberFieldRef, Processable};

use crate::polygon::Vector;

pub struct Camera {
    pub(crate) origin: NumberField<Vector>,
    pub(crate) scale: NumberField<Vector>,
    pub(crate) rotation: NumberField<f32>,
}


impl Component for Camera {
    type Reference<'a> = CameraRef<'a>;

    fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
        CameraRef {
            origin: self.origin.get_ref(),
            scale: self.scale.get_ref(),
            rotation: self.rotation.get_ref(),
        }
    }
}


impl Processable for Camera {
    fn process<E: bina_ecs::entity::Entity>(
        _component: Self::Reference<'_>,
        _my_entity: bina_ecs::entity::EntityReference<E>,
        _universe: &bina_ecs::universe::Universe,
    ) { }
}


#[derive(Clone, Copy)]
pub struct CameraRef<'a> {
    pub origin: NumberFieldRef<'a, Vector>,
    pub scale: NumberFieldRef<'a, Vector>,
    pub rotation: NumberFieldRef<'a, f32>,
}
