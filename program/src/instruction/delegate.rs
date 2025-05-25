use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, sysvars::clock,
    ProgramResult,
};

use crate::{
    pinocchio_add::clock as clock_add,
    state::{
        get_credits, get_stake_state_mut, Delegation, Meta, Stake, StakeFlags, StakeHistorySysvar,
        StakeStateV2,
    },
    PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
};

pub fn process_delegate(accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    let [stake_account_info, vote_account_info, clock_info, _stake_history_info, _stake_config_info, _remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock_add::from_account_info(clock_info)?;
    let stake_history = &StakeHistorySysvar(clock.epoch);

    let credits = get_credits(vote_account_info)?;

    let mut source_stake = get_stake_state_mut(stake_account_info)?;

    match &mut *source_stake {
        StakeStateV2::Initialized(meta) => {
            check_signers(accounts, meta)?;

            let stake_amount = validate_delegated_amount(stake_account_info, meta)?;

            let stake = Stake {
                delegation: Delegation::new(
                    vote_account_info.key(),
                    stake_amount.into(),
                    clock.epoch.into(),
                ),
                credits_observed: credits.into(),
            };

            *source_stake = StakeStateV2::Stake(meta.clone(), stake, StakeFlags::empty());
            Ok(())
        }
        StakeStateV2::Stake(meta, stake, _flags) => {
            check_signers(accounts, meta)?;

            let stake_amount = validate_delegated_amount(stake_account_info, meta)?;

            redelegate_stake(
                stake,
                stake_amount,
                vote_account_info.key(),
                credits,
                clock.epoch,
                stake_history,
            )?;

            Ok(())
        }
        _ => Err(ProgramError::InvalidAccountData),
    }?;

    Ok(())
}

/// Ensure the stake delegation amount is valid.  This checks that the account
/// meets the minimum balance requirements of delegated stake.  If not, return
/// an error.
fn validate_delegated_amount(account: &AccountInfo, meta: &Meta) -> Result<u64, ProgramError> {
    let stake_amount = account
        .lamports()
        .saturating_sub(meta.rent_exempt_reserve.into()); // can't stake the rent

    // Stake accounts may be initialized with a stake amount below the minimum
    // delegation so check that the minimum is met before delegation.
    if stake_amount < crate::get_minimum_delegation() {
        // return Err(StakeError::InsufficientDelegation.into());
        return Err(ProgramError::Custom(12));
    }
    Ok(stake_amount)
}

fn redelegate_stake(
    stake: &mut Stake,
    stake_lamports: u64,
    voter_pubkey: &Pubkey,
    credits: u64,
    epoch: clock::Epoch,
    stake_history: &StakeHistorySysvar,
) -> Result<(), ProgramError> {
    // If stake is currently active:
    if stake.delegation.get_effective_stake(
        epoch,
        stake_history,
        PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
    ) != 0
    {
        // If pubkey of new voter is the same as current,
        // and we are scheduled to start deactivating this epoch,
        // we rescind deactivation
        if stake.delegation.voter_pubkey == *voter_pubkey
            && epoch == u64::from(stake.delegation.deactivation_epoch)
        {
            stake.delegation.deactivation_epoch = u64::MAX.into();
            return Ok(());
        } else {
            // can't redelegate to another pubkey if stake is active.
            // return Err(StakeError::TooSoonToRedelegate.into());
            return Err(ProgramError::Custom(3));
        }
    }
    // Either the stake is freshly activated, is active but has been
    // deactivated this epoch, or has fully de-activated.
    // Redelegation implies either re-activation or un-deactivation

    stake.delegation.stake = stake_lamports.into();
    stake.delegation.activation_epoch = epoch.into();
    stake.delegation.deactivation_epoch = u64::MAX.into();
    stake.delegation.voter_pubkey = *voter_pubkey;
    stake.credits_observed = credits.into();
    Ok(())
}

// TODO probably inline(always)
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
