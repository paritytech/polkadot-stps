/// A trait for converting any Debug type to a String representation.
///
/// This is useful when want to more conveniently convert Debug types to String,:
/// ```diff
/// -format!("{:?}", debuggable)
/// +debuggable.to_debug_string()
/// ```
pub trait ToDebugString {
    fn to_debug_string(&self) -> String;
}
impl<T: core::fmt::Debug> ToDebugString for T {
    fn to_debug_string(&self) -> String {
        format!("{self:?}")
    }
}
