use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

use crate::{
    pinocchio_add::clock,
    state::{get_stake_state_mut, StakeStateV2},
};

pub fn process_deactivate(accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    let [stake_account_info, clock_info, _remaining @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock::from_account_info(clock_info)?;

    let mut stake_account = get_stake_state_mut(stake_account_info)?;
    match &mut *stake_account {
        StakeStateV2::Stake(meta, stake, _stake_flags) => {
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

            stake.deactivate(clock.epoch.into())?;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    Ok(())
}
