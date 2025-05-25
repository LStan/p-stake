use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{
        clock::{self, Clock},
        Sysvar,
    },
    ProgramResult,
};

use crate::state::{
    acceptable_reference_epoch_credits, get_last_epoch, get_stake_state_mut, StakeStateV2,
    MINIMUM_DELINQUENT_EPOCHS_FOR_DEACTIVATION,
};

pub fn process_deactivate_delinquent(accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    let [stake_account_info, delinquent_vote_account_info, reference_vote_account_info, _remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = Clock::get()?;

    // let delinquent_vote_state = get_vote_state(delinquent_vote_account_info)?;
    let delinquent_last_epoch = get_last_epoch(delinquent_vote_account_info)?;

    if !acceptable_reference_epoch_credits(reference_vote_account_info, clock.epoch)? {
        // return Err(StakeError::InsufficientReferenceVotes.into());
        return Err(ProgramError::Custom(9));
    }

    let mut stake_account = get_stake_state_mut(stake_account_info)?;

    if let StakeStateV2::Stake(_meta, stake, _stake_flags) = &mut *stake_account {
        if stake.delegation.voter_pubkey != *delinquent_vote_account_info.key() {
            // return Err(StakeError::VoteAddressMismatch.into());
            return Err(ProgramError::Custom(10));
        }

        // Deactivate the stake account if its delegated vote account has never voted or
        // has not voted in the last
        // `MINIMUM_DELINQUENT_EPOCHS_FOR_DEACTIVATION`
        if eligible_for_deactivate_delinquent(&delinquent_last_epoch, clock.epoch) {
            stake.deactivate(clock.epoch.into())?;
            Ok(())
        } else {
            // Err(StakeError::MinimumDelinquentEpochsForDeactivationNotMet.into())
            Err(ProgramError::Custom(11))
        }
    } else {
        Err(ProgramError::InvalidAccountData)
    }?;

    Ok(())
}

fn eligible_for_deactivate_delinquent(
    last_epoch: &Option<clock::Epoch>,
    current_epoch: clock::Epoch,
) -> bool {
    match last_epoch {
        None => true,
        Some(epoch) => {
            if let Some(minimum_epoch) =
                current_epoch.checked_sub(MINIMUM_DELINQUENT_EPOCHS_FOR_DEACTIVATION)
            {
                *epoch <= minimum_epoch
            } else {
                false
            }
        }
    }
}
