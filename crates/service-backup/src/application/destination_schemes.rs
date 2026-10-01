//! What a backup run in this build does with a destination of each scheme.
//!
//! The domain table ([`SUPPORTED_SCHEMES`]) records what `from_uri` parses and
//! a `cfg!` bool per scheme. This query turns that bool into the answer a
//! caller acts on: whether a run writes to the destination, or parses it and
//! then fails loud at `put` until the runner is rebuilt with the adapter
//! feature.

use crate::domain::SUPPORTED_SCHEMES;

/// What a backup run in this build does with a destination of one scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SinkSupport {
    /// A sink is linked in: `put` and `prune` reach the destination.
    Linked,
    /// The URI parses, but no sink is linked in: `put` and `prune` fail loud
    /// until the runner is rebuilt with the adapter feature.
    ParseOnly,
}

/// One destination scheme, as a backup run in this build sees it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DestinationSchemeView {
    scheme: &'static str,
    description: &'static str,
    support: SinkSupport,
}

impl DestinationSchemeView {
    /// The URI prefix, such as `s3://`.
    pub(crate) fn scheme(&self) -> &'static str {
        self.scheme
    }

    /// A human-readable description of the scheme.
    pub(crate) fn description(&self) -> &'static str {
        self.description
    }

    /// Whether a run in this build writes to it.
    pub(crate) fn support(&self) -> SinkSupport {
        self.support
    }
}

/// Every scheme `BackupDestination::from_uri` accepts, in its parse order,
/// with what a backup run in this build does with it.
pub(crate) fn destination_schemes() -> Vec<DestinationSchemeView> {
    SUPPORTED_SCHEMES
        .iter()
        .map(|info| DestinationSchemeView {
            scheme: info.scheme,
            description: info.description,
            support: if info.sink_available {
                SinkSupport::Linked
            } else {
                SinkSupport::ParseOnly
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scheme_is_listed_in_parse_order_with_its_support() {
        let views = destination_schemes();
        assert_eq!(views.len(), SUPPORTED_SCHEMES.len());
        for (view, info) in views.iter().zip(SUPPORTED_SCHEMES) {
            assert_eq!(view.scheme(), info.scheme);
            assert_eq!(view.description(), info.description);
            assert_eq!(view.support() == SinkSupport::Linked, info.sink_available);
        }
    }

    #[test]
    fn s3_support_follows_the_feature() {
        let s3 = destination_schemes()
            .into_iter()
            .find(|view| view.scheme() == "s3://")
            .unwrap();
        let expected = if cfg!(feature = "s3") {
            SinkSupport::Linked
        } else {
            SinkSupport::ParseOnly
        };
        assert_eq!(s3.support(), expected);
    }
}
