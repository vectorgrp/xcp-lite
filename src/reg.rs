//-----------------------------------------------------------------------------
// Module registry

//-----------------------------------------------------------------------------
// Submodules

// registry and A2l writer
mod registry;
pub use registry::*;

//-----------------------------------------------------------------------------
// Register a calibration parameterin
// Manually add parameter with name "<struct_name>.<struct_field>" in a calibration segment <cal_seg>

// @@@@ ToDo: Signature of add_characteristic changed
// #[macro_export]
// macro_rules! add_characteristic {
//     (  $calseg_name:expr, $self:ident.$field:ident, $comment:expr, $unit:expr, $min:expr, $max:expr ) => {{
//         let offset = (&($self.$field) as *const _ as *const u8 as u64)
//             .wrapping_sub($self as *const _ as *const u8 as u64);
//         assert!(offset < 0x10000, "offset too large");
//         Xcp::get()
//             .get_registry()
//             .lock()
//             .unwrap()
//             .add_characteristic(
//                 $calseg_name,
//                 $calseg_name,
//                 stringify!($field),
//                 $self.$field.get_type(),
//                 offset as u16,
//                 0,
//                 0,
//                 $comment,
//                 $unit,
//                 $min,
//                 $max,
//             );
//     }};
// }
