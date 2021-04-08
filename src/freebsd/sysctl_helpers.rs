use sysctl::{Ctl, CtlType, CtlValue, Sysctl, SysctlError, Temperature};

macro_rules! sysctl_value {
    ($func:ident, $ctype:path, $retype:ty) => {
        fn $func(self) -> Option<$retype> {
            self.and_then(|c| c.value()).ok().and_then(|c| {
                if let $ctype(i) = c {
                    Some(i)
                } else {
                    None
                }
            })
        }
    };
}

pub(super) trait SysctlInner {
    fn node_value(self) -> Option<Vec<u8>>;
    fn int_value(self) -> Option<i32>;
    fn string_value(self) -> Option<String>;
    fn s64_value(self) -> Option<i64>;
    fn struct_value(self) -> Option<Vec<u8>>;
    fn uint_value(self) -> Option<u32>;
    fn long_value(self) -> Option<i64>;
    fn ulong_value(self) -> Option<u64>;
    fn u64_value(self) -> Option<u64>;
    fn u8_value(self) -> Option<u8>;
    fn u16_value(self) -> Option<u16>;
    fn s8_value(self) -> Option<i8>;
    fn s16_value(self) -> Option<i16>;
    fn s32_value(self) -> Option<i32>;
    fn u32_value(self) -> Option<u32>;
    fn temperature_value(self) -> Option<Temperature>;
    fn get_type(self) -> Result<CtlType, SysctlError>;
}

impl SysctlInner for Result<Ctl, SysctlError> {
    sysctl_value!(node_value, CtlValue::Node, Vec<u8>);
    sysctl_value!(int_value, CtlValue::Int, i32);
    sysctl_value!(string_value, CtlValue::String, String);
    sysctl_value!(s64_value, CtlValue::S64, i64);
    sysctl_value!(struct_value, CtlValue::Struct, Vec<u8>);
    sysctl_value!(uint_value, CtlValue::Uint, u32);
    sysctl_value!(long_value, CtlValue::Long, i64);
    sysctl_value!(ulong_value, CtlValue::Ulong, u64);
    sysctl_value!(u64_value, CtlValue::U64, u64);
    sysctl_value!(u8_value, CtlValue::U8, u8);
    sysctl_value!(u16_value, CtlValue::U16, u16);
    sysctl_value!(s8_value, CtlValue::S8, i8);
    sysctl_value!(s16_value, CtlValue::S16, i16);
    sysctl_value!(s32_value, CtlValue::S32, i32);
    sysctl_value!(u32_value, CtlValue::U32, u32);
    sysctl_value!(temperature_value, CtlValue::Temperature, Temperature);
    fn get_type(self) -> Result<CtlType, SysctlError> {
        self.and_then(|c| c.value_type())
    }
}
