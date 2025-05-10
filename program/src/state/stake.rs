use super::{Delegation, PodU64};

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct Stake {
    pub delegation: Delegation,
    /// credits observed is credits from vote account state when delegated or redeemed
    pub credits_observed: PodU64,
}
