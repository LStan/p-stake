use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::{
    state::{get_stake_state, get_stake_state_mut, Meta, StakeHistorySysvar, StakeStateV2},
    PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
};

use super::relocate_lamports;

pub fn process_split(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let split_lamports = u64::from_le_bytes(data[0..8].try_into().unwrap());

    let [source_stake_account_info, destination_stake_account_info, _remaining @ ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = Clock::get()?;
    let stake_history = &StakeHistorySysvar(clock.epoch);

    if destination_stake_account_info.data_len() != StakeStateV2::size_of() {
        return Err(ProgramError::InvalidAccountData);
    }

    {
        let destanation_stake = get_stake_state(destination_stake_account_info)?;

        if let StakeStateV2::Uninitialized = *destanation_stake {
            // we can split into this
        } else {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let mut source_stake = get_stake_state_mut(source_stake_account_info)?;

    let source_lamport_balance = source_stake_account_info.lamports();
    let destination_lamport_balance = destination_stake_account_info.lamports();

    // TODO: this check is done in validate_split_amount and probably could be removed
    if split_lamports > source_lamport_balance {
        return Err(ProgramError::InsufficientFunds);
    }

    match &mut *source_stake {
        StakeStateV2::Stake(source_meta, source_stake, stake_flags) => {
            check_signers(accounts, source_meta)?;

            let minimum_delegation = crate::get_minimum_delegation();

            let effective_stake = source_stake.delegation.get_effective_stake(
                clock.epoch,
                stake_history,
                PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
            );

            let is_active = effective_stake > 0;

            // NOTE this function also internally summons Rent via syscall
            let validated_split_info = validate_split_amount(
                source_lamport_balance,
                destination_lamport_balance,
                split_lamports,
                source_meta,
                // destination_data_len,
                minimum_delegation,
                is_active,
            )?;

            // split the stake, subtract rent_exempt_balance unless
            // the destination account already has those lamports
            // in place.
            // this means that the new stake account will have a stake equivalent to
            // lamports minus rent_exempt_reserve if it starts out with a zero balance
            let (remaining_stake_delta, split_stake_amount) =
                if validated_split_info.source_remaining_balance == 0 {
                    // If split amount equals the full source stake (as implied by 0
                    // source_remaining_balance), the new split stake must equal the same
                    // amount, regardless of any current lamport balance in the split account.
                    // Since split accounts retain the state of their source account, this
                    // prevents any magic activation of stake by prefunding the split account.
                    //
                    // The new split stake also needs to ignore any positive delta between the
                    // original rent_exempt_reserve and the split_rent_exempt_reserve, in order
                    // to prevent magic activation of stake by splitting between accounts of
                    // different sizes.
                    let remaining_stake_delta =
                        split_lamports.saturating_sub(source_meta.rent_exempt_reserve.into());
                    (remaining_stake_delta, remaining_stake_delta)
                } else {
                    // Otherwise, the new split stake should reflect the entire split
                    // requested, less any lamports needed to cover the
                    // split_rent_exempt_reserve.
                    if u64::from(source_stake.delegation.stake).saturating_sub(split_lamports)
                        < minimum_delegation
                    {
                        // StakeError::InsufficientDelegation
                        return Err(ProgramError::Custom(12));
                    }

                    (
                        split_lamports,
                        split_lamports.saturating_sub(
                            validated_split_info
                                .destination_rent_exempt_reserve
                                .saturating_sub(destination_lamport_balance),
                        ),
                    )
                };

            if split_stake_amount < minimum_delegation {
                // StakeError::InsufficientDelegation
                return Err(ProgramError::Custom(12));
            }

            let destination_stake =
                source_stake.split(remaining_stake_delta, split_stake_amount)?;

            let mut destination_meta = source_meta.clone();
            destination_meta.rent_exempt_reserve =
                validated_split_info.destination_rent_exempt_reserve.into();

            // Safety: all checks were done above in get_stake_state,
            // also destination_stake_account_info is not borrowed and not the same as source_stake_account_info
            let destanation_stake = unsafe {
                StakeStateV2::from_bytes_mut(
                    destination_stake_account_info.borrow_mut_data_unchecked(),
                )
            };

            *destanation_stake =
                StakeStateV2::Stake(destination_meta, destination_stake, stake_flags.clone());
        }
        StakeStateV2::Initialized(source_meta) => {
            check_signers(accounts, source_meta)?;

            // NOTE this function also internally summons Rent via syscall
            let validated_split_info = validate_split_amount(
                source_lamport_balance,
                destination_lamport_balance,
                split_lamports,
                source_meta,
                // destination_data_len,
                0,     // additional_required_lamports
                false, // is_active
            )?;

            let mut destination_meta = source_meta.clone();
            destination_meta.rent_exempt_reserve =
                validated_split_info.destination_rent_exempt_reserve.into();

            // Safety: all checks were done above in get_stake_state,
            // also destination_stake_account_info is not borrowed and not the same as source_stake_account_info
            let destanation_stake = unsafe {
                StakeStateV2::from_bytes_mut(
                    destination_stake_account_info.borrow_mut_data_unchecked(),
                )
            };

            *destanation_stake = StakeStateV2::Initialized(destination_meta);
        }
        StakeStateV2::Uninitialized => {
            if !source_stake_account_info.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // Deinitialize state upon zero balance
    if split_lamports == source_lamport_balance {
        *source_stake = StakeStateV2::Uninitialized;
    }

    relocate_lamports(
        source_stake_account_info,
        destination_stake_account_info,
        split_lamports,
    )?;
    Ok(())
}

#[inline]
fn check_signers(accounts: &[AccountInfo], meta: &Meta) -> Result<(), ProgramError> {
    let mut has_signer = false;
    for account in accounts {
        if account.is_signer() && meta.authorized.staker == *account.key() {
            has_signer = true;
        }
    }
    if !has_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

struct ValidatedSplitInfo {
    pub source_remaining_balance: u64,
    pub destination_rent_exempt_reserve: u64,
}

fn validate_split_amount(
    source_lamports: u64,
    destination_lamports: u64,
    split_lamports: u64,
    source_meta: &Meta,
    // destination_data_len: usize,
    additional_required_lamports: u64,
    source_is_active: bool,
) -> Result<ValidatedSplitInfo, ProgramError> {
    // Split amount has to be something
    if split_lamports == 0 {
        return Err(ProgramError::InsufficientFunds);
    }

    // Obviously cannot split more than what the source account has
    if split_lamports > source_lamports {
        return Err(ProgramError::InsufficientFunds);
    }

    // Verify that the source account still has enough lamports left after
    // splitting: EITHER at least the minimum balance, OR zero (in this case the
    // source account is transferring all lamports to new destination account,
    // and the source account will be closed)
    let source_minimum_balance =
        u64::from(source_meta.rent_exempt_reserve).saturating_add(additional_required_lamports);
    let source_remaining_balance = source_lamports.saturating_sub(split_lamports);
    if source_remaining_balance == 0 {
        // full amount is a withdrawal
        // nothing to do here
    } else if source_remaining_balance < source_minimum_balance {
        // the remaining balance is too low to do the split
        return Err(ProgramError::InsufficientFunds);
    } else {
        // all clear!
        // nothing to do here
    }

    let rent = Rent::get()?;
    // let destination_rent_exempt_reserve = rent.minimum_balance(destination_data_len);
    let destination_rent_exempt_reserve = rent.minimum_balance(StakeStateV2::size_of());

    // If the source is active stake, one of these criteria must be met:
    // 1. the destination account must be prefunded with at least the rent-exempt
    //    reserve, or
    // 2. the split must consume 100% of the source
    if source_is_active
        && source_remaining_balance != 0
        && destination_lamports < destination_rent_exempt_reserve
    {
        return Err(ProgramError::InsufficientFunds);
    }

    // Verify the destination account meets the minimum balance requirements
    // This must handle:
    // 1. The destination account having a different rent exempt reserve due to data
    //    size changes
    // 2. The destination account being prefunded, which would lower the minimum
    //    split amount
    let destination_minimum_balance =
        destination_rent_exempt_reserve.saturating_add(additional_required_lamports);
    let destination_balance_deficit =
        destination_minimum_balance.saturating_sub(destination_lamports);
    if split_lamports < destination_balance_deficit {
        return Err(ProgramError::InsufficientFunds);
    }

    Ok(ValidatedSplitInfo {
        source_remaining_balance,
        destination_rent_exempt_reserve,
    })
}
