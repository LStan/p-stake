use pinocchio::program_error::ProgramError;

use super::{Delegation, Epoch, PodU64};

#[repr(C)]
#[derive(Debug, Default, PartialEq, Clone)]
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

    pub fn split(
        &mut self,
        remaining_stake_delta: u64,
        split_stake_amount: u64,
    ) -> Result<Self, ProgramError> {
        let stake = u64::from(self.delegation.stake);
        if remaining_stake_delta > stake {
            // StakeError::InsufficientStake
            return Err(ProgramError::Custom(4));
        }
        self.delegation.stake = (stake - remaining_stake_delta).into();
        let new = Self {
            delegation: Delegation {
                stake: split_stake_amount.into(),
                ..self.delegation
            },
            ..*self
        };
        Ok(new)
    }
}
