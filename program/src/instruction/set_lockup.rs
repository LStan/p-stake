use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::state::{
    get_stake_state, get_stake_state_mut, Epoch, SetLockupSignerArgs, StakeStateV2, UnixTimestamp,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct LockupArgs {
    pub unix_timestamp: Option<UnixTimestamp>,
    pub epoch: Option<Epoch>,
    pub custodian: Option<Pubkey>,
}

impl LockupArgs {
    pub fn from_data(data: &[u8]) -> Result<Self, ProgramError> {
        match data.len() {
            // all none: 1 + 1 + 1
            3 => {
                if (data[0] == 1) || (data[1] == 1) || (data[2] == 1) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(LockupArgs {
                    unix_timestamp: None,
                    epoch: None,
                    custodian: None,
                })
            }
            // (unix_timestamp - some, other - none) or (epoch - some, other none): 9 + 1 + 1
            11 => {
                if !(((data[0] == 1) && (data[9] == 0) && (data[10] == 0))
                    || ((data[0] == 0) && (data[1] == 1) && (data[10] == 0)))
                {
                    return Err(ProgramError::InvalidInstructionData);
                }
                if data[0] == 1 {
                    Ok(LockupArgs {
                        unix_timestamp: Some(unsafe {
                            *(data.as_ptr().add(1) as *const UnixTimestamp)
                        }),
                        epoch: None,
                        custodian: None,
                    })
                } else {
                    Ok(LockupArgs {
                        unix_timestamp: None,
                        epoch: Some(unsafe { *(data.as_ptr().add(2) as *const Epoch) }),
                        custodian: None,
                    })
                }
            }
            // (unix_timestamp and epoch - some, custodian - none): 9 + 9 + 1
            19 => {
                if !((data[0] == 1) && (data[9] == 1) && (data[18] == 0)) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(LockupArgs {
                    unix_timestamp: Some(unsafe {
                        *(data.as_ptr().add(1) as *const UnixTimestamp)
                    }),
                    epoch: Some(unsafe { *(data.as_ptr().add(10) as *const Epoch) }),
                    custodian: None,
                })
            }
            // (custodian - some, other - none): 1 + 1 + 33
            35 => {
                if !((data[0] == 0) && (data[1] == 0) && (data[2] == 1)) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(LockupArgs {
                    unix_timestamp: None,
                    epoch: None,
                    custodian: Some(unsafe { *(data.as_ptr().add(3) as *const Pubkey) }),
                })
            }
            // (custodian - some, either unix_timestamp or epoch - none): 9 + 1 + 33
            43 => {
                if !(((data[0] == 0) && (data[1] == 1) && (data[10] == 1))
                    || ((data[0] == 1) && (data[9] == 0) && (data[10] == 1)))
                {
                    return Err(ProgramError::InvalidInstructionData);
                }
                if data[0] == 1 {
                    Ok(LockupArgs {
                        unix_timestamp: Some(unsafe {
                            *(data.as_ptr().add(1) as *const UnixTimestamp)
                        }),
                        epoch: None,
                        custodian: Some(unsafe { *(data.as_ptr().add(11) as *const Pubkey) }),
                    })
                } else {
                    Ok(LockupArgs {
                        unix_timestamp: None,
                        epoch: Some(unsafe { *(data.as_ptr().add(2) as *const Epoch) }),
                        custodian: Some(unsafe { *(data.as_ptr().add(11) as *const Pubkey) }),
                    })
                }
            }
            // all some: 9 + 9 + 33
            51 => {
                if !((data[0] == 1) && (data[9] == 1) && (data[18] == 1)) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(unsafe { *(data.as_ptr() as *const Self) })
                // Ok(LockupArgs {
                //     unix_timestamp: Some(unsafe {
                //         *(data.as_ptr().add(1) as *const UnixTimestamp)
                //     }),
                //     epoch: Some(unsafe { *(data.as_ptr().add(10) as *const Epoch) }),
                //     custodian: Some(unsafe { *(data.as_ptr().add(19) as *const Pubkey) }),
                // })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct LockupCheckedArgs {
    pub unix_timestamp: Option<UnixTimestamp>,
    pub epoch: Option<Epoch>,
}

impl LockupCheckedArgs {
    pub fn from_data(data: &[u8]) -> Result<Self, ProgramError> {
        match data.len() {
            // all none: 1 + 1
            2 => {
                if (data[0] == 1) || (data[1] == 1) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(LockupCheckedArgs {
                    unix_timestamp: None,
                    epoch: None,
                })
            }
            // (unix_timestamp - some, epoch - none) or (epoch - some, unix_timestamp none): 9 + 1
            10 => {
                if !(((data[0] == 1) && (data[9] == 0)) || ((data[0] == 0) && (data[1] == 1))) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                if data[0] == 1 {
                    Ok(LockupCheckedArgs {
                        unix_timestamp: Some(unsafe {
                            *(data.as_ptr().add(1) as *const UnixTimestamp)
                        }),
                        epoch: None,
                    })
                } else {
                    Ok(LockupCheckedArgs {
                        unix_timestamp: None,
                        epoch: Some(unsafe { *(data.as_ptr().add(2) as *const Epoch) }),
                    })
                }
            }
            // all some: 9 + 9
            18 => {
                if !((data[0] == 1) && (data[9] == 1)) {
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(unsafe { *(data.as_ptr() as *const Self) })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub fn process_set_lockup(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let lockup_args = LockupArgs::from_data(data)?;

    let [stake_account_info, _remaining @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let signer_args = get_set_lockup_signer_args(stake_account_info, accounts)?;

    let clock = Clock::get()?;

    do_set_lookup(stake_account_info, &lockup_args, signer_args, &clock)?;

    Ok(())
}

pub fn process_set_lockup_checked(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let lockup_checked_args = LockupCheckedArgs::from_data(data)?;

    let [stake_account_info, _old_withdraw_or_lockup_authority_info, remaining @ ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let clock = Clock::get()?;

    let custodian = if !remaining.is_empty() {
        let new_lockup_authority_info = unsafe { remaining.get_unchecked(0) };
        if new_lockup_authority_info.is_signer() {
            Some(new_lockup_authority_info.key())
        } else {
            return Err(ProgramError::MissingRequiredSignature);
        }
    } else {
        None
    };

    let signer_args = get_set_lockup_signer_args(stake_account_info, accounts)?;

    let lockup_args = LockupArgs {
        unix_timestamp: lockup_checked_args.unix_timestamp,
        epoch: lockup_checked_args.epoch,
        custodian: custodian.copied(),
    };

    do_set_lookup(stake_account_info, &lockup_args, signer_args, &clock)?;

    Ok(())
}

fn do_set_lookup(
    stake_account_info: &AccountInfo,
    lockup: &LockupArgs,
    signer_args: SetLockupSignerArgs,
    clock: &Clock,
) -> ProgramResult {
    let mut stake_account = get_stake_state_mut(stake_account_info)?;
    match &mut *stake_account {
        StakeStateV2::Initialized(meta) => meta.set_lockup(lockup, signer_args, clock),
        StakeStateV2::Stake(meta, _stake, _stake_flags) => {
            meta.set_lockup(lockup, signer_args, clock)
        }
        _ => Err(ProgramError::InvalidAccountData),
    }
}

fn get_set_lockup_signer_args(
    stake_account_info: &AccountInfo,
    accounts: &[AccountInfo],
) -> Result<SetLockupSignerArgs, ProgramError> {
    let stake_account = get_stake_state(stake_account_info)?;

    let mut has_custodian_signer = false;
    let mut has_withdrawer_signer = false;
    match *stake_account {
        StakeStateV2::Initialized(ref meta) | StakeStateV2::Stake(ref meta, _, _) => {
            for account in accounts {
                if account.is_signer() {
                    if meta.lockup.custodian == *account.key() {
                        has_custodian_signer = true;
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
    Ok(SetLockupSignerArgs {
        has_custodian_signer,
        has_withdrawer_signer,
    })
}

#[cfg(test)]
mod test {
    use crate::state::{Epoch, UnixTimestamp};

    use super::{LockupArgs, LockupCheckedArgs};
    use bincode::serialize;

    #[test]
    fn test_instruction_data_lockup() {
        let unix_timestamp: UnixTimestamp = 3609733389592650838i64.into();
        let epoch: Epoch = 9464321479845648u64.into();
        let custodian = [
            13, 54, 98, 123, 59, 67, 165, 78, 03, 12, 23, 45, 67, 89, 01, 02, 03, 04, 05, 06, 07,
            08, 09, 10, 11, 12, 13, 14, 15, 16, 17, 18,
        ];
        let args_arr = [
            LockupArgs {
                unix_timestamp: None,
                epoch: None,
                custodian: None,
            },
            LockupArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: None,
                custodian: None,
            },
            LockupArgs {
                unix_timestamp: None,
                epoch: Some(epoch),
                custodian: None,
            },
            LockupArgs {
                unix_timestamp: None,
                epoch: None,
                custodian: Some(custodian),
            },
            LockupArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: Some(epoch),
                custodian: None,
            },
            LockupArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: None,
                custodian: Some(custodian),
            },
            LockupArgs {
                unix_timestamp: None,
                epoch: Some(epoch),
                custodian: Some(custodian),
            },
            LockupArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: Some(epoch),
                custodian: Some(custodian),
            },
        ];

        for args in args_arr {
            let data = serialize(&args).unwrap();

            let args_new = LockupArgs::from_data(data.as_ref()).unwrap();
            assert_eq!(args, args_new);
        }
    }

    #[test]
    fn test_instruction_data_lockup_checked() {
        let unix_timestamp: UnixTimestamp = 3609733389592650838i64.into();
        let epoch: Epoch = 9464321479845648u64.into();

        let args_arr = [
            LockupCheckedArgs {
                unix_timestamp: None,
                epoch: None,
            },
            LockupCheckedArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: None,
            },
            LockupCheckedArgs {
                unix_timestamp: None,
                epoch: Some(epoch),
            },
            LockupCheckedArgs {
                unix_timestamp: Some(unix_timestamp),
                epoch: Some(epoch),
            },
        ];

        for args in args_arr {
            let data = serialize(&args).unwrap();

            let args_new = LockupCheckedArgs::from_data(data.as_ref()).unwrap();
            assert_eq!(args, args_new);
        }
    }
}
