use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

use crate::{
    pinocchio_add::clock,
    state::{try_get_stake_state_mut, Lockup, StakeHistorySysvar, StakeStateV2},
    PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
};

use super::relocate_lamports;

pub fn process_withdraw(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // let withdraw_lamports = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let withdraw_lamports = unsafe { *(data.as_ptr() as *const u64) };

    let [source_stake_account_info, destination_info, clock_info, _stake_history_info, withdraw_authority_info, remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !withdraw_authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let withdraw_authority = withdraw_authority_info.key();

    let custodian = if !remaining.is_empty() {
        let lockup_authority_info = unsafe { remaining.get_unchecked(0) };
        if lockup_authority_info.is_signer() {
            Some(lockup_authority_info.key())
        } else {
            return Err(ProgramError::MissingRequiredSignature);
        }
    } else {
        None
    };

    let clock = &clock::from_account_info(clock_info)?;

    let stake_history = &StakeHistorySysvar(clock.epoch);

    let mut stake_account = try_get_stake_state_mut(source_stake_account_info)?;

    // TODO: lockup copy happens here, but could be avoided using a ref or inline the is_in_force check
    let (lockup, reserve, is_staked) = match &*stake_account {
        StakeStateV2::Stake(meta, stake, _stake_flag) => {
            if let Some(custodian) = custodian {
                if meta.authorized.withdrawer != *custodian
                    && meta.authorized.withdrawer != *withdraw_authority
                {
                    return Err(ProgramError::MissingRequiredSignature);
                }
            } else if meta.authorized.withdrawer != *withdraw_authority {
                return Err(ProgramError::MissingRequiredSignature);
            }

            // if we have a deactivation epoch and we're in cooldown
            let staked = if clock.epoch >= u64::from(stake.delegation.deactivation_epoch) {
                stake.delegation.get_effective_stake(
                    clock.epoch,
                    stake_history,
                    PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
                )
            } else {
                // Assume full stake if the stake account hasn't been
                //  de-activated, because in the future the exposed stake
                //  might be higher than stake.stake() due to warmup
                stake.delegation.stake.into()
            };
            let staked_and_reserve = staked
                .checked_add(meta.rent_exempt_reserve.into())
                .ok_or(ProgramError::InsufficientFunds)?;
            (meta.lockup, staked_and_reserve, staked != 0)
        }
        StakeStateV2::Initialized(ref meta) => {
            if let Some(custodian) = custodian {
                if meta.authorized.withdrawer != *custodian
                    && meta.authorized.withdrawer != *withdraw_authority
                {
                    return Err(ProgramError::MissingRequiredSignature);
                }
            } else if meta.authorized.withdrawer != *withdraw_authority {
                return Err(ProgramError::MissingRequiredSignature);
            }
            // stake accounts must have a balance >= rent_exempt_reserve
            (meta.lockup, meta.rent_exempt_reserve.into(), false)
        }
        StakeStateV2::Uninitialized => {
            if let Some(custodian) = custodian {
                if *source_stake_account_info.key() != *custodian
                    && *source_stake_account_info.key() != *withdraw_authority
                {
                    return Err(ProgramError::MissingRequiredSignature);
                }
            } else if *source_stake_account_info.key() != *withdraw_authority {
                return Err(ProgramError::MissingRequiredSignature);
            }

            (Lockup::default(), 0, false) // no lockup, no restrictions
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };

    // verify that lockup has expired or that the withdrawal is signed by the
    // custodian both epoch and unix_timestamp must have passed
    if lockup.is_in_force(clock, custodian) {
        // StakeError::LockupInForce
        return Err(ProgramError::Custom(1));
    }

    let stake_account_lamports = source_stake_account_info.lamports();
    if withdraw_lamports == stake_account_lamports {
        // if the stake is active, we mustn't allow the account to go away
        if is_staked {
            return Err(ProgramError::InsufficientFunds);
        }

        // Deinitialize state upon zero balance
        *stake_account = StakeStateV2::Uninitialized;
    } else {
        // a partial withdrawal must not deplete the reserve
        let withdraw_lamports_and_reserve = withdraw_lamports
            .checked_add(reserve)
            .ok_or(ProgramError::InsufficientFunds)?;
        if withdraw_lamports_and_reserve > stake_account_lamports {
            return Err(ProgramError::InsufficientFunds);
        }
    }

    relocate_lamports(
        source_stake_account_info,
        destination_info,
        withdraw_lamports,
    )?;

    Ok(())
}
