use pinocchio::{program_error::ProgramError, sysvars::clock::Clock};

use crate::instruction::LockupArgs;

use super::{Authorized, Lockup, PodU64};

#[repr(C)]
#[derive(Default, Debug, PartialEq, Clone)]
pub struct Meta {
    pub rent_exempt_reserve: PodU64,
    pub authorized: Authorized,
    pub lockup: Lockup,
}

pub struct SetLockupSignerArgs {
    pub has_custodian_signer: bool,
    pub has_withdrawer_signer: bool,
}

impl Meta {
    pub fn set_lockup(
        &mut self,
        lockup: &LockupArgs,
        signer_args: SetLockupSignerArgs,
        clock: &Clock,
    ) -> Result<(), ProgramError> {
        // post-stake_program_v4 behavior:
        // * custodian can update the lockup while in force
        // * withdraw authority can set a new lockup
        if self.lockup.is_in_force(clock, None) {
            if !signer_args.has_custodian_signer {
                return Err(ProgramError::MissingRequiredSignature);
            }
        } else if !signer_args.has_withdrawer_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if let Some(unix_timestamp) = lockup.unix_timestamp {
            self.lockup.unix_timestamp = unix_timestamp;
        }
        if let Some(epoch) = lockup.epoch {
            self.lockup.epoch = epoch;
        }
        if let Some(custodian) = lockup.custodian {
            self.lockup.custodian = custodian;
        }
        Ok(())
    }
}
