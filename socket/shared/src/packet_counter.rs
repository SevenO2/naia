use std::borrow::Cow;

use atomic_counter::{RelaxedCounter, AtomicCounter};
use log::*;

#[derive(Debug)]
pub struct PacketCounter {
	context: Cow<'static, String>,
	pub sent: RelaxedCounter,
	pub recv: RelaxedCounter,
}
impl PacketCounter {
	pub fn new(context: impl Into<String>) -> Self {
		Self {
			context: Cow::Owned(context.into()),
			sent: Default::default(),
			recv: Default::default(),
		}
	}
	pub fn inc_sent(&self) { self.sent.inc(); }
	pub fn inc_recv(&self) { self.recv.inc(); }
}

// impl Drop for PacketCounter {
// 	fn drop(&mut self) {
// 		println!("{:?}", self);
// 	}
// }