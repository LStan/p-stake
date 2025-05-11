use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, sysvars::clock::Clock,
    ProgramResult,
};

use crate::{
    error::to_program_error,
    pinocchio_add::clock,
    state::{
        get_stake_state, try_get_stake_state_mut, AuthorizeSignerArgs, StakeAuthorize, StakeStateV2,
    },
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct AuthorizeArgs {
    new_authority: Pubkey,
    authority_type: StakeAuthorize,
}

impl AuthorizeArgs {
    // TODO: optimize to avoid extra copy and return ref
    fn from_data(data: &[u8]) -> Result<AuthorizeArgs, ProgramError> {
        if data.len() != core::mem::size_of::<AuthorizeArgs>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        // check authority type variants
        if data[32] > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(unsafe { *(data.as_ptr() as *const Self) })
    }
}

pub fn process_authorize(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let authorize_args = AuthorizeArgs::from_data(data)?;

    let [stake_account_info, clock_info, _stake_or_withdraw_authority_info, remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock::from_account_info(clock_info)?;

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

    let signers_args = get_authorize_signer_args(stake_account_info, custodian, accounts)?;

    do_authorize(
        stake_account_info,
        signers_args,
        &authorize_args.new_authority,
        authorize_args.authority_type,
        custodian,
        clock,
    )?;

    Ok(())
}

pub fn process_authorize_checked(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() != 1 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let authority_type = match data[0] {
        0 => StakeAuthorize::Staker,
        1 => StakeAuthorize::Withdrawer,
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    let [stake_account_info, clock_info, _old_stake_or_withdraw_authority_info, new_stake_or_withdraw_authority_info, remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock::from_account_info(clock_info)?;

    if !new_stake_or_withdraw_authority_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

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

    let signers_args = get_authorize_signer_args(stake_account_info, custodian, accounts)?;

    do_authorize(
        stake_account_info,
        signers_args,
        new_stake_or_withdraw_authority_info.key(),
        authority_type,
        custodian,
        clock,
    )?;

    Ok(())
}

fn do_authorize(
    stake_account_info: &AccountInfo,
    signers_args: AuthorizeSignerArgs,
    new_authority: &Pubkey,
    authority_type: StakeAuthorize,
    custodian: Option<&Pubkey>,
    clock: &Clock,
) -> ProgramResult {
    let mut stake_account = try_get_stake_state_mut(stake_account_info)?;
    match &mut *stake_account {
        StakeStateV2::Initialized(meta) => meta
            .authorized
            .authorize(
                signers_args,
                new_authority,
                authority_type,
                (&meta.lockup, clock, custodian),
            )
            .map_err(to_program_error),
        StakeStateV2::Stake(meta, _stake, _stake_flags) => meta
            .authorized
            .authorize(
                signers_args,
                new_authority,
                authority_type,
                (&meta.lockup, clock, custodian),
            )
            .map_err(to_program_error),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

fn get_authorize_signer_args(
    stake_account_info: &AccountInfo,
    custodian: Option<&Pubkey>,
    accounts: &[AccountInfo],
) -> Result<AuthorizeSignerArgs, ProgramError> {
    let stake_account = get_stake_state(stake_account_info)?;

    let mut has_custodian_signer = false;
    let mut has_staker_signer = false;
    let mut has_withdrawer_signer = false;

    match *stake_account {
        StakeStateV2::Initialized(ref meta) | StakeStateV2::Stake(ref meta, _, _) => {
            for account in accounts {
                if account.is_signer() {
                    if meta.authorized.staker == *account.key() {
                        has_staker_signer = true;
                    }
                    if meta.authorized.withdrawer == *account.key() {
                        has_withdrawer_signer = true;
                    }
                    if let Some(custodian) = custodian {
                        if *custodian == *account.key() {
                            has_custodian_signer = true;
                        }
                    }
                }
            }
        }
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    Ok(AuthorizeSignerArgs {
        has_custodian_signer,
        has_staker_signer,
        has_withdrawer_signer,
    })
}
