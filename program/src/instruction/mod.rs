pub mod authorize;
pub mod deactivate;
pub mod deactivate_delinquent;
pub mod delegate;
pub mod initialize;
pub mod merge;
pub mod move_stake_lamports;
pub mod set_lockup;
pub mod split;
pub mod withdraw;

pub use authorize::*;
pub use deactivate::*;
pub use deactivate_delinquent::*;
pub use delegate::*;
pub use initialize::*;
pub use merge::*;
pub use move_stake_lamports::*;
pub use set_lockup::*;
pub use split::*;
pub use withdraw::*;

use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

fn relocate_lamports(
    source_account_info: &AccountInfo,
    destination_account_info: &AccountInfo,
    lamports: u64,
) -> ProgramResult {
    {
        let mut source_lamports = source_account_info.try_borrow_mut_lamports()?;
        *source_lamports = source_lamports
            .checked_sub(lamports)
            .ok_or(ProgramError::InsufficientFunds)?;
    }

    {
        let mut destination_lamports = destination_account_info.try_borrow_mut_lamports()?;
        *destination_lamports = destination_lamports
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }

    Ok(())
}
