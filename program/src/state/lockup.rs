use pinocchio::{pubkey::Pubkey, sysvars::clock::Clock};

use super::{Epoch, UnixTimestamp};

#[repr(C)]
#[derive(Default, Debug, PartialEq)]
pub struct Lockup {
    /// UnixTimestamp at which this stake will allow withdrawal, unless the
    ///   transaction is signed by the custodian
    pub unix_timestamp: UnixTimestamp,
    /// epoch height at which this stake will allow withdrawal, unless the
    ///   transaction is signed by the custodian
    pub epoch: Epoch,
    /// custodian signature on a transaction exempts the operation from
    ///  lockup constraints
    pub custodian: Pubkey,
}

impl Lockup {
    pub fn is_in_force(&self, clock: &Clock, custodian: Option<&Pubkey>) -> bool {
        if custodian == Some(&self.custodian) {
            return false;
        }
        i64::from(self.unix_timestamp) > clock.unix_timestamp || u64::from(self.epoch) > clock.epoch
    }
}
