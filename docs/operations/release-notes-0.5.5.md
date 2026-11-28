# Release notes 0.5.5

This page lists the changes from `0.5.4` to `0.5.5`.

## Fixed

- openapi-codegen: the Rust models compile when an inline `oneOf` branch has the name of a component, and when a field type contains its own owner (the field is a `Box`) (#669). A consumer of `v0.5.4` pins `v0.5.5`.
