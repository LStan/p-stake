use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Handler for retrieving a slice of sysvar data from the `sol_get_sysvar`
/// syscall.
///
/// # Safety
///
/// `dst` must be large enough to hold `length` bytes.
#[inline]
pub unsafe fn get_sysvar_unchecked(
    dst: &mut [u8],
    sysvar_id: &Pubkey,
    offset: u64,
    length: u64,
) -> Result<(), ProgramError> {
    let sysvar_id = sysvar_id as *const _ as *const u8;
    let var_addr = dst as *mut _ as *mut u8;

    #[cfg(target_os = "solana")]
    let result =
        unsafe { pinocchio::syscalls::sol_get_sysvar(sysvar_id, var_addr, offset, length) };

    #[cfg(not(target_os = "solana"))]
    let result = core::hint::black_box(sysvar_id as u64 + var_addr as u64 + offset + length);

    match result {
        pinocchio::SUCCESS => Ok(()),
        e => Err(e.into()),
    }
}

/// Handler for retrieving a slice of sysvar data from the `sol_get_sysvar`
/// syscall.
#[inline]
pub fn get_sysvar(
    dst: &mut [u8],
    sysvar_id: &Pubkey,
    offset: u64,
    length: u64,
) -> Result<(), ProgramError> {
    // Check that the provided destination buffer is large enough to hold the
    // requested data.
    if dst.len() < length as usize {
        return Err(ProgramError::InvalidArgument);
    }

    unsafe { get_sysvar_unchecked(dst, sysvar_id, offset, length) }
}
