//! The composition root: wiring that may use every layer.
//!
//! Each public constructor here keeps its signature and builds the adapters
//! the application type needs: the HTTPS Google sources and the Google clock
//! for `GoogleVerifier::google`, and the system millisecond clock for
//! `DelegatedAuthenticator::new` and `TokenSource::new`.

mod delegated_authenticator;
mod google_verifier;
mod token_source;
