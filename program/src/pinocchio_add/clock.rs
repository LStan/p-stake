use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::clock::Clock,
};
use pinocchio_pubkey::pubkey;

pub const CLOCK_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");

pub fn from_account_info(account_info: &AccountInfo) -> Result<Ref<'_, Clock>, ProgramError> {
    if account_info.key() != &CLOCK_ID {
        return Err(ProgramError::InvalidArgument);
    }

    let data = account_info.try_borrow_data()?;

    Ok(Ref::map(data, |data| unsafe {
        &*(data.as_ptr() as *const Clock)
    }))
}
