use super::{Authorized, Lockup, PodU64};

#[repr(C)]
#[derive(Default, Debug, PartialEq)]
pub struct Meta {
    pub rent_exempt_reserve: PodU64,
    pub authorized: Authorized,
    pub lockup: Lockup,
}
