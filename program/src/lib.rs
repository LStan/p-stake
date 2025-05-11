#![cfg_attr(not(test), no_std)]

mod entrypoint;

pub mod error;
pub mod instruction;
pub mod pinocchio_add;
pub mod state;

pinocchio_pubkey::declare_id!("Stake11111111111111111111111111111111111111");
