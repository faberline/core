//! `<tool> dockerfile` — the `dockerfile` verb: render a service Dockerfile
//! (`source` or `release`) from parameters.
//!
//! The verb is clap-agnostic: each CLI keeps its own clap registration and
//! calls [`run_render`] with its [`DockerfileParams`] and [`DockerfileRequest`].

pub use crate::application::dockerfile::{
    run_render, DockerfileParams, DockerfileRequest, MarkerRule, Next, OutPathRule, ReleaseRewrite,
    Variant, VersionForm, DEFAULT_RELEASE_FILE, DEFAULT_SOURCE_FILE,
};
