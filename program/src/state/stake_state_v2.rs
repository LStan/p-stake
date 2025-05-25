use pinocchio::{
    account_info::{AccountInfo, Ref, RefMut},
    program_error::ProgramError,
};

use super::{Meta, Stake, StakeFlags};

#[repr(u32)]
#[derive(Debug, Default, PartialEq)]
pub enum StakeStateV2 {
    #[default]
    Uninitialized = 0,
    Initialized(Meta) = 1,
    Stake(Meta, Stake, StakeFlags) = 2,
    RewardsPool = 3,
}

impl StakeStateV2 {
    /// The fixed number of bytes used to serialize each stake account
    pub const fn size_of() -> usize {
        200
    }

    #[inline]
    pub fn from_account_info(
        account_info: &AccountInfo,
    ) -> Result<Ref<StakeStateV2>, ProgramError> {
        if account_info.data_len() < Self::size_of() {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account_info.try_borrow_data()?;
        if data[0] > 3 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Ref::map(data, |data| unsafe { Self::from_bytes(data) }))
    }

    /// # Safety
    ///
    /// The caller must ensure that it is safe to borrow the account data – e.g., there are
    /// no mutable borrows of the account data.
    #[inline]
    pub unsafe fn from_account_info_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&StakeStateV2, ProgramError> {
        if account_info.data_len() < Self::size_of() {
            return Err(ProgramError::InvalidAccountData);
        }
        let data = account_info.borrow_data_unchecked();
        if data[0] > 3 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self::from_bytes(data))
    }

    #[inline]
    pub fn from_account_info_mut(
        account_info: &AccountInfo,
    ) -> Result<RefMut<StakeStateV2>, ProgramError> {
        if account_info.data_len() < Self::size_of() {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account_info.try_borrow_mut_data()?;
        if data[0] > 3 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(RefMut::map(data, |data| unsafe {
            Self::from_bytes_mut(data)
        }))
    }

    /// # Safety
    ///
    /// The caller must ensure that it is safe to borrow the account data – e.g., there are
    /// no mutable borrows of the account data.
    #[inline]
    pub unsafe fn from_account_info_mut_unchecked(
        account_info: &AccountInfo,
    ) -> Result<&mut StakeStateV2, ProgramError> {
        if account_info.data_len() < Self::size_of() {
            return Err(ProgramError::InvalidAccountData);
        }
        let data = account_info.borrow_mut_data_unchecked();
        if data[0] > 3 {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self::from_bytes_mut(data))
    }

    /// # Safety
    ///
    /// The caller must ensure that `bytes` contains a valid representation of `StakeStateV2`.
    #[inline(always)]
    pub unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const Self)
    }

    #[inline(always)]
    pub unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        &mut *(bytes.as_mut_ptr() as *mut Self)
    }
}
