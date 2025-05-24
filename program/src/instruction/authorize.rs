use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, sysvars::clock::Clock,
    ProgramResult,
};

use crate::{
    pinocchio_add::{clock, pubkey::create_with_seed},
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
    #[inline(always)]
    fn from_data(data: &[u8]) -> Result<AuthorizeArgs, ProgramError> {
        if data.len() < core::mem::size_of::<AuthorizeArgs>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        // check authority type variants
        if data[32] > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(unsafe { *(data.as_ptr() as *const Self) })
    }
}

#[derive(Debug)]
pub struct AuthorizeWithSeedArgs<'a> {
    pub new_authority: &'a Pubkey,
    pub authority_type: StakeAuthorize,
    pub authority_seed: &'a [u8],
    pub authority_owner: &'a Pubkey,
}

impl AuthorizeWithSeedArgs<'_> {
    #[inline(always)]
    fn from_data(data: &[u8]) -> Result<AuthorizeWithSeedArgs, ProgramError> {
        if data.len() < 32 + 4 + 8 + 32 {
            return Err(ProgramError::InvalidInstructionData);
        }
        // check authority type variants
        if data[32] > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed_size = u64::from_le_bytes(data[32 + 4..32 + 4 + 8].try_into().unwrap());
        // let seed_size = unsafe { (data.as_ptr().add(32 + 4) as *const u64).read_unaligned() };

        if data.len() < 32 + 4 + 8 + seed_size as usize + 32 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let args = AuthorizeWithSeedArgs {
            new_authority: unsafe { &*(data.as_ptr() as *const Pubkey) },
            authority_type: unsafe { *(data.as_ptr().add(32) as *const StakeAuthorize) },
            authority_seed: unsafe {
                core::slice::from_raw_parts(data.as_ptr().add(32 + 4 + 8), seed_size as usize)
            },
            authority_owner: unsafe {
                &*(data.as_ptr().add(32 + 4 + 8 + seed_size as usize) as *const Pubkey)
            },
        };

        Ok(args)
    }
}

#[derive(Debug)]
pub struct AuthorizeCheckedWithSeedArgs<'a> {
    pub authority_type: StakeAuthorize,
    pub authority_seed: &'a [u8],
    pub authority_owner: &'a Pubkey,
}

impl AuthorizeCheckedWithSeedArgs<'_> {
    #[inline(always)]
    fn from_data(data: &[u8]) -> Result<AuthorizeCheckedWithSeedArgs, ProgramError> {
        if data.len() < 4 + 8 + 32 {
            return Err(ProgramError::InvalidInstructionData);
        }
        // check authority type variants
        if data[32] > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed_size = u64::from_le_bytes(data[4..4 + 8].try_into().unwrap());
        // let seed_size = unsafe { (data.as_ptr().add(4) as *const u64).read_unaligned() };

        if data.len() < 4 + 8 + seed_size as usize + 32 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let args = AuthorizeCheckedWithSeedArgs {
            authority_type: unsafe { *(data.as_ptr() as *const StakeAuthorize) },
            authority_seed: unsafe {
                core::slice::from_raw_parts(data.as_ptr().add(4 + 8), seed_size as usize)
            },
            authority_owner: unsafe {
                &*(data.as_ptr().add(4 + 8 + seed_size as usize) as *const Pubkey)
            },
        };

        Ok(args)
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

    let signers_args = get_authorize_signer_args(stake_account_info, accounts)?;

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

pub fn process_authorize_with_seed(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let authorize_args = AuthorizeWithSeedArgs::from_data(data)?;

    let [stake_account_info, stake_or_withdraw_authority_base_info, clock_info, remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock::from_account_info(clock_info)?;

    let authority = None;

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

    let stake_or_withdraw_auth = if stake_or_withdraw_authority_base_info.is_signer() {
        Some(create_with_seed(
            stake_or_withdraw_authority_base_info.key(),
            &authorize_args.authority_seed,
            &authorize_args.authority_owner,
        )?)
    } else {
        None
    };

    let signers_args = get_authorize_signer_args_with_seed(
        stake_account_info,
        &authority,
        &custodian,
        &stake_or_withdraw_auth,
    )?;

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
    if data.len() < core::mem::size_of::<StakeAuthorize>() {
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

    let signers_args = get_authorize_signer_args(stake_account_info, accounts)?;

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

pub fn process_authorize_checked_with_seed(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let authorize_args = AuthorizeCheckedWithSeedArgs::from_data(data)?;

    let [stake_account_info, old_stake_or_withdraw_authority_base_info, clock_info, new_stake_or_withdraw_authority_info, remaining @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = &clock::from_account_info(clock_info)?;

    let authority = if new_stake_or_withdraw_authority_info.is_signer() {
        Some(new_stake_or_withdraw_authority_info.key())
    } else {
        return Err(ProgramError::MissingRequiredSignature);
    };

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

    let stake_or_withdraw_auth = if old_stake_or_withdraw_authority_base_info.is_signer() {
        Some(create_with_seed(
            old_stake_or_withdraw_authority_base_info.key(),
            &authorize_args.authority_seed,
            &authorize_args.authority_owner,
        )?)
    } else {
        None
    };

    let signers_args = get_authorize_signer_args_with_seed(
        stake_account_info,
        &authority,
        &custodian,
        &stake_or_withdraw_auth,
    )?;

    do_authorize(
        stake_account_info,
        signers_args,
        new_stake_or_withdraw_authority_info.key(),
        authorize_args.authority_type,
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
        StakeStateV2::Initialized(meta) => meta.authorized.authorize(
            signers_args,
            new_authority,
            authority_type,
            (&meta.lockup, clock, custodian),
        ),
        StakeStateV2::Stake(meta, _stake, _stake_flags) => meta.authorized.authorize(
            signers_args,
            new_authority,
            authority_type,
            (&meta.lockup, clock, custodian),
        ),
        _ => Err(ProgramError::InvalidAccountData), // TODO: probably unreachable
    }
}

fn get_authorize_signer_args(
    stake_account_info: &AccountInfo,
    accounts: &[AccountInfo],
) -> Result<AuthorizeSignerArgs, ProgramError> {
    let stake_account = get_stake_state(stake_account_info)?;

    let mut has_staker_signer = false;
    let mut has_withdrawer_signer = false;

    // TODO: check difference between * + ref and &*
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
                }
            }
        }
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    Ok(AuthorizeSignerArgs {
        has_staker_signer,
        has_withdrawer_signer,
    })
}

fn get_authorize_signer_args_with_seed(
    stake_account_info: &AccountInfo,
    authority: &Option<&Pubkey>,
    custodian: &Option<&Pubkey>,
    stake_or_withdraw_auth: &Option<Pubkey>,
) -> Result<AuthorizeSignerArgs, ProgramError> {
    let stake_account = get_stake_state(stake_account_info)?;

    let mut has_staker_signer = false;
    let mut has_withdrawer_signer = false;

    match *stake_account {
        StakeStateV2::Initialized(ref meta) | StakeStateV2::Stake(ref meta, _, _) => {
            if let Some(authority) = *authority {
                if meta.authorized.staker == *authority {
                    has_staker_signer = true;
                }
                if meta.authorized.withdrawer == *authority {
                    has_withdrawer_signer = true;
                }
            };

            if let Some(custodian) = *custodian {
                if meta.authorized.staker == *custodian {
                    has_staker_signer = true;
                }
                if meta.authorized.withdrawer == *custodian {
                    has_withdrawer_signer = true;
                }
            };

            if let Some(stake_or_withdraw_auth) = stake_or_withdraw_auth {
                if meta.authorized.staker == *stake_or_withdraw_auth {
                    has_staker_signer = true;
                }
                if meta.authorized.withdrawer == *stake_or_withdraw_auth {
                    has_withdrawer_signer = true;
                }
            };
        }
        _ => {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    Ok(AuthorizeSignerArgs {
        has_staker_signer,
        has_withdrawer_signer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_authorize_with_seeds_args() {
        let data = &[
            59, 242, 204, 190, 54, 61, 5, 33, 184, 22, 185, 9, 8, 116, 164, 194, 234, 165, 126, 13,
            237, 190, 6, 236, 191, 198, 111, 157, 70, 124, 157, 196, 1, 0, 0, 0, 9, 0, 0, 0, 0, 0,
            0, 0, 116, 101, 115, 116, 32, 115, 101, 101, 100, 59, 242, 204, 190, 54, 61, 5, 33,
            184, 22, 185, 9, 8, 116, 164, 194, 234, 165, 126, 13, 237, 190, 6, 236, 191, 198, 111,
            157, 70, 124, 157, 196,
        ];
        let args = AuthorizeWithSeedArgs::from_data(data).unwrap();
        println!("{:?}", args);
        // assert_eq!(args.authority_type, StakeAuthorize::Withdrawer);
    }
}
