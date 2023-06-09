//! MB driver top level.

use crate::{driver, exception::asynchronous::IRQNumber};

/// Abstraction for MBR:
/// Boot code
/// Partition table entry 1
/// Partition table entry 2
/// Partition table entry 3
/// Partition table entry 4
/// Signature Value
// struct MBRInner {
//     arb: u32,
// }

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub struct MBR {
    // inner: MBRInner,
}

//---
// Public code
//----
impl MBR {
    pub const COMPATIBLE: &'static str = "MBR";
    /// Placeholder public code
    pub fn say_hello(&self) -> &'static str {
        // self.inner.arb = 0xdeadbeef;
        "Hello from the MBR reader!"
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------

impl driver::interface::DeviceDriver for MBR {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
