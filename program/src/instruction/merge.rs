use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

use crate::{
    instruction::relocate_lamports,
    pinocchio_add::clock,
    state::{try_get_stake_state_mut, MergeKind, Meta, StakeHistorySysvar, StakeStateV2},
};

pub fn process_merge(accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    let [destination_stake_account_info, source_stake_account_info, clock_info, _stake_history_info, _remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = clock::from_account_info(clock_info)?;
    let stake_history = &StakeHistorySysvar(clock.epoch);

    if source_stake_account_info.key() == destination_stake_account_info.key() {
        return Err(ProgramError::InvalidArgument);
    }

    let mut source_stake = try_get_stake_state_mut(source_stake_account_info)?;
    let mut destination_stake = try_get_stake_state_mut(destination_stake_account_info)?;

    #[cfg(feature = "logging")]
    pinocchio::msg!("Checking if destination stake is mergeable");
    let destination_merge_kind = MergeKind::get_if_mergeable(
        &*destination_stake,
        destination_stake_account_info.lamports(),
        &*clock,
        stake_history,
    )?;

    // Authorized staker is allowed to split/merge accounts
    check_signers(accounts, destination_merge_kind.meta())?;

    #[cfg(feature = "logging")]
    pinocchio::msg!("Checking if source stake is mergeable");
    let source_merge_kind = MergeKind::get_if_mergeable(
        &*source_stake,
        source_stake_account_info.lamports(),
        &*clock,
        stake_history,
    )?;

    #[cfg(feature = "logging")]
    pinocchio::msg!("Merging stake accounts");
    if let Some(merged_state) = destination_merge_kind.merge(source_merge_kind, &*clock)? {
        // set_stake_state(destination_stake_account_info, &merged_state)?;
        *destination_stake = merged_state;
    }

    // Source is about to be drained, deinitialize its state
    *source_stake = StakeStateV2::Uninitialized;

    // Drain the source stake account
    relocate_lamports(
        source_stake_account_info,
        destination_stake_account_info,
        source_stake_account_info.lamports(),
    )?;

    Ok(())
}

// TODO probably inline(always)
#[inline]
fn check_signers(accounts: &[AccountInfo], meta: &Meta) -> Result<(), ProgramError> {
    let mut has_signer = false;
    for account in accounts {
        if account.is_signer() {
            if meta.authorized.staker == *account.key() {
                has_signer = true;
            }
        }
    }
    if !has_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}
