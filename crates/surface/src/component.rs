//! A component invocation: a named render function with type-erased props.

use std::rc::Rc;

use crate::element::Element;

/// Component = render function + typed props erased behind `Any`.
#[derive(Clone)]
pub struct Component {
    pub name: &'static str,
    pub render: ComponentFn,
    pub props: Rc<dyn std::any::Any>,
}

pub type ComponentFn = fn(&Rc<dyn std::any::Any>) -> Element;
