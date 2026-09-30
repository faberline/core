use crate::application::certificate::reconcile::Reconciler;
use crate::domain::certificate::issuer::Issuer;
use crate::domain::certificate::profile::InstanceScope;
use crate::domain::certificate::projection::Owner;
use crate::domain::certificate::secret_store::SecretStore;
use crate::infrastructure::certificate::csr::RcgenCsrGenerator;
use crate::infrastructure::certificate::leaf_parser::X509LeafParser;

impl<'a> Reconciler<'a> {
    /// A reconciler for one instance's scope and owner, over `store` and
    /// `issuer`. Keys and CSRs come from rcgen; stored leaves are read with
    /// `x509_parser`.
    pub fn new(
        scope: &'a InstanceScope,
        owner: &'a Owner,
        store: &'a dyn SecretStore,
        issuer: &'a dyn Issuer,
    ) -> Self {
        Self::with_ports(
            scope,
            owner,
            store,
            issuer,
            &RcgenCsrGenerator,
            &X509LeafParser,
        )
    }
}
