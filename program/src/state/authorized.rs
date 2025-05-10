use pinocchio::pubkey::Pubkey;

#[repr(C)]
#[derive(Default, Debug, PartialEq)]
pub struct Authorized {
    pub staker: Pubkey,
    pub withdrawer: Pubkey,
}
