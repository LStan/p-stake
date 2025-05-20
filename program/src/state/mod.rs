pub mod authorized;
pub mod delegation;
pub mod lockup;
pub mod merge_kind;
pub mod meta;
pub mod pod;
pub mod stake;
pub mod stake_flags;
pub mod stake_history_entry;
pub mod stake_history_sysvar;
pub mod stake_state_v2;
pub mod vote_state;

pub use authorized::*;
pub use delegation::*;
pub use lockup::*;
pub use merge_kind::*;
pub use meta::*;
pub use pod::*;
pub use stake::*;
pub use stake_flags::*;
pub use stake_history_entry::*;
pub use stake_history_sysvar::*;
pub use stake_state_v2::*;
pub use vote_state::*;

use pinocchio::{
    account_info::{AccountInfo, Ref, RefMut},
    program_error::ProgramError,
};

pub type Epoch = PodU64;
pub type UnixTimestamp = PodI64;

pub fn get_stake_state(
    stake_account_info: &AccountInfo,
) -> Result<Ref<StakeStateV2>, ProgramError> {
    if stake_account_info.is_owned_by(&crate::ID) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    StakeStateV2::from_account_info(stake_account_info)
}

/// # Safety
///
/// The caller must ensure that it is safe to borrow the account data – e.g., there are
/// no mutable borrows of the account data.
pub unsafe fn get_stake_state_unchecked(
    stake_account_info: &AccountInfo,
) -> Result<&StakeStateV2, ProgramError> {
    if stake_account_info.owner() != &crate::ID {
        return Err(ProgramError::InvalidAccountOwner);
    }

    StakeStateV2::from_account_info_unchecked(stake_account_info)
}

pub fn try_get_stake_state_mut(
    stake_account_info: &AccountInfo,
) -> Result<RefMut<StakeStateV2>, ProgramError> {
    if stake_account_info.is_owned_by(&crate::ID) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    StakeStateV2::try_from_account_info_mut(stake_account_info)
}
