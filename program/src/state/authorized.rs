use pinocchio::{pubkey::Pubkey, sysvars::clock::Clock};

use crate::error::InstructionError;

use super::Lockup;

#[repr(C)]
#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct Authorized {
    pub staker: Pubkey,
    pub withdrawer: Pubkey,
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StakeAuthorize {
    Staker = 0,
    Withdrawer = 1,
}

pub struct AuthorizeSignerArgs {
    pub has_custodian_signer: bool,
    pub has_staker_signer: bool,
    pub has_withdrawer_signer: bool,
}

impl Authorized {
    pub fn authorize(
        &mut self,
        signer_args: AuthorizeSignerArgs,
        new_authorized: &Pubkey,
        stake_authorize: StakeAuthorize,
        lockup_custodian_args: (&Lockup, &Clock, Option<&Pubkey>),
    ) -> Result<(), InstructionError> {
        match stake_authorize {
            StakeAuthorize::Staker => {
                // Allow either the staker or the withdrawer to change the staker key
                if !signer_args.has_staker_signer && !signer_args.has_withdrawer_signer {
                    return Err(InstructionError::MissingRequiredSignature);
                }
                self.staker = *new_authorized
            }
            StakeAuthorize::Withdrawer => {
                let (lockup, clock, custodian) = lockup_custodian_args;
                if lockup.is_in_force(clock, None) {
                    match custodian {
                        None => {
                            // TODO check error type
                            return Err(InstructionError::Custom(7));
                        }
                        Some(custodian) => {
                            // TODO: optimize, this check is not needed because custodian is always a signer if not None
                            if !signer_args.has_custodian_signer {
                                // TODO check error type
                                return Err(InstructionError::Custom(8));
                            }

                            if lockup.is_in_force(clock, Some(custodian)) {
                                // TODO check error type
                                return Err(InstructionError::Custom(1));
                            }
                        }
                    }
                }
                if !signer_args.has_withdrawer_signer {
                    return Err(InstructionError::MissingRequiredSignature);
                }
                self.withdrawer = *new_authorized
            }
        }
        Ok(())
    }
}
