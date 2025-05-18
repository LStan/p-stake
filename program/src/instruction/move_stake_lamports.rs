use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::state::{get_stake_state, MergeKind, StakeHistorySysvar};

use super::relocate_lamports;

pub fn process_move_lamports(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // let lamports = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let lamports = unsafe { *(data.as_ptr() as *const u64) };

    let [source_stake_account_info, destination_stake_account_info, stake_authority_info, _remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let (source_merge_kind, _) = move_stake_or_lamports_shared_checks(
        source_stake_account_info,
        lamports,
        destination_stake_account_info,
        stake_authority_info,
    )?;

    let source_free_lamports = match source_merge_kind {
        MergeKind::FullyActive(source_meta, source_stake) => source_stake_account_info
            .lamports()
            .saturating_sub(source_stake.delegation.stake.into())
            .saturating_sub(source_meta.rent_exempt_reserve.into()),
        MergeKind::Inactive(source_meta, source_lamports, _) => {
            source_lamports.saturating_sub(source_meta.rent_exempt_reserve.into())
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };

    if lamports > source_free_lamports {
        return Err(ProgramError::InvalidArgument);
    }

    relocate_lamports(
        source_stake_account_info,
        destination_stake_account_info,
        lamports,
    )?;

    Ok(())
}

// TODO: lamports are used only for one check and can be removed
fn move_stake_or_lamports_shared_checks(
    source_stake_account_info: &AccountInfo,
    lamports: u64,
    destination_stake_account_info: &AccountInfo,
    stake_authority_info: &AccountInfo,
) -> Result<(MergeKind, MergeKind), ProgramError> {
    // authority must sign
    if !stake_authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // confirm not the same account
    if *source_stake_account_info.key() == *destination_stake_account_info.key() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // source and destination must be writable
    // runtime guards against unowned writes, but MoveStake and MoveLamports are defined by SIMD
    // we check explicitly to avoid any possibility of a successful no-op that never attempts to write
    if !source_stake_account_info.is_writable() || !destination_stake_account_info.is_writable() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // must move something
    if lamports == 0 {
        return Err(ProgramError::InvalidArgument);
    }

    let clock = Clock::get()?;
    let stake_history = StakeHistorySysvar(clock.epoch);

    // get_if_mergeable ensures accounts are not partly activated or in any form of deactivating
    // we still need to exclude activating state ourselves
    let source_merge_kind = MergeKind::get_if_mergeable(
        &*get_stake_state(source_stake_account_info)?,
        source_stake_account_info.lamports(),
        &clock,
        &stake_history,
    )?;

    // Authorized staker is allowed to move stake
    if source_merge_kind.meta().authorized.staker != *stake_authority_info.key() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // same transient assurance as with source
    let destination_merge_kind = MergeKind::get_if_mergeable(
        &*get_stake_state(destination_stake_account_info)?,
        destination_stake_account_info.lamports(),
        &clock,
        &stake_history,
    )?;

    // ensure all authorities match and lockups match if lockup is in force
    MergeKind::metas_can_merge(
        source_merge_kind.meta(),
        destination_merge_kind.meta(),
        &clock,
    )?;

    Ok((source_merge_kind, destination_merge_kind))
}
