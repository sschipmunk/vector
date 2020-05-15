use crate::transforms::{
    util::pick::{IterPicker, Passthrough, PickOnce},
    Transform,
};

mod cri;
mod docker;

pub type Parser = PickOnce<IterPicker<Vec<Box<dyn Transform>>>>;

pub fn build() -> Parser {
    let pickers = vec![docker::build(), cri::build(), Box::new(Passthrough)];
    PickOnce::new(IterPicker(pickers))
}
