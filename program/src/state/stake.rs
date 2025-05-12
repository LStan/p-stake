use pinocchio::program_error::ProgramError;

use super::{Delegation, Epoch, PodU64};

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct Stake {
    pub delegation: Delegation,
    /// credits observed is credits from vote account state when delegated or redeemed
    pub credits_observed: PodU64,
}

impl Stake {
    pub fn deactivate(&mut self, epoch: Epoch) -> Result<(), ProgramError> {
        if u64::from(self.delegation.deactivation_epoch) != u64::MAX {
            // StakeError::AlreadyDeactivated
            Err(ProgramError::Custom(2))
        } else {
            self.delegation.deactivation_epoch = epoch;
            Ok(())
        }
    }
}
