use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::state::{
    get_stake_state, merge_delegation_stake_and_credits_observed, try_get_stake_state_mut,
    MergeKind, StakeFlags, StakeHistorySysvar, StakeStateV2,
};

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

pub fn process_move_stake(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
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

    let (source_merge_kind, destination_merge_kind) = move_stake_or_lamports_shared_checks(
        source_stake_account_info,
        lamports,
        destination_stake_account_info,
        stake_authority_info,
    )?;

    // source must be fully active
    let MergeKind::FullyActive(source_meta, mut source_stake) = source_merge_kind else {
        return Err(ProgramError::InvalidAccountData);
    };

    let minimum_delegation = crate::get_minimum_delegation();
    let source_effective_stake: u64 = source_stake.delegation.stake.into();

    // source cannot move more stake than it has, regardless of how many lamports it
    // has
    let source_final_stake = source_effective_stake
        .checked_sub(lamports)
        .ok_or(ProgramError::InvalidArgument)?;

    // unless all stake is being moved, source must retain at least the minimum
    // delegation
    if source_final_stake != 0 && source_final_stake < minimum_delegation {
        return Err(ProgramError::InvalidArgument);
    }

    let mut destanation_stake_account = try_get_stake_state_mut(destination_stake_account_info)?;

    // destination must be fully active or fully inactive
    let destination_meta_rent_exempt_reserve = match destination_merge_kind {
        MergeKind::FullyActive(destination_meta, mut destination_stake) => {
            // if active, destination must be delegated to the same vote account as source
            if source_stake.delegation.voter_pubkey != destination_stake.delegation.voter_pubkey {
                // return Err(StakeError::VoteAddressMismatch.into());
                return Err(ProgramError::Custom(10));
            }

            let destination_effective_stake: u64 = destination_stake.delegation.stake.into();
            let destination_final_stake = destination_effective_stake
                .checked_add(lamports)
                .ok_or(ProgramError::ArithmeticOverflow)?;

            // ensure destination meets miniumum delegation
            // since it is already active, this only really applies if the minimum is raised
            if destination_final_stake < minimum_delegation {
                return Err(ProgramError::InvalidArgument);
            }

            merge_delegation_stake_and_credits_observed(
                &mut destination_stake,
                lamports,
                source_stake.credits_observed.into(),
            )?;

            let rent_exempt_reserve = destination_meta.rent_exempt_reserve;

            // StakeFlags::empty() is valid here because the only existing stake flag,
            // MUST_FULLY_ACTIVATE_BEFORE_DEACTIVATION_IS_PERMITTED, does not apply to
            // active stakes
            *destanation_stake_account =
                StakeStateV2::Stake(destination_meta, destination_stake, StakeFlags::empty());

            rent_exempt_reserve
        }
        MergeKind::Inactive(destination_meta, _, _) => {
            // if destination is inactive, it must be given at least the minimum delegation
            if lamports < minimum_delegation {
                return Err(ProgramError::InvalidArgument);
            }

            let mut destination_stake = source_stake.clone();
            destination_stake.delegation.stake = lamports.into();

            let rent_exempt_reserve = destination_meta.rent_exempt_reserve;

            // StakeFlags::empty() is valid here because the only existing stake flag,
            // MUST_FULLY_ACTIVATE_BEFORE_DEACTIVATION_IS_PERMITTED, is cleared when a stake
            // is activated
            *destanation_stake_account =
                StakeStateV2::Stake(destination_meta, destination_stake, StakeFlags::empty());

            rent_exempt_reserve
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };

    let mut source_stake_account = try_get_stake_state_mut(source_stake_account_info)?;

    let source_meta_rent_exempt_reserve = source_meta.rent_exempt_reserve;

    if source_final_stake == 0 {
        *source_stake_account = StakeStateV2::Initialized(source_meta);
    } else {
        source_stake.delegation.stake = source_final_stake.into();

        // StakeFlags::empty() is valid here because the only existing stake flag,
        // MUST_FULLY_ACTIVATE_BEFORE_DEACTIVATION_IS_PERMITTED, does not apply to
        // active stakes
        *source_stake_account = StakeStateV2::Stake(source_meta, source_stake, StakeFlags::empty());
    }

    relocate_lamports(
        source_stake_account_info,
        destination_stake_account_info,
        lamports,
    )?;

    // this should be impossible, but because we do all our math with delegations,
    // best to guard it
    if source_stake_account_info.lamports() < u64::from(source_meta_rent_exempt_reserve)
        || destination_stake_account_info.lamports()
            < u64::from(destination_meta_rent_exempt_reserve)
    {
        #[cfg(feature = "logging")]
        pinocchio::msg!("Delegation calculations violated lamport balance assumptions");
        return Err(ProgramError::InvalidArgument);
    }

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
