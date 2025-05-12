#[allow(unused_imports)]
use pinocchio::{
    program_error::ProgramError,
    pubkey::{Pubkey, MAX_SEED_LEN, PUBKEY_BYTES},
};

#[inline]
pub fn create_with_seed_unchecked(
    base: &Pubkey,
    seed: &[u8],
    owner: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    #[cfg(target_os = "solana")]
    {
        let mut bytes = core::mem::MaybeUninit::<[u8; PUBKEY_BYTES]>::uninit();

        let vals = &[base, seed, owner];

        let result = unsafe {
            pinocchio::syscalls::sol_sha256(
                vals as *const _ as *const u8,
                vals.len() as u64,
                bytes.as_mut_ptr() as *mut _,
            )
        };

        match result {
            // SAFETY: The syscall has initialized the bytes.
            pinocchio::SUCCESS => Ok(unsafe { bytes.assume_init() }),
            _ => Err(result.into()),
        }
    }

    #[cfg(not(target_os = "solana"))]
    {
        core::hint::black_box((base, seed, owner));
        panic!("create_with_seed is only available on target `solana`")
    }
}

#[inline]
pub fn create_with_seed(
    base: &Pubkey,
    seed: &[u8],
    owner: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    if seed.len() > MAX_SEED_LEN {
        return Err(ProgramError::MaxSeedLengthExceeded);
    }

    create_with_seed_unchecked(base, seed, owner)
}
