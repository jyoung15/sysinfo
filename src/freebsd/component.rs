use crate::ComponentExt;

/// Component
pub struct Component {}

impl ComponentExt for Component {
    fn get_temperature(&self) -> f32 {
        todo!()
    }

    fn get_max(&self) -> f32 {
        todo!()
    }

    fn get_critical(&self) -> Option<f32> {
        todo!()
    }

    fn get_label(&self) -> &str {
        todo!()
    }

    fn refresh(&mut self) {
        todo!()
    }
}
