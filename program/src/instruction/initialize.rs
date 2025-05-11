use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, sysvars::rent::Rent, ProgramResult,
};

use crate::state::{try_get_stake_state_mut, Authorized, Lockup, Meta, StakeStateV2};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct InitializeArgs {
    authorized: Authorized,
    lockup: Lockup,
}

impl InitializeArgs {
    // TODO: optimize to avoid extra copy and return ref
    fn from_data(data: &[u8]) -> Result<InitializeArgs, ProgramError> {
        if data.len() != core::mem::size_of::<InitializeArgs>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(unsafe { *(data.as_ptr() as *const Self) })
    }
}

pub fn process_initialize(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let initialize_args = InitializeArgs::from_data(data)?;

    let [stake_account_info, rent_info, _remaining @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let rent = &Rent::from_account_info(rent_info)?;

    do_initialize(
        stake_account_info,
        initialize_args.authorized,
        initialize_args.lockup,
        rent,
    )?;

    Ok(())
}

pub fn process_initialize_checked(accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    let [stake_account_info, rent_info, stake_authority_info, withdraw_authority_info, _remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let rent = &Rent::from_account_info(rent_info)?;

    if !withdraw_authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let authorized = Authorized {
        staker: *stake_authority_info.key(),
        withdrawer: *withdraw_authority_info.key(),
    };

    do_initialize(stake_account_info, authorized, Lockup::default(), rent)?;

    Ok(())
}

fn do_initialize(
    stake_account_info: &AccountInfo,
    authorized: Authorized,
    lockup: Lockup,
    rent: &Rent,
) -> ProgramResult {
    let mut stake_account: pinocchio::account_info::RefMut<'_, StakeStateV2> =
        try_get_stake_state_mut(stake_account_info)?;

    match &mut *stake_account {
        StakeStateV2::Uninitialized => {
            let rent_exempt_reserve = rent.minimum_balance(stake_account_info.data_len());
            if stake_account_info.lamports() >= rent_exempt_reserve {
                *stake_account = StakeStateV2::Initialized(Meta {
                    rent_exempt_reserve: rent_exempt_reserve.into(),
                    authorized,
                    lockup,
                });
                Ok(())
            } else {
                return Err(ProgramError::InsufficientFunds);
            }
        }
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    }
}
