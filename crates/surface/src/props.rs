//! The host props of an intrinsic element.

use crate::callback::Callback;

/// Host props shared by framework runtimes and renderers.
#[derive(Clone, Default, Debug)]
pub struct Props {
    pub class_name: Option<String>,
    pub style: Option<String>,
    pub on_click: Option<Callback<()>>,
    pub on_change: Option<Callback<String>>,
    pub on_checked_change: Option<Callback<bool>>,
    pub id: Option<String>,
    pub value: Option<String>,
    pub input_type: Option<String>,
    pub placeholder: Option<String>,
    pub checked: Option<bool>,
    pub aria_label: Option<String>,
    pub html_for: Option<String>,
    pub disabled: bool,
}
